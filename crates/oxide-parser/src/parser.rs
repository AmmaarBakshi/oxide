use crate::ast::{Condition, Executable, List, Statement};
use crate::token::Token;

pub struct Parser {
    pub(crate) tokens: Vec<Token>,
    pub(crate) cursor: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0 }
    }

    pub fn parse(&mut self) -> Vec<Executable> {
        self.parse_list(|_| false)
    }

    /// Parses statements joined by newlines, `;`, `&&` and `||`, up to the
    /// first token at the start of a statement that `stop` accepts (left
    /// unconsumed), or the end of the input.
    pub(crate) fn parse_list(&mut self, stop: impl Fn(&Token) -> bool) -> List {
        let mut executables = Vec::new();
        let mut current_condition = Condition::Always;

        while self.cursor < self.tokens.len() {
            // Skip separators
            while self.cursor < self.tokens.len()
                && matches!(self.tokens[self.cursor], Token::Newline | Token::Semicolon)
            {
                self.cursor += 1;
            }
            if self.cursor >= self.tokens.len() || stop(&self.tokens[self.cursor]) {
                break;
            }

            let start_pos = self.cursor;

            if let Some(statement) = self.parse_statement() {
                // `cmd &` runs in the background; the `&` also ends it.
                let background = self.tokens.get(self.cursor) == Some(&Token::Background);
                if background {
                    self.cursor += 1;
                }
                executables.push(Executable {
                    statement,
                    condition: current_condition.clone(),
                    background,
                });
            }

            // Safety valve to prevent infinite loops
            if self.cursor == start_pos {
                self.cursor += 1;
            }

            current_condition = Condition::Always;
            if let Some(op @ (Token::And | Token::Or)) = self.tokens.get(self.cursor) {
                current_condition = if *op == Token::And {
                    Condition::And
                } else {
                    Condition::Or
                };
                self.cursor += 1;
                // `&&` or `||` at the end of a line carries on to the next.
                while self.tokens.get(self.cursor) == Some(&Token::Newline) {
                    self.cursor += 1;
                }
            }
        }
        executables
    }

    pub(crate) fn parse_statement(&mut self) -> Option<Statement> {
        if self.cursor >= self.tokens.len() {
            return None;
        }

        // 1. Check if the statement starts with a logic keyword
        if let Token::Word(w) = &self.tokens[self.cursor] {
            match w.as_str() {
                "if" => return self.parse_if_statement(),
                "while" => return self.parse_while_statement(),
                "for" => return self.parse_for_statement(),
                "case" => return self.parse_case_statement(),
                "function" => return self.parse_function(true),
                _ => {} // Fall through to pipeline parsing
            }
            if self.at_function_definition() {
                return self.parse_function(false);
            }
        }

        // 2. If it's not a logic block, it must be a standard command/pipeline
        self.parse_pipeline_or_command()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Assignment, Command};
    use crate::lexer::Lexer;

    fn command(input: &str) -> Command {
        let mut executables = Parser::new(Lexer::new(input).tokenize()).parse();
        assert_eq!(executables.len(), 1, "expected one statement in {input:?}");
        match executables.remove(0).statement {
            Statement::Command(cmd) => cmd,
            _ => panic!("expected a simple command in {input:?}"),
        }
    }

    fn assignment(name: &str, value: &str) -> Assignment {
        Assignment {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn leading_assignments_are_split_off_the_command() {
        let cmd = command("FOO=1 _BAR=\"two words\" echo X=3");
        assert_eq!(
            cmd.assignments,
            [assignment("FOO", "1"), assignment("_BAR", "\"two words\"")]
        );
        assert_eq!(cmd.program, "echo");
        assert_eq!(cmd.args, ["X=3"]);
    }

    #[test]
    fn a_line_of_assignments_has_no_program() {
        let cmd = command("A=1 B=");
        assert_eq!(cmd.program, "");
        assert_eq!(cmd.assignments, [assignment("A", "1"), assignment("B", "")]);
    }

    #[test]
    fn stderr_redirects_are_parsed() {
        let cmd = command("make 2> err.txt > out.txt");
        assert_eq!(cmd.errfile.as_deref(), Some("err.txt"));
        assert!(!cmd.err_append);
        assert_eq!(cmd.outfile.as_deref(), Some("out.txt"));

        let cmd = command("make 2>>log.txt");
        assert_eq!(cmd.errfile.as_deref(), Some("log.txt"));
        assert!(cmd.err_append);
        assert_eq!(cmd.args, Vec::<String>::new());
    }

    #[test]
    fn joined_streams_are_parsed() {
        let cmd = command("make > all.txt 2>&1");
        assert_eq!(cmd.outfile.as_deref(), Some("all.txt"));
        assert!(cmd.err_to_out && !cmd.out_to_err);

        let cmd = command("make &>> all.txt");
        assert_eq!(cmd.outfile.as_deref(), Some("all.txt"));
        assert!(cmd.append && cmd.err_to_out);

        let cmd = command("echo oops >&2");
        assert_eq!(cmd.args, ["oops"]);
        assert!(cmd.out_to_err && !cmd.err_to_out);
    }

    #[test]
    fn a_heredoc_belongs_to_its_command() {
        use crate::ast::HereInput;
        let cmd = command("cat <<'END'\n$literal\nEND\n");
        assert_eq!(cmd.program, "cat");
        assert_eq!(
            cmd.here,
            Some(HereInput::Document {
                body: "$literal\n".into(),
                expand: false
            })
        );
    }

    #[test]
    fn a_here_string_belongs_to_its_command() {
        use crate::ast::HereInput;
        let cmd = command("grep b <<< \"$X b\" > out");
        assert_eq!(cmd.args, ["b"]);
        assert_eq!(cmd.here, Some(HereInput::String("\"$X b\"".into())));
        assert_eq!(cmd.outfile.as_deref(), Some("out"));
    }

    fn first_statement(input: &str) -> Statement {
        Parser::new(Lexer::new(input).tokenize())
            .parse()
            .remove(0)
            .statement
    }

    #[test]
    fn an_if_condition_is_a_list_of_commands() {
        let Statement::If {
            condition,
            body,
            else_if,
            else_body,
        } = first_statement(
            "if test -f x && grep -q y z { echo a } elif false { echo b } else { echo c }",
        )
        else {
            panic!("expected an if");
        };
        assert_eq!(condition.len(), 2);
        assert!(matches!(condition[1].condition, Condition::And));
        assert_eq!(body.len(), 1);
        assert_eq!(else_if.len(), 1);
        assert_eq!(else_body.map(|b| b.len()), Some(1));
    }

    #[test]
    fn else_may_start_on_the_next_line() {
        let Statement::If { else_body, .. } = first_statement("if false {\n  a\n}\nelse {\n  b\n}")
        else {
            panic!("expected an if");
        };
        assert!(else_body.is_some());
    }

    #[test]
    fn a_block_keeps_its_and_or_chains() {
        let Statement::While { body, .. } = first_statement("while true { a && b || c }") else {
            panic!("expected a while");
        };
        let conditions: Vec<_> = body.iter().map(|e| e.condition.clone()).collect();
        assert!(matches!(
            conditions[..],
            [Condition::Always, Condition::And, Condition::Or]
        ));
    }

    #[test]
    fn functions_can_be_defined_three_ways() {
        for input in [
            "greet() { echo hi; echo there }",
            "greet () {\n  echo hi\n  echo there\n}",
            "function greet {\n  echo hi; echo there\n}",
            "function greet()\n{\n  echo hi && echo there\n}",
        ] {
            let Statement::Function { name, body } = first_statement(input) else {
                panic!("{input:?} is not a function definition");
            };
            assert_eq!(name, "greet", "{input:?}");
            assert_eq!(body.len(), 2, "{input:?}");
        }
    }

    #[test]
    fn a_call_with_empty_parens_as_an_argument_is_not_a_definition() {
        assert!(matches!(first_statement("echo ()"), Statement::Command(_)));
        assert!(matches!(first_statement("f()"), Statement::Command(_)));
    }

    #[test]
    fn case_arms_take_patterns_and_bodies() {
        let Statement::Case { word, arms } = first_statement(
            "case $file in\n  *.rs | *.toml) echo rust; echo again ;;\n  (docs) echo docs ;;\n  *) echo other\nesac",
        ) else {
            panic!("expected a case");
        };
        assert_eq!(word, "$file");
        let patterns: Vec<_> = arms.iter().map(|arm| arm.patterns.clone()).collect();
        assert_eq!(patterns, [vec!["*.rs", "*.toml"], vec!["docs"], vec!["*"]]);
        let lengths: Vec<_> = arms.iter().map(|arm| arm.body.len()).collect();
        assert_eq!(lengths, [2, 1, 1]);
    }

    #[test]
    fn a_case_can_sit_on_one_line() {
        let Statement::Case { arms, .. } = first_statement("case x in a) one ;; b) two ;; esac")
        else {
            panic!("expected a case");
        };
        assert_eq!(arms.len(), 2);
    }

    #[test]
    fn an_ampersand_backgrounds_a_statement_and_ends_it() {
        let list = Parser::new(Lexer::new("sleep 1 & echo hi; ls | wc &").tokenize()).parse();
        let background: Vec<_> = list.iter().map(|e| e.background).collect();
        assert_eq!(background, [true, false, true]);
        assert!(matches!(list[2].statement, Statement::Pipeline(_)));
    }

    #[test]
    fn words_that_only_look_like_assignments_are_commands() {
        for word in ["=x", "1X=2", "\"X\"=1", "A-B=1"] {
            let cmd = command(word);
            assert!(cmd.assignments.is_empty(), "{word} was an assignment");
            assert_eq!(cmd.program, word);
        }
    }
}
