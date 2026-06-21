use std::fs;
use std::path::Path;

use app::automate::artifact_resolution::normalize_artifact_path;

#[test]
fn normalizes_relative_artifact_path_from_workspace_root() {
    let workspace = crate::support::temp_dir("workflow_runner_relative_artifact");
    let artifact = workspace.join(".audit").join("audit.md");
    fs::create_dir_all(artifact.parent().expect("parent")).expect("creation du dossier");
    fs::write(&artifact, "audit").expect("ecriture de l'artefact");

    let normalized =
        normalize_artifact_path(Path::new(".audit\\audit.md"), &workspace).expect("normalisation");

    assert_eq!(
        normalized,
        fs::canonicalize(&artifact).expect("chemin canonical")
    );

    let _ = fs::remove_dir_all(workspace);
}
