use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn temp_dir(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "rust_agent_{prefix}_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    ))
}

pub fn create_fake_codex_bin(root: &Path) -> PathBuf {
    let bin_dir = root.join("bin");
    fs::create_dir_all(&bin_dir).expect("creation du dossier bin");

    let source = r#"
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if let Ok(log_path) = env::var("FAKE_CODEX_LOG") {
        let exe = env::current_exe()
            .ok()
            .and_then(|path| path.into_os_string().into_string().ok())
            .unwrap_or_default();
        let line = if args.is_empty() { String::new() } else { args.join(" ") };
        let _ = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .and_then(|mut file| {
                std::io::Write::write_all(&mut file, format!("{exe} {line}\n").as_bytes())
            });
    }

    let mut out = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--output-last-message" && index + 1 < args.len() {
            out = Some(PathBuf::from(&args[index + 1]));
            index += 2;
            continue;
        }
        index += 1;
    }

    let Some(out) = out else {
        std::process::exit(1);
    };

    let _ = fs::write(out, "fake final message");
}
"#;

    let source_path = bin_dir.join("fake_codex.rs");
    let executable = bin_dir.join("codex.exe");
    fs::write(&source_path, source).expect("ecriture du faux code source");
    let status = Command::new("rustc")
        .current_dir(&bin_dir)
        .args([
            "--edition=2021",
            source_path.to_str().expect("source utf-8"),
            "-o",
            executable.to_str().expect("exe utf-8"),
        ])
        .status()
        .expect("compilation du faux codex");
    assert!(status.success(), "rustc doit compiler le faux codex");
    executable
}

pub fn build_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_app"))
}

pub fn join_path_dirs<I>(dirs: I) -> OsString
where
    I: IntoIterator<Item = PathBuf>,
{
    std::env::join_paths(dirs).expect("construction du PATH de test")
}
