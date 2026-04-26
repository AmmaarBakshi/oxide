use crate::value::Value;
use std::collections::HashMap;

/// Takes a raw string of text and attempts to parse it into an Oxide Value
pub fn parse(raw_text: &str) -> Result<Value, String> {
    // 1. Let Serde do the heavy lifting of reading the raw text
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(raw_text);

    match parsed {
        Ok(serde_val) => Ok(convert_serde_to_oxide(serde_val)),
        Err(e) => Err(format!("Failed to parse JSON: {}", e)),
    }
}

/// A recursive helper to map Serde's generic data types to our custom Oxide data types
fn convert_serde_to_oxide(serde_val: serde_json::Value) -> Value {
    match serde_val {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::String(s) => Value::String(s),
        
        // Serde lumps all numbers together. We need to split them into Ints and Floats
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null 
            }
        }
        
        // RECURSION: If it's a list, loop through and convert every item inside!
        serde_json::Value::Array(arr) => {
            let mut list = Vec::new();
            for item in arr {
                list.push(convert_serde_to_oxide(item));
            }
            Value::List(list)
        }
        
        // RECURSION: If it's an object, loop through and convert every key-value pair!
        serde_json::Value::Object(obj) => {
            let mut record = HashMap::new();
            for (key, val) in obj {
                record.insert(key, convert_serde_to_oxide(val));
            }
            Value::Record(record)
        }
    }
}