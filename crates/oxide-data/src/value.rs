use std::collections::HashMap;

/// The universal data type that flows through the Oxide pipeline
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    List(Vec<Value>),
    Record(HashMap<String, Value>), 
}

impl Value {
    // A quick helper to turn a Value back into a string for printing
    pub fn to_string_lossy(&self) -> String {
        match self {
            Value::Null => "".to_string(),
            Value::String(s) => s.clone(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::List(l) => format!("[{} items]", l.len()),
            Value::Record(r) => format!("{{{} keys}}", r.len()),
        }
    }
}