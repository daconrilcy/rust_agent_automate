use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cli::{ParseOutcome, next_value, parse_timeout};
use crate::codex::{
    CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT, ReasoningEffort,
};
use crate::reporting::{self, ReportSpec};
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

#[derive(Debug, Clone, Copy)]
pub struct ServiceCommandDescriptor<'a> {
    pub default_output_dir: &'a str,
    pub command_name: &'a str,
    pub saved_label: &'a str,
    pub final_label: &'a str,
    pub missing_message_label: &'a str,
    pub clean_detector: Option<fn(&str) -> bool>,
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

pub fn execute_service_command(
    command: &PreparedServiceCommand,
    descriptor: ServiceCommandDescriptor<'_>,
    intro: String,
    save: fn(&Path, &str) -> io::Result<PathBuf>,
) -> Result<crate::reporting::CompletedReport, crate::reporting::ReportFailure> {
    run_service_command(
        &command.request,
        command.timeout,
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

pub fn run_service_command(
    request: &CodexRequest,
    timeout: Duration,
    spec: ServiceRunSpec<'_>,
) -> Result<crate::reporting::CompletedReport, crate::reporting::ReportFailure> {
    eprintln!("{}", spec.intro);
    let report_spec = spec.report_spec();
    let result = reporting::run_codex_report(request, timeout, report_spec);

    match &result {
        Ok(report) => reporting::print_completed_report(report, &spec.report_spec()),
        Err(error) => reporting::print_report_failure(error, &spec.report_spec()),
    }

    result
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
}
