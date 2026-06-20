use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::codex::{DEFAULT_MODEL, DEFAULT_REASONING_EFFORT, ReasoningEffort};

#[derive(Debug, PartialEq, Eq)]
pub struct AutomateCommand {
    pub workspace_root: PathBuf,
    pub workflow_path: PathBuf,
    pub workflow: Workflow,
    pub initial_prompt: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RefactorAutomateCommand {
    pub launch_workspace_root: PathBuf,
    pub workflow: Workflow,
    pub initial_prompt: String,
    pub target_dir: PathBuf,
    pub output_root: PathBuf,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Workflow {
    #[serde(default)]
    pub defaults: WorkflowDefaults,
    pub steps: Vec<WorkflowStep>,
    #[serde(default)]
    pub loop_policy: Option<LoopPolicy>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct WorkflowDefaults {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub reasoning: Option<ReasoningEffort>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

impl Default for WorkflowDefaults {
    fn default() -> Self {
        Self {
            model: Some(DEFAULT_MODEL.to_string()),
            reasoning: Some(DEFAULT_REASONING_EFFORT),
            timeout_seconds: Some(1_800),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct WorkflowStep {
    pub name: String,
    pub rust_command: Vec<String>,
    #[serde(skip, default)]
    pub kind: WorkflowStepKind,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub reasoning: Option<ReasoningEffort>,
    #[serde(default = "default_fresh_codex_call")]
    pub fresh_codex_call: bool,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
pub enum WorkflowStepKind {
    ServiceCommand,
    NestedCommand,
    #[default]
    DirectRun,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct LoopPolicy {
    pub audit_step: String,
    #[serde(default = "default_max_cycles")]
    pub max_cycles: u32,
    #[serde(default = "default_clean_markers")]
    pub clean_markers: Vec<String>,
}

pub fn load_workflow(path: &Path) -> Result<Workflow, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("impossible de lire le workflow {}: {error}", path.display()))?;

    parse_workflow(&content)
}

pub fn parse_workflow(content: &str) -> Result<Workflow, String> {
    let workflow: Workflow = serde_json::from_str(content)
        .map_err(|error| format!("workflow JSON invalide: {error}"))?;

    validate_workflow(workflow, super::classify_step_kind)
}

pub fn default_refactor_workflow() -> Workflow {
    parse_workflow(DEFAULT_REFACTOR_WORKFLOW)
        .expect("le workflow de refactoring integre est valide")
}

fn validate_workflow(
    workflow: Workflow,
    classify_step_kind: fn(&[String]) -> WorkflowStepKind,
) -> Result<Workflow, String> {
    if workflow.steps.is_empty() {
        return Err("le workflow doit definir au moins une etape".to_string());
    }

    let mut workflow = workflow;

    let mut seen_names = BTreeSet::new();

    for step in &mut workflow.steps {
        if step.name.trim().is_empty() {
            return Err("chaque etape doit avoir un nom non vide".to_string());
        }
        if step.rust_command.is_empty() {
            return Err(format!("l'etape '{}' doit definir rust_command", step.name));
        }
        if !seen_names.insert(step.name.clone()) {
            return Err(format!(
                "le nom d'etape '{}' doit etre unique dans le workflow",
                step.name
            ));
        }

        step.kind = classify_step_kind(&step.rust_command);
    }

    validate_artifact_references(&workflow)?;

    if let Some(policy) = &workflow.loop_policy {
        let Some(audit_step) = workflow
            .steps
            .iter()
            .find(|step| step.name == policy.audit_step)
        else {
            return Err(format!(
                "loop_policy.audit_step '{}' ne correspond a aucune etape",
                policy.audit_step
            ));
        };

        if matches!(audit_step.kind, WorkflowStepKind::DirectRun) {
            return Err(format!(
                "loop_policy.audit_step '{}' doit etre une commande structuree, pas une execution directe",
                policy.audit_step
            ));
        }
    }

    Ok(workflow)
}

fn validate_artifact_references(workflow: &Workflow) -> Result<(), String> {
    for (index, step) in workflow.steps.iter().enumerate() {
        for referenced_step in referenced_artifacts(&step.rust_command) {
            let Some(referenced_index) = workflow
                .steps
                .iter()
                .position(|candidate| candidate.name == referenced_step)
            else {
                return Err(format!(
                    "l'etape '{}' reference un artefact inconnu '{}'",
                    step.name, referenced_step
                ));
            };

            if referenced_index >= index {
                return Err(format!(
                    "l'etape '{}' reference l'artefact futur '{}' avant sa production",
                    step.name, referenced_step
                ));
            }

            if matches!(
                workflow.steps[referenced_index].kind,
                WorkflowStepKind::DirectRun
            ) {
                return Err(format!(
                    "l'etape '{}' reference l'artefact '{}' mais cette etape est une execution directe sans resultat structure",
                    step.name, referenced_step
                ));
            }
        }
    }

    Ok(())
}

fn referenced_artifacts(command: &[String]) -> Vec<&str> {
    let mut references = Vec::new();

    for value in command {
        let mut remainder = value.as_str();
        while let Some(start) = remainder.find("{artifact:") {
            let suffix = &remainder[start + "{artifact:".len()..];
            let Some(end) = suffix.find('}') else {
                break;
            };
            let candidate = &suffix[..end];
            if !candidate.is_empty() {
                references.push(candidate);
            }
            remainder = &suffix[end + 1..];
        }
    }

    references
}

fn default_max_cycles() -> u32 {
    2
}

fn default_fresh_codex_call() -> bool {
    true
}

fn default_clean_markers() -> Vec<String> {
    vec![
        "no actionable findings".to_string(),
        "aucun finding actionnable".to_string(),
        "aucune correction requise".to_string(),
    ]
}

const DEFAULT_REFACTOR_WORKFLOW: &str = include_str!("../../../../workflows/refactor.json");
