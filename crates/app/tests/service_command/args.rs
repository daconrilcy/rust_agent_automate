use std::path::PathBuf;
use std::time::Duration;

use app::service_command::args::{
    parse_required_path, parse_required_path_with_optional_named_path, parse_with_common_options,
};
use app::{ParseOutcome, ReasoningEffort, RequiredPathParseSpec, ServiceCommandOptions};

#[test]
fn parse_with_common_options_collects_positionals() {
    let args = ["audit.md", "--verbose"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut options = ServiceCommandOptions::new(Duration::from_secs(900));
    let mut seen = Vec::new();

    parse_with_common_options(&args, &mut options, |index, value| {
        seen.push((index, value.to_string()));
        Ok(1)
    })
    .expect("valid parse");

    assert_eq!(seen, vec![(0, "audit.md".to_string())]);
    assert!(options.verbose);
}

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

    let parsed = parse_required_path_with_optional_named_path(
        &args,
        &mut options,
        RequiredPathParseSpec {
            required_option_name: "--plan",
            optional_option_name: "--implementation",
            command_name: "implementation-audit",
            required_label: "un plan",
            required_example: "cargo run -p app -- implementation-audit .plan\\plan.md",
            duplicate_required_message: "plan duplique",
            duplicate_optional_message: "implementation dupliquee",
            allow_positional: true,
        },
    )
    .expect("parse valide");

    assert_eq!(parsed.required_path, PathBuf::from("plan.md"));
    assert_eq!(parsed.optional_path, Some(PathBuf::from("crates\\app")));
    assert_eq!(options.timeout, Duration::from_secs(42));
}

#[test]
fn parses_required_path_arguments() {
    let args = ["Cargo.toml"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut options = ServiceCommandOptions::new(Duration::from_secs(900));

    let parsed = parse_required_path(
        &args,
        &mut options,
        RequiredPathParseSpec {
            required_option_name: "--audit",
            optional_option_name: "",
            command_name: "plan",
            required_label: "un chemin d'audit",
            required_example: "cargo run -p app -- plan Cargo.toml",
            duplicate_required_message: "audit duplique",
            duplicate_optional_message: "unused",
            allow_positional: true,
        },
    )
    .expect("parse valide");

    assert_eq!(parsed, Some(PathBuf::from("Cargo.toml")));
}
