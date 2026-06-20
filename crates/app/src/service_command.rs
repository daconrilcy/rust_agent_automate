use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cli::{ParseOutcome, next_value, parse_timeout};
use crate::codex::{
    CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT, ReasoningEffort,
};
use crate::reporting::{self, CompletedReport, ReportFailure, ReportSpec};
use crate::service_paths::{self, ExecutionContext};

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

pub struct ServiceRunSpec<'a> {
    pub intro: String,
    pub command_name: &'a str,
    pub saved_label: &'a str,
    pub final_label: &'a str,
    pub missing_message_label: &'a str,
    pub output_dir: &'a Path,
    pub save: fn(&Path, &str) -> io::Result<PathBuf>,
    pub clean_detector: Option<fn(&str) -> bool>,
}

impl<'a> ServiceRunSpec<'a> {
    fn report_spec(&self) -> ReportSpec<'a> {
        ReportSpec {
            command_name: self.command_name,
            saved_label: self.saved_label,
            final_label: self.final_label,
            missing_message_label: self.missing_message_label,
            output_dir: self.output_dir,
            save: self.save,
            clean_detector: self.clean_detector,
        }
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
pub struct ServiceCommandDescriptor<'a> {
    pub default_output_dir: &'a str,
    pub command_name: &'a str,
    pub artifact_stem: &'a str,
    pub saved_label: &'a str,
    pub final_label: &'a str,
    pub missing_message_label: &'a str,
    pub clean_detector: Option<fn(&str) -> bool>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ServiceCommandDispatch {
    Audit(crate::audit::AuditCommand),
    Plan(crate::plan::PlanCommand),
    ImplementationAudit(crate::implementation_audit::ImplementationAuditCommand),
    Review(crate::review::ReviewCommand),
    FixLoop(crate::fix_loop::FixLoopCommand),
}

impl ServiceCommandDispatch {
    pub fn command_name(&self) -> &'static str {
        match self {
            Self::Audit(_) => "audit",
            Self::Plan(_) => "plan",
            Self::ImplementationAudit(_) => "implementation-audit",
            Self::Review(_) => "review",
            Self::FixLoop(_) => "fix-loop",
        }
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
        match self {
            Self::Audit(command) => crate::run_audit(command),
            Self::Plan(command) => crate::run_plan(command),
            Self::ImplementationAudit(command) => crate::run_implementation_audit(command),
            Self::Review(command) => crate::run_review(command),
            Self::FixLoop(command) => crate::run_fix_loop(command),
        }
    }

    pub fn execute_silently(&self) -> Result<CompletedReport, ReportFailure> {
        match self {
            Self::Audit(command) => crate::audit::run_silently(command),
            Self::Plan(command) => crate::plan::run_silently(command),
            Self::ImplementationAudit(command) => crate::implementation_audit::run_silently(command),
            Self::Review(command) => crate::review::run_silently(command),
            Self::FixLoop(command) => crate::fix_loop::run_silently(command),
        }
    }
}

pub fn parse_common_option(
    args: &[String],
    index: usize,
    options: &mut ServiceCommandOptions,
) -> Result<Option<usize>, ParseOutcome> {
    match args[index].as_str() {
        "-h" | "--help" => Err(ParseOutcome::Help),
        "--model" => {
            options.model = next_value(args, index, "--model")?.to_owned();
            Ok(Some(2))
        }
        "--reasoning" => {
            options.reasoning_effort = next_value(args, index, "--reasoning")?
                .parse()
                .map_err(ParseOutcome::Error)?;
            Ok(Some(2))
        }
        "--output-dir" => {
            options.output_dir = Some(PathBuf::from(next_value(args, index, "--output-dir")?));
            Ok(Some(2))
        }
        "--timeout-seconds" => {
            options.timeout = parse_timeout(next_value(args, index, "--timeout-seconds")?)?;
            Ok(Some(2))
        }
        "--verbose" => {
            options.verbose = true;
            Ok(Some(1))
        }
        "--continue-codex" => {
            options.resume_last = true;
            Ok(Some(1))
        }
        _ => Ok(None),
    }
}

pub fn parse_with_common_options<F>(
    args: &[String],
    options: &mut ServiceCommandOptions,
    mut argument_handler: F,
) -> Result<(), ParseOutcome>
where
    F: FnMut(usize, &str) -> Result<usize, ParseOutcome>,
{
    let mut index = 0;
    while index < args.len() {
        if let Some(consumed) = parse_common_option(args, index, options)? {
            index += consumed;
            continue;
        }

        let consumed = argument_handler(index, args[index].as_str())?;
        if consumed == 0 {
            return Err(ParseOutcome::Error(format!(
                "le parseur partage doit consommer au moins un argument: {}",
                args[index]
            )));
        }
        index += consumed;
    }

    Ok(())
}

