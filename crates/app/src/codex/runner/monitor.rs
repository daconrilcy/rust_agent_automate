use std::fs::File;
use std::io;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::RunResult;
use super::capture::ExecCapturePaths;

pub fn configure_exec_child(
    command: &mut Command,
    capture: &ExecCapturePaths,
    verbose: bool,
    use_color_never: bool,
    append_exec_capture_args: impl Fn(&mut Command, &std::path::Path, bool),
) -> io::Result<()> {
    append_exec_capture_args(command, capture.output_file(), use_color_never);
    command.stdin(Stdio::null());

    if verbose {
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        let stderr = File::create(
            capture
                .stderr_file()
                .expect("stderr_file is set for non-verbose exec"),
        )?;
        command.stdout(Stdio::null()).stderr(Stdio::from(stderr));
    }

    Ok(())
}

pub fn finish_on_final_message(
    child: &mut Child,
    capture: &ExecCapturePaths,
) -> io::Result<Option<RunResult>> {
    let Some(final_message) = capture.read_final_message_if_ready()? else {
        return Ok(None);
    };
    let status = wait_or_terminate(child, Duration::from_secs(2))?;
    let stderr = capture.read_stderr()?;

    Ok(Some(RunResult {
        status,
        final_message: Some(final_message),
        stdout: String::new(),
        stderr,
    }))
}

pub fn finish_on_process_exit(
    child: &mut Child,
    capture: &ExecCapturePaths,
) -> io::Result<Option<RunResult>> {
    let Some(status) = child.try_wait()? else {
        return Ok(None);
    };
    let final_message = capture.read_final_message()?;
    let stderr = capture.read_stderr()?;

    Ok(Some(RunResult {
        status,
        final_message,
        stdout: String::new(),
        stderr,
    }))
}

pub fn timeout_error(
    child: &mut Child,
    capture: &ExecCapturePaths,
    timeout: Duration,
) -> io::Result<io::Error> {
    let _ = terminate_process_tree(child);
    let _ = child.wait();
    let stderr = capture.read_stderr_without_removing().unwrap_or_default();
    let stderr = tail_for_error(&stderr, 4_000);
    let diagnostics = capture.timeout_diagnostics();
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
