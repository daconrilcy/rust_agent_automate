//! Internal-first library surface for the `app` binary and its integration tests.
//! The exported modules below are supported as crate-local seams, not as a long-term
//! general-purpose public API contract.

pub mod artifact;
mod artifact_subject;
mod audit;
pub mod automate;
mod cli;
pub mod codex;
mod command_registry;
mod fix_loop;
mod implementation_audit;
mod plan;
pub mod prompt;
mod reporting;
mod review;
pub mod service_command;
pub mod service_paths;

pub use artifact_subject::ReviewSubject;
pub use automate::{WorkflowStepKind, default_refactor_workflow, parse_workflow};
pub use cli::{CliCommand, ParseOutcome, parse_args, parse_timeout, print_help};
pub use codex::{
    AgentContext, CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT,
    ReasoningEffort, RunResult, process_exit_code,
};
pub use command_registry::registered_commands;
pub use reporting::{
    CommandOutcome, ReportFailure, ReportSpec, command_failure_outcome,
    detect_clean_implementation_audit, finalize_report, write_command_outcome,
};
pub use service_command::{
    RequiredPathParseSpec, ServiceCommandDescriptor, ServiceCommandKind, ServiceCommandOptions,
};
