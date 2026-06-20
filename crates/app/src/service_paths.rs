use std::fs;
use std::path::{Path, PathBuf};

pub const WORKSPACE_ROOT_ENV: &str = "RUST_AGENT_WORKSPACE_ROOT";
pub const WORKSPACE_ROOT_OVERRIDE_ENV: &str = "RUST_AGENT_USE_WORKSPACE_ROOT";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRequirement {
    File,
    Directory,
    FileOrDirectory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionContext {
    workspace_root: PathBuf,
    output_root: PathBuf,
}

impl ExecutionContext {
    pub fn from_workspace_root(workspace_root: PathBuf) -> Self {
        Self {
            output_root: workspace_root.clone(),
            workspace_root,
        }
    }

    pub fn with_output_root(mut self, output_root: PathBuf) -> Self {
        self.output_root = output_root;
        self
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn output_root(&self) -> &Path {
        &self.output_root
    }
}

pub fn resolve_existing_path(
    path: PathBuf,
    label: &str,
    requirement: PathRequirement,
    context: &ExecutionContext,
) -> Result<PathBuf, String> {
    let path = if path.is_absolute() {
        path
    } else {
        context.workspace_root().join(path)
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
    if std::env::var_os(WORKSPACE_ROOT_OVERRIDE_ENV).is_some() {
        match std::env::var_os(WORKSPACE_ROOT_ENV) {
            Some(path) => canonicalize_workspace_root(PathBuf::from(path)),
            None => read_current_dir(),
        }
    } else {
        read_current_dir()
    }
}

pub fn current_execution_context() -> Result<ExecutionContext, String> {
    current_workspace_root().map(ExecutionContext::from_workspace_root)
}

pub fn resolve_output_dir(
    output_dir: Option<PathBuf>,
    context: &ExecutionContext,
    default_dir_name: &str,
) -> PathBuf {
    output_dir.unwrap_or_else(|| context.output_root().join(default_dir_name))
}

fn canonicalize_workspace_root(path: PathBuf) -> Result<PathBuf, String> {
    fs::canonicalize(&path).map_err(|error| {
        format!(
            "impossible de resoudre le workspace {}: {error}",
            path.display()
        )
    })
}

fn read_current_dir() -> Result<PathBuf, String> {
    std::env::current_dir()
        .map_err(|error| format!("impossible de lire le repertoire courant: {error}"))
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
        let context = ExecutionContext::from_workspace_root(
            std::env::current_dir().expect("le dossier courant doit etre lisible"),
        );
        let path = resolve_existing_path(
            PathBuf::from("Cargo.toml"),
            "fichier",
            PathRequirement::File,
            &context,
        )
        .expect("le fichier doit exister");

        assert!(path.is_absolute());
        assert!(path.ends_with("Cargo.toml"));
    }

    #[cfg(windows)]
    #[test]
    fn resolves_verbatim_directory_path() {
        let current_dir = std::env::current_dir().expect("le dossier courant doit etre lisible");
        let context = ExecutionContext::from_workspace_root(current_dir.clone());
        let path = resolve_existing_path(
            verbatim_path(&current_dir),
            "dossier",
            PathRequirement::Directory,
            &context,
        )
        .expect("le dossier verbatim doit etre accepte");

        assert!(path.is_dir());
    }

    #[cfg(windows)]
    #[test]
    fn resolves_verbatim_file_path() {
        let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let context = ExecutionContext::from_workspace_root(
            std::env::current_dir().expect("le dossier courant doit etre lisible"),
        );
        let path = resolve_existing_path(
            verbatim_path(&manifest_path),
            "fichier",
            PathRequirement::File,
            &context,
        )
        .expect("le fichier verbatim doit etre accepte");

        assert!(path.is_file());
        assert!(path.ends_with("Cargo.toml"));
    }

    #[test]
    fn resolve_output_dir_uses_default_dir_name() {
        let context = ExecutionContext::from_workspace_root(PathBuf::from("C:\\dev\\rust_agent"));
        let output_dir = resolve_output_dir(None, &context, ".audit");

        assert_eq!(output_dir, context.workspace_root().join(".audit"));
    }

    #[test]
    fn resolve_output_dir_keeps_explicit_value() {
        let context = ExecutionContext::from_workspace_root(PathBuf::from("C:\\dev\\rust_agent"));
        let explicit = PathBuf::from("C:\\tmp\\custom");
        let output_dir = resolve_output_dir(Some(explicit.clone()), &context, ".audit");

        assert_eq!(output_dir, explicit);
    }

    #[test]
    fn execution_context_can_track_target_and_output_roots() {
        let context = ExecutionContext::from_workspace_root(PathBuf::from("C:\\dev\\rust_agent"))
            .with_output_root(PathBuf::from("C:\\dev\\rust_agent\\crates\\app"));

        assert_eq!(
            context.output_root(),
            Path::new("C:\\dev\\rust_agent\\crates\\app")
        );
    }

    #[test]
    fn resolves_relative_file_path_from_explicit_workspace_root() {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let context = ExecutionContext::from_workspace_root(workspace_root.clone());

        let path = resolve_existing_path(
            PathBuf::from("Cargo.toml"),
            "fichier",
            PathRequirement::File,
            &context,
        )
        .expect("le fichier relatif doit etre resolu depuis le workspace explicite");

        assert_eq!(
            path,
            fs::canonicalize(workspace_root.join("Cargo.toml")).expect("canonical path")
        );
    }
}
