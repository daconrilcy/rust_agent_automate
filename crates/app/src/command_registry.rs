use crate::cli::{CliCommand, ParseOutcome};
use crate::service_command::ServiceCommandDispatch;
use crate::service_paths::ExecutionContext;

#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub accepts_codex_options: bool,
    pub usage: &'static [&'static str],
    pub examples: &'static [&'static str],
    pub parser: Option<fn(&[String], Option<&ExecutionContext>) -> Result<CliCommand, ParseOutcome>>,
}

pub const DIRECT_RUN_USAGE: &[&str] = &[r#"cargo run -p app -- [--model <nom>] [--reasoning <low|medium|high>] [--mode <interactive|exec>] [--verbose] [prompt]"#];

pub const DIRECT_RUN_EXAMPLES: &[&str] = &[
    r#"cargo run -q -p app -- --model gpt-5.4 --reasoning low"#,
    r#"cargo run -q -p app -- --mode exec --model gpt-5.4 --reasoning low "Explique ce depot""#,
    r#"cargo run -q -p app -- --mode exec --verbose --model gpt-5.4 --reasoning low "Explique ce depot""#,
];

pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "audit",
        aliases: &[],
        accepts_codex_options: true,
        usage: &[
            "cargo run -p app -- audit [--target <chemin>] [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
        ],
        examples: &[
            "cargo run -q -p app -- audit",
            "cargo run -q -p app -- audit --target ..\\mon-projet",
            "cargo run -q -p app -- audit --output-dir .audit",
            "cargo run -q -p app -- audit --timeout-seconds 120",
        ],
        parser: Some(|args, context| {
            let command = match context {
                Some(context) => crate::audit::parse_args_for_context(args, context)?,
                None => crate::audit::parse_args(args)?,
            };
            Ok(CliCommand::Service(ServiceCommandDispatch::Audit(command)))
        }),
    },
    CommandSpec {
        name: "plan",
        aliases: &[],
        accepts_codex_options: true,
        usage: &[
            "cargo run -p app -- plan <chemin-audit> [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
        ],
        examples: &[
            "cargo run -q -p app -- plan .audit\\audit-1781887189.md",
            "cargo run -q -p app -- plan --audit .audit\\audit-1781887189.md --output-dir .plan",
        ],
        parser: Some(|args, context| {
            let command = match context {
                Some(context) => crate::plan::parse_args_for_context(args, context)?,
                None => crate::plan::parse_args(args)?,
            };
            Ok(CliCommand::Service(ServiceCommandDispatch::Plan(command)))
        }),
    },
    CommandSpec {
        name: "implementation-audit",
        aliases: &["impl-audit"],
        accepts_codex_options: true,
        usage: &[
            "cargo run -p app -- implementation-audit <chemin-plan> [--implementation <chemin>] [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
            "cargo run -p app -- impl-audit <chemin-plan> [--implementation <chemin>] [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
        ],
        examples: &[
            "cargo run -q -p app -- implementation-audit .plan\\plan-1781894465.md",
            "cargo run -q -p app -- implementation-audit --plan .plan\\plan-1781894465.md --implementation crates\\app",
        ],
        parser: Some(|args, context| {
            let command = match context {
                Some(context) => crate::implementation_audit::parse_args_for_context(args, context)?,
                None => crate::implementation_audit::parse_args(args)?,
            };
            Ok(CliCommand::Service(
                ServiceCommandDispatch::ImplementationAudit(command),
            ))
        }),
    },
    CommandSpec {
        name: "review",
        aliases: &[],
        accepts_codex_options: true,
        usage: &[
            "cargo run -p app -- review <plan|audit|implementation> <chemin> [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
        ],
        examples: &[
            "cargo run -q -p app -- review plan .plan\\plan-1781894465.md",
            "cargo run -q -p app -- review --type audit --artifact .audit\\audit-1781887189.md",
            "cargo run -q -p app -- review implementation crates\\app",
        ],
        parser: Some(|args, context| {
            let command = match context {
                Some(context) => crate::review::parse_args_for_context(args, context)?,
                None => crate::review::parse_args(args)?,
            };
            Ok(CliCommand::Service(ServiceCommandDispatch::Review(command)))
        }),
    },
    CommandSpec {
        name: "fix-loop",
        aliases: &["loop"],
        accepts_codex_options: true,
        usage: &[
            "cargo run -p app -- fix-loop <plan|audit|implementation> <chemin> [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
            "cargo run -p app -- loop <plan|audit|implementation> <chemin> [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]",
        ],
        examples: &[
            "cargo run -q -p app -- fix-loop plan .plan\\plan-1781894465.md",
            "cargo run -q -p app -- fix-loop audit .audit\\audit-1781887189.md",
            "cargo run -q -p app -- fix-loop implementation crates\\app",
            "cargo run -q -p app -- loop implementation crates\\app",
        ],
        parser: Some(|args, context| {
            let command = match context {
                Some(context) => crate::fix_loop::parse_args_for_context(args, context)?,
                None => crate::fix_loop::parse_args(args)?,
            };
            Ok(CliCommand::Service(ServiceCommandDispatch::FixLoop(command)))
        }),
    },
    CommandSpec {
        name: "automate",
        aliases: &[],
        accepts_codex_options: false,
        usage: &["cargo run -p app -- automate <workflow.json> [prompt]"],
        examples: &["cargo run -q -p app -- automate .\\workflow.json \"Durcir ce module\""],
        parser: Some(|args, _context| crate::automate::parse_automate_args(args).map(CliCommand::Automate)),
    },
    CommandSpec {
        name: "refactor-automate",
        aliases: &["refactor-auto"],
        accepts_codex_options: false,
        usage: &[
            "cargo run -p app -- refactor-automate [--target <dossier>] [--workflow <workflow.json>] [prompt]",
        ],
        examples: &[
            "cargo run -q -p app -- refactor-automate --target crates\\app \"Refactoring SOLID/KISS/DRY\"",
        ],
        parser: Some(|args, _context| {
            crate::automate::parse_refactor_automate_args(args).map(CliCommand::RefactorAutomate)
        }),
    },
];

