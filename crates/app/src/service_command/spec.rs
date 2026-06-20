use std::path::PathBuf;
use std::time::Duration;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CodexMode, ReasoningEffort};

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
