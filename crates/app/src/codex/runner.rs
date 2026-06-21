use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::discovery;
use super::request::{CodexMode, CodexRequest};
#[path = "runner/capture.rs"]
pub mod capture;
#[path = "runner/monitor.rs"]
mod monitor;

use capture::ExecCapturePaths;

#[derive(Debug)]
pub struct RunResult {
    pub status: ExitStatus,
    pub final_message: Option<String>,
    pub stdout: String,
    pub stderr: String,
}

pub fn build_command(
    executable: PathBuf,
    request: &CodexRequest,
    inside_git_repository: bool,
) -> Command {
    let mut command = Command::new(executable);
    command.args(base_command_args(request, inside_git_repository));
    if let Some(working_dir) = &request.working_dir {
        command.current_dir(working_dir);
    }
    command.env_remove(crate::reporting::COMMAND_OUTCOME_PATH_ENV);
    discovery::configure_child_path(&mut command);
    command
}

pub fn run_exec(
    mut command: Command,
    verbose: bool,
    use_color_never: bool,
) -> io::Result<RunResult> {
    let capture = ExecCapturePaths::for_exec_run(true);

    append_exec_capture_args(&mut command, capture.output_file(), use_color_never);

    if verbose {
        let status = command.status()?;
        let final_message = capture.read_final_message()?;

        return Ok(RunResult {
            status,
            final_message,
            stdout: String::new(),
            stderr: String::new(),
        });
    }

    command.stdin(Stdio::null());

    let output = command.output()?;
    let final_message = capture.read_final_message()?;

    Ok(RunResult {
        status: output.status,
        final_message,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub fn run_exec_until_final_message(
    mut command: Command,
    verbose: bool,
    timeout: Duration,
    use_color_never: bool,
) -> io::Result<RunResult> {
    let capture = ExecCapturePaths::for_exec_run(verbose);

    monitor::configure_exec_child(
        &mut command,
        &capture,
        verbose,
        use_color_never,
        append_exec_capture_args,
    )?;

    let mut child = command.spawn()?;
    let start = Instant::now();

    loop {
        if let Some(result) = monitor::finish_on_final_message(&mut child, &capture)? {
            return Ok(result);
        }

        if let Some(result) = monitor::finish_on_process_exit(&mut child, &capture)? {
            return Ok(result);
        }

        if start.elapsed() >= timeout {
            return Err(monitor::timeout_error(&mut child, &capture, timeout)?);
        }

        thread::sleep(Duration::from_millis(200));
    }
}

pub fn base_command_args(request: &CodexRequest, inside_git_repository: bool) -> Vec<String> {
    let mut args = Vec::new();
    args.extend(request.permissions.cli_args());

    if request.mode == CodexMode::Exec {
        args.push("exec".to_string());

        if request.resume_last {
            args.push("resume".to_string());
            args.push("--last".to_string());
        }

        if !inside_git_repository {
            args.push("--skip-git-repo-check".to_string());
        }
    }

    args.push("--model".to_string());
    args.push(request.model.clone());
    args.push("--config".to_string());
    args.push(format!(
        "model_reasoning_effort=\"{}\"",
        request.reasoning_effort
    ));

    if let Some(prompt) = request.prompt_with_agent_context() {
        args.push(prompt);
    }

    args
}

pub fn append_exec_capture_args(command: &mut Command, output_file: &Path, use_color_never: bool) {
    command.arg("--output-last-message").arg(output_file);

    if use_color_never {
        command.arg("--color").arg("never");
    }
}
