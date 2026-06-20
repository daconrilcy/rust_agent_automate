use std::path::PathBuf;

use crate::service_paths::{self, PathRequirement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewSubject {
    Plan,
    Audit,
    Implementation,
}

impl ReviewSubject {
    pub fn as_cli_value(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Audit => "audit",
            Self::Implementation => "implementation",
        }
    }

    pub(crate) fn artifact_name(self) -> &'static str {
        match self {
            Self::Plan => "implementation plan",
            Self::Audit => "audit report",
            Self::Implementation => "implementation",
        }
    }

    pub(crate) fn path_requirement(self) -> PathRequirement {
        match self {
            Self::Plan | Self::Audit => PathRequirement::File,
            Self::Implementation => PathRequirement::FileOrDirectory,
        }
    }
}

impl std::fmt::Display for ReviewSubject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_cli_value())
    }
}

impl std::str::FromStr for ReviewSubject {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "plan" => Ok(Self::Plan),
            "audit" => Ok(Self::Audit),
            "implementation" => Ok(Self::Implementation),
            _ => Err(format!(
                "type de review invalide: {value}. Valeurs attendues: plan, audit, implementation"
            )),
        }
    }
}

pub fn resolve_artifact_path(
    subject: ReviewSubject,
    path: PathBuf,
    context: &service_paths::ExecutionContext,
) -> Result<PathBuf, String> {
    let path = service_paths::resolve_existing_path(
        path,
        &format!("de review {subject}"),
        subject.path_requirement(),
        context,
    )?;

    match subject {
        ReviewSubject::Plan | ReviewSubject::Audit if !path.is_file() => Err(format!(
            "le chemin de review {subject} doit etre un fichier: {}",
            path.display()
        )),
        ReviewSubject::Implementation if !path.is_file() && !path.is_dir() => Err(format!(
            "le chemin de review implementation doit etre un fichier ou un dossier: {}",
            path.display()
        )),
        _ => Ok(path),
    }
}
