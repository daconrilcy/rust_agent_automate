use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRequirement {
    File,
    Directory,
    FileOrDirectory,
}

pub fn resolve_existing_path(
    path: PathBuf,
    label: &str,
    requirement: PathRequirement,
) -> Result<PathBuf, String> {
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|error| format!("impossible de lire le repertoire courant: {error}"))?
            .join(path)
    };

    let metadata = fs::metadata(&path).map_err(|error| {
        format!(
            "impossible d'acceder au chemin {label} {}: {error}",
            path.display()
        )
    })?;

    match requirement {
        PathRequirement::File if !metadata.is_file() => {
            return Err(format!(
                "le chemin {label} doit etre un fichier: {}",
                path.display()
            ));
        }
        PathRequirement::Directory if !metadata.is_dir() => {
            return Err(format!(
                "le chemin {label} doit etre un dossier: {}",
                path.display()
            ));
        }
        PathRequirement::FileOrDirectory if !metadata.is_file() && !metadata.is_dir() => {
            return Err(format!(
                "le chemin {label} doit etre un fichier ou un dossier: {}",
                path.display()
            ));
        }
        _ => {}
    }

    fs::canonicalize(&path).map_err(|error| {
        format!(
            "impossible de resoudre le chemin {label} {}: {error}",
            path.display()
        )
    })
}

pub fn current_workspace_root() -> Result<PathBuf, String> {
    std::env::current_dir()
        .map_err(|error| format!("impossible de lire le repertoire courant: {error}"))
}

pub fn resolve_output_dir(
    output_dir: Option<PathBuf>,
    workspace_root: &Path,
    default_dir_name: &str,
) -> PathBuf {
    output_dir.unwrap_or_else(|| workspace_root.join(default_dir_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn verbatim_path(path: &Path) -> PathBuf {
        let text = path.display().to_string();
        if text.starts_with(r"\\?\") {
            PathBuf::from(text)
        } else {
            PathBuf::from(format!(r"\\?\{text}"))
        }
    }

    #[test]
    fn resolves_relative_file_path() {
        let path = resolve_existing_path(
            PathBuf::from("Cargo.toml"),
            "fichier",
            PathRequirement::File,
        )
        .expect("le fichier doit exister");

        assert!(path.is_absolute());
        assert!(path.ends_with("Cargo.toml"));
    }

    #[cfg(windows)]
    #[test]
    fn resolves_verbatim_directory_path() {
        let current_dir = std::env::current_dir().expect("le dossier courant doit etre lisible");
        let path = resolve_existing_path(
            verbatim_path(&current_dir),
            "dossier",
            PathRequirement::Directory,
        )
        .expect("le dossier verbatim doit etre accepte");

        assert!(path.is_dir());
    }

    #[cfg(windows)]
    #[test]
    fn resolves_verbatim_file_path() {
        let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let path = resolve_existing_path(
            verbatim_path(&manifest_path),
            "fichier",
            PathRequirement::File,
        )
        .expect("le fichier verbatim doit etre accepte");

        assert!(path.is_file());
        assert!(path.ends_with("Cargo.toml"));
    }

    #[test]
    fn resolve_output_dir_uses_default_dir_name() {
        let workspace_root = Path::new("C:\\dev\\rust_agent");
        let output_dir = resolve_output_dir(None, workspace_root, ".audit");

        assert_eq!(output_dir, workspace_root.join(".audit"));
    }

    #[test]
    fn resolve_output_dir_keeps_explicit_value() {
        let workspace_root = Path::new("C:\\dev\\rust_agent");
        let explicit = PathBuf::from("C:\\tmp\\custom");
        let output_dir = resolve_output_dir(Some(explicit.clone()), workspace_root, ".audit");

        assert_eq!(output_dir, explicit);
    }
}