pub fn parse_subject_and_artifact<T, F>(
    args: &[String],
    options: &mut ServiceCommandOptions,
    subject_name: &str,
    artifact_name: &str,
    command_name: &str,
    parse_subject: F,
) -> Result<ParsedSubjectArtifact<T>, ParseOutcome>
where
    F: Fn(&str) -> Result<T, String>,
{
    let mut subject: Option<T> = None;
    let mut artifact_path: Option<PathBuf> = None;

    parse_with_common_options(args, options, |index, value| match value {
        value if value == subject_name => {
            let value = next_value(args, index, subject_name)?;
            if subject.is_some() {
                return Err(ParseOutcome::Error(format!(
                    "le type de {command_name} a deja ete fourni"
                )));
            }
            subject = Some(parse_subject(value).map_err(ParseOutcome::Error)?);
            Ok(2)
        }
        value if value == artifact_name => {
            let value = next_value(args, index, artifact_name)?;
            if artifact_path.is_some() {
                return Err(ParseOutcome::Error(format!(
                    "l'artefact de {command_name} a deja ete fourni"
                )));
            }
            artifact_path = Some(PathBuf::from(value));
            Ok(2)
        }
        value if value.starts_with("--") => {
            Err(ParseOutcome::Error(format!("option inconnue: {value}")))
        }
        value => {
            if subject.is_none() {
                subject = Some(parse_subject(value).map_err(ParseOutcome::Error)?);
                Ok(1)
            } else if artifact_path.is_none() {
                artifact_path = Some(PathBuf::from(value));
                Ok(1)
            } else {
                Err(ParseOutcome::Error(format!(
                    "argument inattendu pour {command_name}: {value}"
                )))
            }
        }
    })?;

    let subject = subject.ok_or_else(|| {
        ParseOutcome::Error(format!(
            "la commande {command_name} requiert un type: plan, audit ou implementation"
        ))
    })?;
    let artifact_path = artifact_path.ok_or_else(|| {
        ParseOutcome::Error(format!(
            "la commande {command_name} requiert un artefact. Exemple: cargo run -p app -- {command_name} plan .plan\\plan.md"
        ))
    })?;

    Ok(ParsedSubjectArtifact {
        subject,
        artifact_path,
    })
}

pub fn parse_required_path_with_optional_named_path(
    args: &[String],
    options: &mut ServiceCommandOptions,
    required_option_name: &str,
    optional_option_name: &str,
    command_name: &str,
    required_label: &str,
    required_example: &str,
    duplicate_required_message: &str,
    duplicate_optional_message: &str,
) -> Result<ParsedRequiredPath, ParseOutcome> {
    let mut required_path: Option<PathBuf> = None;
    let mut optional_path: Option<PathBuf> = None;

    parse_with_common_options(args, options, |index, value| match value {
        value if value == required_option_name => {
            let value = next_value(args, index, required_option_name)?;
            if required_path.is_some() {
                return Err(ParseOutcome::Error(duplicate_required_message.to_string()));
            }
            required_path = Some(PathBuf::from(value));
            Ok(2)
        }
        value if value == optional_option_name => {
            let value = next_value(args, index, optional_option_name)?;
            if optional_path.is_some() {
                return Err(ParseOutcome::Error(duplicate_optional_message.to_string()));
            }
            optional_path = Some(PathBuf::from(value));
            Ok(2)
        }
        value if value.starts_with("--") => {
            Err(ParseOutcome::Error(format!("option inconnue: {value}")))
        }
        value => {
            if required_path.is_some() {
                Err(ParseOutcome::Error(format!(
                    "argument inattendu pour {command_name}: {value}"
                )))
            } else {
                required_path = Some(PathBuf::from(value));
                Ok(1)
            }
        }
    })?;

    let required_path = required_path.ok_or_else(|| {
        ParseOutcome::Error(format!(
            "la commande {command_name} requiert {required_label}. Exemple: {required_example}"
        ))
    })?;

    Ok(ParsedRequiredPath {
        required_path,
        optional_path,
    })
}

pub fn resolve_context() -> Result<ExecutionContext, String> {
    service_paths::current_execution_context()
}

