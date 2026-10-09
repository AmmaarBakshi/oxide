use crate::ast::{Condition, Executable, Statement};
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
        let mut executables = Vec::new();
        let mut current_condition = Condition::Always;

        while self.cursor < self.tokens.len() {
            // Skip separators
            while self.cursor < self.tokens.len()
                && matches!(self.tokens[self.cursor], Token::Newline | Token::Semicolon)
            {
                self.cursor += 1;
            }
            if self.cursor >= self.tokens.len() {
                break;
            }

            let start_pos = self.cursor;

            if let Some(statement) = self.parse_statement() {
                executables.push(Executable {
                    statement,
                    condition: current_condition.clone(),
                });
            }

            // Safety valve to prevent infinite loops
            if self.cursor == start_pos {
                self.cursor += 1;
            }

            if self.cursor < self.tokens.len() {
                match &self.tokens[self.cursor] {
                    Token::And => {
                        current_condition = Condition::And;
                        self.cursor += 1;
                    }
                    Token::Or => {
                        current_condition = Condition::Or;
                        self.cursor += 1;
                    }
                    _ => {
                        current_condition = Condition::Always;
                    }
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
                _ => {} // Fall through to pipeline parsing
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
    fn words_that_only_look_like_assignments_are_commands() {
        for word in ["=x", "1X=2", "\"X\"=1", "A-B=1"] {
            let cmd = command(word);
            assert!(cmd.assignments.is_empty(), "{word} was an assignment");
            assert_eq!(cmd.program, word);
        }
    }
}
