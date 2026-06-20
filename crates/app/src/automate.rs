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

use crate::cli::{ParseOutcome, next_value};
use crate::service_paths;

#[allow(unused_imports)]
pub use step_outcome::{AutomateReport, StepResult};
#[allow(unused_imports)]
pub use workflow_model::{
    AutomateCommand, LoopPolicy, RefactorAutomateCommand, Workflow, WorkflowDefaults, WorkflowStep,
    WorkflowStepKind, default_refactor_workflow, load_workflow, parse_workflow, resolve_target_dir,
};
#[allow(unused_imports)]
pub use workflow_runner::run_workflow;

pub fn parse_automate_args(args: &[String]) -> Result<AutomateCommand, ParseOutcome> {
    let mut workflow_path: Option<PathBuf> = None;
    let mut prompt_parts: Vec<String> = Vec::new();

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "--workflow" => {
                let value = next_value(args, index, "--workflow")?;
                if workflow_path.is_some() {
                    return Err(ParseOutcome::Error(
                        "le workflow automate a deja ete fourni".to_string(),
                    ));
                }
                workflow_path = Some(PathBuf::from(value));
                index += 2;
            }
            value if value.starts_with("--") => {
                return Err(ParseOutcome::Error(format!("option inconnue: {value}")));
            }
            value => {
                if workflow_path.is_none() {
                    workflow_path = Some(PathBuf::from(value));
                    index += 1;
                    continue;
                }

                prompt_parts.extend_from_slice(&args[index..]);
                break;
            }
        }
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

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "--workflow" => {
                let value = next_value(args, index, "--workflow")?;
                workflow_path = Some(PathBuf::from(value));
                index += 2;
            }
            "--target" => {
                let value = next_value(args, index, "--target")?;
                target_dir = Some(PathBuf::from(value));
                index += 2;
            }
            value if value.starts_with("--") => {
                return Err(ParseOutcome::Error(format!("option inconnue: {value}")));
            }
            _ => {
                prompt_parts.extend_from_slice(&args[index..]);
                break;
            }
        }
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
