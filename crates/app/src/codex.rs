use std::fmt;
use std::fs;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

pub const DEFAULT_MODEL: &str = "gpt-5.4";
pub const DEFAULT_REASONING_EFFORT: ReasoningEffort = ReasoningEffort::Low;

impl ReasoningEffort {
    pub fn as_config_value(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl fmt::Display for ReasoningEffort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_config_value())
    }
}

impl std::str::FromStr for ReasoningEffort {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => Err(format!(
                "niveau de raisonnement invalide: {value}. Valeurs attendues: low, medium, high"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexMode {
    Interactive,
    Exec,
}

impl std::str::FromStr for CodexMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "interactive" => Ok(Self::Interactive),
            "exec" => Ok(Self::Exec),
            _ => Err(format!(
                "mode invalide: {value}. Valeurs attendues: interactive, exec"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexRequest {
    pub model: String,
    pub reasoning_effort: ReasoningEffort,
    pub mode: CodexMode,
    pub prompt: Option<String>,
    pub verbose: bool,
}

impl CodexRequest {
    pub fn new(
        model: impl Into<String>,
        reasoning_effort: ReasoningEffort,
        mode: CodexMode,
        prompt: Option<String>,
        verbose: bool,
    ) -> Self {
        Self {
            model: model.into(),
            reasoning_effort,
            mode,
            prompt,
            verbose,
        }
    }
}

#[derive(Debug)]
pub struct RunResult {
    pub status: ExitStatus,
    pub final_message: Option<String>,
    pub stdout: String,
    pub stderr: String,
}

pub fn run(request: &CodexRequest) -> io::Result<RunResult> {
    let executable = resolve_codex_executable()?;
    let inside_git_repository = is_inside_git_repository()?;
    let mut command = build_command(executable, request, inside_git_repository);

    match request.mode {
        CodexMode::Interactive => {
            let status = command.status()?;
            Ok(RunResult {
                status,
                final_message: None,
                stdout: String::new(),
                stderr: String::new(),
            })
        }
        CodexMode::Exec => run_exec(command, request.verbose),
    }
}

pub fn run_until_final_message(request: &CodexRequest, timeout: Duration) -> io::Result<RunResult> {
    let executable = resolve_codex_executable()?;
    let inside_git_repository = is_inside_git_repository()?;
    let command = build_command(executable, request, inside_git_repository);

    match request.mode {
        CodexMode::Interactive => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "run_until_final_message requiert le mode exec",
        )),
        CodexMode::Exec => run_exec_until_final_message(command, request.verbose, timeout),
    }
}

fn build_command(
    executable: PathBuf,
    request: &CodexRequest,
    inside_git_repository: bool,
) -> Command {
    let mut command = Command::new(executable);
    command.args(base_command_args(request, inside_git_repository));
    configure_child_path(&mut command);
    command
}

fn base_command_args(request: &CodexRequest, inside_git_repository: bool) -> Vec<String> {
    let mut args = Vec::new();

    if request.mode == CodexMode::Exec {
        args.push("exec".to_string());

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

fn run_exec(mut command: Command, verbose: bool) -> io::Result<RunResult> {
    let output_file = temp_output_file_path();

    command
        .arg("--output-last-message")
        .arg(&output_file)
        .arg("--color")
        .arg("never");

    if verbose {
        let status = command.status()?;
        let final_message = read_final_message(&output_file)?;

        return Ok(RunResult {
            status,
            final_message,
            stdout: String::new(),
            stderr: String::new(),
        });
    }

    command.stdin(Stdio::null());

    let output = command.output()?;
    let final_message = read_final_message(&output_file)?;

    Ok(RunResult {
        status: output.status,
        final_message,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn run_exec_until_final_message(
    mut command: Command,
    verbose: bool,
    timeout: Duration,
) -> io::Result<RunResult> {
    let output_file = temp_output_file_path();
    let stderr_file = (!verbose).then(temp_stderr_file_path);

    command
        .arg("--output-last-message")
        .arg(&output_file)
        .arg("--color")
        .arg("never")
        .stdin(Stdio::null());

    if verbose {
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        let stderr = File::create(stderr_file.as_ref().expect("stderr_file is set"))?;
        command.stdout(Stdio::null()).stderr(Stdio::from(stderr));
    }

    let mut child = command.spawn()?;
    let start = Instant::now();

    loop {
        if let Some(final_message) = read_final_message_if_ready(&output_file)? {
            let status = wait_or_terminate(&mut child, Duration::from_secs(2))?;
            let stderr = read_optional_file(stderr_file.as_deref())?;

            return Ok(RunResult {
                status,
                final_message: Some(final_message),
                stdout: String::new(),
                stderr,
            });
        }

        if let Some(status) = child.try_wait()? {
            let final_message = read_final_message(&output_file)?;
            let stderr = read_optional_file(stderr_file.as_deref())?;

            return Ok(RunResult {
                status,
                final_message,
                stdout: String::new(),
                stderr,
            });
        }

        if start.elapsed() >= timeout {
            let _ = terminate_process_tree(&mut child);
            let _ = child.wait();
            let stderr =
                read_optional_file_without_removing(stderr_file.as_deref()).unwrap_or_default();
            let stderr = tail_for_error(&stderr, 4_000);
            let diagnostics = timeout_diagnostics(&output_file, stderr_file.as_deref());
            let detail = if stderr.trim().is_empty() {
                diagnostics
            } else {
                format!("{diagnostics}\nDerniere sortie stderr de codex:\n{stderr}")
            };

            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "codex n'a pas produit de message final dans les {} secondes{detail}",
                    timeout.as_secs(),
                ),
            ));
        }

        thread::sleep(Duration::from_millis(200));
    }
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

fn read_final_message(path: &Path) -> io::Result<Option<String>> {
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

fn resolve_codex_executable() -> io::Result<PathBuf> {
    let mut candidates = executable_candidates();
    candidates.dedup();

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, codex_not_found_message()))
}

fn executable_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from("codex.exe"), PathBuf::from("codex")];

    if let Some(path) = std::env::var_os("CODEX_CLI_PATH") {
        candidates.insert(0, PathBuf::from(path));
    }

    if let Some(user_profile) = std::env::var_os("USERPROFILE") {
        let user_profile = PathBuf::from(user_profile);

        candidates.extend(vscode_extension_candidates(&user_profile));
        candidates.push(user_profile.join("AppData\\Roaming\\npm\\codex.exe"));
        candidates.push(user_profile.join("AppData\\Roaming\\npm\\codex.cmd"));
    }

    candidates.push(PathBuf::from("codex.cmd"));

    candidates
}

fn configure_child_path(command: &mut Command) {
    let Some(path) = child_path_with_tool_dirs_first() else {
        return;
    };

    command.env("PATH", path);
}

fn child_path_with_tool_dirs_first() -> Option<std::ffi::OsString> {
    let existing_path = std::env::var_os("PATH")?;
    let tool_dirs = codex_tool_dirs();

    if tool_dirs.is_empty() {
        return None;
    }

    prepend_path_dirs(existing_path, &tool_dirs)
}

fn codex_tool_dirs() -> Vec<PathBuf> {
    let Some(user_profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return Vec::new();
    };

    vscode_extension_tool_dirs(&user_profile)
}

fn vscode_extension_tool_dirs(user_profile: &Path) -> Vec<PathBuf> {
    let extensions_dir = user_profile.join(".vscode\\extensions");
    let Ok(entries) = fs::read_dir(extensions_dir) else {
        return Vec::new();
    };

    let mut dirs = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("openai.chatgpt-"))
        })
        .map(|path| path.join("bin\\windows-x86_64"))
        .filter(|path| path.join("rg.exe").is_file())
        .collect::<Vec<_>>();

