use std::io;
use std::path::{Path, PathBuf};

use crate::reporting::{CompletedReport, ReportFailure};

#[derive(Debug, PartialEq, Eq)]
pub enum ServiceCommandDispatch {
    Audit(crate::audit::AuditCommand),
    Plan(crate::plan::PlanCommand),
    ImplementationAudit(crate::implementation_audit::ImplementationAuditCommand),
    Review(crate::review::ReviewCommand),
    FixLoop(crate::fix_loop::FixLoopCommand),
}

impl ServiceCommandDispatch {
    pub fn kind(&self) -> super::ServiceCommandKind {
        match self {
            Self::Audit(_) => super::ServiceCommandKind::Audit,
            Self::Plan(_) => super::ServiceCommandKind::Plan,
            Self::ImplementationAudit(_) => super::ServiceCommandKind::ImplementationAudit,
            Self::Review(_) => super::ServiceCommandKind::Review,
            Self::FixLoop(_) => super::ServiceCommandKind::FixLoop,
        }
    }

    pub fn command_name(&self) -> &'static str {
        self.kind().spec().name
    }

    pub fn request(&self) -> &crate::codex::CodexRequest {
        match self {
            Self::Audit(command) => &command.service.request,
            Self::Plan(command) => &command.service.request,
            Self::ImplementationAudit(command) => &command.service.request,
            Self::Review(command) => &command.service.request,
            Self::FixLoop(command) => &command.service.request,
        }
    }

    pub fn execute(&self) -> Result<CompletedReport, ReportFailure> {
        let lifecycle = self.lifecycle();
        super::execute_service_command(
            lifecycle.service,
            lifecycle.descriptor,
            lifecycle.intro,
            lifecycle.save,
        )
    }

    pub fn execute_silently(&self) -> Result<CompletedReport, ReportFailure> {
        let lifecycle = self.lifecycle();
        super::execute_service_command_silently(
            lifecycle.service,
            lifecycle.descriptor,
            lifecycle.save,
        )
    }

    fn lifecycle(&self) -> ServiceCommandLifecycle<'_> {
        match self {
            Self::Audit(command) => ServiceCommandLifecycle {
                service: &command.service,
                descriptor: crate::audit::descriptor(),
                intro: format!(
                    "Audit Codex en cours sur {} (timeout: {} secondes)...",
                    command.target_dir.display(),
                    command.service.timeout.as_secs()
                ),
                save: crate::audit::save_report,
            },
            Self::Plan(command) => ServiceCommandLifecycle {
                service: &command.service,
                descriptor: crate::plan::descriptor(),
                intro: format!(
                    "Plan Codex en cours depuis {} (timeout: {} secondes)...",
                    command.audit_path.display(),
                    command.service.timeout.as_secs()
                ),
                save: crate::plan::save_plan,
            },
            Self::ImplementationAudit(command) => ServiceCommandLifecycle {
                service: &command.service,
                descriptor: crate::implementation_audit::descriptor(),
                intro: format!(
                    "Audit d'implementation Codex en cours depuis {} sur {} (timeout: {} secondes)...",
                    command.plan_path.display(),
                    command.implementation_path.as_deref().map_or_else(
                        || "git diff / workspace".to_string(),
                        |path| path.display().to_string()
                    ),
                    command.service.timeout.as_secs()
                ),
                save: crate::implementation_audit::save_audit,
            },
            Self::Review(command) => ServiceCommandLifecycle {
                service: &command.service,
                descriptor: crate::review::descriptor(),
                intro: format!(
                    "Review adversariale Codex en cours ({}) sur {} (timeout: {} secondes)...",
                    command.subject,
                    command.artifact_path.display(),
                    command.service.timeout.as_secs()
                ),
                save: crate::review::save_review,
            },
            Self::FixLoop(command) => ServiceCommandLifecycle {
                service: &command.service,
                descriptor: crate::fix_loop::descriptor(),
                intro: format!(
                    "Boucle review/correction Codex en cours ({}) sur {} (timeout: {} secondes)...",
                    command.input_kind,
                    command.artifact_path.display(),
                    command.service.timeout.as_secs()
                ),
                save: crate::fix_loop::save_report,
            },
        }
    }
}

struct ServiceCommandLifecycle<'a> {
    service: &'a crate::service_command::PreparedServiceCommand,
    descriptor: crate::service_command::ServiceCommandDescriptor<'static>,
    intro: String,
    save: fn(&Path, &str) -> io::Result<PathBuf>,
}
