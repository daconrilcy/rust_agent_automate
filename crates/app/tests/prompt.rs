use std::borrow::Cow;
use std::path::Path;

use app::prompt::{PromptSection, render_prompt, render_structured_prompt};

#[test]
fn render_prompt_preserves_result_label_and_paths() {
    let prompt = render_prompt(
        "Opening line.",
        Path::new("C:\\dev\\rust_agent"),
        Path::new("C:\\dev\\rust_agent\\.review"),
        "final review",
        "Closing line.",
    );

    assert!(prompt.contains("Opening line."));
    assert!(prompt.contains("C:\\dev\\rust_agent"));
    assert!(prompt.contains("final review will be saved"));
    assert!(prompt.contains("C:\\dev\\rust_agent\\.review"));
    assert!(prompt.contains("Closing line."));
}

#[test]
fn render_structured_prompt_keeps_ordered_sections() {
    let prompt = render_structured_prompt(
        "Opening line.",
        Path::new("C:\\dev\\rust_agent"),
        Path::new("C:\\dev\\rust_agent\\.review"),
        "final review",
        &[
            PromptSection {
                heading: Cow::Borrowed(""),
                body: Cow::Borrowed("First body."),
            },
            PromptSection {
                heading: Cow::Borrowed("Checklist"),
                body: Cow::Borrowed("Second body."),
            },
        ],
    );

    assert!(prompt.contains("Opening line."));
    assert!(prompt.contains("final review will be saved"));
    assert!(prompt.contains("First body.\nChecklist\nSecond body."));
}
