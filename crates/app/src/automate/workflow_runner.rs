use std::collections::BTreeMap;
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
            let command_outcome = command_outcome_for_step(workflow, step, &context, &output)?;
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
        if let Some(policy) = workflow.loop_policy.as_ref() {
            if evaluate_clean_stop(policy, &context)? {
                report.clean_stop = true;
                break;
            }
        }
    }

    Ok(report)
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

    #[test]
    fn format_stream_omits_empty_content() {
        assert_eq!(format_stream("stdout", ""), "");
        assert_eq!(format_stream("stderr", "  "), "");
    }
}
