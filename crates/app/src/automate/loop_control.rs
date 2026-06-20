use std::io;

use super::workflow_model::LoopPolicy;
use super::workflow_runner::RunContext;

pub(crate) fn evaluate_clean_stop(policy: &LoopPolicy, context: &RunContext) -> io::Result<bool> {
    if let Some(clean) = context.clean_by_step.get(&policy.audit_step).copied() {
        return Ok(clean);
    }

    let artifact_hint = context
        .artifacts_by_step
        .get(&policy.audit_step)
        .map(|path| format!(" artefact observe: {}", path.display()))
        .unwrap_or_default();

    Err(io::Error::other(format!(
        "l'etape automate '{}' doit produire un resultat structure avec le statut clean avant l'evaluation de loop_policy.{}",
        policy.audit_step, artifact_hint
    )))
}
