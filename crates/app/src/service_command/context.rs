use std::path::PathBuf;

use crate::service_paths::{self, ExecutionContext};

use super::{
    ParsedRequiredPath, PreparedParseContext, PreparedServiceCommand, ServiceCommandDescriptor,
    ServiceCommandOptions,
};

pub(crate) struct PreparedPromptedService<T> {
    pub workspace_root: PathBuf,
    pub service: PreparedServiceCommand,
    pub resolved: T,
}

pub(crate) fn resolve_context()
-> Result<ExecutionContext, crate::service_paths::PathResolutionError> {
    service_paths::current_execution_context()
}

pub(crate) fn resolve_output_dir(
    options: &ServiceCommandOptions,
    context: &ExecutionContext,
    default_dir_name: &str,
) -> PathBuf {
    service_paths::resolve_output_dir(options.output_dir.clone(), context, default_dir_name)
}

pub(crate) fn prepare_service_command(
    options: &ServiceCommandOptions,
    context: &ExecutionContext,
    descriptor: ServiceCommandDescriptor<'_>,
    prompt: String,
) -> PreparedServiceCommand {
    PreparedServiceCommand {
        request: options
            .build_request(prompt)
            .with_working_dir(context.workspace_root().to_path_buf()),
        workspace_root: context.workspace_root().to_path_buf(),
        output_dir: resolve_output_dir(options, context, descriptor.default_output_dir),
        timeout: options.timeout,
    }
}

pub(crate) fn prepare_parse_context_for_context(
    options: &ServiceCommandOptions,
    descriptor: ServiceCommandDescriptor<'_>,
    context: ExecutionContext,
) -> PreparedParseContext {
    let workspace_root = context.workspace_root().to_path_buf();
    let output_dir = resolve_output_dir(options, &context, descriptor.default_output_dir);

    PreparedParseContext {
        context,
        workspace_root,
        output_dir,
    }
}

pub(crate) fn prepare_service_from_prompt(
    options: &ServiceCommandOptions,
    parse_context: &PreparedParseContext,
    descriptor: ServiceCommandDescriptor<'_>,
    prompt: String,
) -> PreparedServiceCommand {
    prepare_service_command(options, &parse_context.context, descriptor, prompt)
}

pub(crate) fn prepare_prompted_service<T, E, F>(
    options: &ServiceCommandOptions,
    descriptor: ServiceCommandDescriptor<'_>,
    context: ExecutionContext,
    build: F,
) -> Result<PreparedPromptedService<T>, E>
where
    F: FnOnce(&PreparedParseContext) -> Result<(T, String), E>,
{
    let parse_context = prepare_parse_context_for_context(options, descriptor, context);
    let workspace_root = parse_context.workspace_root.clone();
    let (resolved, prompt) = build(&parse_context)?;
    let service = prepare_service_from_prompt(options, &parse_context, descriptor, prompt);

    Ok(PreparedPromptedService {
        workspace_root,
        service,
        resolved,
    })
}

pub(crate) fn prepare_required_path_service<T, FResolve, FPrompt>(
    options: &ServiceCommandOptions,
    descriptor: ServiceCommandDescriptor<'_>,
    context: ExecutionContext,
    parsed: ParsedRequiredPath,
    resolve_required: FResolve,
    build_prompt: FPrompt,
) -> Result<PreparedPromptedService<T>, crate::cli::ParseOutcome>
where
    FResolve: FnOnce(
        PathBuf,
        Option<PathBuf>,
        &ExecutionContext,
    ) -> Result<T, service_paths::PathResolutionError>,
    FPrompt: FnOnce(&PreparedParseContext, &T) -> String,
{
    prepare_prompted_service(options, descriptor, context, |parse_context| {
        let resolved = resolve_required(
            parsed.required_path,
            parsed.optional_path,
            &parse_context.context,
        )
        .map_err(|error| crate::cli::ParseOutcome::Error(error.to_string()))?;
        let prompt = build_prompt(parse_context, &resolved);
        Ok((resolved, prompt))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn prepare_service_command_uses_workspace_root_as_codex_working_directory() {
        let options = ServiceCommandOptions::new(Duration::from_secs(900));
        let context = ExecutionContext::from_workspace_root(PathBuf::from("C:\\repo"))
            .with_output_root(PathBuf::from("C:\\repo\\out"));
        let descriptor = ServiceCommandDescriptor {
            default_output_dir: ".audit",
            command_name: "audit",
            artifact_stem: "audit",
            saved_label: "audit",
            final_label: "audit",
            missing_message_label: "audit",
            clean_detector: None,
        };

        let command = prepare_service_command(&options, &context, descriptor, "Prompt".to_string());

        assert_eq!(command.request.working_dir, Some(PathBuf::from("C:\\repo")));
    }

    #[test]
    fn prepare_prompted_service_reuses_parse_context_for_prompt_and_request() {
        let options = ServiceCommandOptions::new(Duration::from_secs(900));
        let context = ExecutionContext::from_workspace_root(PathBuf::from("C:\\repo"))
            .with_output_root(PathBuf::from("C:\\repo\\out"));
        let descriptor = ServiceCommandDescriptor {
            default_output_dir: ".audit",
            command_name: "audit",
            artifact_stem: "audit",
            saved_label: "audit",
            final_label: "audit",
            missing_message_label: "audit",
            clean_detector: None,
        };

        let prepared = prepare_prompted_service(&options, descriptor, context, |parse_context| {
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
}
