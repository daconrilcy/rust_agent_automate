mod discovery;
mod repo;
mod request;
mod runner;

use std::io;
use std::time::Duration;

pub use request::{
    CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT, ReasoningEffort,
};
pub use runner::RunResult;

pub fn process_exit_code(status_code: Option<i32>) -> i32 {
    status_code
        .filter(|code| (0..=255).contains(code))
        .unwrap_or(1)
}

pub fn run(request: &CodexRequest) -> io::Result<RunResult> {
    let executable = discovery::resolve_codex_executable()?;
    let inside_git_repository = repo::is_inside_git_repository_at(request.working_dir.as_deref())?;
    let mut command = runner::build_command(executable, request, inside_git_repository);

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
        CodexMode::Exec => runner::run_exec(command, request.verbose, !request.resume_last),
    }
}

pub fn run_until_final_message(request: &CodexRequest, timeout: Duration) -> io::Result<RunResult> {
    let executable = discovery::resolve_codex_executable()?;
    let inside_git_repository = repo::is_inside_git_repository_at(request.working_dir.as_deref())?;
    let command = runner::build_command(executable, request, inside_git_repository);

    match request.mode {
        CodexMode::Interactive => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "run_until_final_message requiert le mode exec",
        )),
        CodexMode::Exec => runner::run_exec_until_final_message(
            command,
            request.verbose,
            timeout,
            !request.resume_last,
        ),
    }
}
