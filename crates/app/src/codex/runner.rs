use std::fs;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::discovery;
use super::request::{CodexMode, CodexRequest};

struct ExecCapturePaths {
    output_file: PathBuf,
    stderr_file: Option<PathBuf>,
}

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
    command.env_remove(crate::reporting::COMMAND_OUTCOME_PATH_ENV);
    discovery::configure_child_path(&mut command);
    command
}

pub fn run_exec(
    mut command: Command,
    verbose: bool,
    use_color_never: bool,
) -> io::Result<RunResult> {
    let capture = ExecCapturePaths {
        output_file: temp_output_file_path(),
        stderr_file: None,
    };

    append_exec_capture_args(&mut command, &capture.output_file, use_color_never);

    if verbose {
        let status = command.status()?;
        let final_message = read_final_message(&capture.output_file)?;

        return Ok(RunResult {
            status,
            final_message,
            stdout: String::new(),
            stderr: String::new(),
        });
    }

    command.stdin(Stdio::null());

    let output = command.output()?;
    let final_message = read_final_message(&capture.output_file)?;

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
    let capture = ExecCapturePaths {
        output_file: temp_output_file_path(),
        stderr_file: (!verbose).then(temp_stderr_file_path),
    };

    configure_exec_child(&mut command, &capture, verbose, use_color_never)?;

    let mut child = command.spawn()?;
    let start = Instant::now();

    loop {
        if let Some(result) = finish_on_final_message(&mut child, &capture)? {
            return Ok(result);
        }

        if let Some(result) = finish_on_process_exit(&mut child, &capture)? {
            return Ok(result);
        }

        if start.elapsed() >= timeout {
            return Err(timeout_error(&mut child, &capture, timeout)?);
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

fn configure_exec_child(
    command: &mut Command,
    capture: &ExecCapturePaths,
    verbose: bool,
    use_color_never: bool,
) -> io::Result<()> {
    append_exec_capture_args(command, &capture.output_file, use_color_never);
    command.stdin(Stdio::null());

    if verbose {
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        let stderr = File::create(
            capture
                .stderr_file
                .as_ref()
                .expect("stderr_file is set for non-verbose exec"),
        )?;
        command.stdout(Stdio::null()).stderr(Stdio::from(stderr));
    }

    Ok(())
}

fn finish_on_final_message(
    child: &mut Child,
    capture: &ExecCapturePaths,
) -> io::Result<Option<RunResult>> {
    let Some(final_message) = read_final_message_if_ready(&capture.output_file)? else {
        return Ok(None);
    };
    let status = wait_or_terminate(child, Duration::from_secs(2))?;
    let stderr = read_optional_file(capture.stderr_file.as_deref())?;

    Ok(Some(RunResult {
        status,
        final_message: Some(final_message),
        stdout: String::new(),
        stderr,
    }))
}

fn finish_on_process_exit(
    child: &mut Child,
    capture: &ExecCapturePaths,
) -> io::Result<Option<RunResult>> {
    let Some(status) = child.try_wait()? else {
        return Ok(None);
    };
    let final_message = read_final_message(&capture.output_file)?;
    let stderr = read_optional_file(capture.stderr_file.as_deref())?;

    Ok(Some(RunResult {
        status,
        final_message,
        stdout: String::new(),
        stderr,
    }))
}

fn timeout_error(
    child: &mut Child,
    capture: &ExecCapturePaths,
    timeout: Duration,
) -> io::Result<io::Error> {
    let _ = terminate_process_tree(child);
    let _ = child.wait();
    let stderr =
        read_optional_file_without_removing(capture.stderr_file.as_deref()).unwrap_or_default();
    let stderr = tail_for_error(&stderr, 4_000);
    let diagnostics = timeout_diagnostics(&capture.output_file, capture.stderr_file.as_deref());
    let detail = if stderr.trim().is_empty() {
        diagnostics
    } else {
        format!("{diagnostics}\nDerniere sortie stderr de codex:\n{stderr}")
    };

    Ok(io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "codex n'a pas produit de message final dans les {} secondes{detail}",
            timeout.as_secs(),
        ),
    ))
}

fn wait_or_terminate(child: &mut Child, grace_period: Duration) -> io::Result<ExitStatus> {
    let start = Instant::now();

    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }

        if start.elapsed() >= grace_period {
            let _ = terminate_process_tree(child);
            return child.wait();
        }

        thread::sleep(Duration::from_millis(50));
    }
}

pub(crate) fn read_final_message(path: &Path) -> io::Result<Option<String>> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    let _ = fs::remove_file(path);

    let trimmed = content.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn read_final_message_if_ready(path: &Path) -> io::Result<Option<String>> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    let trimmed = content.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        let _ = fs::remove_file(path);
        Ok(Some(trimmed.to_string()))
    }
}

fn temp_output_file_path() -> PathBuf {
    temp_codex_file_path("last_message")
}

fn temp_stderr_file_path() -> PathBuf {
    temp_codex_file_path("stderr")
}

fn temp_codex_file_path(kind: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    std::env::temp_dir().join(format!(
        "rust_agent_codex_{}_{}_{}.txt",
        std::process::id(),
        timestamp,
        kind
    ))
}

fn read_optional_file(path: Option<&Path>) -> io::Result<String> {
    let Some(path) = path else {
        return Ok(String::new());
    };

    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };

    let _ = fs::remove_file(path);
    Ok(content)
}

fn read_optional_file_without_removing(path: Option<&Path>) -> io::Result<String> {
    let Some(path) = path else {
        return Ok(String::new());
    };

    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error),
    }
}

fn timeout_diagnostics(output_file: &Path, stderr_file: Option<&Path>) -> String {
    let mut message = format!(
        "\nDiagnostics conserves pour inspection:\n- fichier message final: {}",
        output_file.display()
    );

    if let Some(stderr_file) = stderr_file {
        message.push_str(&format!(
            "\n- fichier stderr codex: {}",
            stderr_file.display()
        ));
    }

    message
}

fn tail_for_error(content: &str, max_chars: usize) -> String {
    let char_count = content.chars().count();

    if char_count <= max_chars {
        return content.to_string();
    }

    let tail = content
        .chars()
        .skip(char_count.saturating_sub(max_chars))
        .collect::<String>();

    format!("...{tail}")
}

fn terminate_process_tree(child: &mut Child) -> io::Result<()> {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        Ok(())
    }

    #[cfg(not(windows))]
    {
        child.kill()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexMode, CodexRequest, ReasoningEffort};

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
    fn read_final_message_returns_none_for_missing_file() {
        let missing = std::env::temp_dir().join(format!(
            "rust_agent_missing_{}.txt",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));

        let result = read_final_message(&missing).expect("la lecture doit gerer un fichier absent");

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

        let result = read_final_message(&path).expect("lecture du fichier temporaire");

        assert_eq!(result.as_deref(), Some("reponse finale"));
        assert!(!path.exists(), "le fichier temporaire doit etre supprime");
    }

    #[test]
    fn timeout_diagnostics_mentions_capture_files() {
        let message = timeout_diagnostics(Path::new("last.txt"), Some(Path::new("stderr.txt")));

        assert!(message.contains("last.txt"));
        assert!(message.contains("stderr.txt"));
    }
}
