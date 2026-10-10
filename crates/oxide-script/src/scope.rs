use std::collections::HashMap;

pub struct Scope {
    variables: HashMap<String, String>,
    parent: Option<Box<Scope>>,
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
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

/// The variables one function call made `local`, with the values they hid.
///
/// Locals are dynamically scoped, as in other shells: while the function
/// (and anything it calls) runs, the local is simply the variable. When the
/// call ends, [`Frame::restore`] puts back what was there before.
#[derive(Default)]
pub struct Frame {
    saved: Vec<(String, Option<String>)>,
}

impl Frame {
    /// Records `name`'s value from before it was made local. Only the first
    /// declaration in a call counts; declaring it again keeps that value.
    pub fn save(&mut self, name: &str, previous: Option<String>) {
        if !self.saved.iter().any(|(saved, _)| saved == name) {
            self.saved.push((name.to_string(), previous));
        }
    }

    /// Puts every variable this call made local back as it was.
    pub fn restore(self, vars: &mut HashMap<String, String>) {
        for (name, previous) in self.saved.into_iter().rev() {
            match previous {
                Some(value) => vars.insert(name, value),
                None => vars.remove(&name),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_puts_back_what_was_hidden() {
        let mut vars = HashMap::from([("kept".to_string(), "outer".to_string())]);
        let mut frame = Frame::default();
        frame.save("kept", vars.get("kept").cloned());
        vars.insert("kept".into(), "inner".into());
        frame.save("fresh", None);
        vars.insert("fresh".into(), "inner".into());
        // A second declaration doesn't overwrite the first saved value.
        frame.save("kept", Some("inner".into()));

        frame.restore(&mut vars);
        assert_eq!(vars.get("kept").map(String::as_str), Some("outer"));
        assert_eq!(vars.get("fresh"), None);
    }
}
