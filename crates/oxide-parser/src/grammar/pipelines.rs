use crate::ast::{Assignment, Command, HereInput, Statement};
use crate::parser::Parser;
use crate::token::Token;

impl Parser {
    /// Parses standard commands, arguments, pipes (|), and redirects (>)
    pub(crate) fn parse_pipeline_or_command(&mut self) -> Option<Statement> {
        let mut pipeline = Vec::new();
        let mut current_cmd: Option<Command> = None;

        while self.cursor < self.tokens.len() {
            match &self.tokens[self.cursor] {
                Token::Word(w) | Token::StringLiteral(w) => {
                    let cmd = current_cmd.get_or_insert_with(|| Command::new(String::new()));
                    if !cmd.program.is_empty() {
                        cmd.args.push(w.clone());
                    } else if let Some(assignment) = Assignment::parse(w) {
                        // Only words before the program can be assignments;
                        // `echo X=1` just prints its argument.
                        cmd.assignments.push(assignment);
                    } else {
                        cmd.program = w.clone();
                    }
                    self.cursor += 1;
                }
                Token::Pipe => {
                    if let Some(cmd) = current_cmd.take() {
                        pipeline.push(cmd);
                    }
                    self.cursor += 1;
                    // A pipe at the end of a line carries on to the next.
                    while self.tokens.get(self.cursor) == Some(&Token::Newline) {
                        self.cursor += 1;
                    }
                }
                Token::HereDoc { body, expand } => {
                    let here = HereInput::Document {
                        body: body.clone(),
                        expand: *expand,
                    };
                    current_cmd
                        .get_or_insert_with(|| Command::new(String::new()))
                        .here = Some(here);
                    self.cursor += 1;
                }
                Token::HereString => {
                    self.cursor += 1;
                    if let Some(word) = self.redirect_target() {
                        current_cmd
                            .get_or_insert_with(|| Command::new(String::new()))
                            .here = Some(HereInput::String(word));
                    }
                }
                Token::ErrToOut | Token::OutToErr => {
                    let cmd = current_cmd.get_or_insert_with(|| Command::new(String::new()));
                    if self.tokens[self.cursor] == Token::ErrToOut {
                        cmd.err_to_out = true;
                    } else {
                        cmd.out_to_err = true;
                    }
                    self.cursor += 1;
                }
                Token::RedirectOut
                | Token::RedirectAppend
                | Token::RedirectIn
                | Token::RedirectErr
                | Token::RedirectErrAppend
                | Token::RedirectAll
                | Token::RedirectAllAppend => {
                    let op = self.tokens[self.cursor].clone();
                    self.cursor += 1;
                    let Some(target) = self.redirect_target() else {
                        continue;
                    };
                    // A redirect may come before the program: `> out.txt echo hi`.
                    let cmd = current_cmd.get_or_insert_with(|| Command::new(String::new()));
                    match op {
                        Token::RedirectOut | Token::RedirectAppend => {
                            cmd.outfile = Some(target);
                            cmd.append = op == Token::RedirectAppend;
                        }
                        Token::RedirectIn => cmd.infile = Some(target),
                        Token::RedirectAll | Token::RedirectAllAppend => {
                            cmd.outfile = Some(target);
                            cmd.append = op == Token::RedirectAllAppend;
                            cmd.err_to_out = true;
                        }
                        _ => {
                            cmd.errfile = Some(target);
                            cmd.err_append = op == Token::RedirectErrAppend;
                        }
                    }
                }
                // Stop parsing this pipeline if we hit logic operators or block closers
                Token::And
                | Token::Or
                | Token::LBrace
                | Token::RBrace
                | Token::Newline
                | Token::DoubleSemi
                | Token::Background => break,
                _ => self.cursor += 1,
            }
        }

        // Push the last command into the pipeline list
        if let Some(cmd) = current_cmd {
            pipeline.push(cmd);
        }

        if pipeline.is_empty() {
            None
        } else if pipeline.len() == 1 {
            // It's just a single command (e.g., `ls -la`)
            Some(Statement::Command(pipeline.pop().unwrap()))
        } else {
            // It's a chained pipeline (e.g., `ls -la | grep src`)
            Some(Statement::Pipeline(pipeline))
        }
    }

    /// Takes the word naming a redirect's file, if one follows.
    fn redirect_target(&mut self) -> Option<String> {
        match self.tokens.get(self.cursor) {
            Some(Token::Word(file) | Token::StringLiteral(file)) => {
                let file = file.clone();
                self.cursor += 1;
                Some(file)
            }
            _ => None,
        }
    }
}
