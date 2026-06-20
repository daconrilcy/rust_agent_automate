use std::path::PathBuf;

mod args;
mod context;
mod dispatch;
mod exec;
mod spec;

pub(crate) use args::{
    ServiceCommandParseError, parse_required_path, parse_required_path_with_optional_named_path,
    parse_subject_and_artifact,
};
pub(crate) use context::{
    prepare_prompted_service, prepare_required_path_service, resolve_context,
};
pub(crate) use dispatch::ServiceCommandDispatch;
pub(crate) use exec::{
    execute_service_command, execute_service_command_silently, save_markdown_artifact,
};
pub(crate) use spec::{SERVICE_COMMAND_SPECS, ServiceCommandDescriptor, ServiceCommandSpec};
pub use spec::{ServiceCommandKind, ServiceCommandOptions};

use crate::cli::ParseOutcome;
use crate::codex::CodexRequest;
use crate::service_paths::ExecutionContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedServiceCommand {
    pub request: CodexRequest,
    pub workspace_root: PathBuf,
    pub output_dir: PathBuf,
    pub timeout: std::time::Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedParseContext {
    pub context: ExecutionContext,
    pub workspace_root: PathBuf,
    pub output_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedSubjectArtifact<T> {
    pub subject: T,
    pub artifact_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedRequiredPath {
    pub required_path: PathBuf,
    pub optional_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RequiredPathParseSpec<'a> {
    pub required_option_name: &'a str,
    pub optional_option_name: &'a str,
    pub command_name: &'a str,
    pub required_label: &'a str,
    pub required_example: &'a str,
    pub duplicate_required_message: &'a str,
    pub duplicate_optional_message: &'a str,
    pub allow_positional: bool,
}

pub(crate) fn dispatch_service_args_for_context(
    args: &[String],
    context: &ExecutionContext,
) -> Result<Option<ServiceCommandDispatch>, crate::cli::ParseOutcome> {
    let Some(command) = args.first().map(String::as_str) else {
        return Ok(None);
    };
    let Some(kind) = ServiceCommandKind::from_name(command) else {
        return Ok(None);
    };

    kind.parse_for_context(&args[1..], context).map(Some)
}

pub(crate) fn parse_error(error: impl std::fmt::Display) -> ParseOutcome {
    ParseOutcome::Error(error.to_string())
}
