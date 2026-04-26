use oxide_data::value::Value;

pub fn execute(args: &[String], input_data: Value) -> Value {
    if args.is_empty() {
        eprintln!("oxide: get: missing field name (e.g., 'get users')");
        return input_data;
    }

    let target_key = &args[0];

    // Check what kind of data came through the pipeline
    match input_data {
        Value::Record(mut map) => {
            // Try to extract the requested field
            if let Some(val) = map.remove(target_key) {
                val
            } else {
                eprintln!("oxide: get: field '{}' not found", target_key);
                Value::Null
            }
        }
        Value::List(_) => {
            eprintln!("oxide: get: cannot get a specific field from a List (yet)");
            Value::Null
        }
        _ => {
            eprintln!("oxide: get: input must be a Record object");
            Value::Null
        }
    }
}