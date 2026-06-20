#[path = "automate/step_args.rs"]
pub mod step_args;
#[path = "automate/step_outcome.rs"]
pub mod step_outcome;
#[path = "automate/workflow_model.rs"]
pub mod workflow_model;
#[path = "automate/workflow_runner.rs"]
pub mod workflow_runner;

#[allow(unused_imports)]
pub use step_outcome::{AutomateReport, StepResult};
#[allow(unused_imports)]
pub use workflow_model::{
    AutomateCommand, LoopPolicy, RefactorAutomateCommand, Workflow, WorkflowDefaults, WorkflowStep,
    default_refactor_workflow, load_workflow, parse_workflow, resolve_target_dir,
};
#[allow(unused_imports)]
pub use workflow_runner::run_workflow;
