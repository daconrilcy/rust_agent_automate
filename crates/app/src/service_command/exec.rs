use std::io;
use std::path::{Path, PathBuf};

use crate::reporting::{self, CompletedReport, ReportFailure, ReportSpec};

use super::{PreparedServiceCommand, ServiceCommandDescriptor};

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
