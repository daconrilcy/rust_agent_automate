use std::borrow::Cow;
use std::path::Path;

pub struct PromptSection<'a> {
    pub heading: Cow<'a, str>,
    pub body: Cow<'a, str>,
}

#[allow(dead_code)]
pub fn render_prompt(
    opening: &str,
    workspace_root: &Path,
    output_dir: &Path,
    result_label: &str,
    closing: &str,
) -> String {
    format!(
        concat!(
            "{}\n",
            "The current local workspace running this command is \"{}\" and the {} will be saved by the wrapper under \"{}\".\n",
            "{}"
        ),
        opening,
        workspace_root.display(),
        result_label,
        output_dir.display(),
        closing
    )
}

pub fn render_structured_prompt(
    opening: &str,
    workspace_root: &Path,
    output_dir: &Path,
    result_label: &str,
    sections: &[PromptSection<'_>],
) -> String {
    let mut prompt = format!(
        "{opening}\nThe current local workspace running this command is \"{}\" and the {} will be saved by the wrapper under \"{}\".\n",
        workspace_root.display(),
        result_label,
        output_dir.display()
    );

    for section in sections {
        if !section.heading.is_empty() {
            prompt.push_str(&section.heading);
            prompt.push('\n');
        }
        prompt.push_str(&section.body);
        prompt.push('\n');
    }

    while prompt.ends_with('\n') {
        prompt.pop();
    }

    prompt
}