pub fn resolve_output_dir(
    options: &ServiceCommandOptions,
    context: &ExecutionContext,
    default_dir_name: &str,
) -> PathBuf {
    service_paths::resolve_output_dir(options.output_dir.clone(), context, default_dir_name)
}

pub fn prepare_service_command(
    options: &ServiceCommandOptions,
    context: &ExecutionContext,
    descriptor: ServiceCommandDescriptor<'_>,
    prompt: String,
) -> PreparedServiceCommand {
    PreparedServiceCommand {
        request: options.build_request(prompt),
        workspace_root: context.workspace_root().to_path_buf(),
        output_dir: resolve_output_dir(options, context, descriptor.default_output_dir),
        timeout: options.timeout,
    }
}

pub fn prepare_parse_context(
    options: &ServiceCommandOptions,
    descriptor: ServiceCommandDescriptor<'_>,
) -> Result<PreparedParseContext, String> {
    let context = resolve_context()?;
    Ok(prepare_parse_context_for_context(options, descriptor, context))
}

pub fn prepare_parse_context_for_context(
    options: &ServiceCommandOptions,
    descriptor: ServiceCommandDescriptor<'_>,
    context: ExecutionContext,
) -> PreparedParseContext {
    let workspace_root = context.workspace_root().to_path_buf();
    let output_dir = resolve_output_dir(options, &context, descriptor.default_output_dir);

    PreparedParseContext {
        context,
        workspace_root,
        output_dir,
    }
}

pub fn prepare_service_from_prompt(
    options: &ServiceCommandOptions,
    parse_context: &PreparedParseContext,
    descriptor: ServiceCommandDescriptor<'_>,
    prompt: String,
) -> PreparedServiceCommand {
    prepare_service_command(options, &parse_context.context, descriptor, prompt)
}

pub fn execute_service_command(
    command: &PreparedServiceCommand,
    descriptor: ServiceCommandDescriptor<'_>,
    intro: String,
    save: fn(&Path, &str) -> io::Result<PathBuf>,
) -> Result<CompletedReport, ReportFailure> {
    run_service_command(
        command,
        ServiceRunSpec {
            intro,
            command_name: descriptor.command_name,
            saved_label: descriptor.saved_label,
            final_label: descriptor.final_label,
            missing_message_label: descriptor.missing_message_label,
            output_dir: &command.output_dir,
            save,
            clean_detector: descriptor.clean_detector,
        },
    )
}

pub fn execute_service_command_silently(
    command: &PreparedServiceCommand,
    descriptor: ServiceCommandDescriptor<'_>,
    save: fn(&Path, &str) -> io::Result<PathBuf>,
) -> Result<CompletedReport, ReportFailure> {
    run_service_command_silently(
        command,
        ReportSpec {
            command_name: descriptor.command_name,
            saved_label: descriptor.saved_label,
            final_label: descriptor.final_label,
            missing_message_label: descriptor.missing_message_label,
            output_dir: &command.output_dir,
            save,
            clean_detector: descriptor.clean_detector,
        },
    )
}

pub fn run_service_command(
    command: &PreparedServiceCommand,
    spec: ServiceRunSpec<'_>,
) -> Result<CompletedReport, ReportFailure> {
    eprintln!("{}", spec.intro);
    let result = run_service_command_silently(command, spec.report_spec());

    match &result {
        Ok(report) => reporting::print_completed_report(report, &spec.report_spec()),
        Err(error) => reporting::print_report_failure(error, &spec.report_spec()),
    }

    result
}

pub fn run_service_command_silently(
    command: &PreparedServiceCommand,
    spec: ReportSpec<'_>,
) -> Result<CompletedReport, ReportFailure> {
    reporting::run_codex_report(&command.request, command.timeout, spec)
}

pub fn save_markdown_artifact(
    output_dir: &Path,
    descriptor: ServiceCommandDescriptor<'_>,
    content: &str,
) -> io::Result<PathBuf> {
    crate::artifact::save_timestamped_markdown(output_dir, descriptor.artifact_stem, content)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            "--plan",
            "--implementation",
            "implementation-audit",
            "un plan",
            "cargo run -p app -- implementation-audit .plan\\plan.md",
            "plan duplique",
            "implementation dupliquee",
        )
        .expect("parse valide");

        assert_eq!(parsed.required_path, PathBuf::from("plan.md"));
        assert_eq!(parsed.optional_path, Some(PathBuf::from("crates\\app")));
        assert_eq!(options.timeout, Duration::from_secs(42));
    }
}
