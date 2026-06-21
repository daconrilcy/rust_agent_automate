mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use app::{ReviewSubject, parse_args, registered_commands};

struct CurrentDirGuard {
    _lock: MutexGuard<'static, ()>,
    previous: PathBuf,
}

impl CurrentDirGuard {
    fn change_to(path: &Path) -> Self {
        let lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::current_dir().expect("cwd lisible");
        std::env::set_current_dir(path).expect("placement dans le workspace");
        Self {
            _lock: lock,
            previous,
        }
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.previous).expect("restauration du cwd");
    }
}

fn cwd_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn verbatim_path(path: &Path) -> PathBuf {
    let text = path.display().to_string();
    if text.starts_with(r"\\?\") {
        PathBuf::from(text)
    } else {
        PathBuf::from(format!(r"\\?\{text}"))
    }
}

#[test]
fn registered_commands_expose_usage_examples_and_aliases() {
    for spec in registered_commands() {
        assert!(!spec.usage.is_empty(), "usage missing for {}", spec.name);
        assert!(
            !spec.examples.is_empty(),
            "examples missing for {}",
            spec.name
        );
    }

    let names = registered_commands()
        .into_iter()
        .flat_map(|spec| std::iter::once(spec.name).chain(spec.aliases.iter().copied()))
        .collect::<Vec<_>>();

    assert!(names.contains(&"audit"));
    assert!(names.contains(&"impl-audit"));
    assert!(names.contains(&"loop"));
    assert!(names.contains(&"refactor-auto"));
    assert!(!names.contains(&"unknown"));
}

#[test]
#[cfg(windows)]
fn audit_accepts_windows_verbatim_target_path() {
    let workspace = support::temp_dir("cli_verbatim_target");
    fs::create_dir_all(&workspace).expect("creation du workspace");

    let _cwd = CurrentDirGuard::change_to(&workspace);
    let parsed = parse_args(&[
        "audit".to_string(),
        "--target".to_string(),
        verbatim_path(&workspace)
            .to_str()
            .expect("verbatim utf-8")
            .to_string(),
    ]);

    let command = parsed.expect("parse audit");
    let audit = command.as_audit().expect("commande audit attendue");

    assert_eq!(
        audit.target_dir,
        fs::canonicalize(&workspace).expect("chemin canonique")
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn review_aliases_and_subjects_still_parse_through_public_cli() {
    let workspace = support::temp_dir("cli_review_parse");
    let artifact = workspace.join(".plan").join("plan.md");
    fs::create_dir_all(&workspace).expect("creation du workspace");
    fs::create_dir_all(artifact.parent().expect("parent du plan")).expect("creation du dossier");
    fs::write(&artifact, "# plan").expect("ecriture du plan");

    let _cwd = CurrentDirGuard::change_to(&workspace);
    let parsed = parse_args(&[
        "review".to_string(),
        "plan".to_string(),
        artifact.display().to_string(),
    ]);

    let command = parsed.expect("parse review");
    let review = command.as_review().expect("commande review attendue");

    assert_eq!(review.subject, ReviewSubject::Plan);

    let _ = fs::remove_dir_all(workspace);
}
