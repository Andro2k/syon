//! Tareas de mantenimiento del repositorio (`cargo xtask <tarea>`).

mod arch_check;
mod docs_check;
mod json;

use std::{env, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    match env::args().nth(1).as_deref() {
        Some("docs-check") => run_docs_check(),
        Some("arch-check") => run_arch_check(),
        _ => {
            eprintln!("uso: cargo xtask <docs-check|arch-check>");
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

fn run_arch_check() -> ExitCode {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let members = match arch_check::load_members(&root) {
        Ok(members) => members,
        Err(error) => {
            eprintln!("arch-check: {error}");
            return ExitCode::FAILURE;
        }
    };
    let problems = arch_check::check(&members);
    if problems.is_empty() {
        println!("arch-check: sin problemas ({} crates)", members.len());
        ExitCode::SUCCESS
    } else {
        for problem in &problems {
            eprintln!("arch-check: {problem}");
        }
        ExitCode::FAILURE
    }
}
