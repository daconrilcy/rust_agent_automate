mod support;

pub use app::ParseOutcome;
pub use app::{CodexMode, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT, ReasoningEffort};
pub use std::time::Duration;

use std::path::Path;

use app::CliCommand;
use support::{command_kind, normalize_path, parse, request_for};

fn refactor_workflow_path() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("workflows")
        .join("refactor.json")
        .to_string_lossy()
        .into_owned()
}

#[path = "service_parser/automate_cases.rs"]
mod automate_cases;
#[path = "service_parser/run_cases.rs"]
mod run_cases;
#[path = "service_parser/service_cases.rs"]
mod service_cases;
