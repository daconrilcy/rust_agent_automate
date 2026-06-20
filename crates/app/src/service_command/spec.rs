use std::path::PathBuf;
use std::time::Duration;
use std::{io, path::Path};

use crate::codex::{
    CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT, ReasoningEffort,
};
use crate::service_paths::ExecutionContext;

use super::dispatch::ServiceCommandDispatch;

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
    pub descriptor: ServiceCommandDescriptor<'static>,
    pub save_artifact: fn(&Path, &str) -> io::Result<PathBuf>,
    pub parse_for_context: fn(
        &[String],
        &ExecutionContext,
    ) -> Result<ServiceCommandDispatch, crate::cli::ParseOutcome>,
}

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
    descriptor: crate::audit::AUDIT_DESCRIPTOR,
    save_artifact: crate::audit::save_report,
    parse_for_context: |args, context| {
        crate::audit::parse_args_for_context(args, context).map(ServiceCommandDispatch::Audit)
    },
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
    descriptor: crate::plan::PLAN_DESCRIPTOR,
    save_artifact: crate::plan::save_plan,
    parse_for_context: |args, context| {
        crate::plan::parse_args_for_context(args, context).map(ServiceCommandDispatch::Plan)
    },
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
    descriptor: crate::implementation_audit::IMPLEMENTATION_AUDIT_DESCRIPTOR,
    save_artifact: crate::implementation_audit::save_audit,
    parse_for_context: |args, context| {
        crate::implementation_audit::parse_args_for_context(args, context)
            .map(ServiceCommandDispatch::ImplementationAudit)
    },
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
    descriptor: crate::review::REVIEW_DESCRIPTOR,
    save_artifact: crate::review::save_review,
    parse_for_context: |args, context| {
        crate::review::parse_args_for_context(args, context).map(ServiceCommandDispatch::Review)
    },
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
    descriptor: crate::fix_loop::FIX_LOOP_DESCRIPTOR,
    save_artifact: crate::fix_loop::save_report,
    parse_for_context: |args, context| {
        crate::fix_loop::parse_args_for_context(args, context).map(ServiceCommandDispatch::FixLoop)
    },
};

pub const SERVICE_COMMAND_SPECS: &[ServiceCommandSpec] = &[
    AUDIT_SERVICE_COMMAND_SPEC,
    PLAN_SERVICE_COMMAND_SPEC,
    IMPLEMENTATION_AUDIT_SERVICE_COMMAND_SPEC,
    REVIEW_SERVICE_COMMAND_SPEC,
    FIX_LOOP_SERVICE_COMMAND_SPEC,
];
