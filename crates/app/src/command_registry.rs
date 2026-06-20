use crate::cli::{CliCommand, ParseOutcome};
use crate::service_command::{SERVICE_COMMAND_SPECS, ServiceCommandDispatch, ServiceCommandKind};
use crate::service_paths::ExecutionContext;

type CommandParser = fn(&[String], Option<&ExecutionContext>) -> Result<CliCommand, ParseOutcome>;

#[derive(Debug, Clone, Copy)]
struct StaticCommandSpec {
    name: &'static str,
    aliases: &'static [&'static str],
    accepts_codex_options: bool,
    usage: &'static [&'static str],
    examples: &'static [&'static str],
    parser: CommandParser,
}

#[derive(Debug, Clone, Copy)]
pub struct RegisteredCommand<'a> {
    pub name: &'a str,
    pub aliases: &'a [&'a str],
    pub accepts_codex_options: bool,
    pub usage: &'a [&'a str],
    pub examples: &'a [&'a str],
}

pub const DIRECT_RUN_USAGE: &[&str] = &[
    r#"cargo run -p app -- [--model <nom>] [--reasoning <low|medium|high>] [--mode <interactive|exec>] [--verbose] [prompt]"#,
];

pub const DIRECT_RUN_EXAMPLES: &[&str] = &[
    r#"cargo run -q -p app -- --model gpt-5.4 --reasoning low"#,
    r#"cargo run -q -p app -- --mode exec --model gpt-5.4 --reasoning low "Explique ce depot""#,
    r#"cargo run -q -p app -- --mode exec --verbose --model gpt-5.4 --reasoning low "Explique ce depot""#,
];

const STATIC_COMMANDS: &[StaticCommandSpec] = &[
    StaticCommandSpec {
        name: "automate",
        aliases: &[],
        accepts_codex_options: false,
        usage: &["cargo run -p app -- automate <workflow.json> [prompt]"],
        examples: &["cargo run -q -p app -- automate .\\workflow.json \"Durcir ce module\""],
        parser: |args, _context| {
            crate::automate::parse_automate_args(args).map(CliCommand::Automate)
        },
    },
    StaticCommandSpec {
        name: "refactor-automate",
        aliases: &["refactor-auto"],
        accepts_codex_options: false,
        usage: &[
            "cargo run -p app -- refactor-automate [--target <dossier>] [--workflow <workflow.json>] [prompt]",
        ],
        examples: &[
            "cargo run -q -p app -- refactor-automate --target crates\\app \"Refactoring SOLID/KISS/DRY\"",
        ],
        parser: |args, _context| {
            crate::automate::parse_refactor_automate_args(args).map(CliCommand::RefactorAutomate)
        },
    },
];

pub fn registered_commands() -> Vec<RegisteredCommand<'static>> {
    let service_commands = SERVICE_COMMAND_SPECS.iter().map(|spec| RegisteredCommand {
        name: spec.name,
        aliases: spec.aliases,
        accepts_codex_options: spec.accepts_codex_options,
        usage: spec.usage,
        examples: spec.examples,
    });
    let static_commands = STATIC_COMMANDS.iter().map(|spec| RegisteredCommand {
        name: spec.name,
        aliases: spec.aliases,
        accepts_codex_options: spec.accepts_codex_options,
        usage: spec.usage,
        examples: spec.examples,
    });

    service_commands.chain(static_commands).collect()
}

pub fn is_known_subcommand(value: &str) -> bool {
    SERVICE_COMMAND_SPECS
        .iter()
        .any(|spec| spec.name == value || spec.aliases.contains(&value))
        || STATIC_COMMANDS
            .iter()
            .any(|spec| spec.name == value || spec.aliases.contains(&value))
}

pub fn canonical_name(value: Option<&str>) -> Option<&'static str> {
    let value = value?;

    SERVICE_COMMAND_SPECS
        .iter()
        .find(|spec| spec.name == value || spec.aliases.contains(&value))
        .map(|spec| spec.name)
        .or_else(|| {
            STATIC_COMMANDS
                .iter()
                .find(|spec| spec.name == value || spec.aliases.contains(&value))
                .map(|spec| spec.name)
        })
}

