use std::path::PathBuf;

mod args;
mod context;
mod dispatch;
mod exec;
mod spec;

#[allow(unused_imports)]
pub use args::{
    parse_required_path_with_optional_named_path, parse_subject_and_artifact,
    parse_with_common_options,
};
pub use context::{
    prepare_parse_context_for_context, prepare_service_from_prompt, resolve_context,
};
pub use dispatch::ServiceCommandDispatch;
pub use exec::{execute_service_command, execute_service_command_silently, save_markdown_artifact};
#[allow(unused_imports)]
pub use spec::{
    AUDIT_SERVICE_COMMAND_SPEC, FIX_LOOP_SERVICE_COMMAND_SPEC,
    IMPLEMENTATION_AUDIT_SERVICE_COMMAND_SPEC, PLAN_SERVICE_COMMAND_SPEC,
    REVIEW_SERVICE_COMMAND_SPEC, SERVICE_COMMAND_SPECS, ServiceCommandDescriptor,
    ServiceCommandKind, ServiceCommandOptions, ServiceCommandSpec,
};

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
pub struct PreparedParseContext {
    pub context: ExecutionContext,
    pub workspace_root: PathBuf,
    pub output_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSubjectArtifact<T> {
    pub subject: T,
    pub artifact_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRequiredPath {
    pub required_path: PathBuf,
    pub optional_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
pub struct RequiredPathParseSpec<'a> {
    pub required_option_name: &'a str,
    pub optional_option_name: &'a str,
    pub command_name: &'a str,
    pub required_label: &'a str,
    pub required_example: &'a str,
    pub duplicate_required_message: &'a str,
    pub duplicate_optional_message: &'a str,
}
