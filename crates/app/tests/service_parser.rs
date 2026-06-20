mod support;

pub use app::{ParseOutcome, parse_timeout};

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use app::CliCommand;
use app::codex::{CodexMode, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT, ReasoningEffort};
use app::service_command::{
    ServiceCommandDispatch, ServiceCommandOptions, parse_with_common_options,
};
use support::{command_kind, normalize_path, parse, request_for};

#[test]
fn parses_common_service_options() {
    let args = [
        "--model",
        "gpt-5.6",
        "--reasoning",
        "medium",
        "--output-dir",
        "C:\\tmp\\out",
        "--timeout-seconds",
        "42",
        "--verbose",
        "--continue-codex",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    let mut options = ServiceCommandOptions::new(Duration::from_secs(900));
    parse_with_common_options(&args, &mut options, |_index, value| {
        Err(ParseOutcome::Error(format!(
            "unexpected positional: {value}"
        )))
    })
    .expect("options communes valides");

    assert_eq!(options.model, "gpt-5.6");
    assert_eq!(options.reasoning_effort, ReasoningEffort::Medium);
    assert_eq!(options.output_dir, Some(PathBuf::from("C:\\tmp\\out")));
    assert_eq!(options.timeout, Duration::from_secs(42));
    assert!(options.verbose);
    assert!(options.resume_last);
}

#[test]
fn parse_with_common_options_collects_positionals() {
    let args = ["audit.md", "--verbose"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut options = ServiceCommandOptions::new(Duration::from_secs(900));
    parse_with_common_options(&args, &mut options, |index, value| {
        if (index == 0 && value == "audit.md") || value == "--verbose" {
            Ok(1)
        } else {
            Err(ParseOutcome::Error(format!("unexpected: {value}")))
        }
    })
    .expect("valid parse");
    assert!(options.verbose);
}

#[test]
fn parses_required_path_with_optional_named_path_arguments() {
    let args = [
        "plan.md",
        "--implementation",
        "crates\\app",
        "--timeout-seconds",
        "42",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    let mut options = ServiceCommandOptions::new(Duration::from_secs(900));

    let parsed = app::service_command::parse_required_path_with_optional_named_path(
        &args,
        &mut options,
        app::service_command::RequiredPathParseSpec {
            required_option_name: "--plan",
            optional_option_name: "--implementation",
            command_name: "implementation-audit",
            required_label: "un plan",
            required_example: "cargo run -p app -- implementation-audit .plan\\plan.md",
            duplicate_required_message: "plan duplique",
            duplicate_optional_message: "implementation dupliquee",
        },
    )
    .expect("parse valide");

    assert_eq!(parsed.required_path, PathBuf::from("plan.md"));
    assert_eq!(parsed.optional_path, Some(PathBuf::from("crates\\app")));
    assert_eq!(options.timeout, Duration::from_secs(42));
}

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
        app::service_command::ServiceCommandKind::from_name("implementation-audit"),
        Some(app::service_command::ServiceCommandKind::ImplementationAudit)
    );
    assert_eq!(
        app::service_command::ServiceCommandKind::from_name("impl-audit"),
        Some(app::service_command::ServiceCommandKind::ImplementationAudit)
    );
    assert_eq!(
        app::service_command::ServiceCommandKind::from_name("loop"),
        Some(app::service_command::ServiceCommandKind::FixLoop)
    );
    assert_eq!(
        app::service_command::ServiceCommandKind::from_name("unknown"),
        None
    );
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
