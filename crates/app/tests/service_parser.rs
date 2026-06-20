mod support;

pub use app::{ParseOutcome, parse_timeout};

use std::path::Path;
use std::time::Duration;

use app::CliCommand;
use app::{
    CodexMode, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT, ReasoningEffort, ServiceCommandKind,
    ServiceCommandOptions,
};
use support::{command_kind, normalize_path, parse, request_for};

#[test]
fn service_command_specs_cover_aliases_and_exec_request_shape() {
    let options = ServiceCommandOptions {
        model: "gpt-5.7".to_string(),
        reasoning_effort: ReasoningEffort::High,
        verbose: true,
        resume_last: true,
        output_dir: None,
        timeout: Duration::from_secs(12),
    };

    let request = options.build_request("Prompt".to_string());
    assert_eq!(request.model, "gpt-5.7");
    assert_eq!(request.reasoning_effort, ReasoningEffort::High);
    assert_eq!(request.mode, CodexMode::Exec);
    assert_eq!(request.prompt.as_deref(), Some("Prompt"));
    assert!(request.verbose);
    assert!(request.resume_last);
    assert_eq!(request.working_dir, None);

    assert_eq!(
        ServiceCommandKind::from_name("implementation-audit"),
        Some(ServiceCommandKind::ImplementationAudit)
    );
    assert_eq!(
        ServiceCommandKind::from_name("impl-audit"),
        Some(ServiceCommandKind::ImplementationAudit)
    );
    assert_eq!(
        ServiceCommandKind::from_name("loop"),
        Some(ServiceCommandKind::FixLoop)
    );
    assert_eq!(ServiceCommandKind::from_name("unknown"), None);
}

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
