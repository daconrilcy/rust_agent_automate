use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) fn normalize_artifact_path(path: &Path, workspace_root: &Path) -> io::Result<PathBuf> {
    let workspace_root = fs::canonicalize(workspace_root).map_err(|error| {
        io::Error::other(format!(
            "impossible de normaliser le workspace automate {}: {error}",
            workspace_root.display()
        ))
    })?;
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };

    let metadata = fs::metadata(&resolved).map_err(|error| {
        io::Error::other(format!(
            "artefact introuvable ou inaccessible {}: {error}",
            resolved.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(io::Error::other(format!(
            "l'artefact structure doit etre un fichier: {}",
            resolved.display()
        )));
    }

    let normalized = fs::canonicalize(&resolved).map_err(|error| {
        io::Error::other(format!(
            "impossible de normaliser l'artefact {}: {error}",
            resolved.display()
        ))
    })?;
    if !normalized.starts_with(&workspace_root) {
        return Err(io::Error::other(format!(
            "l'artefact structure doit rester dans le workspace automate {}: {}",
            workspace_root.display(),
            normalized.display()
        )));
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_relative_artifact_path_from_workspace_root() {
        let workspace = std::env::temp_dir().join("rust_agent_workflow_runner_relative_artifact");
        let artifact = workspace.join(".audit").join("audit.md");
        fs::create_dir_all(artifact.parent().expect("parent")).expect("creation du dossier");
        fs::write(&artifact, "audit").expect("ecriture de l'artefact");

        let normalized = normalize_artifact_path(Path::new(".audit\\audit.md"), &workspace)
            .expect("normalisation");

        assert_eq!(
            normalized,
            fs::canonicalize(&artifact).expect("chemin canonical")
        );

        let _ = fs::remove_dir_all(workspace);
    }
}