    dirs.sort_by(|left, right| right.cmp(left));
    dirs
}

fn prepend_path_dirs(
    existing_path: std::ffi::OsString,
    front_dirs: &[PathBuf],
) -> Option<std::ffi::OsString> {
    let mut paths = Vec::new();

    for dir in front_dirs {
        push_unique_path(&mut paths, dir.clone());
    }

    for path in std::env::split_paths(&existing_path) {
        push_unique_path(&mut paths, path);
    }

    std::env::join_paths(paths).ok()
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if paths.iter().any(|existing| paths_equal(existing, &path)) {
        return;
    }

    paths.push(path);
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

fn codex_not_found_message() -> String {
    "impossible de trouver l'executable Codex. Verifie que Codex est installe ou definis la variable d'environnement CODEX_CLI_PATH".to_string()
}

fn is_inside_git_repository() -> io::Result<bool> {
    let mut current = std::env::current_dir()?;

    loop {
        if current.join(".git").exists() {
            return Ok(true);
        }

        if !current.pop() {
            return Ok(false);
        }
    }
}

fn vscode_extension_candidates(user_profile: &Path) -> Vec<PathBuf> {
    let extensions_dir = user_profile.join(".vscode\\extensions");
    let Ok(entries) = fs::read_dir(extensions_dir) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("openai.chatgpt-"))
        })
        .map(|path| path.join("bin\\windows-x86_64\\codex.exe"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
    fn prepends_tool_dirs_without_duplicates() {
        let existing =
            std::env::join_paths([PathBuf::from("C:\\Windows"), PathBuf::from("C:\\Tools")])
                .expect("construction du PATH de test");
        let tool_dirs = vec![PathBuf::from("C:\\Tools"), PathBuf::from("C:\\Codex\\bin")];

        let updated = prepend_path_dirs(existing, &tool_dirs).expect("PATH valide");
        let paths = std::env::split_paths(&updated).collect::<Vec<_>>();

        assert_eq!(paths[0], PathBuf::from("C:\\Tools"));
        assert_eq!(paths[1], PathBuf::from("C:\\Codex\\bin"));
        assert_eq!(paths[2], PathBuf::from("C:\\Windows"));
        assert_eq!(paths.len(), 3);
    }
}
