mod ast;
mod codegen;
mod errors;
mod lexer;
mod parser;

use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: welang <source-file>");
        process::exit(1);
    }

    let filename = &args[1];

    let source = match std::fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", filename, e);
            process::exit(1);
        }
    };

    match compile(&source) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Compilation error: {}", e);
            process::exit(1);
        }
    }
}

/// Top-level compilation pipeline: lex -> parse -> codegen.
fn compile(source: &str) -> Result<(), errors::CompileError> {
    let tokens = lexer::lex(source)?;
    let ast = parser::parse(&tokens)?;
    codegen::generate(&ast)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_empty_source() {
        let result = compile("");
        assert!(result.is_ok(), "empty source should compile: {:?}", result);
    }
}
