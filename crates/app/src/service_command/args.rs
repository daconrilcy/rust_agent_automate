use std::path::PathBuf;

use crate::cli::{ParseOutcome, mark_seen, next_value};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceCommandParseError {
    Help,
    Message(String),
}

impl std::fmt::Display for ServiceCommandParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Help => f.write_str("help requested"),
            Self::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ServiceCommandParseError {}

impl From<ParseOutcome> for ServiceCommandParseError {
    fn from(value: ParseOutcome) -> Self {
        match value {
            ParseOutcome::Help => Self::Help,
            ParseOutcome::Error(message) => Self::Message(message),
        }
    }
}

impl ServiceCommandParseError {
    pub fn into_parse_outcome(self) -> ParseOutcome {
        match self {
            Self::Help => ParseOutcome::Help,
            Self::Message(message) => ParseOutcome::Error(message),
        }
    }
}

fn parse_outcome(error: ServiceCommandParseError) -> ParseOutcome {
    error.into_parse_outcome()
}

fn parse_common_option(
    args: &[String],
    index: usize,
    options: &mut ServiceCommandOptions,
    seen: &mut CommonOptionSeen,
) -> Result<Option<usize>, ServiceCommandParseError> {
    match args[index].as_str() {
        "-h" | "--help" => Err(ServiceCommandParseError::Help),
        "--model" => {
            mark_seen(&mut seen.model, "--model").map_err(ServiceCommandParseError::from)?;
            options.model = next_value(args, index, "--model")?.to_owned();
            Ok(Some(2))
        }
        "--reasoning" => {
            mark_seen(&mut seen.reasoning, "--reasoning")
                .map_err(ServiceCommandParseError::from)?;
            options.reasoning_effort = next_value(args, index, "--reasoning")?
                .parse()
                .map_err(ServiceCommandParseError::Message)?;
            Ok(Some(2))
        }
        "--output-dir" => {
            mark_seen(&mut seen.output_dir, "--output-dir")
                .map_err(ServiceCommandParseError::from)?;
            options.output_dir = Some(PathBuf::from(next_value(args, index, "--output-dir")?));
            Ok(Some(2))
        }
        "--timeout-seconds" => {
            mark_seen(&mut seen.timeout, "--timeout-seconds")
                .map_err(ServiceCommandParseError::from)?;
            options.timeout =
                crate::cli::parse_timeout(next_value(args, index, "--timeout-seconds")?)
                    .map_err(ServiceCommandParseError::from)?;
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

fn duplicate_argument(command_name: &str, label: &str) -> ServiceCommandParseError {
    ServiceCommandParseError::Message(format!("{label} de {command_name} a deja ete fourni"))
}

fn reject_unknown_option(value: &str) -> Result<usize, ServiceCommandParseError> {
    Err(ServiceCommandParseError::Message(format!(
        "option inconnue: {value}"
    )))
}

fn capture_named_path(
    slot: &mut Option<PathBuf>,
    args: &[String],
    index: usize,
    option_name: &str,
    duplicate_error: ServiceCommandParseError,
) -> Result<usize, ServiceCommandParseError> {
    let value = next_value(args, index, option_name).map_err(ServiceCommandParseError::from)?;
    if slot.is_some() {
        return Err(duplicate_error);
    }
    *slot = Some(PathBuf::from(value));
    Ok(2)
}

fn capture_positional_path(
    slot: &mut Option<PathBuf>,
    command_name: &str,
    value: &str,
) -> Result<usize, ServiceCommandParseError> {
    if slot.is_some() {
        return Err(ServiceCommandParseError::Message(format!(
            "argument inattendu pour {command_name}: {value}"
        )));
    }

    *slot = Some(PathBuf::from(value));
    Ok(1)
}

pub fn parse_with_common_options<F>(
    args: &[String],
    options: &mut ServiceCommandOptions,
    mut argument_handler: F,
) -> Result<(), ParseOutcome>
where
    F: FnMut(usize, &str) -> Result<usize, ParseOutcome>,
{
    parse_with_common_options_internal(args, options, |index, value| {
        argument_handler(index, value).map_err(ServiceCommandParseError::from)
    })
    .map_err(parse_outcome)
}

fn parse_with_common_options_internal<F>(
    args: &[String],
    options: &mut ServiceCommandOptions,
    mut argument_handler: F,
) -> Result<(), ServiceCommandParseError>
where
    F: FnMut(usize, &str) -> Result<usize, ServiceCommandParseError>,
{
    let mut seen = CommonOptionSeen::default();
    let mut index = 0;
    while index < args.len() {
        let consumed = if let Some(consumed) = parse_common_option(args, index, options, &mut seen)?
        {
            consumed
        } else {
            argument_handler(index, args[index].as_str())?
        };

        if consumed == 0 {
            return Err(ServiceCommandParseError::Message(format!(
                "le parseur partage doit consommer au moins un argument: {}",
                args[index]
            )));
        }

        index += consumed;
    }
    Ok(())
}

pub fn parse_subject_and_artifact<T, F>(
    args: &[String],
    options: &mut ServiceCommandOptions,
    subject_name: &str,
    artifact_name: &str,
    command_name: &str,
    parse_subject: F,
) -> Result<ParsedSubjectArtifact<T>, ServiceCommandParseError>
where
    F: Fn(&str) -> Result<T, ServiceCommandParseError>,
{
    let mut subject: Option<T> = None;
    let mut artifact_path: Option<PathBuf> = None;
    let mut seen = CommonOptionSeen::default();
    let mut index = 0;

    while index < args.len() {
        if let Some(consumed) = parse_common_option(args, index, options, &mut seen)
            .map_err(ServiceCommandParseError::from)?
        {
            index += consumed;
            continue;
        }

        let value = args[index].as_str();
        let consumed = match value {
            value if value == subject_name => {
                let value = next_value(args, index, subject_name)
                    .map_err(ServiceCommandParseError::from)?;
                if subject.is_some() {
                    return Err(ServiceCommandParseError::from(duplicate_argument(
                        command_name,
                        "le type",
                    )));
                }
                subject = Some(parse_subject(value)?);
                2
            }
            value if value == artifact_name => capture_named_path(
                &mut artifact_path,
                args,
                index,
                artifact_name,
                duplicate_argument(command_name, "l'artefact"),
            )
            .map_err(ServiceCommandParseError::from)?,
            value if value.starts_with("--") => {
                return Err(ServiceCommandParseError::from(
                    reject_unknown_option(value).expect_err("unknown option must error"),
                ));
            }
            value => {
                if subject.is_none() {
                    subject = Some(parse_subject(value)?);
                    1
                } else {
                    capture_positional_path(&mut artifact_path, command_name, value)
                        .map_err(ServiceCommandParseError::from)?
                }
            }
        };

        if consumed == 0 {
            return Err(ServiceCommandParseError::Message(format!(
                "le parseur partage doit consommer au moins un argument: {}",
                args[index]
            )));
        }

        index += consumed;
    }

    let subject = subject.ok_or_else(|| {
        ServiceCommandParseError::Message(format!(
            "la commande {command_name} requiert un type: plan, audit ou implementation"
        ))
    })?;
    let artifact_path = artifact_path.ok_or_else(|| {
        ServiceCommandParseError::Message(format!(
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

    parse_with_common_options_internal(args, options, |index, value| match value {
        value if value == spec.required_option_name => capture_named_path(
            &mut required_path,
            args,
            index,
            spec.required_option_name,
            ServiceCommandParseError::Message(spec.duplicate_required_message.to_string()),
        ),
        value if value == spec.optional_option_name => capture_named_path(
            &mut optional_path,
            args,
            index,
            spec.optional_option_name,
            ServiceCommandParseError::Message(spec.duplicate_optional_message.to_string()),
        ),
        value if value.starts_with("--") => reject_unknown_option(value),
        value => capture_positional_path(&mut required_path, spec.command_name, value),
    })
    .map_err(parse_outcome)?;

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
