use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::step_outcome::{
    AutomateReport, StepExecution, StepResult, command_outcome_for_step, evaluate_clean_stop,
    run_step,
};
use super::workflow_model::{Workflow, WorkflowStep};

#[derive(Debug, Default)]
pub struct RunContext {
    pub workspace_root: PathBuf,
    pub initial_prompt: String,
    pub target_dir: PathBuf,
    pub current_cycle: u32,
    pub last_output: String,
    pub last_artifact: Option<PathBuf>,
    pub artifacts_by_step: BTreeMap<String, PathBuf>,
    pub clean_by_step: BTreeMap<String, bool>,
}

pub fn run_workflow(
    workflow: &Workflow,
    initial_prompt: &str,
    workspace_root: &Path,
    target_dir: &Path,
) -> io::Result<AutomateReport> {
    run_workflow_with_executor(
        workflow,
        initial_prompt,
        workspace_root,
        target_dir,
        run_step,
    )
}

pub fn run_workflow_with_executor<F>(
    workflow: &Workflow,
    initial_prompt: &str,
    workspace_root: &Path,
    target_dir: &Path,
    mut run_step: F,
) -> io::Result<AutomateReport>
where
    F: FnMut(&Workflow, &WorkflowStep, &RunContext) -> io::Result<StepExecution>,
{
    let mut context = RunContext {
        workspace_root: workspace_root.to_path_buf(),
        initial_prompt: initial_prompt.to_string(),
        target_dir: target_dir.to_path_buf(),
        current_cycle: 1,
        ..RunContext::default()
    };
    let max_cycles = workflow
        .loop_policy
        .as_ref()
        .map_or(1, |policy| policy.max_cycles.max(1));
    let mut report = AutomateReport {
        completed_cycles: 0,
        clean_stop: false,
        step_results: Vec::new(),
    };

    for cycle in 1..=max_cycles {
        context.current_cycle = cycle;
        for step in &workflow.steps {
            eprintln!(
                "Automate cycle {cycle}/{max_cycles}: etape '{}'...",
                step.name
            );
            let output = run_step(workflow, step, &context)?;
            let command_outcome = validated_command_outcome(workflow, step, &context, &output)?;
            if let Some(path) = &command_outcome.artifact_path {
                context
                    .artifacts_by_step
                    .insert(step.name.clone(), path.clone());
                context.last_artifact = Some(path.clone());
            }
            if let Some(clean) = command_outcome.clean {
                context.clean_by_step.insert(step.name.clone(), clean);
            }
            context.last_output = output.stdout.clone();
            report.step_results.push(StepResult {
                cycle,
                name: step.name.clone(),
                status_code: command_outcome.status_code,
                artifact_path: command_outcome.artifact_path,
            });
            if !output.success {
                return Err(io::Error::other(format!(
                    "l'etape automate '{}' a echoue avec le statut {}{}{}",
                    step.name,
                    command_outcome
                        .status_code
                        .map_or_else(|| "inconnu".to_string(), |c| c.to_string()),
                    format_stream("stdout", &output.stdout),
                    format_stream("stderr", &output.stderr)
                )));
            }
        }
        report.completed_cycles = cycle;
        if let Some(policy) = workflow.loop_policy.as_ref()
            && evaluate_clean_stop(policy, &context)?
        {
            report.clean_stop = true;
            break;
        }
    }

    Ok(report)
}

fn validated_command_outcome(
    workflow: &Workflow,
    step: &WorkflowStep,
    context: &RunContext,
    output: &StepExecution,
) -> io::Result<crate::reporting::CommandOutcome> {
    // Workflow validation decides which steps may omit structured output and
    // artifact paths are normalized here before later placeholder expansion.
    let mut outcome = command_outcome_for_step(workflow, step, context, output)?;
    if let Some(path) = outcome.artifact_path.take() {
        outcome.artifact_path = Some(normalize_artifact_path(&path, &context.workspace_root)?);
    }

    Ok(outcome)
}

