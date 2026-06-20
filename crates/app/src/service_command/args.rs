use std::path::PathBuf;

use crate::cli::{ParseLoopControl, ParseOutcome, next_value, scan_args};

use super::{
    ParsedRequiredPath, ParsedSubjectArtifact, RequiredPathParseSpec, ServiceCommandOptions,
};

#[derive(Default)]
struct CommonOptionSeen {
    model: bool,
    reasoning: bool,
    output_dir: bool,
    timeout: bool,
}

fn parse_common_option(
    args: &[String],
    index: usize,
    options: &mut ServiceCommandOptions,
    seen: &mut CommonOptionSeen,
) -> Result<Option<usize>, ParseOutcome> {
    match args[index].as_str() {
        "-h" | "--help" => Err(ParseOutcome::Help),
        "--model" => {
            if seen.model {
                return Err(ParseOutcome::Error(
                    "l'option --model a deja ete fournie".to_string(),
                ));
            }
            seen.model = true;
            options.model = next_value(args, index, "--model")?.to_owned();
            Ok(Some(2))
        }
        "--reasoning" => {
            if seen.reasoning {
                return Err(ParseOutcome::Error(
                    "l'option --reasoning a deja ete fournie".to_string(),
                ));
            }
            seen.reasoning = true;
            options.reasoning_effort = next_value(args, index, "--reasoning")?
                .parse()
                .map_err(ParseOutcome::Error)?;
            Ok(Some(2))
        }
        "--output-dir" => {
            if seen.output_dir {
                return Err(ParseOutcome::Error(
                    "l'option --output-dir a deja ete fournie".to_string(),
                ));
            }
            seen.output_dir = true;
            options.output_dir = Some(PathBuf::from(next_value(args, index, "--output-dir")?));
            Ok(Some(2))
        }
        "--timeout-seconds" => {
            if seen.timeout {
                return Err(ParseOutcome::Error(
                    "l'option --timeout-seconds a deja ete fournie".to_string(),
                ));
            }
            seen.timeout = true;
            options.timeout =
                crate::cli::parse_timeout(next_value(args, index, "--timeout-seconds")?)?;
            Ok(Some(2))
        }
        "--verbose" => {
            options.verbose = true;
            Ok(Some(1))
        }
        "--continue-codex" => {
            options.resume_last = true;
            Ok(Some(1))
        }
        _ => Ok(None),
    }
}

pub fn parse_with_common_options<F>(
    args: &[String],
    options: &mut ServiceCommandOptions,
    mut argument_handler: F,
) -> Result<(), ParseOutcome>
where
    F: FnMut(usize, &str) -> Result<usize, ParseOutcome>,
{
    let mut seen = CommonOptionSeen::default();
    scan_args(args, |index, _value| {
        if let Some(consumed) = parse_common_option(args, index, options, &mut seen)? {
            return Ok(ParseLoopControl::Continue(consumed));
        }

        let consumed = argument_handler(index, args[index].as_str())?;
        Ok(ParseLoopControl::Continue(consumed))
    })?;
    Ok(())
}

pub fn parse_subject_and_artifact<T, F>(
    args: &[String],
    options: &mut ServiceCommandOptions,
    subject_name: &str,
    artifact_name: &str,
    command_name: &str,
    parse_subject: F,
) -> Result<ParsedSubjectArtifact<T>, ParseOutcome>
where
    F: Fn(&str) -> Result<T, String>,
{
    let mut subject: Option<T> = None;
    let mut artifact_path: Option<PathBuf> = None;

    parse_with_common_options(args, options, |index, value| match value {
        value if value == subject_name => {
            let value = next_value(args, index, subject_name)?;
            if subject.is_some() {
                return Err(ParseOutcome::Error(format!(
                    "le type de {command_name} a deja ete fourni"
                )));
            }
            subject = Some(parse_subject(value).map_err(ParseOutcome::Error)?);
            Ok(2)
        }
        value if value == artifact_name => {
            let value = next_value(args, index, artifact_name)?;
            if artifact_path.is_some() {
                return Err(ParseOutcome::Error(format!(
                    "l'artefact de {command_name} a deja ete fourni"
                )));
            }
            artifact_path = Some(PathBuf::from(value));
            Ok(2)
        }
        value if value.starts_with("--") => {
            Err(ParseOutcome::Error(format!("option inconnue: {value}")))
        }
        value => {
            if subject.is_none() {
                subject = Some(parse_subject(value).map_err(ParseOutcome::Error)?);
                Ok(1)
            } else if artifact_path.is_none() {
                artifact_path = Some(PathBuf::from(value));
                Ok(1)
            } else {
                Err(ParseOutcome::Error(format!(
                    "argument inattendu pour {command_name}: {value}"
                )))
            }
        }
    })?;

    let subject = subject.ok_or_else(|| {
        ParseOutcome::Error(format!(
            "la commande {command_name} requiert un type: plan, audit ou implementation"
        ))
    })?;
    let artifact_path = artifact_path.ok_or_else(|| {
        ParseOutcome::Error(format!(
            "la commande {command_name} requiert un artefact. Exemple: cargo run -p app -- {command_name} plan .plan\\plan.md"
        ))
    })?;

    Ok(ParsedSubjectArtifact {
        subject,
        artifact_path,
    })
}

pub fn parse_required_path_with_optional_named_path(
    args: &[String],
    options: &mut ServiceCommandOptions,
    spec: RequiredPathParseSpec<'_>,
) -> Result<ParsedRequiredPath, ParseOutcome> {
    let mut required_path: Option<PathBuf> = None;
    let mut optional_path: Option<PathBuf> = None;

    parse_with_common_options(args, options, |index, value| match value {
        value if value == spec.required_option_name => {
            let value = next_value(args, index, spec.required_option_name)?;
            if required_path.is_some() {
                return Err(ParseOutcome::Error(
                    spec.duplicate_required_message.to_string(),
                ));
            }
            required_path = Some(PathBuf::from(value));
            Ok(2)
        }
        value if value == spec.optional_option_name => {
            let value = next_value(args, index, spec.optional_option_name)?;
            if optional_path.is_some() {
                return Err(ParseOutcome::Error(
                    spec.duplicate_optional_message.to_string(),
                ));
            }
            optional_path = Some(PathBuf::from(value));
            Ok(2)
        }
        value if value.starts_with("--") => {
            Err(ParseOutcome::Error(format!("option inconnue: {value}")))
        }
        value => {
            if required_path.is_some() {
                Err(ParseOutcome::Error(format!(
                    "argument inattendu pour {}: {value}",
                    spec.command_name
                )))
            } else {
                required_path = Some(PathBuf::from(value));
                Ok(1)
            }
        }
    })?;

    let required_path = required_path.ok_or_else(|| {
        ParseOutcome::Error(format!(
            "la commande {} requiert {}. Exemple: {}",
            spec.command_name, spec.required_label, spec.required_example
        ))
    })?;

    Ok(ParsedRequiredPath {
        required_path,
        optional_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReasoningEffort;
    use std::time::Duration;

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
        let mut index = 0;
        let mut seen = CommonOptionSeen::default();

        while index < args.len() {
            let consumed = parse_common_option(&args, index, &mut options, &mut seen)
                .expect("options communes valides");
            index += consumed.expect("chaque option doit etre reconnue");
        }

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
            },
        )
        .expect("parse valide");

        assert_eq!(parsed.required_path, PathBuf::from("plan.md"));
        assert_eq!(parsed.optional_path, Some(PathBuf::from("crates\\app")));
        assert_eq!(options.timeout, Duration::from_secs(42));
    }
}
