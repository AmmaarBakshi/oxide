use std::collections::HashMap;
use oxide_parser::ast::Statement; // This should now resolve correctly

pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Statement>,
}

pub struct FunctionRegistry {
    functions: HashMap<String, Function>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self { functions: HashMap::new() }
    }

    pub fn define(&mut self, func: Function) {
        self.functions.insert(func.name.clone(), func);
    }

    pub fn get(&self, name: &str) -> Option<&Function> {
        self.functions.get(name)
    }
}