#[path = "../src/artifact.rs"]
mod artifact;
#[path = "../src/audit.rs"]
mod audit;
#[path = "../src/automate.rs"]
mod automate;
#[path = "../src/cli.rs"]
mod cli;
#[path = "../src/codex.rs"]
mod codex;
#[path = "../src/command_registry.rs"]
mod command_registry;
#[path = "../src/fix_loop.rs"]
mod fix_loop;
#[path = "../src/implementation_audit.rs"]
mod implementation_audit;
#[path = "../src/plan.rs"]
mod plan;
#[path = "../src/reporting.rs"]
mod reporting;
#[path = "../src/review.rs"]
mod review;
#[path = "../src/service_command.rs"]
mod service_command;
#[path = "../src/service_paths.rs"]
mod service_paths;

pub use cli::{ParseOutcome, next_value, parse_timeout};

use std::time::Duration;

use service_command::{ServiceCommandOptions, parse_with_common_options};

#[test]
fn shared_parser_handles_named_and_positional_inputs() {
    let args = [
        "plan.md",
        "--artifact",
        "impl",
        "--timeout-seconds",
        "42",
        "--verbose",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    let mut common = ServiceCommandOptions::new(Duration::from_secs(900));
    let mut artifact = None;

    parse_with_common_options(&args, &mut common, |index, value| {
        if value == "--artifact" {
            artifact = Some(args[index + 1].clone());
            return Ok(2);
        }
        Ok(1)
    })
    .expect("parse");

    assert_eq!(artifact.as_deref(), Some("impl"));
    assert_eq!(common.timeout, Duration::from_secs(42));
    assert!(common.verbose);
}
