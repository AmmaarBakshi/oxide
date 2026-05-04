use std::collections::HashMap;

pub type NativeCallback = fn(Vec<String>) -> String;

pub struct StdLib {
    native_functions: HashMap<String, NativeCallback>,
}

impl StdLib {
    pub fn new() -> Self {
        let mut lib = Self { native_functions: HashMap::new() };
        
        // Add a 'math_add' native function
        lib.native_functions.insert("math_add".to_string(), |args| {
            let sum: i32 = args.iter().map(|s| s.parse::<i32>().unwrap_or(0)).sum();
            sum.to_string()
        });

        // Add a 'str_upper' native function
        lib.native_functions.insert("str_upper".to_string(), |args| {
            args.join(" ").to_uppercase()
        });

        lib
    }

    pub fn call(&self, name: &str, args: Vec<String>) -> Option<String> {
        self.native_functions.get(name).map(|f| f(args))
    }
}