fn normalize_artifact_path(path: &Path, workspace_root: &Path) -> io::Result<PathBuf> {
    let workspace_root = fs::canonicalize(workspace_root).map_err(|error| {
        io::Error::other(format!(
            "impossible de normaliser le workspace automate {}: {error}",
            workspace_root.display()
        ))
    })?;
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };

    let metadata = fs::metadata(&resolved).map_err(|error| {
        io::Error::other(format!(
            "artefact introuvable ou inaccessible {}: {error}",
            resolved.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(io::Error::other(format!(
            "l'artefact structure doit etre un fichier: {}",
            resolved.display()
        )));
    }

    let normalized = fs::canonicalize(&resolved).map_err(|error| {
        io::Error::other(format!(
            "impossible de normaliser l'artefact {}: {error}",
            resolved.display()
        ))
    })?;
    if !normalized.starts_with(&workspace_root) {
        return Err(io::Error::other(format!(
            "l'artefact structure doit rester dans le workspace automate {}: {}",
            workspace_root.display(),
            normalized.display()
        )));
    }

    Ok(normalized)
}

fn format_stream(label: &str, content: &str) -> String {
    let content = content.trim();
    if content.is_empty() {
        String::new()
    } else {
        format!("\n{label}:\n{content}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automate::workflow_model::parse_workflow;
    use crate::reporting::CommandOutcome;
    use std::path::Path;

    #[test]
    fn format_stream_omits_empty_content() {
        assert_eq!(format_stream("stdout", ""), "");
        assert_eq!(format_stream("stderr", "  "), "");
    }

    #[test]
    fn normalizes_relative_artifact_path_from_workspace_root() {
        let workspace = std::env::temp_dir().join("rust_agent_workflow_runner_relative_artifact");
        let artifact = workspace.join(".audit").join("audit.md");
        fs::create_dir_all(artifact.parent().expect("parent")).expect("creation du dossier");
        fs::write(&artifact, "audit").expect("ecriture de l'artefact");

        let normalized = normalize_artifact_path(Path::new(".audit\\audit.md"), &workspace)
            .expect("normalisation");

        assert_eq!(
            normalized,
            fs::canonicalize(&artifact).expect("chemin canonical")
        );

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn run_workflow_rejects_missing_persisted_artifact() {
        let workspace = std::env::temp_dir().join("rust_agent_workflow_missing_artifact");
        fs::create_dir_all(&workspace).expect("creation workspace");
        let workflow = parse_workflow(
            r#"{"steps":[{"name":"audit","rust_command":["audit","--target","{target}"]}]}"#,
        )
        .expect("workflow valide");

        let error = run_workflow_with_executor(
            &workflow,
            "Durcir",
            &workspace,
            &workspace,
            |_workflow, step, _context| {
                Ok(StepExecution {
                    status_code: Some(0),
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                    command_outcome: Some(CommandOutcome {
                        command_name: step.name.clone(),
                        status_code: Some(0),
                        final_message_present: true,
                        artifact_path: Some(PathBuf::from("C:\\repo\\.audit\\missing.md")),
                        clean: None,
                    }),
                })
            },
        )
        .expect_err("un artefact manquant doit echouer");

        assert!(
            error
                .to_string()
                .contains("artefact introuvable ou inaccessible")
        );
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn run_workflow_rejects_artifact_outside_workspace_root() {
        let workspace = std::env::temp_dir().join("rust_agent_workflow_workspace_root");
        let outside = std::env::temp_dir()
            .join("rust_agent_workflow_outside")
            .join("audit.md");
        fs::create_dir_all(&workspace).expect("creation workspace");
        fs::create_dir_all(outside.parent().expect("parent outside")).expect("creation outside");
        fs::write(&outside, "audit").expect("artefact outside");
        let workflow = parse_workflow(
            r#"{"steps":[{"name":"audit","rust_command":["audit","--target","{target}"]}]}"#,
        )
        .expect("workflow valide");

        let error = run_workflow_with_executor(
            &workflow,
            "Durcir",
            &workspace,
            &workspace,
            |_workflow, step, _context| {
                Ok(StepExecution {
                    status_code: Some(0),
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                    command_outcome: Some(CommandOutcome {
                        command_name: step.name.clone(),
                        status_code: Some(0),
                        final_message_present: true,
                        artifact_path: Some(outside.clone()),
                        clean: None,
                    }),
                })
            },
        )
        .expect_err("un artefact hors workspace doit echouer");

        assert!(
            error
                .to_string()
                .contains("doit rester dans le workspace automate")
        );
        let _ = fs::remove_dir_all(workspace);
        let _ = fs::remove_dir_all(outside.parent().expect("parent outside"));
    }
}
