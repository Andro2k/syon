//! Binario de SYON. Es el único sitio que toca el entorno y la consola; la lógica está en el engine.

mod chat_input;
mod cli;
mod commands;
mod credentials;
mod events;

use cli::Command;
use std::process::ExitCode;

fn main() -> ExitCode {
    let command = match cli::parse(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("{message}\n\n{}", cli::USAGE);
            return ExitCode::from(2);
        }
    };
    match command {
        Command::Run => commands::run(),
        Command::Login(platform) => commands::login(platform),
        Command::Version => {
            println!("syon {}", commands::VERSION);
            ExitCode::SUCCESS
        }
        Command::Help => {
            println!("{}", cli::USAGE);
            ExitCode::SUCCESS
        }
    }
}
