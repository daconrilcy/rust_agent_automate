use std::path::PathBuf;
use std::time::Duration;

use app::service_command::context::{prepare_prompted_service, prepare_service_command};
use app::service_paths::ExecutionContext;
use app::{ServiceCommandDescriptor, ServiceCommandOptions};

fn descriptor() -> ServiceCommandDescriptor<'static> {
    ServiceCommandDescriptor {
        default_output_dir: ".audit",
        command_name: "audit",
        artifact_stem: "audit",
        saved_label: "audit",
        final_label: "audit",
        missing_message_label: "audit",
        clean_detector: None,
    }
}

#[test]
fn prepare_service_command_uses_workspace_root_as_codex_working_directory() {
    let options = ServiceCommandOptions::new(Duration::from_secs(900));
    let context = ExecutionContext::from_workspace_root(PathBuf::from("C:\\repo"))
        .with_output_root(PathBuf::from("C:\\repo\\out"));

    let command = prepare_service_command(&options, &context, descriptor(), "Prompt".to_string());

    assert_eq!(command.request.working_dir, Some(PathBuf::from("C:\\repo")));
}

#[test]
fn prepare_prompted_service_reuses_parse_context_for_prompt_and_request() {
    let options = ServiceCommandOptions::new(Duration::from_secs(900));
    let context = ExecutionContext::from_workspace_root(PathBuf::from("C:\\repo"))
        .with_output_root(PathBuf::from("C:\\repo\\out"));

    let prepared = prepare_prompted_service(&options, descriptor(), context, |parse_context| {
        Ok::<_, ()>((
            parse_context.output_dir.clone(),
            format!("Prompt {}", parse_context.output_dir.display()),
        ))
    })
    .expect("preparation du service");

    assert_eq!(prepared.workspace_root, PathBuf::from("C:\\repo"));
    assert_eq!(prepared.resolved, PathBuf::from("C:\\repo\\out\\.audit"));
    assert_eq!(
        prepared.service.request.prompt.as_deref(),
        Some("Prompt C:\\repo\\out\\.audit")
    );
}
