pub use app::cli::{ParseOutcome, next_value, parse_timeout};

use std::time::Duration;

use app::service_command::{ServiceCommandOptions, parse_with_common_options};

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
