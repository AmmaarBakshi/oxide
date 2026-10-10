use crate::heredoc::{self, Delimiter};
use crate::token::Token;
use std::iter::Peekable;
use std::str::Chars;

pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
    /// Set when the input ran out inside a quote, `$(`, backtick, here-doc,
    /// or right after a backslash: more lines would continue it.
    unterminated: bool,
    /// Here-documents opened on the current line, waiting for its end to
    /// collect their bodies: the index of each `HereDoc` token, and its
    /// delimiter.
    pending_heredocs: Vec<(usize, Delimiter)>,
    /// Delimiters of here-documents whose terminating line never came.
    unfinished_heredocs: Vec<String>,
}

/// The delimiters of here-documents in `input` that are still waiting for
/// their terminating line, so the REPL knows to ask for the body.
pub fn unfinished_heredocs(input: &str) -> Vec<String> {
    let mut lexer = Lexer::new(input);
    lexer.tokenize();
    lexer.unfinished_heredocs
}

/// Whether `input` stops in the middle of something and needs more lines to
/// finish it: an open quote, `$(` or backtick, a here-document still
/// missing its delimiter line, more `{` than `}`, or a last line ending in
/// `|`, `&&`, `||` or `\`.
pub fn is_incomplete(input: &str) -> bool {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize();
    if lexer.unterminated {
        return true;
    }
    // `{` opens a block and `case` a case statement; `}` and `esac` close
    // them. A keyword only counts where a command could start.
    let mut open_blocks = 0;
    let mut command_start = true;
    for token in &tokens {
        open_blocks += match token {
            Token::LBrace => 1,
            Token::RBrace => -1,
            Token::Word(w) if command_start && w == "case" => 1,
            Token::Word(w) if command_start && w == "esac" => -1,
            _ => 0,
        };
        command_start = matches!(
            token,
            Token::Newline
                | Token::Semicolon
                | Token::DoubleSemi
                | Token::LBrace
                | Token::RBrace
                | Token::And
                | Token::Or
                | Token::Pipe
        );
    }
    if open_blocks > 0 {
        return true;
    }
    matches!(
        tokens.iter().rev().find(|token| **token != Token::Newline),
        Some(Token::Pipe | Token::And | Token::Or)
    )
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Lexer {
            input: input.chars().peekable(),
            unterminated: false,
            pending_heredocs: Vec::new(),
            unfinished_heredocs: Vec::new(),
        }
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        while let Some(&c) = self.input.peek() {
            match c {
                // Skip horizontal whitespace, but handle newlines
                ' ' | '\t' | '\r' => {
                    self.input.next();
                }
                '\n' => {
                    self.input.next();
                    tokens.push(Token::Newline);
                    self.collect_heredocs(&mut tokens);
                }
                // Handle Pipes and OR (|, ||)
                '|' => {
                    self.input.next(); // consume the first |
                    if self.input.peek() == Some(&'|') {
                        self.input.next(); // consume the second |
                        tokens.push(Token::Or);
                    } else {
                        tokens.push(Token::Pipe);
                    }
                }
                // Handle Background and AND (&, &&), and &> / &>> which
                // send both stdout and stderr to a file.
                '&' => {
                    let token = if self.eat("&&") {
                        Token::And
                    } else if self.eat("&>>") {
                        Token::RedirectAllAppend
                    } else if self.eat("&>") {
                        Token::RedirectAll
                    } else {
                        self.input.next();
                        Token::Background
                    };
                    tokens.push(token);
                }
                // Handle Redirects (>, >>, and >&2 for "stdout to stderr")
                '>' => {
                    let token = if self.eat(">&2") {
                        Token::OutToErr
                    } else if self.eat(">>") {
                        Token::RedirectAppend
                    } else {
                        self.input.next();
                        Token::RedirectOut
                    };
                    tokens.push(token);
                }
                '<' => {
                    if self.eat("<<<") {
                        tokens.push(Token::HereString);
                    } else if self.eat("<<-") {
                        self.heredoc(true, &mut tokens);
                    } else if self.eat("<<") {
                        self.heredoc(false, &mut tokens);
                    } else {
                        self.input.next();
                        tokens.push(Token::RedirectIn);
                    }
                }
                ';' => {
                    if self.eat(";;") {
                        tokens.push(Token::DoubleSemi);
                    } else {
                        self.input.next();
                        tokens.push(Token::Newline);
                    }
                }
                // A `{` on its own opens a block; one with text right after it
                // starts a word to brace-expand, like `{a,b}`.
                '{' if self.brace_stands_alone() => {
                    self.input.next();
                    tokens.push(Token::LBrace);
                }
                '}' => {
                    self.input.next();
                    tokens.push(Token::RBrace);
                }
                // A `#` starting a word comments out the rest of the line.
                '#' => while self.input.next_if(|&c| c != '\n').is_some() {},
                // Everything else starts a word: a command, flag or path.
                _ => {
                    let word = self.word();
                    if word == "2" && self.input.peek() == Some(&'>') {
                        // `2>`, `2>>` and `2>&1` redirect stderr. Only a bare
                        // 2 right against the `>` counts: `echo 2 > f` writes "2".
                        let token = if self.eat(">&1") {
                            Token::ErrToOut
                        } else if self.eat(">>") {
                            Token::RedirectErrAppend
                        } else {
                            self.input.next();
                            Token::RedirectErr
                        };
                        tokens.push(token);
                    } else if !word.is_empty() {
                        // (A backslash-newline on its own reads as nothing.)
                        tokens.push(Token::Word(word));
                    }
                }
            }
        }
        // A here-document whose line never ended has no body yet.
        for (_, delimiter) in std::mem::take(&mut self.pending_heredocs) {
            self.unfinished_heredocs.push(delimiter.word);
            self.unterminated = true;
        }
        tokens
    }

    /// Reads the delimiter after `<<` (or `<<-`) and leaves a `HereDoc`
    /// token whose body is filled in at the end of the line.
    fn heredoc(&mut self, strip_tabs: bool, tokens: &mut Vec<Token>) {
        while self.input.next_if(|&c| c == ' ' || c == '\t').is_some() {}
        let raw = self.word();
        let delimiter = Delimiter::parse(&raw, strip_tabs);
        tokens.push(Token::HereDoc {
            body: String::new(),
            expand: delimiter.expand,
        });
        self.pending_heredocs.push((tokens.len() - 1, delimiter));
    }

    /// At the end of a line, reads the bodies of the here-documents it
    /// opened, in order, from the lines that follow.
    fn collect_heredocs(&mut self, tokens: &mut [Token]) {
        for (index, delimiter) in std::mem::take(&mut self.pending_heredocs) {
            let lines = std::iter::from_fn(|| self.line());
            let (body, found) = heredoc::collect_body(lines, &delimiter);
            if !found {
                self.unterminated = true;
                self.unfinished_heredocs.push(delimiter.word.clone());
            }
            tokens[index] = Token::HereDoc {
                body,
                expand: delimiter.expand,
            };
        }
    }

    /// The next line of input, without its newline; `None` at the end.
    fn line(&mut self) -> Option<String> {
        self.input.peek()?;
        let mut line = String::new();
        for c in self.input.by_ref() {
            if c == '\n' {
                break;
            }
            line.push(c);
        }
        Some(line)
    }

    /// Whether the `{` about to be read is followed by whitespace or the end
    /// of the input, which makes it a block's opening brace.
    fn brace_stands_alone(&self) -> bool {
        let mut ahead = self.input.clone();
        ahead.next();
        ahead.peek().is_none_or(|c| c.is_whitespace() || *c == ';')
    }

    /// Consumes `s` if the input continues with it, and says whether it did.
    fn eat(&mut self, s: &str) -> bool {
        let mut ahead = self.input.clone();
        for expected in s.chars() {
            if ahead.next() != Some(expected) {
                return false;
            }
        }
        self.input = ahead;
        true
    }

    /// Reads one word, keeping its quotes and backslashes as written. They
    /// are interpreted later, when the word is expanded; keeping them is what
    /// lets `NAME="two words"` stay a single word and `'$HOME'` stay literal.
    fn word(&mut self) -> String {
        let mut word = String::new();
        // Braces opened inside this word, so `{a,b}` keeps its `}` while a
        // bare `}` still closes a block.
        let mut braces = 0;
        while let Some(&c) = self.input.peek() {
            match c {
                '{' => {
                    braces += 1;
                    word.push(c);
                    self.input.next();
                }
                '}' if braces > 0 => {
                    braces -= 1;
                    word.push(c);
                    self.input.next();
                }
                '\'' | '"' => self.quoted(c, &mut word),
                '`' => self.backticks(&mut word),
                '\\' => {
                    self.input.next();
                    match self.input.next() {
                        // A backslash-newline joins two lines into one; at the
                        // very end of the input, the next line is still to come.
                        Some('\n') => {
                            if self.input.peek().is_none() {
                                self.unterminated = true;
                            }
                        }
                        Some(next) => {
                            word.push('\\');
                            word.push(next);
                        }
                        None => {
                            word.push('\\');
                            self.unterminated = true;
                        }
                    }
                }
                // Keep a `${...}` expansion intact even though `{`/`}` are
                // otherwise block delimiters — it belongs to this word. The
                // same goes for a `$(...)` command substitution and the
                // spaces, pipes and quotes inside it.
                '$' => {
                    word.push(c);
                    self.input.next();
                    match self.input.peek() {
                        Some('{') => {
                            for b in self.input.by_ref() {
                                word.push(b);
                                if b == '}' {
                                    break;
                                }
                            }
                        }
                        Some('(') => self.substitution(&mut word),
                        _ => {}
                    }
                }
                c if c.is_whitespace() || "><|&;}".contains(c) => break,
                _ => {
                    word.push(c);
                    self.input.next();
                }
            }
        }
        word
    }

    /// Copies a quoted section onto `word`, opening and closing quotes
    /// included. An unterminated quote runs to the end of the input.
    fn quoted(&mut self, quote: char, word: &mut String) {
        word.push(quote);
        self.input.next();
        loop {
            let Some(c) = self.input.next() else {
                self.unterminated = true;
                break;
            };
            word.push(c);
            if c == quote {
                break;
            }
            if quote == '"' {
                match c {
                    // Inside double quotes a backslash can escape the
                    // closing quote...
                    '\\' => {
                        if let Some(next) = self.input.next() {
                            word.push(next);
                        }
                    }
                    // ...and a substitution may hold quotes of its own:
                    // "$(echo "hi")" is one word.
                    '$' if self.input.peek() == Some(&'(') => self.substitution(word),
                    '`' => self.rest_of_backticks(word),
                    _ => {}
                }
            }
        }
    }

    /// Copies a `` `...` `` command substitution onto `word`, backticks
    /// included. A backslash keeps the character after it, so `` \` `` does
    /// not end it.
    fn backticks(&mut self, word: &mut String) {
        word.push('`');
        self.input.next();
        self.rest_of_backticks(word);
    }

    /// Copies the rest of a backtick substitution whose opening backtick is
    /// already on `word`, up to and including the closing one.
    fn rest_of_backticks(&mut self, word: &mut String) {
        loop {
            let Some(c) = self.input.next() else {
                self.unterminated = true;
                break;
            };
            word.push(c);
            match c {
                '`' => break,
                '\\' => {
                    if let Some(next) = self.input.next() {
                        word.push(next);
                    }
                }
                _ => {}
            }
        }
    }

    /// Copies a `$(...)` command substitution onto `word`, from its `(` to
    /// the `)` that balances it, skipping over parentheses inside quotes.
    /// An unbalanced one runs to the end of the input.
    fn substitution(&mut self, word: &mut String) {
        let mut depth = 0;
        while let Some(&c) = self.input.peek() {
            match c {
                '\'' | '"' => {
                    self.quoted(c, word);
                    continue;
                }
                '\\' => {
                    word.push(c);
                    self.input.next();
                    if let Some(next) = self.input.next() {
                        word.push(next);
                    }
                    continue;
                }
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            word.push(c);
            self.input.next();
            if depth == 0 {
                return;
            }
        }
        self.unterminated = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(input: &str) -> Vec<Token> {
        Lexer::new(input).tokenize()
    }

    fn word(text: &str) -> Token {
        Token::Word(text.to_string())
    }

    #[test]
    fn quotes_stay_inside_the_word() {
        assert_eq!(
            words("echo \"a b\"c 'd e'"),
            [word("echo"), word("\"a b\"c"), word("'d e'")]
        );
        assert_eq!(words("NAME=\"two words\""), [word("NAME=\"two words\"")]);
    }

    #[test]
    fn operators_inside_quotes_are_text() {
        assert_eq!(words("echo 'a | b; c'"), [word("echo"), word("'a | b; c'")]);
    }

    #[test]
    fn backslash_keeps_a_separator_in_the_word() {
        assert_eq!(words("touch a\\ b"), [word("touch"), word("a\\ b")]);
    }

    #[test]
    fn two_right_against_an_arrow_redirects_stderr() {
        assert_eq!(
            words("cmd 2>err 2>>log"),
            [
                word("cmd"),
                Token::RedirectErr,
                word("err"),
                Token::RedirectErrAppend,
                word("log")
            ]
        );
        // With a space, or as part of a longer word, it is just text.
        assert_eq!(
            words("echo 2 >f x2>g"),
            [
                word("echo"),
                word("2"),
                Token::RedirectOut,
                word("f"),
                word("x2"),
                Token::RedirectOut,
                word("g")
            ]
        );
    }

    #[test]
    fn stream_joining_operators() {
        assert_eq!(
            words("a 2>&1 >&2 &>f &>>g & b && c"),
            [
                word("a"),
                Token::ErrToOut,
                Token::OutToErr,
                Token::RedirectAll,
                word("f"),
                Token::RedirectAllAppend,
                word("g"),
                Token::Background,
                word("b"),
                Token::And,
                word("c")
            ]
        );
    }

    #[test]
    fn command_substitution_is_one_word() {
        assert_eq!(
            words("echo $(ls -a | grep x; echo y) done"),
            [
                word("echo"),
                word("$(ls -a | grep x; echo y)"),
                word("done")
            ]
        );
        assert_eq!(
            words("X=pre$(a $(b c) ')' \"(\")post"),
            [word("X=pre$(a $(b c) ')' \"(\")post")]
        );
    }

    #[test]
    fn command_substitution_inside_double_quotes_keeps_its_quotes() {
        assert_eq!(
            words("echo \"$(echo \"hi there\")\" x"),
            [word("echo"), word("\"$(echo \"hi there\")\""), word("x")]
        );
    }

    #[test]
    fn backticks_are_one_word() {
        assert_eq!(
            words("echo `ls -a | wc` x"),
            [word("echo"), word("`ls -a | wc`"), word("x")]
        );
        assert_eq!(
            words("echo \"a `b \\` c` d\""),
            [word("echo"), word("\"a `b \\` c` d\"")]
        );
    }

    #[test]
    fn hash_starts_a_comment_only_at_the_start_of_a_word() {
        assert_eq!(
            words("echo a#b # the rest\necho c"),
            [
                word("echo"),
                word("a#b"),
                Token::Newline,
                word("echo"),
                word("c")
            ]
        );
        assert_eq!(words("echo '# kept'"), [word("echo"), word("'# kept'")]);
    }

    #[test]
    fn unfinished_input_is_incomplete() {
        for input in [
            "echo 'open",
            "echo \"open",
            "echo $(open",
            "echo `open",
            "if 1 {",
            "if 1 { if 2 {\n}",
            "ls |",
            "true &&\n",
            "false ||",
            "echo a \\",
            "echo a \\\n",
            "case $x in\n  a) echo a ;;",
        ] {
            assert!(is_incomplete(input), "{input:?} should be incomplete");
        }
    }

    #[test]
    fn finished_input_is_complete() {
        for input in [
            "echo 'closed'",
            "echo $(a) `b`",
            "if 1 { echo }",
            "ls | wc",
            "echo # open quote ' in a comment",
            "echo a \\\n b",
            "case $x in a) echo a ;; esac",
            "echo case",
            "",
        ] {
            assert!(!is_incomplete(input), "{input:?} should be complete");
        }
    }

    #[test]
    fn a_heredoc_takes_the_lines_up_to_its_delimiter() {
        assert_eq!(
            words("cat <<EOF > out\nline $X\n  EOF\nEOF\necho after"),
            [
                word("cat"),
                Token::HereDoc {
                    body: "line $X\n  EOF\n".into(),
                    expand: true
                },
                Token::RedirectOut,
                word("out"),
                Token::Newline,
                word("echo"),
                word("after")
            ]
        );
    }

    #[test]
    fn two_heredocs_on_one_line_read_their_bodies_in_turn() {
        let tokens = words("a <<ONE; b <<'TWO'\n1\nONE\n2\nTWO\n");
        assert_eq!(
            tokens[1],
            Token::HereDoc {
                body: "1\n".into(),
                expand: true
            }
        );
        assert_eq!(
            tokens[4],
            Token::HereDoc {
                body: "2\n".into(),
                expand: false
            }
        );
    }

    #[test]
    fn three_arrows_are_a_here_string() {
        assert_eq!(
            words("grep x <<< \"a x\""),
            [word("grep"), word("x"), Token::HereString, word("\"a x\"")]
        );
    }

    #[test]
    fn unfinished_heredocs_are_listed() {
        assert_eq!(
            unfinished_heredocs("cat <<EOF; cat <<'END'"),
            ["EOF", "END"]
        );
        assert_eq!(unfinished_heredocs("cat <<EOF\nhalf\n"), ["EOF"]);
        assert!(unfinished_heredocs("cat <<EOF\nEOF").is_empty());
    }

    #[test]
    fn a_heredoc_without_its_delimiter_is_incomplete() {
        assert!(is_incomplete("cat <<EOF"));
        assert!(is_incomplete("cat <<EOF\nbody\n"));
        assert!(!is_incomplete("cat <<EOF\nbody\nEOF\n"));
        assert!(!is_incomplete("cat <<-EOF\n\tbody\n\tEOF"));
    }

    #[test]
    fn braces_in_a_word_are_text_but_alone_they_are_a_block() {
        assert_eq!(
            words("echo {a,b} pre{x,y}post {}"),
            [
                word("echo"),
                word("{a,b}"),
                word("pre{x,y}post"),
                word("{}")
            ]
        );
        assert_eq!(
            words("if 1 { echo x{1,2} }"),
            [
                word("if"),
                word("1"),
                Token::LBrace,
                word("echo"),
                word("x{1,2}"),
                Token::RBrace
            ]
        );
    }

    #[test]
    fn backslash_newline_joins_lines() {
        assert_eq!(
            words("echo a \\\n  b"),
            [word("echo"), word("a"), word("b")]
        );
    }
}
