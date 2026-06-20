#[path = "automate/artifact_resolution.rs"]
pub mod artifact_resolution;
#[path = "automate/loop_control.rs"]
pub mod loop_control;
#[path = "automate/step_args.rs"]
pub mod step_args;
#[path = "automate/step_outcome.rs"]
pub mod step_outcome;
#[path = "automate/transport.rs"]
mod transport;
#[path = "automate/workflow_model.rs"]
mod workflow_model;
#[path = "automate/workflow_runner.rs"]
mod workflow_runner;

use std::path::PathBuf;

use crate::cli::{ParseLoopControl, ParseOutcome, mark_seen_with_message, next_value, scan_args};
use crate::command_registry;
use crate::service_paths::{self, ExecutionContext, PathRequirement};

#[allow(unused_imports)]
pub use step_outcome::{AutomateReport, StepResult};
#[allow(unused_imports)]
pub use workflow_model::{
    AutomateCommand, LoopPolicy, RefactorAutomateCommand, Workflow, WorkflowDefaults, WorkflowStep,
    WorkflowStepKind, default_refactor_workflow, load_workflow, parse_workflow,
};
#[allow(unused_imports)]
pub use workflow_runner::run_workflow;

pub(crate) fn classify_step_kind(rust_command: &[String]) -> WorkflowStepKind {
    let Some(first) = rust_command.first().map(String::as_str) else {
        return WorkflowStepKind::DirectRun;
    };

    if command_registry::accepts_codex_options(first) {
        WorkflowStepKind::ServiceCommand
    } else if command_registry::is_direct_run(Some(first)) {
        WorkflowStepKind::DirectRun
    } else {
        WorkflowStepKind::NestedCommand
    }
}

pub fn resolve_target_dir(path: PathBuf, context: &ExecutionContext) -> Result<PathBuf, String> {
    service_paths::resolve_existing_path(path, "dossier cible", PathRequirement::Directory, context)
        .map_err(|error| error.replace("le chemin dossier cible", "le dossier cible"))
}

pub fn parse_automate_args(args: &[String]) -> Result<AutomateCommand, ParseOutcome> {
    let mut workflow_path: Option<PathBuf> = None;
    let mut prompt_parts: Vec<String> = Vec::new();
    let mut named_workflow = false;

    let prompt_start = scan_args(args, |index, value| match value {
        "-h" | "--help" => return Err(ParseOutcome::Help),
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
    })?;

    if let Some(index) = prompt_start {
        prompt_parts.extend_from_slice(&args[index..]);
    }

    let workflow_path = workflow_path.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande automate requiert un workflow JSON. Exemple: cargo run -p app -- automate workflow.json \"Objectif\""
                .to_string(),
        )
    })?;
    let workflow = load_workflow(&workflow_path).map_err(ParseOutcome::Error)?;
    let workspace_root = service_paths::current_workspace_root().map_err(ParseOutcome::Error)?;
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
    let mut workflow_path: Option<PathBuf> = None;
    let mut target_dir: Option<PathBuf> = None;
    let mut prompt_parts: Vec<String> = Vec::new();
    let mut seen_workflow = false;
    let mut seen_target = false;

    let prompt_start = scan_args(args, |index, value| match value {
        "-h" | "--help" => return Err(ParseOutcome::Help),
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
    })?;

    if let Some(index) = prompt_start {
        prompt_parts.extend_from_slice(&args[index..]);
    }

    let workflow = match workflow_path {
        Some(path) => load_workflow(&path).map_err(ParseOutcome::Error)?,
        None => default_refactor_workflow(),
    };
    let launch_workspace_root =
        service_paths::current_workspace_root().map_err(ParseOutcome::Error)?;
    let context =
        service_paths::ExecutionContext::from_workspace_root(launch_workspace_root.clone())
            .with_output_root(
                resolve_target_dir(
                    target_dir.unwrap_or(launch_workspace_root.clone()),
                    &service_paths::ExecutionContext::from_workspace_root(
                        launch_workspace_root.clone(),
                    ),
                )
                .map_err(ParseOutcome::Error)?,
            );
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
