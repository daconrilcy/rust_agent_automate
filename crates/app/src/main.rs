use std::process;

use app::process_exit_code;
use app::{ParseOutcome, parse_args, print_help};

fn main() {
    let _ = dotenvy::dotenv();

    let args: Vec<String> = std::env::args().skip(1).collect();

    match parse_args(&args) {
        Ok(command) => process::exit(process_exit_code(Some(command.execute()))),
        Err(ParseOutcome::Help) => print_help(),
        Err(ParseOutcome::Error(message)) => {
            eprintln!("{message}");
            eprintln!();
            print_help();
            process::exit(1);
        }
    }
}
