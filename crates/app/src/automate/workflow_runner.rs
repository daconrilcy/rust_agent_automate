use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use super::loop_control::evaluate_clean_stop;
use super::step_outcome::{
    AutomateReport, StepExecution, StepOutcome, StepResult, run_step,
    validate_and_normalize_outcome,
};
use super::workflow_model::{Workflow, WorkflowStep};
use crate::codex::{AgentContext, AgentPermissions};

#[derive(Debug, Default)]
pub struct RunContext {
    pub workspace_root: PathBuf,
    pub initial_prompt: String,
    pub target_dir: PathBuf,
    pub agent_context: AgentContext,
    pub permissions: AgentPermissions,
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
    agent_context: &AgentContext,
    permissions: &AgentPermissions,
) -> io::Result<AutomateReport> {
    run_workflow_with_executor(
        workflow,
        initial_prompt,
        workspace_root,
        target_dir,
        agent_context,
        permissions,
        run_step,
    )
    .map_err(io::Error::other)
}

pub fn run_workflow_with_executor<F>(
    workflow: &Workflow,
    initial_prompt: &str,
    workspace_root: &Path,
    target_dir: &Path,
    agent_context: &AgentContext,
    permissions: &AgentPermissions,
    mut run_step: F,
) -> Result<AutomateReport, super::AutomationError>
where
    F: FnMut(
        &Workflow,
        &WorkflowStep,
        &RunContext,
    ) -> Result<StepExecution, super::AutomationError>,
{
    let mut context = RunContext {
        workspace_root: workspace_root.to_path_buf(),
        initial_prompt: initial_prompt.to_string(),
        target_dir: target_dir.to_path_buf(),
        agent_context: agent_context.clone(),
        permissions: permissions.clone(),
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
            let command_outcome =
                validate_and_normalize_outcome(workflow, step, &context, &output)?;
            apply_step_outcome(&mut context, step, &output, &command_outcome);
            append_step_result(&mut report, cycle, step, &command_outcome);
            if !output.success {
                return Err(step_failure(
                    step,
                    &output,
                    report.step_results.last().expect("step result"),
                ));
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

fn apply_step_outcome(
    context: &mut RunContext,
    step: &WorkflowStep,
    output: &StepExecution,
    command_outcome: &StepOutcome,
) {
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
}

fn append_step_result(
    report: &mut AutomateReport,
    cycle: u32,
    step: &WorkflowStep,
    command_outcome: &StepOutcome,
) {
    report.step_results.push(StepResult {
        cycle,
        name: step.name.clone(),
        status_code: command_outcome.status_code,
        artifact_path: command_outcome.artifact_path.clone(),
    });
}

fn step_failure(
    step: &WorkflowStep,
    output: &StepExecution,
    result: &StepResult,
) -> super::AutomationError {
    super::AutomationError::StepFailed {
        step_name: step.name.clone(),
        status_code: result.status_code,
        stdout: output.stdout.clone(),
        stderr: output.stderr.clone(),
    }
}
