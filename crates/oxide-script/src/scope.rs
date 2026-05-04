use std::collections::HashMap;

pub struct Scope {
    variables: HashMap<String, String>,
    parent: Option<Box<Scope>>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            parent: None,
        }
    }

    pub fn set(&mut self, name: String, value: String) {
        self.variables.insert(name, value);
    }

    pub fn get(&self, name: &str) -> Option<String> {
        if let Some(val) = self.variables.get(name) {
            Some(val.clone())
        } else if let Some(ref parent) = self.parent {
            parent.get(name)
        } else {
            None
        }
    }
}