//! Tareas de mantenimiento del repositorio (`cargo xtask <tarea>`).

mod docs_check;

use std::{env, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    match env::args().nth(1).as_deref() {
        Some("docs-check") => run_docs_check(),
        _ => {
            eprintln!("uso: cargo xtask docs-check");
            ExitCode::from(2)
        }
    }
}

fn run_docs_check() -> ExitCode {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let problems = docs_check::check(&root);
    if problems.is_empty() {
        println!("docs-check: sin problemas");
        ExitCode::SUCCESS
    } else {
        for problem in &problems {
            eprintln!("docs-check: {problem}");
        }
        ExitCode::FAILURE
    }
}
