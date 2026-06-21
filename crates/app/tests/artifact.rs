mod support;

use std::fs;

use app::artifact::save_timestamped_markdown;

#[test]
fn save_timestamped_markdown_does_not_overwrite_existing_file() {
    let output_dir = support::temp_dir("artifact_test");

    let first = save_timestamped_markdown(&output_dir, "plan", "first").expect("premiere ecriture");
    let second =
        save_timestamped_markdown(&output_dir, "plan", "second").expect("deuxieme ecriture");

    assert_ne!(first, second);
    assert_eq!(
        fs::read_to_string(first).expect("lecture du premier artefact"),
        "first"
    );
    assert_eq!(
        fs::read_to_string(second).expect("lecture du deuxieme artefact"),
        "second"
    );

    let _ = fs::remove_dir_all(output_dir);
}
