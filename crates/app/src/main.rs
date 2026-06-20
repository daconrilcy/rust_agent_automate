use std::process;

use app::cli::{self, ParseOutcome};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match cli::parse_args(&args) {
        Ok(command) => process::exit(app::codex::process_exit_code(Some(command.execute()))),
        Err(ParseOutcome::Help) => cli::print_help(),
        Err(ParseOutcome::Error(message)) => {
            eprintln!("{message}");
            eprintln!();
            cli::print_help();
            process::exit(1);
        }
    }
}
