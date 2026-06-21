pub mod artifact_resolution;
mod loop_control;
mod parse;
pub mod step_args;
pub mod step_outcome;
pub mod transport;
pub mod workflow_model;
pub mod workflow_runner;

pub use parse::AutomationError;
pub(crate) use parse::classify_step_kind;
pub(crate) use parse::{parse_automate_args, parse_refactor_automate_args};
pub(crate) use workflow_model::{
    AutomateCommand, RefactorAutomateCommand, Workflow, WorkflowParseError,
};
pub use workflow_model::{WorkflowStepKind, default_refactor_workflow, parse_workflow};
pub(crate) use workflow_runner::run_workflow;
