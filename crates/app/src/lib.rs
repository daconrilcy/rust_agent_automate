//! Thin application facade for the `app` binary and behavior-level integration tests.

mod artifact;
mod artifact_subject;
mod audit;
mod automate;
mod cli;
mod codex;
mod command_registry;
mod fix_loop;
mod implementation_audit;
mod plan;
mod prompt;
mod reporting;
mod review;
mod service_command;
mod service_paths;

pub use artifact_subject::ReviewSubject;
pub use audit::AuditCommand;
pub use automate::{
    AutomateCommand, RefactorAutomateCommand, Workflow, WorkflowStepKind,
    default_refactor_workflow, parse_workflow,
};
pub use cli::{CliCommand, ParseOutcome, parse_args, print_help};
pub use codex::{
    AgentContext, AgentPermissions, ApprovalPolicy, CodexMode, CodexRequest, DEFAULT_MODEL,
    DEFAULT_REASONING_EFFORT, ReasoningEffort, SandboxMode, process_exit_code,
};
pub use command_registry::{RegisteredCommand, registered_commands};
pub use fix_loop::FixLoopCommand;
pub use implementation_audit::ImplementationAuditCommand;
pub use plan::PlanCommand;
pub use review::ReviewCommand;
pub use service_command::{PreparedServiceCommand, ServiceCommandDispatch};
