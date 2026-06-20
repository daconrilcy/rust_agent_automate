use std::path::PathBuf;
use std::time::Duration;

mod args;
mod context;
mod exec;

use crate::codex::{
    CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT, ReasoningEffort,
};
use crate::reporting::{CompletedReport, ReportFailure};
use crate::service_paths::ExecutionContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceCommandOptions {
    pub model: String,
    pub reasoning_effort: ReasoningEffort,
    pub verbose: bool,
    pub resume_last: bool,
    pub output_dir: Option<PathBuf>,
    pub timeout: Duration,
}

impl ServiceCommandOptions {
    pub fn new(default_timeout: Duration) -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            reasoning_effort: DEFAULT_REASONING_EFFORT,
            verbose: false,
            resume_last: false,
            output_dir: None,
            timeout: default_timeout,
        }
    }

    pub fn build_request(&self, prompt: String) -> CodexRequest {
        CodexRequest::new(
            self.model.clone(),
            self.reasoning_effort,
            CodexMode::Exec,
            Some(prompt),
            self.verbose,
        )
        .with_resume_last(self.resume_last)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedServiceCommand {
    pub request: CodexRequest,
    pub workspace_root: PathBuf,
    pub output_dir: PathBuf,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedParseContext {
    pub context: ExecutionContext,
    pub workspace_root: PathBuf,
    pub output_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSubjectArtifact<T> {
    pub subject: T,
    pub artifact_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRequiredPath {
    pub required_path: PathBuf,
    pub optional_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
pub struct RequiredPathParseSpec<'a> {
    pub required_option_name: &'a str,
    pub optional_option_name: &'a str,
    pub command_name: &'a str,
    pub required_label: &'a str,
    pub required_example: &'a str,
    pub duplicate_required_message: &'a str,
    pub duplicate_optional_message: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct ServiceCommandDescriptor<'a> {
    pub default_output_dir: &'a str,
    pub command_name: &'a str,
    pub artifact_stem: &'a str,
    pub saved_label: &'a str,
    pub final_label: &'a str,
    pub missing_message_label: &'a str,
    pub clean_detector: Option<fn(&str) -> bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceCommandKind {
    Audit,
    Plan,
    ImplementationAudit,
    Review,
    FixLoop,
}

#[derive(Debug, Clone, Copy)]
pub struct ServiceCommandSpec {
    pub kind: ServiceCommandKind,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub accepts_codex_options: bool,
    pub usage: &'static [&'static str],
    pub examples: &'static [&'static str],
    pub parse_for_context: fn(
        &[String],
        &ExecutionContext,
    ) -> Result<ServiceCommandDispatch, crate::cli::ParseOutcome>,
    pub execute: fn(&ServiceCommandDispatch) -> Result<CompletedReport, ReportFailure>,
    pub execute_silently: fn(&ServiceCommandDispatch) -> Result<CompletedReport, ReportFailure>,
}

pub const AUDIT_SERVICE_COMMAND_SPEC: ServiceCommandSpec = ServiceCommandSpec {
    kind: ServiceCommandKind::Audit,
    name: "audit",
    aliases: &[],
    accepts_codex_options: true,
    usage: &[
        "cargo run -p app -- audit [--target <chemin>] [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
    ],
    examples: &[
        "cargo run -q -p app -- audit",
        "cargo run -q -p app -- audit --target ..\\mon-projet",
        "cargo run -q -p app -- audit --output-dir .audit",
        "cargo run -q -p app -- audit --timeout-seconds 120",
    ],
    parse_for_context: |args, context| {
        crate::audit::parse_args_for_context(args, context).map(ServiceCommandDispatch::Audit)
    },
    execute: execute_audit,
    execute_silently: execute_audit_silently,
};

pub const PLAN_SERVICE_COMMAND_SPEC: ServiceCommandSpec = ServiceCommandSpec {
    kind: ServiceCommandKind::Plan,
    name: "plan",
    aliases: &[],
    accepts_codex_options: true,
    usage: &[
        "cargo run -p app -- plan <chemin-audit> [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
    ],
    examples: &[
        "cargo run -q -p app -- plan .audit\\audit-1781887189.md",
        "cargo run -q -p app -- plan --audit .audit\\audit-1781887189.md --output-dir .plan",
    ],
    parse_for_context: |args, context| {
        crate::plan::parse_args_for_context(args, context).map(ServiceCommandDispatch::Plan)
    },
    execute: execute_plan,
    execute_silently: execute_plan_silently,
};

pub const IMPLEMENTATION_AUDIT_SERVICE_COMMAND_SPEC: ServiceCommandSpec = ServiceCommandSpec {
    kind: ServiceCommandKind::ImplementationAudit,
    name: "implementation-audit",
    aliases: &["impl-audit"],
    accepts_codex_options: true,
    usage: &[
        "cargo run -p app -- implementation-audit <chemin-plan> [--implementation <chemin>] [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
        "cargo run -p app -- impl-audit <chemin-plan> [--implementation <chemin>] [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
    ],
    examples: &[
        "cargo run -q -p app -- implementation-audit .plan\\plan-1781894465.md",
        "cargo run -q -p app -- implementation-audit --plan .plan\\plan-1781894465.md --implementation crates\\app",
    ],
    parse_for_context: |args, context| {
        crate::implementation_audit::parse_args_for_context(args, context)
            .map(ServiceCommandDispatch::ImplementationAudit)
    },
    execute: execute_implementation_audit,
    execute_silently: execute_implementation_audit_silently,
};

pub const REVIEW_SERVICE_COMMAND_SPEC: ServiceCommandSpec = ServiceCommandSpec {
    kind: ServiceCommandKind::Review,
    name: "review",
    aliases: &[],
    accepts_codex_options: true,
    usage: &[
        "cargo run -p app -- review <plan|audit|implementation> <chemin> [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
    ],
    examples: &[
        "cargo run -q -p app -- review plan .plan\\plan-1781894465.md",
        "cargo run -q -p app -- review --type audit --artifact .audit\\audit-1781887189.md",
        "cargo run -q -p app -- review implementation crates\\app",
    ],
    parse_for_context: |args, context| {
        crate::review::parse_args_for_context(args, context).map(ServiceCommandDispatch::Review)
    },
    execute: execute_review,
    execute_silently: execute_review_silently,
};

pub const FIX_LOOP_SERVICE_COMMAND_SPEC: ServiceCommandSpec = ServiceCommandSpec {
    kind: ServiceCommandKind::FixLoop,
    name: "fix-loop",
    aliases: &["loop"],
    accepts_codex_options: true,
    usage: &[
        "cargo run -p app -- fix-loop <plan|audit|implementation> <chemin> [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
        "cargo run -p app -- loop <plan|audit|implementation> <chemin> [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
    ],
    examples: &[
        "cargo run -q -p app -- fix-loop plan .plan\\plan-1781894465.md",
        "cargo run -q -p app -- fix-loop audit .audit\\audit-1781887189.md",
        "cargo run -q -p app -- fix-loop implementation crates\\app",
        "cargo run -q -p app -- loop implementation crates\\app",
    ],
    parse_for_context: |args, context| {
        crate::fix_loop::parse_args_for_context(args, context).map(ServiceCommandDispatch::FixLoop)
    },
    execute: execute_fix_loop,
    execute_silently: execute_fix_loop_silently,
};

pub const SERVICE_COMMAND_SPECS: &[ServiceCommandSpec] = &[
    AUDIT_SERVICE_COMMAND_SPEC,
    PLAN_SERVICE_COMMAND_SPEC,
    IMPLEMENTATION_AUDIT_SERVICE_COMMAND_SPEC,
    REVIEW_SERVICE_COMMAND_SPEC,
    FIX_LOOP_SERVICE_COMMAND_SPEC,
];

#[derive(Debug, PartialEq, Eq)]
pub enum ServiceCommandDispatch {
    Audit(crate::audit::AuditCommand),
    Plan(crate::plan::PlanCommand),
    ImplementationAudit(crate::implementation_audit::ImplementationAuditCommand),
    Review(crate::review::ReviewCommand),
    FixLoop(crate::fix_loop::FixLoopCommand),
}

impl ServiceCommandDispatch {
    pub fn kind(&self) -> ServiceCommandKind {
        match self {
            Self::Audit(_) => ServiceCommandKind::Audit,
            Self::Plan(_) => ServiceCommandKind::Plan,
            Self::ImplementationAudit(_) => ServiceCommandKind::ImplementationAudit,
            Self::Review(_) => ServiceCommandKind::Review,
            Self::FixLoop(_) => ServiceCommandKind::FixLoop,
        }
    }

    pub fn command_name(&self) -> &'static str {
        self.kind().spec().name
    }

    pub fn request(&self) -> &CodexRequest {
        match self {
            Self::Audit(command) => &command.service.request,
            Self::Plan(command) => &command.service.request,
            Self::ImplementationAudit(command) => &command.service.request,
            Self::Review(command) => &command.service.request,
            Self::FixLoop(command) => &command.service.request,
        }
    }

    pub fn execute(&self) -> Result<CompletedReport, ReportFailure> {
        self.kind().execute(self)
    }

    pub fn execute_silently(&self) -> Result<CompletedReport, ReportFailure> {
        self.kind().execute_silently(self)
    }
}

impl ServiceCommandKind {
    pub fn spec(self) -> &'static ServiceCommandSpec {
        SERVICE_COMMAND_SPECS
            .iter()
            .find(|spec| spec.kind == self)
            .expect("service command spec")
    }

    pub fn from_name(name: &str) -> Option<Self> {
        SERVICE_COMMAND_SPECS
            .iter()
            .find(|spec| spec.name == name || spec.aliases.contains(&name))
            .map(|spec| spec.kind)
    }

    pub fn parse_for_context(
        self,
        args: &[String],
        context: &ExecutionContext,
    ) -> Result<ServiceCommandDispatch, crate::cli::ParseOutcome> {
        (self.spec().parse_for_context)(args, context)
    }

    fn execute(self, dispatch: &ServiceCommandDispatch) -> Result<CompletedReport, ReportFailure> {
        (self.spec().execute)(dispatch)
    }

    fn execute_silently(
        self,
        dispatch: &ServiceCommandDispatch,
    ) -> Result<CompletedReport, ReportFailure> {
        (self.spec().execute_silently)(dispatch)
    }
}

fn execute_audit(dispatch: &ServiceCommandDispatch) -> Result<CompletedReport, ReportFailure> {
    match dispatch {
        ServiceCommandDispatch::Audit(command) => crate::run_audit(command),
        _ => unreachable!("service dispatch kind mismatch"),
    }
}

fn execute_audit_silently(
    dispatch: &ServiceCommandDispatch,
) -> Result<CompletedReport, ReportFailure> {
    match dispatch {
        ServiceCommandDispatch::Audit(command) => crate::audit::run_silently(command),
        _ => unreachable!("service dispatch kind mismatch"),
    }
}

fn execute_plan(dispatch: &ServiceCommandDispatch) -> Result<CompletedReport, ReportFailure> {
    match dispatch {
        ServiceCommandDispatch::Plan(command) => crate::run_plan(command),
        _ => unreachable!("service dispatch kind mismatch"),
    }
}

fn execute_plan_silently(
    dispatch: &ServiceCommandDispatch,
) -> Result<CompletedReport, ReportFailure> {
    match dispatch {
        ServiceCommandDispatch::Plan(command) => crate::plan::run_silently(command),
        _ => unreachable!("service dispatch kind mismatch"),
    }
}

fn execute_implementation_audit(
    dispatch: &ServiceCommandDispatch,
) -> Result<CompletedReport, ReportFailure> {
    match dispatch {
        ServiceCommandDispatch::ImplementationAudit(command) => {
            crate::run_implementation_audit(command)
        }
        _ => unreachable!("service dispatch kind mismatch"),
    }
}

fn execute_implementation_audit_silently(
    dispatch: &ServiceCommandDispatch,
) -> Result<CompletedReport, ReportFailure> {
    match dispatch {
        ServiceCommandDispatch::ImplementationAudit(command) => {
            crate::implementation_audit::run_silently(command)
        }
        _ => unreachable!("service dispatch kind mismatch"),
    }
}

fn execute_review(dispatch: &ServiceCommandDispatch) -> Result<CompletedReport, ReportFailure> {
    match dispatch {
        ServiceCommandDispatch::Review(command) => crate::run_review(command),
        _ => unreachable!("service dispatch kind mismatch"),
    }
}

fn execute_review_silently(
    dispatch: &ServiceCommandDispatch,
) -> Result<CompletedReport, ReportFailure> {
    match dispatch {
        ServiceCommandDispatch::Review(command) => crate::review::run_silently(command),
        _ => unreachable!("service dispatch kind mismatch"),
    }
}

fn execute_fix_loop(dispatch: &ServiceCommandDispatch) -> Result<CompletedReport, ReportFailure> {
    match dispatch {
        ServiceCommandDispatch::FixLoop(command) => crate::run_fix_loop(command),
        _ => unreachable!("service dispatch kind mismatch"),
    }
}

fn execute_fix_loop_silently(
    dispatch: &ServiceCommandDispatch,
) -> Result<CompletedReport, ReportFailure> {
    match dispatch {
        ServiceCommandDispatch::FixLoop(command) => crate::fix_loop::run_silently(command),
        _ => unreachable!("service dispatch kind mismatch"),
    }
}

// Command modules own prompt construction and command-specific validation.
// This shared module stops at reusable argument parsing, workspace/output
// context preparation, and generic Codex report execution.
pub use args::{
    parse_required_path_with_optional_named_path, parse_subject_and_artifact,
    parse_with_common_options,
};
pub use context::{
    prepare_parse_context_for_context, prepare_service_from_prompt, resolve_context,
};
pub use exec::{execute_service_command, execute_service_command_silently, save_markdown_artifact};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::ParseOutcome;
    use crate::service_command::args::parse_common_option;
    use crate::service_command::context::prepare_service_command;

    #[test]
    fn parses_common_service_options() {
        let args = [
            "--model",
            "gpt-5.6",
            "--reasoning",
            "medium",
            "--output-dir",
            "C:\\tmp\\out",
            "--timeout-seconds",
            "42",
            "--verbose",
            "--continue-codex",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        let mut options = ServiceCommandOptions::new(Duration::from_secs(900));
        let mut index = 0;

        while index < args.len() {
            let consumed =
                parse_common_option(&args, index, &mut options).expect("options communes valides");
            index += consumed.expect("chaque option doit etre reconnue");
        }

        assert_eq!(options.model, "gpt-5.6");
        assert_eq!(options.reasoning_effort, ReasoningEffort::Medium);
        assert_eq!(options.output_dir, Some(PathBuf::from("C:\\tmp\\out")));
        assert_eq!(options.timeout, Duration::from_secs(42));
        assert!(options.verbose);
        assert!(options.resume_last);
    }

    #[test]
    fn builds_exec_request_from_common_options() {
        let options = ServiceCommandOptions {
            model: "gpt-5.7".to_string(),
            reasoning_effort: ReasoningEffort::High,
            verbose: true,
            resume_last: true,
            output_dir: None,
            timeout: Duration::from_secs(12),
        };

        let request = options.build_request("Prompt".to_string());

        assert_eq!(request.model, "gpt-5.7");
        assert_eq!(request.reasoning_effort, ReasoningEffort::High);
        assert_eq!(request.mode, CodexMode::Exec);
        assert_eq!(request.prompt.as_deref(), Some("Prompt"));
        assert!(request.verbose);
        assert!(request.resume_last);
        assert_eq!(request.working_dir, None);
    }

    #[test]
    fn parse_with_common_options_collects_positionals() {
        let args = ["audit.md", "--verbose"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let mut options = ServiceCommandOptions::new(Duration::from_secs(900));
        parse_with_common_options(&args, &mut options, |index, value| {
            if (index == 0 && value == "audit.md") || value == "--verbose" {
                Ok(1)
            } else {
                Err(ParseOutcome::Error(format!("unexpected: {value}")))
            }
        })
        .expect("valid parse");
        assert!(options.verbose);
    }

    #[test]
    fn parses_required_path_with_optional_named_path() {
        let args = [
            "plan.md",
            "--implementation",
            "crates\\app",
            "--timeout-seconds",
            "42",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        let mut options = ServiceCommandOptions::new(Duration::from_secs(900));

        let parsed = parse_required_path_with_optional_named_path(
            &args,
            &mut options,
            RequiredPathParseSpec {
                required_option_name: "--plan",
                optional_option_name: "--implementation",
                command_name: "implementation-audit",
                required_label: "un plan",
                required_example: "cargo run -p app -- implementation-audit .plan\\plan.md",
                duplicate_required_message: "plan duplique",
                duplicate_optional_message: "implementation dupliquee",
            },
        )
        .expect("parse valide");

        assert_eq!(parsed.required_path, PathBuf::from("plan.md"));
        assert_eq!(parsed.optional_path, Some(PathBuf::from("crates\\app")));
        assert_eq!(options.timeout, Duration::from_secs(42));
    }

    #[test]
    fn prepare_service_command_uses_workspace_root_as_codex_working_directory() {
        let options = ServiceCommandOptions::new(Duration::from_secs(900));
        let context = ExecutionContext::from_workspace_root(PathBuf::from("C:\\repo"))
            .with_output_root(PathBuf::from("C:\\repo\\out"));
        let descriptor = ServiceCommandDescriptor {
            default_output_dir: ".audit",
            command_name: "audit",
            artifact_stem: "audit",
            saved_label: "audit",
            final_label: "audit",
            missing_message_label: "audit",
            clean_detector: None,
        };

        let command = prepare_service_command(&options, &context, descriptor, "Prompt".to_string());

        assert_eq!(command.request.working_dir, Some(PathBuf::from("C:\\repo")));
    }

    #[test]
    fn service_command_specs_cover_aliases_and_lookup() {
        assert_eq!(
            ServiceCommandKind::from_name("implementation-audit"),
            Some(ServiceCommandKind::ImplementationAudit)
        );
        assert_eq!(
            ServiceCommandKind::from_name("impl-audit"),
            Some(ServiceCommandKind::ImplementationAudit)
        );
        assert_eq!(
            ServiceCommandKind::from_name("loop"),
            Some(ServiceCommandKind::FixLoop)
        );
        assert_eq!(ServiceCommandKind::from_name("unknown"), None);
    }
}
