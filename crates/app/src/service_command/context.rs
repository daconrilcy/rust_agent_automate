use std::path::PathBuf;

use crate::service_paths::{self, ExecutionContext};

use super::{
    ParsedRequiredPath, PreparedParseContext, PreparedServiceCommand, ServiceCommandDescriptor,
    ServiceCommandOptions,
};

pub struct PreparedPromptedService<T> {
    pub workspace_root: PathBuf,
    pub service: PreparedServiceCommand,
    pub resolved: T,
}

pub fn resolve_context() -> Result<ExecutionContext, crate::service_paths::PathResolutionError> {
    service_paths::current_execution_context()
}

pub fn resolve_output_dir(
    options: &ServiceCommandOptions,
    context: &ExecutionContext,
    default_dir_name: &str,
) -> PathBuf {
    service_paths::resolve_output_dir(options.output_dir.clone(), context, default_dir_name)
}

pub fn prepare_service_command(
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

pub fn prepare_parse_context_for_context(
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

pub fn prepare_service_from_prompt(
    options: &ServiceCommandOptions,
    parse_context: &PreparedParseContext,
    descriptor: ServiceCommandDescriptor<'_>,
    prompt: String,
) -> PreparedServiceCommand {
    prepare_service_command(options, &parse_context.context, descriptor, prompt)
}

pub fn prepare_prompted_service<T, E, F>(
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

pub fn prepare_required_path_service<T, FResolve, FPrompt>(
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
