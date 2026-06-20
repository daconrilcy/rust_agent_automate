use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::discovery;
use super::request::{CodexMode, CodexRequest};
#[path = "runner/capture.rs"]
mod capture;
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

pub(crate) fn base_command_args(
    request: &CodexRequest,
    inside_git_repository: bool,
) -> Vec<String> {
    let mut args = Vec::new();

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

    if let Some(prompt) = &request.prompt {
        args.push(prompt.clone());
    }

    args
}

pub(crate) fn append_exec_capture_args(
    command: &mut Command,
    output_file: &Path,
    use_color_never: bool,
) {
    command.arg("--output-last-message").arg(output_file);

    if use_color_never {
        command.arg("--color").arg("never");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexMode, CodexRequest, ReasoningEffort};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn interactive_mode_builds_minimal_arguments() {
        let request = CodexRequest::new(
            "gpt-5.4",
            ReasoningEffort::Medium,
            CodexMode::Interactive,
            None,
            false,
        );

        let args = base_command_args(&request, true);

        assert_eq!(
            args,
            vec![
                "--model".to_string(),
                "gpt-5.4".to_string(),
                "--config".to_string(),
                "model_reasoning_effort=\"medium\"".to_string(),
            ]
        );
    }

    #[test]
    fn exec_mode_outside_git_adds_skip_check_and_prompt() {
        let request = CodexRequest::new(
            "gpt-5.5",
            ReasoningEffort::High,
            CodexMode::Exec,
            Some("Analyse ce repo".to_string()),
            false,
        );

        let args = base_command_args(&request, false);

        assert_eq!(
            args,
            vec![
                "exec".to_string(),
                "--skip-git-repo-check".to_string(),
                "--model".to_string(),
                "gpt-5.5".to_string(),
                "--config".to_string(),
                "model_reasoning_effort=\"high\"".to_string(),
                "Analyse ce repo".to_string(),
            ]
        );
    }

    #[test]
    fn exec_resume_mode_adds_resume_last_before_prompt() {
        let request = CodexRequest::new(
            "gpt-5.5",
            ReasoningEffort::High,
            CodexMode::Exec,
            Some("Continue".to_string()),
            false,
        )
        .with_resume_last(true);

        let args = base_command_args(&request, false);

        assert_eq!(
            args,
            vec![
                "exec".to_string(),
                "resume".to_string(),
                "--last".to_string(),
                "--skip-git-repo-check".to_string(),
                "--model".to_string(),
                "gpt-5.5".to_string(),
                "--config".to_string(),
                "model_reasoning_effort=\"high\"".to_string(),
                "Continue".to_string(),
            ]
        );
    }

    #[test]
    fn capture_args_can_omit_color_for_resume_commands() {
        let mut command = Command::new("codex");
        append_exec_capture_args(&mut command, Path::new("last.txt"), false);

        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(args, vec!["--output-last-message", "last.txt"]);
    }

    #[test]
    fn capture_args_keep_color_for_fresh_exec_commands() {
        let mut command = Command::new("codex");
        append_exec_capture_args(&mut command, Path::new("last.txt"), true);

        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            vec!["--output-last-message", "last.txt", "--color", "never"]
        );
    }

    #[test]
    fn build_command_prevents_report_outcome_env_leaking_to_codex() {
        let request = CodexRequest::new(
            "gpt-5.5",
            ReasoningEffort::Low,
            CodexMode::Exec,
            Some("Run tests".to_string()),
            false,
        );

        let command = build_command(PathBuf::from("codex"), &request, true);

        let outcome_env = command
            .get_envs()
            .find(|(name, _value)| *name == crate::reporting::COMMAND_OUTCOME_PATH_ENV);
        assert_eq!(
            outcome_env,
            Some((
                std::ffi::OsStr::new(crate::reporting::COMMAND_OUTCOME_PATH_ENV),
                None
            ))
        );
    }

    #[test]
    fn build_command_uses_request_working_directory_when_provided() {
        let request = CodexRequest::new(
            "gpt-5.5",
            ReasoningEffort::Low,
            CodexMode::Exec,
            Some("Run tests".to_string()),
            false,
        )
        .with_working_dir(PathBuf::from("C:\\repo\\target"));

        let command = build_command(PathBuf::from("codex"), &request, true);

        assert_eq!(
            command.get_current_dir(),
            Some(std::path::Path::new("C:\\repo\\target"))
        );
    }

    #[test]
    fn read_final_message_returns_none_for_missing_file() {
        let missing = std::env::temp_dir().join(format!(
            "rust_agent_missing_{}.txt",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));

        let result =
            capture::read_final_message(&missing).expect("la lecture doit gerer un fichier absent");

        assert_eq!(result, None);
    }

    #[test]
    fn read_final_message_trims_and_deletes_file() {
        let path = std::env::temp_dir().join(format!(
            "rust_agent_capture_{}.txt",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));

        fs::write(&path, "  reponse finale  \n").expect("ecriture du fichier temporaire");

        let result = capture::read_final_message(&path).expect("lecture du fichier temporaire");

        assert_eq!(result.as_deref(), Some("reponse finale"));
        assert!(!path.exists(), "le fichier temporaire doit etre supprime");
    }

    #[test]
    fn read_final_message_if_ready_preserves_empty_file() {
        let path = std::env::temp_dir().join(format!(
            "rust_agent_capture_ready_empty_{}.txt",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));

        fs::write(&path, " \n\t ").expect("ecriture du fichier temporaire");

        let result = capture::read_final_message_if_ready(&path)
            .expect("lecture du fichier temporaire vide");

        assert_eq!(result, None);
        assert!(path.exists(), "le fichier vide doit rester disponible");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn read_final_message_if_ready_trims_and_deletes_file() {
        let path = std::env::temp_dir().join(format!(
            "rust_agent_capture_ready_{}.txt",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));

        fs::write(&path, "  reponse prete  \n").expect("ecriture du fichier temporaire");

        let result =
            capture::read_final_message_if_ready(&path).expect("lecture du fichier temporaire");

        assert_eq!(result.as_deref(), Some("reponse prete"));
        assert!(!path.exists(), "le fichier temporaire doit etre supprime");
    }

    #[test]
    fn timeout_diagnostics_mentions_capture_files() {
        let message =
            capture::timeout_diagnostics(Path::new("last.txt"), Some(Path::new("stderr.txt")));

        assert!(message.contains("last.txt"));
        assert!(message.contains("stderr.txt"));
    }
}