pub fn accepts_codex_options(value: &str) -> bool {
    SERVICE_COMMAND_SPECS
        .iter()
        .find(|spec| spec.name == value || spec.aliases.contains(&value))
        .map(|spec| spec.accepts_codex_options)
        .or_else(|| {
            STATIC_COMMANDS
                .iter()
                .find(|spec| spec.name == value || spec.aliases.contains(&value))
                .map(|spec| spec.accepts_codex_options)
        })
        .unwrap_or(false)
}

pub fn is_direct_run(value: Option<&str>) -> bool {
    value.is_none_or(|value| value.starts_with("--") || !is_known_subcommand(value))
}

pub fn usage_lines() -> impl Iterator<Item = &'static str> {
    SERVICE_COMMAND_SPECS
        .iter()
        .flat_map(|spec| spec.usage.iter().copied())
        .chain(
            STATIC_COMMANDS
                .iter()
                .flat_map(|spec| spec.usage.iter().copied()),
        )
}

pub fn example_lines() -> impl Iterator<Item = &'static str> {
    SERVICE_COMMAND_SPECS
        .iter()
        .flat_map(|spec| spec.examples.iter().copied())
        .chain(
            STATIC_COMMANDS
                .iter()
                .flat_map(|spec| spec.examples.iter().copied()),
        )
}

pub fn parse_registered_subcommand(args: &[String]) -> Option<Result<CliCommand, ParseOutcome>> {
    parse_registered_subcommand_for_context(args, None)
}

pub fn parse_registered_subcommand_for_context(
    args: &[String],
    context: Option<&ExecutionContext>,
) -> Option<Result<CliCommand, ParseOutcome>> {
    // The registry owns name/alias discovery and parser routing only.
    // Command modules keep their own parse rules and prompt construction.
    let command = canonical_name(args.first().map(String::as_str))?;

    if ServiceCommandKind::from_name(command).is_some() {
        return Some(parse_service_command(args, context));
    }

    STATIC_COMMANDS
        .iter()
        .find(|spec| spec.name == command)
        .map(|spec| spec.parser)
        .map(|parser| parser(&args[1..], context))
}

fn parse_service_command(
    args: &[String],
    context: Option<&ExecutionContext>,
) -> Result<CliCommand, ParseOutcome> {
    let kind = ServiceCommandKind::from_name(args.first().map(String::as_str).unwrap_or_default())
        .expect("known service command");
    let context = match context {
        Some(context) => context.clone(),
        None => crate::service_command::resolve_context().map_err(ParseOutcome::Error)?,
    };
    let dispatch = kind.parse_for_context(&args[1..], &context)?;
    Ok(CliCommand::Service(dispatch))
}

pub fn parse_service_subcommand_for_context(
    args: &[String],
    context: &ExecutionContext,
) -> Option<Result<ServiceCommandDispatch, ParseOutcome>> {
    match parse_registered_subcommand_for_context(args, Some(context))? {
        Ok(CliCommand::Service(command)) => Some(Ok(command)),
        Ok(_) => None,
        Err(error) => Some(Err(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_known_subcommands_and_aliases() {
        assert!(is_known_subcommand("audit"));
        assert!(is_known_subcommand("impl-audit"));
        assert!(is_known_subcommand("loop"));
        assert!(is_known_subcommand("refactor-auto"));
        assert!(!is_known_subcommand("unknown"));
    }

    #[test]
    fn classifies_codex_option_commands() {
        assert!(accepts_codex_options("audit"));
        assert!(accepts_codex_options("impl-audit"));
        assert!(!accepts_codex_options("automate"));
    }

    #[test]
    fn resolves_canonical_names_for_aliases() {
        assert_eq!(canonical_name(Some("loop")), Some("fix-loop"));
        assert_eq!(
            canonical_name(Some("refactor-auto")),
            Some("refactor-automate")
        );
        assert_eq!(canonical_name(Some("unknown")), None);
        assert_eq!(canonical_name(None), None);
    }

    #[test]
    fn registered_commands_expose_parsers() {
        for spec in registered_commands() {
            assert!(!spec.usage.is_empty(), "usage missing for {}", spec.name);
            assert!(
                !spec.examples.is_empty(),
                "examples missing for {}",
                spec.name
            );
        }
    }
}
