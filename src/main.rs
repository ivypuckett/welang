mod lisp;

use std::env;
use std::fs;
use std::path::Path;
use std::process::{self, Command};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: we <file>");
        process::exit(1);
    }

    let source_path = &args[1];
    let source = fs::read_to_string(source_path).unwrap_or_else(|e| {
        eprintln!("we: cannot read '{}': {}", source_path, e);
        process::exit(1);
    });

    let exprs = lisp::parser::parse(&source).unwrap_or_else(|e| {
        let line_text = source.lines().nth(e.line - 1).unwrap_or("").trim_end();
        eprintln!("{}:{}: error: {}", source_path, e.line, e);
        if !line_text.is_empty() {
            eprintln!("  {}", line_text);
        }
        process::exit(1);
    });

    let obj_bytes = lisp::codegen::compile(&exprs).unwrap_or_else(|e| {
        eprintln!("we: compile error: {}", e);
        process::exit(1);
    });

    let stem = Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let obj_path = format!("{}.o", stem);
    let bin_path = stem.to_string();

    fs::write(&obj_path, &obj_bytes).unwrap_or_else(|e| {
        eprintln!("we: cannot write object file '{}': {}", obj_path, e);
        process::exit(1);
    });

    let status = Command::new("cc")
        .args(["-no-pie", "-o", &bin_path, &obj_path])
        .status()
        .unwrap_or_else(|e| {
            let _ = fs::remove_file(&obj_path);
            eprintln!("we: cannot run linker: {}", e);
            process::exit(1);
        });

    let _ = fs::remove_file(&obj_path);

    if !status.success() {
        eprintln!("we: linking failed");
        process::exit(1);
    }

    eprintln!("we: wrote '{}'", bin_path);
}
