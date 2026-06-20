mod artifact_resolution;
mod loop_control;
mod parse;
mod step_args;
mod step_outcome;
mod transport;
mod workflow_model;
mod workflow_runner;

pub(crate) use parse::{AutomationError, classify_step_kind};
pub(crate) use parse::{parse_automate_args, parse_refactor_automate_args};
pub(crate) use workflow_model::{
    AutomateCommand, RefactorAutomateCommand, Workflow, WorkflowParseError,
};
pub use workflow_model::{WorkflowStepKind, default_refactor_workflow, parse_workflow};
pub(crate) use workflow_runner::run_workflow;