pub fn is_known_subcommand(value: &str) -> bool {
    COMMANDS
        .iter()
        .any(|spec| spec.name == value || spec.aliases.contains(&value))
}

pub fn canonical_name(value: Option<&str>) -> Option<&'static str> {
    let value = value?;

    COMMANDS
        .iter()
        .find(|spec| spec.name == value || spec.aliases.contains(&value))
        .map(|spec| spec.name)
}

pub fn accepts_codex_options(value: &str) -> bool {
    COMMANDS
        .iter()
        .find(|spec| spec.name == value || spec.aliases.contains(&value))
        .is_some_and(|spec| spec.accepts_codex_options)
}

pub fn is_direct_run(value: Option<&str>) -> bool {
    value.is_none_or(|value| value.starts_with("--") || !is_known_subcommand(value))
}

pub fn usage_lines() -> impl Iterator<Item = &'static str> {
    COMMANDS.iter().flat_map(|spec| spec.usage.iter().copied())
}

pub fn example_lines() -> impl Iterator<Item = &'static str> {
    COMMANDS
        .iter()
        .flat_map(|spec| spec.examples.iter().copied())
}

pub fn parse_registered_subcommand(args: &[String]) -> Option<Result<CliCommand, ParseOutcome>> {
    parse_registered_subcommand_for_context(args, None)
}

pub fn parse_registered_subcommand_for_context(
    args: &[String],
    context: Option<&ExecutionContext>,
) -> Option<Result<CliCommand, ParseOutcome>> {
    let command = canonical_name(args.first().map(String::as_str))?;

    COMMANDS
        .iter()
        .find(|spec| spec.name == command)
        .and_then(|spec| spec.parser)
        .map(|parser| parser(&args[1..], context))
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
        for spec in COMMANDS {
            assert!(
                spec.parser.is_some(),
                "parser missing for registered command {}",
                spec.name
            );
        }
    }
}
