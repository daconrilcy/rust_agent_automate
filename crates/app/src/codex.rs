#[path = "codex/discovery.rs"]
mod discovery;
#[path = "codex/repo.rs"]
mod repo;
#[path = "codex/request.rs"]
mod request;
#[path = "codex/runner.rs"]
mod runner;

use std::io;
use std::time::Duration;

pub use request::{
    CodexMode, CodexRequest, ReasoningEffort, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT,
};
pub use runner::RunResult;

pub fn process_exit_code(status_code: Option<i32>) -> i32 {
    match status_code {
        Some(code @ 0..=255) => code,
        _ => 1,
    }
}

pub fn run(request: &CodexRequest) -> io::Result<RunResult> {
    let executable = discovery::resolve_codex_executable()?;
    let inside_git_repository = repo::is_inside_git_repository()?;
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
    let inside_git_repository = repo::is_inside_git_repository()?;
    let command = runner::build_command(executable, request, inside_git_repository);

    match request.mode {
        CodexMode::Interactive => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "run_until_final_message requiert le mode exec",
        )),
        CodexMode::Exec => {
            runner::run_exec_until_final_message(command, request.verbose, timeout, !request.resume_last)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_exit_code_maps_non_portable_child_statuses_to_generic_failure() {
        assert_eq!(process_exit_code(Some(0)), 0);
        assert_eq!(process_exit_code(Some(7)), 7);
        assert_eq!(process_exit_code(Some(-1)), 1);
        assert_eq!(process_exit_code(Some(256)), 1);
        assert_eq!(process_exit_code(None), 1);
    }
}
