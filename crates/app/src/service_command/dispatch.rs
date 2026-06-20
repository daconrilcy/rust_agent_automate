use std::io;
use std::path::{Path, PathBuf};

use crate::reporting::{CompletedReport, ReportFailure};
use crate::service_command::{PreparedServiceCommand, ServiceCommandDescriptor};

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

    pub fn output_dir(&self) -> &Path {
        match self {
            Self::Audit(command) => &command.service.output_dir,
            Self::Plan(command) => &command.service.output_dir,
            Self::ImplementationAudit(command) => &command.service.output_dir,
            Self::Review(command) => &command.service.output_dir,
            Self::FixLoop(command) => &command.service.output_dir,
        }
    }

    pub fn timeout(&self) -> std::time::Duration {
        match self {
            Self::Audit(command) => command.service.timeout,
            Self::Plan(command) => command.service.timeout,
            Self::ImplementationAudit(command) => command.service.timeout,
            Self::Review(command) => command.service.timeout,
            Self::FixLoop(command) => command.service.timeout,
        }
    }

    pub fn as_audit(&self) -> Option<&crate::audit::AuditCommand> {
        match self {
            Self::Audit(command) => Some(command),
            _ => None,
        }
    }

    pub fn as_plan(&self) -> Option<&crate::plan::PlanCommand> {
        match self {
            Self::Plan(command) => Some(command),
            _ => None,
        }
    }

    pub fn as_implementation_audit(
        &self,
    ) -> Option<&crate::implementation_audit::ImplementationAuditCommand> {
        match self {
            Self::ImplementationAudit(command) => Some(command),
            _ => None,
        }
    }

    pub fn as_review(&self) -> Option<&crate::review::ReviewCommand> {
        match self {
            Self::Review(command) => Some(command),
            _ => None,
        }
    }

    pub fn as_fix_loop(&self) -> Option<&crate::fix_loop::FixLoopCommand> {
        match self {
            Self::FixLoop(command) => Some(command),
            _ => None,
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
            Self::Audit(command) => command.lifecycle(),
            Self::Plan(command) => command.lifecycle(),
            Self::ImplementationAudit(command) => command.lifecycle(),
            Self::Review(command) => command.lifecycle(),
            Self::FixLoop(command) => command.lifecycle(),
        }
    }
}

struct ServiceCommandLifecycle<'a> {
    service: &'a PreparedServiceCommand,
    descriptor: ServiceCommandDescriptor<'static>,
    intro: String,
    save: fn(&Path, &str) -> io::Result<PathBuf>,
}

impl<'a> ServiceCommandLifecycle<'a> {
    fn new(
        service: &'a PreparedServiceCommand,
        descriptor: ServiceCommandDescriptor<'static>,
        save: fn(&Path, &str) -> io::Result<PathBuf>,
        intro: String,
    ) -> Self {
        Self {
            service,
            descriptor,
            intro,
            save,
        }
    }
}

trait ServiceCommandLifecycleSource {
    fn kind(&self) -> super::ServiceCommandKind;
    fn service(&self) -> &PreparedServiceCommand;
    fn intro(&self) -> String;

    fn lifecycle(&self) -> ServiceCommandLifecycle<'_> {
        let spec = self.kind().spec();
        ServiceCommandLifecycle::new(
            self.service(),
            spec.descriptor,
            spec.save_artifact,
            self.intro(),
        )
    }
}

impl ServiceCommandLifecycleSource for crate::audit::AuditCommand {
    fn kind(&self) -> super::ServiceCommandKind {
        super::ServiceCommandKind::Audit
    }

    fn service(&self) -> &PreparedServiceCommand {
        &self.service
    }

    fn intro(&self) -> String {
        format!(
            "Audit Codex en cours sur {} (timeout: {} secondes)...",
            self.target_dir.display(),
            self.service.timeout.as_secs()
        )
    }
}

impl ServiceCommandLifecycleSource for crate::plan::PlanCommand {
    fn kind(&self) -> super::ServiceCommandKind {
        super::ServiceCommandKind::Plan
    }

    fn service(&self) -> &PreparedServiceCommand {
        &self.service
    }

    fn intro(&self) -> String {
        format!(
            "Plan Codex en cours depuis {} (timeout: {} secondes)...",
            self.audit_path.display(),
            self.service.timeout.as_secs()
        )
    }
}

impl ServiceCommandLifecycleSource for crate::implementation_audit::ImplementationAuditCommand {
    fn kind(&self) -> super::ServiceCommandKind {
        super::ServiceCommandKind::ImplementationAudit
    }

    fn service(&self) -> &PreparedServiceCommand {
        &self.service
    }

    fn intro(&self) -> String {
        format!(
            "Audit d'implementation Codex en cours depuis {} sur {} (timeout: {} secondes)...",
            self.plan_path.display(),
            self.implementation_path.as_deref().map_or_else(
                || "git diff / workspace".to_string(),
                |path| path.display().to_string()
            ),
            self.service.timeout.as_secs()
        )
    }
}

impl ServiceCommandLifecycleSource for crate::review::ReviewCommand {
    fn kind(&self) -> super::ServiceCommandKind {
        super::ServiceCommandKind::Review
    }

    fn service(&self) -> &PreparedServiceCommand {
        &self.service
    }

    fn intro(&self) -> String {
        format!(
            "Review adversariale Codex en cours ({}) sur {} (timeout: {} secondes)...",
            self.subject,
            self.artifact_path.display(),
            self.service.timeout.as_secs()
        )
    }
}

impl ServiceCommandLifecycleSource for crate::fix_loop::FixLoopCommand {
    fn kind(&self) -> super::ServiceCommandKind {
        super::ServiceCommandKind::FixLoop
    }

    fn service(&self) -> &PreparedServiceCommand {
        &self.service
    }

    fn intro(&self) -> String {
        format!(
            "Boucle review/correction Codex en cours ({}) sur {} (timeout: {} secondes)...",
            self.input_kind,
            self.artifact_path.display(),
            self.service.timeout.as_secs()
        )
    }
}
