use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use super::loop_control::evaluate_clean_stop;
use super::step_outcome::{
    AutomateReport, StepExecution, StepOutcome, StepResult, run_step,
    validate_and_normalize_outcome,
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

fn step_failure(step: &WorkflowStep, output: &StepExecution, result: &StepResult) -> io::Error {
    io::Error::other(format!(
        "l'etape automate '{}' a echoue avec le statut {}{}{}",
        step.name,
        result
            .status_code
            .map_or_else(|| "inconnu".to_string(), |c| c.to_string()),
        format_stream("stdout", &output.stdout),
        format_stream("stderr", &output.stderr)
    ))
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
    use crate::{CommandOutcome, Workflow, parse_workflow};
    use std::cell::RefCell;
    use std::fs;
    use std::rc::Rc;

    #[test]
    fn format_stream_omits_empty_content() {
        assert_eq!(format_stream("stdout", ""), "");
        assert_eq!(format_stream("stderr", "  "), "");
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
                    command_outcome: Some(crate::reporting::CommandOutcome {
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
                    command_outcome: Some(crate::reporting::CommandOutcome {
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

    #[test]
    fn apply_step_outcome_updates_context_state() {
        let mut context = RunContext::default();
        let step = WorkflowStep {
            name: "audit".to_string(),
            rust_command: vec!["audit".to_string()],
            kind: crate::automate::workflow_model::WorkflowStepKind::ServiceCommand,
            fresh_codex_call: true,
            model: None,
            reasoning: None,
            timeout_seconds: None,
        };
        let output = StepExecution {
            status_code: Some(0),
            success: true,
            stdout: "done".to_string(),
            stderr: String::new(),
            command_outcome: None,
        };
        let outcome = StepOutcome {
            status_code: Some(0),
            artifact_path: Some(PathBuf::from("C:\\repo\\.audit\\audit.md")),
            clean: Some(true),
        };

        apply_step_outcome(&mut context, &step, &output, &outcome);

        assert_eq!(context.last_output, "done");
        assert_eq!(
            context.last_artifact,
            Some(PathBuf::from("C:\\repo\\.audit\\audit.md"))
        );
        assert_eq!(context.clean_by_step.get("audit"), Some(&true));
    }

    #[test]
    fn direct_run_without_structured_outcome_is_reported() {
        let workflow = parse_workflow(
            r#"{
              "defaults": {"model":"gpt-x","reasoning":"medium"},
              "steps":[{"name":"implementation","rust_command":["audit this repo"],"fresh_codex_call":true}]
            }"#,
        )
        .expect("workflow valide");
        let report = run_workflow_with_executor(
            &workflow,
            "Prompt",
            Path::new("C:\\repo"),
            Path::new("C:\\repo"),
            |_workflow, _step, _context| {
                Ok(StepExecution {
                    status_code: Some(0),
                    success: true,
                    stdout: "implementation terminee".to_string(),
                    stderr: String::new(),
                    command_outcome: None,
                })
            },
        )
        .expect("une etape directe doit etre acceptee");

        assert_eq!(report.step_results[0].status_code, Some(0));
        assert_eq!(report.step_results[0].artifact_path, None);
    }

    #[test]
    fn command_outcome_requires_structured_result_for_nested_subcommands() {
        let workflow = parse_workflow(
            r#"{
              "steps":[{"name":"nested","rust_command":["automate","workflow.json","Prompt"]}]
            }"#,
        )
        .expect("workflow valide");
        let error = run_workflow_with_executor(
            &workflow,
            "Prompt",
            Path::new("C:\\repo"),
            Path::new("C:\\repo"),
            |_workflow, _step, _context| {
                Ok(StepExecution {
                    status_code: Some(0),
                    success: true,
                    stdout: "automate termine".to_string(),
                    stderr: String::new(),
                    command_outcome: None,
                })
            },
        )
        .expect_err("une sous-commande imbriquee doit rester structuree");

        assert!(
            error
                .to_string()
                .contains("doit produire un resultat structure")
        );
    }

    #[test]
    fn run_workflow_rejects_missing_structured_outcome() {
        let workflow = parse_workflow(
            r#"{"steps":[{"name":"audit","rust_command":["audit","--target","{target}"]}]}"#,
        )
        .expect("workflow valide");

        let error = run_workflow_with_executor(
            &workflow,
            "Durcir",
            Path::new("C:\\repo"),
            Path::new("C:\\repo"),
            |_workflow, _step, _context| {
                Ok(StepExecution {
                    status_code: Some(0),
                    success: true,
                    stdout: "Plan enregistre dans C:\\repo\\.audit\\audit.md".to_string(),
                    stderr: String::new(),
                    command_outcome: None,
                })
            },
        )
        .expect_err("un resultat structure est requis");

        assert!(
            error
                .to_string()
                .contains("doit produire un resultat structure")
        );
    }

    #[test]
    fn run_workflow_accepts_direct_run_without_structured_outcome() {
        let workflow = parse_workflow(
            r#"{"steps":[{"name":"implementation","rust_command":["--mode","exec","Implement"]}]}"#,
        )
        .expect("workflow valide");

        let report = run_workflow_with_executor(
            &workflow,
            "Durcir",
            Path::new("C:\\repo"),
            Path::new("C:\\repo"),
            |_workflow, _step, _context| {
                Ok(StepExecution {
                    status_code: Some(0),
                    success: true,
                    stdout: "implementation terminee".to_string(),
                    stderr: String::new(),
                    command_outcome: None,
                })
            },
        )
        .expect("une etape directe n'a pas d'artefact structure");

        assert_eq!(report.completed_cycles, 1);
        assert_eq!(report.step_results[0].status_code, Some(0));
        assert_eq!(report.step_results[0].artifact_path, None);
    }

    #[test]
    fn run_workflow_uses_structured_clean_status_for_loop_stop() {
        let workflow: Workflow = parse_workflow(
            r#"{
              "steps":[
                {"name":"alignment_audit","rust_command":["implementation-audit","plan.md"]},
                {"name":"commit","rust_command":["--mode","exec","Commit"]}
              ],
              "loop_policy":{"audit_step":"alignment_audit","max_cycles":3}
            }"#,
        )
        .expect("workflow valide");

        let workspace = std::env::temp_dir().join("rust_agent_workflow_loop_stop");
        let artifact = workspace.join(".audit").join("artifact.md");
        fs::create_dir_all(artifact.parent().expect("parent")).expect("dossier audit");
        fs::write(&artifact, "artifact").expect("artefact audit");
        let calls = Rc::new(RefCell::new(0_u32));
        let seen = Rc::clone(&calls);

        let report = run_workflow_with_executor(
            &workflow,
            "Durcir",
            &workspace,
            &workspace,
            move |_workflow, step, _context| {
                *seen.borrow_mut() += 1;
                Ok(StepExecution {
                    status_code: Some(0),
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                    command_outcome: Some(CommandOutcome {
                        command_name: step.name.clone(),
                        status_code: Some(0),
                        final_message_present: true,
                        artifact_path: Some(artifact.clone()),
                        clean: (step.name == "alignment_audit").then_some(true),
                    }),
                })
            },
        )
        .expect("workflow execute");

        assert!(report.clean_stop);
        assert_eq!(report.completed_cycles, 1);
        assert_eq!(*calls.borrow(), 2);
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn run_workflow_rejects_loop_audit_without_structured_clean_status() {
        let workflow: Workflow = parse_workflow(
            r#"{
              "steps":[
                {"name":"alignment_audit","rust_command":["implementation-audit","plan.md"]}
              ],
              "loop_policy":{"audit_step":"alignment_audit","max_cycles":2}
            }"#,
        )
        .expect("workflow valide");

        let workspace = std::env::temp_dir().join("rust_agent_workflow_missing_clean");
        let artifact = workspace.join(".audit").join("artifact.md");
        fs::create_dir_all(artifact.parent().expect("parent")).expect("dossier audit");
        fs::write(&artifact, "artifact").expect("artefact audit");
        let error = run_workflow_with_executor(
            &workflow,
            "Durcir",
            &workspace,
            &workspace,
            move |_workflow, step, _context| {
                Ok(StepExecution {
                    status_code: Some(0),
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                    command_outcome: Some(CommandOutcome {
                        command_name: step.name.clone(),
                        status_code: Some(0),
                        final_message_present: true,
                        artifact_path: Some(artifact.clone()),
                        clean: None,
                    }),
                })
            },
        )
        .expect_err("loop_policy doit exiger un statut clean structure");

        assert!(error.to_string().contains("statut clean"));
        let _ = fs::remove_dir_all(workspace);
    }
}
