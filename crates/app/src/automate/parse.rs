use std::path::PathBuf;

use crate::cli::{ParseLoopControl, ParseOutcome, mark_seen_with_message, next_value, scan_args};
use crate::command_registry;
use crate::service_paths::{self, ExecutionContext, PathRequirement};

use super::workflow_model::load_workflow;
use super::{
    AutomateCommand, RefactorAutomateCommand, WorkflowParseError, default_refactor_workflow,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AutomationError {
    UnsupportedServiceCommand {
        command_name: String,
    },
    InternalParseFailure {
        message: String,
    },
    MissingStructuredResult {
        step_name: String,
    },
    StructuredResultDecode {
        message: String,
    },
    ArtifactNormalization {
        message: String,
    },
    LoopPolicyState {
        message: String,
    },
    StepFailed {
        step_name: String,
        status_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
}

impl AutomationError {
    pub(crate) fn unsupported_service_command(command_name: Option<&str>) -> Self {
        Self::UnsupportedServiceCommand {
            command_name: command_name.unwrap_or_default().to_string(),
        }
    }

    pub(crate) fn internal_parse_failure(error: crate::cli::ParseOutcome) -> Self {
        Self::InternalParseFailure {
            message: format!("{error:?}"),
        }
    }

    pub(crate) fn missing_structured_result(step_name: &str) -> Self {
        Self::MissingStructuredResult {
            step_name: step_name.to_string(),
        }
    }

    pub(crate) fn structured_result_decode(error: impl Into<String>) -> Self {
        Self::StructuredResultDecode {
            message: error.into(),
        }
    }

    pub(crate) fn artifact_normalization(error: impl Into<String>) -> Self {
        Self::ArtifactNormalization {
            message: error.into(),
        }
    }

    pub(crate) fn loop_policy_state(error: impl Into<String>) -> Self {
        Self::LoopPolicyState {
            message: error.into(),
        }
    }
}

impl std::fmt::Display for AutomationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedServiceCommand { command_name } => write!(
                f,
                "l'etape automate '{}' n'est pas une commande de service prise en charge",
                command_name
            ),
            Self::InternalParseFailure { message } => {
                write!(f, "echec du parseur interne: {message}")
            }
            Self::MissingStructuredResult { step_name } => write!(
                f,
                "l'etape automate '{}' doit produire un resultat structure",
                step_name
            ),
            Self::StructuredResultDecode { message }
            | Self::ArtifactNormalization { message }
            | Self::LoopPolicyState { message } => f.write_str(message),
            Self::StepFailed {
                step_name,
                status_code,
                stdout,
                stderr,
            } => {
                write!(
                    f,
                    "l'etape automate '{}' a echoue avec le statut {}",
                    step_name,
                    status_code.map_or_else(|| "inconnu".to_string(), |c| c.to_string())
                )?;
                if !stdout.trim().is_empty() {
                    write!(f, "\nstdout:\n{}", stdout.trim())?;
                }
                if !stderr.trim().is_empty() {
                    write!(f, "\nstderr:\n{}", stderr.trim())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for AutomationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TargetDirResolutionError {
    message: String,
}

impl TargetDirResolutionError {
    fn new(message: String) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for TargetDirResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for TargetDirResolutionError {}

#[derive(Debug)]
enum AutomateParseError {
    ParseOutcome(ParseOutcome),
    WorkflowParse(WorkflowParseError),
    WorkspaceRoot(crate::service_paths::PathResolutionError),
    TargetDir(TargetDirResolutionError),
}

impl From<ParseOutcome> for AutomateParseError {
    fn from(value: ParseOutcome) -> Self {
        Self::ParseOutcome(value)
    }
}

impl From<WorkflowParseError> for AutomateParseError {
    fn from(value: WorkflowParseError) -> Self {
        Self::WorkflowParse(value)
    }
}

impl From<crate::service_paths::PathResolutionError> for AutomateParseError {
    fn from(value: crate::service_paths::PathResolutionError) -> Self {
        Self::WorkspaceRoot(value)
    }
}

impl From<TargetDirResolutionError> for AutomateParseError {
    fn from(value: TargetDirResolutionError) -> Self {
        Self::TargetDir(value)
    }
}

impl AutomateParseError {
    fn into_parse_outcome(self) -> ParseOutcome {
        match self {
            Self::ParseOutcome(outcome) => outcome,
            Self::WorkflowParse(error) => ParseOutcome::Error(error.to_string()),
            Self::WorkspaceRoot(error) => ParseOutcome::Error(error.to_string()),
            Self::TargetDir(error) => ParseOutcome::Error(error.to_string()),
        }
    }
}

pub(crate) fn classify_step_kind(rust_command: &[String]) -> super::WorkflowStepKind {
    let Some(first) = rust_command.first().map(String::as_str) else {
        return super::WorkflowStepKind::DirectRun;
    };

    if command_registry::accepts_codex_options(first) {
        super::WorkflowStepKind::ServiceCommand
    } else if command_registry::is_direct_run(Some(first)) {
        super::WorkflowStepKind::DirectRun
    } else {
        super::WorkflowStepKind::NestedCommand
    }
}

fn resolve_target_dir(
    path: PathBuf,
    context: &ExecutionContext,
) -> Result<PathBuf, TargetDirResolutionError> {
    service_paths::resolve_existing_path(path, "dossier cible", PathRequirement::Directory, context)
        .map_err(|error| {
            TargetDirResolutionError::new(
                error
                    .to_string()
                    .replace("le chemin dossier cible", "le dossier cible"),
            )
        })
}

pub fn parse_automate_args(args: &[String]) -> Result<AutomateCommand, ParseOutcome> {
    parse_automate_args_internal(args).map_err(AutomateParseError::into_parse_outcome)
}

fn parse_automate_args_internal(args: &[String]) -> Result<AutomateCommand, AutomateParseError> {
    let mut workflow_path: Option<PathBuf> = None;
    let mut prompt_parts: Vec<String> = Vec::new();
    let mut named_workflow = false;

    let prompt_start = scan_args(args, |index, value| match value {
        "-h" | "--help" => Err(ParseOutcome::Help),
        "--workflow" => {
            mark_seen_with_message(
                &mut named_workflow,
                "le workflow automate a deja ete fourni",
            )?;
            let value = next_value(args, index, "--workflow")?;
            if workflow_path.is_some() {
                return Err(ParseOutcome::Error(
                    "le workflow automate a deja ete fourni".to_string(),
                ));
            }
            workflow_path = Some(PathBuf::from(value));
            Ok(ParseLoopControl::Continue(2))
        }
        value if value.starts_with("--") => {
            Err(ParseOutcome::Error(format!("option inconnue: {value}")))
        }
        value => {
            if workflow_path.is_none() {
                workflow_path = Some(PathBuf::from(value));
                Ok(ParseLoopControl::Continue(1))
            } else {
                Ok(ParseLoopControl::CaptureRest)
            }
        }
    })
    .map_err(AutomateParseError::from)?;

    if let Some(index) = prompt_start {
        prompt_parts.extend_from_slice(&args[index..]);
    }

    let workflow_path = workflow_path.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande automate requiert un workflow JSON. Exemple: cargo run -p app -- automate workflow.json \"Objectif\""
                .to_string(),
        )
    })
    .map_err(AutomateParseError::from)?;
    let workflow = load_workflow(&workflow_path)?;
    let workspace_root = service_paths::current_workspace_root()?;
    let initial_prompt = if prompt_parts.is_empty() {
        String::from("Executer le workflow automate fourni.")
    } else {
        prompt_parts.join(" ")
    };

    Ok(AutomateCommand {
        workspace_root,
        workflow_path,
        workflow,
        initial_prompt,
    })
}

pub fn parse_refactor_automate_args(
    args: &[String],
) -> Result<RefactorAutomateCommand, ParseOutcome> {
    parse_refactor_automate_args_internal(args).map_err(AutomateParseError::into_parse_outcome)
}

fn parse_refactor_automate_args_internal(
    args: &[String],
) -> Result<RefactorAutomateCommand, AutomateParseError> {
    let mut workflow_path: Option<PathBuf> = None;
    let mut target_dir: Option<PathBuf> = None;
    let mut prompt_parts: Vec<String> = Vec::new();
    let mut seen_workflow = false;
    let mut seen_target = false;

    let prompt_start = scan_args(args, |index, value| match value {
        "-h" | "--help" => Err(ParseOutcome::Help),
        "--workflow" => {
            mark_seen_with_message(
                &mut seen_workflow,
                "le workflow de refactor-automate a deja ete fourni",
            )?;
            let value = next_value(args, index, "--workflow")?;
            workflow_path = Some(PathBuf::from(value));
            Ok(ParseLoopControl::Continue(2))
        }
        "--target" => {
            mark_seen_with_message(
                &mut seen_target,
                "la cible de refactor-automate a deja ete fournie",
            )?;
            let value = next_value(args, index, "--target")?;
            target_dir = Some(PathBuf::from(value));
            Ok(ParseLoopControl::Continue(2))
        }
        value if value.starts_with("--") => {
            Err(ParseOutcome::Error(format!("option inconnue: {value}")))
        }
        _ => Ok(ParseLoopControl::CaptureRest),
    })
    .map_err(AutomateParseError::from)?;

    if let Some(index) = prompt_start {
        prompt_parts.extend_from_slice(&args[index..]);
    }

    let workflow = match workflow_path {
        Some(path) => load_workflow(&path)?,
        None => default_refactor_workflow(),
    };
    let launch_workspace_root = service_paths::current_workspace_root()?;
    let context =
        service_paths::ExecutionContext::from_workspace_root(launch_workspace_root.clone())
            .with_output_root(resolve_target_dir(
                target_dir.unwrap_or(launch_workspace_root.clone()),
                &service_paths::ExecutionContext::from_workspace_root(
                    launch_workspace_root.clone(),
                ),
            )?);
    let target_dir = context.output_root().to_path_buf();
    let output_root = context.output_root().to_path_buf();
    let initial_prompt = if prompt_parts.is_empty() {
        format!(
            "Refactorer {} pour ameliorer structure, maintenabilite, evolutivite et robustesse en respectant SOLID, YAGNI, KISS et DRY.",
            target_dir.display()
        )
    } else {
        prompt_parts.join(" ")
    };

    Ok(RefactorAutomateCommand {
        launch_workspace_root,
        workflow,
        initial_prompt,
        target_dir,
        output_root,
    })
}
