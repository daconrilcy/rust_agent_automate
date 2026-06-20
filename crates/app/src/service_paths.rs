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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathResolutionError {
    Access {
        label: String,
        path: PathBuf,
        error: String,
    },
    WrongKind {
        label: String,
        path: PathBuf,
        expected: &'static str,
    },
    CanonicalizePath {
        label: String,
        path: PathBuf,
        error: String,
    },
    CanonicalizeWorkspaceRoot {
        path: PathBuf,
        error: String,
    },
    ReadCurrentDir {
        error: String,
    },
}

impl std::fmt::Display for PathResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Access { label, path, error } => write!(
                f,
                "impossible d'acceder au chemin {label} {}: {error}",
                path.display()
            ),
            Self::WrongKind {
                label,
                path,
                expected,
            } => write!(
                f,
                "le chemin {label} doit etre {expected}: {}",
                path.display()
            ),
            Self::CanonicalizePath { label, path, error } => write!(
                f,
                "impossible de resoudre le chemin {label} {}: {error}",
                path.display()
            ),
            Self::CanonicalizeWorkspaceRoot { path, error } => write!(
                f,
                "impossible de resoudre le workspace {}: {error}",
                path.display()
            ),
            Self::ReadCurrentDir { error } => {
                write!(f, "impossible de lire le repertoire courant: {error}")
            }
        }
    }
}

impl std::error::Error for PathResolutionError {}

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
) -> Result<PathBuf, PathResolutionError> {
    let path = if path.is_absolute() {
        path
    } else {
        context.workspace_root().join(path)
    };

    let metadata = fs::metadata(&path).map_err(|error| PathResolutionError::Access {
        label: label.to_string(),
        path: path.clone(),
        error: error.to_string(),
    })?;

    match requirement {
        PathRequirement::File if !metadata.is_file() => {
            return Err(PathResolutionError::WrongKind {
                label: label.to_string(),
                path,
                expected: "un fichier",
            });
        }
        PathRequirement::Directory if !metadata.is_dir() => {
            return Err(PathResolutionError::WrongKind {
                label: label.to_string(),
                path,
                expected: "un dossier",
            });
        }
        PathRequirement::FileOrDirectory if !metadata.is_file() && !metadata.is_dir() => {
            return Err(PathResolutionError::WrongKind {
                label: label.to_string(),
                path,
                expected: "un fichier ou un dossier",
            });
        }
        _ => {}
    }

    fs::canonicalize(&path).map_err(|error| PathResolutionError::CanonicalizePath {
        label: label.to_string(),
        path,
        error: error.to_string(),
    })
}

pub fn current_workspace_root() -> Result<PathBuf, PathResolutionError> {
    if std::env::var_os(WORKSPACE_ROOT_OVERRIDE_ENV).is_some() {
        match std::env::var_os(WORKSPACE_ROOT_ENV) {
            Some(path) => canonicalize_workspace_root(PathBuf::from(path)),
            None => read_current_dir(),
        }
    } else {
        read_current_dir()
    }
}

pub fn current_execution_context() -> Result<ExecutionContext, PathResolutionError> {
    current_workspace_root().map(ExecutionContext::from_workspace_root)
}

pub fn resolve_output_dir(
    output_dir: Option<PathBuf>,
    context: &ExecutionContext,
    default_dir_name: &str,
) -> PathBuf {
    match output_dir {
        Some(path) if path.is_absolute() => path,
        Some(path) => context.output_root().join(path),
        None => context.output_root().join(default_dir_name),
    }
}

fn canonicalize_workspace_root(path: PathBuf) -> Result<PathBuf, PathResolutionError> {
    fs::canonicalize(&path).map_err(|error| PathResolutionError::CanonicalizeWorkspaceRoot {
        path,
        error: error.to_string(),
    })
}

fn read_current_dir() -> Result<PathBuf, PathResolutionError> {
    std::env::current_dir().map_err(|error| PathResolutionError::ReadCurrentDir {
        error: error.to_string(),
    })
}
