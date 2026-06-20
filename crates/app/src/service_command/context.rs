use std::path::PathBuf;

use crate::service_paths::{self, ExecutionContext};

use super::{
    PreparedParseContext, PreparedServiceCommand, ServiceCommandDescriptor, ServiceCommandOptions,
};

pub fn resolve_context() -> Result<ExecutionContext, String> {
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

pub fn prepare_parse_context(
    options: &ServiceCommandOptions,
    descriptor: ServiceCommandDescriptor<'_>,
) -> Result<PreparedParseContext, String> {
    let context = resolve_context()?;
    Ok(prepare_parse_context_for_context(
        options, descriptor, context,
    ))
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
