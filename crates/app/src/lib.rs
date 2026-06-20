//! Internal-first library surface for the `app` binary and its integration tests.
//! The exported modules below are supported as crate-local seams, not as a long-term
//! general-purpose public API contract.

mod artifact;
mod artifact_subject;
mod audit;
pub mod automate;
mod cli;
pub mod codex;
mod command_registry;
mod fix_loop;
mod implementation_audit;
mod plan;
mod prompt;
pub mod reporting;
mod review;
pub mod service_command;
mod service_paths;

pub use artifact_subject::ReviewSubject;
pub use cli::{CliCommand, ParseOutcome, parse_args, parse_timeout, print_help};
pub use command_registry::registered_commands;
