use std::fs;
use oxide_parser::lexer::Lexer;
use oxide_parser::parser::Parser;
use oxide_script::Runtime;

pub fn execute(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("oxide: source: missing filename");
        return 1;
    }

    let filename = &args[0];
    match fs::read_to_string(filename) {
        Ok(contents) => {
            // 1. Tokenize the script
            let tokens = Lexer::new(&contents).tokenize();

            // 2. Parse into AST
            let mut parser = Parser::new(tokens);
            let executables = parser.parse();

            // 3. Convert Executables to Statements for the Runtime
            let statements = executables.into_iter().map(|e| e.statement).collect();

            // 4. Initialize a temporary runtime to execute the script
            let mut runtime = Runtime::new();
            runtime.run_script(statements);
            
            // Return 0 for success!
            0 
        }
        Err(e) => {
            eprintln!("oxide: source: failed to read '{}': {}", filename, e);
            1
        }
    }
}