use crate::scope::Scope;
use crate::functions::FunctionRegistry;
use crate::stdlib::StdLib;
use crate::modules::ModuleManager;
use oxide_parser::ast::Statement; // Fixes E0425, E0433

pub struct Runtime {
    pub scope: Scope,
    pub functions: FunctionRegistry,
    pub stdlib: StdLib,
    pub modules: ModuleManager,
}

impl Runtime {
    pub fn run_script(&mut self, statements: Vec<Statement>) {
        for stmt in statements {
            match stmt {
                // Bug #1 Fix: Match the tuple (name, args)
                Statement::Command(name, args) => {
                    println!("DEBUG: Running command: {}", cmd.program);
                    // Use the fields directly from the cmd struct
                    self.execute_command(cmd);
                }
                Statement::If { condition, body, else_if, else_body } => {
                    // Bug #5 Fix: Call eval_condition
                    if self.eval_condition(&condition) {
                        self.run_script(body);
                    } else {
                        let mut matched = false;
                        for (elif_cond, elif_body) in else_if {
                            if self.eval_condition(&elif_cond) {
                                self.run_script(elif_body);
                                matched = true;
                                break;
                            }
                        }
                        if !matched {
                            if let Some(eb) = else_body { self.run_script(eb); }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Renamed from evaluate_condition to eval_condition
    fn eval_condition(&mut self, condition: &str) -> bool {
        // Your logic for [ $STATUS -eq 1 ] goes here
        !condition.is_empty() 
    }
}