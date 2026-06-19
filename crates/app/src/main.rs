mod artifact;
mod codex;
mod fix_loop;
mod implementation_audit;
mod plan;
mod review;

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

use codex::{CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT};
use fix_loop::FixLoopCommand;
use implementation_audit::ImplementationAuditCommand;
use plan::PlanCommand;
use review::{ReviewCommand, ReviewSubject};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match parse_args(&args) {
        Ok(CliCommand::Run(request)) => run_request(&request),
        Ok(CliCommand::Audit(request)) => run_audit(&request),
        Ok(CliCommand::Plan(request)) => run_plan(&request),
        Ok(CliCommand::ImplementationAudit(request)) => run_implementation_audit(&request),
        Ok(CliCommand::Review(request)) => run_review(&request),
        Ok(CliCommand::FixLoop(request)) => run_fix_loop(&request),
        Err(ParseOutcome::Help) => print_help(),
        Err(ParseOutcome::Error(message)) => {
            eprintln!("{message}");
            eprintln!();
            print_help();
            process::exit(1);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum CliCommand {
    Run(CodexRequest),
    Audit(AuditCommand),
    Plan(PlanCommand),
    ImplementationAudit(ImplementationAuditCommand),
    Review(ReviewCommand),
    FixLoop(FixLoopCommand),
}

#[derive(Debug, PartialEq, Eq)]
struct AuditCommand {
    request: CodexRequest,
    workspace_root: PathBuf,
    target_dir: PathBuf,
    output_dir: PathBuf,
    timeout: Duration,
}

#[derive(Debug, PartialEq, Eq)]
enum ParseOutcome {
    Help,
    Error(String),
}

fn parse_args(args: &[String]) -> Result<CliCommand, ParseOutcome> {
    if matches!(args.first().map(String::as_str), Some("audit")) {
        return parse_audit_args(&args[1..]).map(CliCommand::Audit);
    }

    if matches!(args.first().map(String::as_str), Some("plan")) {
        return parse_plan_args(&args[1..]).map(CliCommand::Plan);
    }

    if matches!(
        args.first().map(String::as_str),
        Some("implementation-audit" | "impl-audit")
    ) {
        return parse_implementation_audit_args(&args[1..]).map(CliCommand::ImplementationAudit);
    }

    if matches!(args.first().map(String::as_str), Some("review")) {
        return parse_review_args(&args[1..]).map(CliCommand::Review);
    }

    if matches!(args.first().map(String::as_str), Some("fix-loop" | "loop")) {
        return parse_fix_loop_args(&args[1..]).map(CliCommand::FixLoop);
    }

    parse_run_args(args).map(CliCommand::Run)
}

fn parse_run_args(args: &[String]) -> Result<CodexRequest, ParseOutcome> {
    let mut model = String::from(DEFAULT_MODEL);
    let mut reasoning_effort = DEFAULT_REASONING_EFFORT;
    let mut mode = CodexMode::Interactive;
    let mut verbose = false;
    let mut prompt_parts: Vec<String> = Vec::new();

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "--model" => {
                let value = next_value(args, index, "--model")?;
                model = value.to_owned();
                index += 2;
            }
            "--reasoning" => {
                let value = next_value(args, index, "--reasoning")?;
                reasoning_effort = value.parse().map_err(ParseOutcome::Error)?;
                index += 2;
            }
            "--mode" => {
                let value = next_value(args, index, "--mode")?;
                mode = value.parse().map_err(ParseOutcome::Error)?;
                index += 2;
            }
            "--verbose" => {
                verbose = true;
                index += 1;
            }
            value if value.starts_with("--") => {
                return Err(ParseOutcome::Error(format!("option inconnue: {value}")));
            }
            _ => {
                prompt_parts.extend_from_slice(&args[index..]);
                break;
            }
        }
    }

    let prompt = if prompt_parts.is_empty() {
        None
    } else {
        Some(prompt_parts.join(" "))
    };

    if mode == CodexMode::Exec && prompt.is_none() {
        return Err(ParseOutcome::Error(
            "le mode exec requiert un prompt. Exemple: cargo run -p app -- --mode exec \"Ecris un resume du projet\""
                .to_string(),
        ));
    }

    Ok(CodexRequest::new(
        model,
        reasoning_effort,
        mode,
        prompt,
        verbose,
    ))
}

fn parse_audit_args(args: &[String]) -> Result<AuditCommand, ParseOutcome> {
    let mut model = String::from(DEFAULT_MODEL);
    let mut reasoning_effort = DEFAULT_REASONING_EFFORT;
    let mut verbose = false;
    let mut target_dir: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut timeout = Duration::from_secs(900);

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "--model" => {
                let value = next_value(args, index, "--model")?;
                model = value.to_owned();
                index += 2;
            }
            "--reasoning" => {
                let value = next_value(args, index, "--reasoning")?;
                reasoning_effort = value.parse().map_err(ParseOutcome::Error)?;
                index += 2;
            }
            "--target" => {
                let value = next_value(args, index, "--target")?;
                target_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--output-dir" => {
                let value = next_value(args, index, "--output-dir")?;
                output_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--timeout-seconds" => {
                let value = next_value(args, index, "--timeout-seconds")?;
                timeout = parse_timeout(value)?;
                index += 2;
            }
            "--verbose" => {
                verbose = true;
                index += 1;
            }
            value if value.starts_with("--") => {
                return Err(ParseOutcome::Error(format!("option inconnue: {value}")));
            }
            value => {
                return Err(ParseOutcome::Error(format!(
                    "argument inattendu pour audit: {value}"
                )));
            }
        }
    }

    let workspace_root = env::current_dir().map_err(|error| {
        ParseOutcome::Error(format!("impossible de lire le repertoire courant: {error}"))
    })?;
    let target_dir = resolve_audit_target(target_dir.unwrap_or_else(|| workspace_root.clone()))?;
    let output_dir = output_dir.unwrap_or_else(|| workspace_root.join(".audit"));
    let prompt = build_audit_prompt(&workspace_root, &target_dir, &output_dir);

    Ok(AuditCommand {
        request: CodexRequest::new(
            model,
            reasoning_effort,
            CodexMode::Exec,
            Some(prompt),
            verbose,
        ),
        workspace_root,
        target_dir,
        output_dir,
        timeout,
    })
}

fn parse_plan_args(args: &[String]) -> Result<PlanCommand, ParseOutcome> {
    let mut model = String::from(DEFAULT_MODEL);
    let mut reasoning_effort = DEFAULT_REASONING_EFFORT;
    let mut verbose = false;
    let mut audit_path: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut timeout = Duration::from_secs(900);

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "--model" => {
                let value = next_value(args, index, "--model")?;
                model = value.to_owned();
                index += 2;
            }
            "--reasoning" => {
                let value = next_value(args, index, "--reasoning")?;
                reasoning_effort = value.parse().map_err(ParseOutcome::Error)?;
                index += 2;
            }
            "--audit" => {
                let value = next_value(args, index, "--audit")?;
                if audit_path.is_some() {
                    return Err(ParseOutcome::Error(
                        "l'audit a deja ete fourni pour la commande plan".to_string(),
                    ));
                }
                audit_path = Some(PathBuf::from(value));
                index += 2;
            }
            "--output-dir" => {
                let value = next_value(args, index, "--output-dir")?;
                output_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--timeout-seconds" => {
                let value = next_value(args, index, "--timeout-seconds")?;
                timeout = parse_timeout(value)?;
                index += 2;
            }
            "--verbose" => {
                verbose = true;
                index += 1;
            }
            value if value.starts_with("--") => {
                return Err(ParseOutcome::Error(format!("option inconnue: {value}")));
            }
            value => {
                if audit_path.is_some() {
                    return Err(ParseOutcome::Error(format!(
                        "argument inattendu pour plan: {value}"
                    )));
                }

                audit_path = Some(PathBuf::from(value));
                index += 1;
            }
        }
    }

    let workspace_root = env::current_dir().map_err(|error| {
        ParseOutcome::Error(format!("impossible de lire le repertoire courant: {error}"))
    })?;
    let audit_path = plan::resolve_audit_file(audit_path.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande plan requiert un chemin d'audit. Exemple: cargo run -p app -- plan .audit\\audit.md"
                .to_string(),
        )
    })?)
    .map_err(ParseOutcome::Error)?;
    let output_dir = output_dir.unwrap_or_else(|| workspace_root.join(".plan"));
    let prompt = plan::build_prompt(&workspace_root, &audit_path, &output_dir);

    Ok(PlanCommand {
        request: CodexRequest::new(
            model,
            reasoning_effort,
            CodexMode::Exec,
            Some(prompt),
            verbose,
        ),
        workspace_root,
        audit_path,
        output_dir,
        timeout,
    })
}

fn parse_implementation_audit_args(
    args: &[String],
) -> Result<ImplementationAuditCommand, ParseOutcome> {
    let mut model = String::from(DEFAULT_MODEL);
    let mut reasoning_effort = DEFAULT_REASONING_EFFORT;
    let mut verbose = false;
    let mut plan_path: Option<PathBuf> = None;
    let mut implementation_path: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut timeout = Duration::from_secs(900);

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "--model" => {
                let value = next_value(args, index, "--model")?;
                model = value.to_owned();
                index += 2;
            }
            "--reasoning" => {
                let value = next_value(args, index, "--reasoning")?;
                reasoning_effort = value.parse().map_err(ParseOutcome::Error)?;
                index += 2;
            }
            "--plan" => {
                let value = next_value(args, index, "--plan")?;
                if plan_path.is_some() {
                    return Err(ParseOutcome::Error(
                        "le plan d'implementation a deja ete fourni".to_string(),
                    ));
                }
                plan_path = Some(PathBuf::from(value));
                index += 2;
            }
            "--implementation" => {
                let value = next_value(args, index, "--implementation")?;
                if implementation_path.is_some() {
                    return Err(ParseOutcome::Error(
                        "le chemin d'implementation a deja ete fourni".to_string(),
                    ));
                }
                implementation_path = Some(PathBuf::from(value));
                index += 2;
            }
            "--output-dir" => {
                let value = next_value(args, index, "--output-dir")?;
                output_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--timeout-seconds" => {
                let value = next_value(args, index, "--timeout-seconds")?;
                timeout = parse_timeout(value)?;
                index += 2;
            }
            "--verbose" => {
                verbose = true;
                index += 1;
            }
            value if value.starts_with("--") => {
                return Err(ParseOutcome::Error(format!("option inconnue: {value}")));
            }
            value => {
                if plan_path.is_some() {
                    return Err(ParseOutcome::Error(format!(
                        "argument inattendu pour implementation-audit: {value}"
                    )));
                }

                plan_path = Some(PathBuf::from(value));
                index += 1;
            }
        }
    }

    let plan_path = plan_path.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande implementation-audit requiert un plan. Exemple: cargo run -p app -- implementation-audit .plan\\plan.md"
                .to_string(),
        )
    })?;
    let workspace_root = env::current_dir().map_err(|error| {
        ParseOutcome::Error(format!("impossible de lire le repertoire courant: {error}"))
    })?;
    let plan_path =
        implementation_audit::resolve_plan_file(plan_path).map_err(ParseOutcome::Error)?;
    let implementation_path = implementation_path
        .map(implementation_audit::resolve_implementation_path)
        .transpose()
        .map_err(ParseOutcome::Error)?;
    let output_dir = output_dir.unwrap_or_else(|| workspace_root.join(".audit"));
    let prompt = implementation_audit::build_prompt(
        &workspace_root,
        &plan_path,
        implementation_path.as_deref(),
        &output_dir,
    );

    Ok(ImplementationAuditCommand {
        request: CodexRequest::new(
            model,
            reasoning_effort,
            CodexMode::Exec,
            Some(prompt),
            verbose,
        ),
        workspace_root,
        plan_path,
        implementation_path,
        output_dir,
        timeout,
    })
}

fn parse_review_args(args: &[String]) -> Result<ReviewCommand, ParseOutcome> {
    let mut model = String::from(DEFAULT_MODEL);
    let mut reasoning_effort = DEFAULT_REASONING_EFFORT;
    let mut verbose = false;
    let mut subject: Option<ReviewSubject> = None;
    let mut artifact_path: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut timeout = Duration::from_secs(900);

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "--model" => {
                let value = next_value(args, index, "--model")?;
                model = value.to_owned();
                index += 2;
            }
            "--reasoning" => {
                let value = next_value(args, index, "--reasoning")?;
                reasoning_effort = value.parse().map_err(ParseOutcome::Error)?;
                index += 2;
            }
            "--type" => {
                let value = next_value(args, index, "--type")?;
                if subject.is_some() {
                    return Err(ParseOutcome::Error(
                        "le type de review a deja ete fourni".to_string(),
                    ));
                }
                subject = Some(value.parse().map_err(ParseOutcome::Error)?);
                index += 2;
            }
            "--artifact" => {
                let value = next_value(args, index, "--artifact")?;
                if artifact_path.is_some() {
                    return Err(ParseOutcome::Error(
                        "l'artefact de review a deja ete fourni".to_string(),
                    ));
                }
                artifact_path = Some(PathBuf::from(value));
                index += 2;
            }
            "--output-dir" => {
                let value = next_value(args, index, "--output-dir")?;
                output_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--timeout-seconds" => {
                let value = next_value(args, index, "--timeout-seconds")?;
                timeout = parse_timeout(value)?;
                index += 2;
            }
            "--verbose" => {
                verbose = true;
                index += 1;
            }
            value if value.starts_with("--") => {
                return Err(ParseOutcome::Error(format!("option inconnue: {value}")));
            }
            value => {
                if subject.is_none() {
                    subject = Some(value.parse().map_err(ParseOutcome::Error)?);
                    index += 1;
                    continue;
                }

                if artifact_path.is_none() {
                    artifact_path = Some(PathBuf::from(value));
                    index += 1;
                    continue;
                }

                return Err(ParseOutcome::Error(format!(
                    "argument inattendu pour review: {value}"
                )));
            }
        }
    }

    let subject = subject.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande review requiert un type: plan, audit ou implementation".to_string(),
        )
    })?;
    let artifact_path = artifact_path.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande review requiert un artefact. Exemple: cargo run -p app -- review plan .plan\\plan.md"
                .to_string(),
        )
    })?;
    let workspace_root = env::current_dir().map_err(|error| {
        ParseOutcome::Error(format!("impossible de lire le repertoire courant: {error}"))
    })?;
    let artifact_path =
        review::resolve_artifact_path(subject, artifact_path).map_err(ParseOutcome::Error)?;
    let output_dir = output_dir.unwrap_or_else(|| workspace_root.join(".review"));
    let prompt = review::build_prompt(&workspace_root, subject, &artifact_path, &output_dir);

    Ok(ReviewCommand {
        request: CodexRequest::new(
            model,
            reasoning_effort,
            CodexMode::Exec,
            Some(prompt),
            verbose,
        ),
        workspace_root,
        subject,
        artifact_path,
        output_dir,
        timeout,
    })
}

fn parse_fix_loop_args(args: &[String]) -> Result<FixLoopCommand, ParseOutcome> {
    let mut model = String::from(DEFAULT_MODEL);
    let mut reasoning_effort = DEFAULT_REASONING_EFFORT;
    let mut verbose = false;
    let mut input_kind: Option<ReviewSubject> = None;
    let mut artifact_path: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut timeout = Duration::from_secs(1800);

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "--model" => {
                let value = next_value(args, index, "--model")?;
                model = value.to_owned();
                index += 2;
            }
            "--reasoning" => {
                let value = next_value(args, index, "--reasoning")?;
                reasoning_effort = value.parse().map_err(ParseOutcome::Error)?;
                index += 2;
            }
            "--type" => {
                let value = next_value(args, index, "--type")?;
                if input_kind.is_some() {
                    return Err(ParseOutcome::Error(
                        "le type d'entree de fix-loop a deja ete fourni".to_string(),
                    ));
                }
                input_kind = Some(parse_fix_loop_input_kind(value)?);
                index += 2;
            }
            "--artifact" => {
                let value = next_value(args, index, "--artifact")?;
                if artifact_path.is_some() {
                    return Err(ParseOutcome::Error(
                        "l'artefact de fix-loop a deja ete fourni".to_string(),
                    ));
                }
                artifact_path = Some(PathBuf::from(value));
                index += 2;
            }
            "--output-dir" => {
                let value = next_value(args, index, "--output-dir")?;
                output_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--timeout-seconds" => {
                let value = next_value(args, index, "--timeout-seconds")?;
                timeout = parse_timeout(value)?;
                index += 2;
            }
            "--verbose" => {
                verbose = true;
                index += 1;
            }
            value if value.starts_with("--") => {
                return Err(ParseOutcome::Error(format!("option inconnue: {value}")));
            }
            value => {
                if input_kind.is_none() {
                    input_kind = Some(parse_fix_loop_input_kind(value)?);
                    index += 1;
                    continue;
                }

                if artifact_path.is_none() {
                    artifact_path = Some(PathBuf::from(value));
                    index += 1;
                    continue;
                }

                return Err(ParseOutcome::Error(format!(
                    "argument inattendu pour fix-loop: {value}"
                )));
            }
        }
    }

    let input_kind = input_kind.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande fix-loop requiert un type: plan, audit ou implementation".to_string(),
        )
    })?;
    let artifact_path = artifact_path.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande fix-loop requiert un artefact. Exemple: cargo run -p app -- fix-loop plan .plan\\plan.md"
                .to_string(),
        )
    })?;
    let workspace_root = env::current_dir().map_err(|error| {
        ParseOutcome::Error(format!("impossible de lire le repertoire courant: {error}"))
    })?;
    let artifact_path =
        fix_loop::resolve_artifact_path(input_kind, artifact_path).map_err(ParseOutcome::Error)?;
    let output_dir = output_dir.unwrap_or_else(|| workspace_root.join(".fix-loop"));
    let prompt = fix_loop::build_prompt(&workspace_root, input_kind, &artifact_path, &output_dir);

    Ok(FixLoopCommand {
        request: CodexRequest::new(
            model,
            reasoning_effort,
            CodexMode::Exec,
            Some(prompt),
            verbose,
        ),
        workspace_root,
        input_kind,
        artifact_path,
        output_dir,
        timeout,
    })
}

fn parse_fix_loop_input_kind(value: &str) -> Result<ReviewSubject, ParseOutcome> {
    value.parse().map_err(|_| {
        ParseOutcome::Error(format!(
            "type d'entree fix-loop invalide: {value}. Valeurs attendues: plan, audit, implementation"
        ))
    })
}

fn next_value<'a>(
    args: &'a [String],
    index: usize,
    option_name: &str,
) -> Result<&'a str, ParseOutcome> {
    args.get(index + 1)
        .map(String::as_str)
        .ok_or_else(|| ParseOutcome::Error(format!("valeur manquante pour {option_name}")))
}

fn print_help() {
    println!(
        "Usage:
  cargo run -p app -- [--model <nom>] [--reasoning <low|medium|high>] [--mode <interactive|exec>] [--verbose] [prompt]
  cargo run -p app -- audit [--target <chemin>] [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]
  cargo run -p app -- plan <chemin-audit> [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]
  cargo run -p app -- implementation-audit <chemin-plan> [--implementation <chemin>] [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]
  cargo run -p app -- impl-audit <chemin-plan> [--implementation <chemin>] [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]
  cargo run -p app -- review <plan|audit|implementation> <chemin> [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]
  cargo run -p app -- fix-loop <plan|audit|implementation> <chemin> [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]
  cargo run -p app -- loop <plan|audit|implementation> <chemin> [--model <nom>] [--reasoning <low|medium|high>] [--verbose] [--output-dir <chemin>] [--timeout-seconds <secondes>]

Exemples:
  cargo run -q -p app -- --model gpt-5.4 --reasoning low
  cargo run -q -p app -- --mode exec --model gpt-5.4 --reasoning low \"Explique ce depot\"
  cargo run -q -p app -- --mode exec --verbose --model gpt-5.4 --reasoning low \"Explique ce depot\"
  cargo run -q -p app -- audit
  cargo run -q -p app -- audit --target ..\\mon-projet
  cargo run -q -p app -- audit --output-dir .audit
  cargo run -q -p app -- audit --timeout-seconds 120
  cargo run -q -p app -- plan .audit\\audit-1781887189.md
  cargo run -q -p app -- plan --audit .audit\\audit-1781887189.md --output-dir .plan
  cargo run -q -p app -- implementation-audit .plan\\plan-1781894465.md
  cargo run -q -p app -- implementation-audit --plan .plan\\plan-1781894465.md --implementation crates\\app
  cargo run -q -p app -- review plan .plan\\plan-1781894465.md
  cargo run -q -p app -- review --type audit --artifact .audit\\audit-1781887189.md
  cargo run -q -p app -- review implementation crates\\app
  cargo run -q -p app -- fix-loop plan .plan\\plan-1781894465.md
  cargo run -q -p app -- fix-loop audit .audit\\audit-1781887189.md
  cargo run -q -p app -- fix-loop implementation crates\\app
  cargo run -q -p app -- loop implementation crates\\app"
    );
}

fn print_failure_details(stdout: &str, stderr: &str) {
    let stderr = stderr.trim();
    let stdout = stdout.trim();

    if !stderr.is_empty() {
        eprintln!("{stderr}");
        return;
    }

    if !stdout.is_empty() {
        eprintln!("{stdout}");
    }
}

fn run_request(request: &CodexRequest) {
    match codex::run(request) {
        Ok(result) => {
            if result.status.success() {
                if let Some(message) = result.final_message {
                    println!("{message}");
                }
            } else {
                print_failure_details(&result.stdout, &result.stderr);
                process::exit(result.status.code().unwrap_or(1));
            }
        }
        Err(error) => {
            eprintln!("echec lors de l'appel a codex: {error}");
            process::exit(1);
        }
    }
}

fn run_audit(command: &AuditCommand) {
    eprintln!(
        "Audit Codex en cours sur {} (timeout: {} secondes)...",
        command.target_dir.display(),
        command.timeout.as_secs()
    );

    match codex::run_until_final_message(&command.request, command.timeout) {
        Ok(result) => {
            let Some(message) = result.final_message else {
                if !result.status.success() {
                    print_failure_details(&result.stdout, &result.stderr);
                    process::exit(result.status.code().unwrap_or(1));
                }

                eprintln!("codex n'a pas retourne de message final pour l'audit");
                process::exit(1);
            };

            if !result.status.success() {
                eprintln!(
                    "codex a produit un rapport final mais s'est termine avec le statut {}. Le rapport est conserve.",
                    result.status
                );
                print_failure_details(&result.stdout, &result.stderr);
            }

            match save_audit_report(&command.output_dir, &message) {
                Ok(path) => {
                    println!("Audit enregistre dans {}", path.display());
                    println!();
                    println!("{message}");
                }
                Err(error) => {
                    eprintln!("echec lors de l'enregistrement de l'audit: {error}");
                    process::exit(1);
                }
            }
        }
        Err(error) => {
            eprintln!("echec lors de l'appel a codex: {error}");
            process::exit(1);
        }
    }
}

fn run_plan(command: &PlanCommand) {
    eprintln!(
        "Plan Codex en cours depuis {} (timeout: {} secondes)...",
        command.audit_path.display(),
        command.timeout.as_secs()
    );

    match codex::run_until_final_message(&command.request, command.timeout) {
        Ok(result) => {
            let Some(message) = result.final_message else {
                if !result.status.success() {
                    print_failure_details(&result.stdout, &result.stderr);
                    process::exit(result.status.code().unwrap_or(1));
                }

                eprintln!("codex n'a pas retourne de message final pour le plan");
                process::exit(1);
            };

            if !result.status.success() {
                eprintln!(
                    "codex a produit un plan final mais s'est termine avec le statut {}. Le plan est conserve.",
                    result.status
                );
                print_failure_details(&result.stdout, &result.stderr);
            }

            match plan::save_plan(&command.output_dir, &message) {
                Ok(path) => {
                    println!("Plan enregistre dans {}", path.display());
                    println!();
                    println!("{message}");
                }
                Err(error) => {
                    eprintln!("echec lors de l'enregistrement du plan: {error}");
                    process::exit(1);
                }
            }
        }
        Err(error) => {
            eprintln!("echec lors de l'appel a codex: {error}");
            process::exit(1);
        }
    }
}

fn run_implementation_audit(command: &ImplementationAuditCommand) {
    let scope = command.implementation_path.as_deref().map_or_else(
        || "git diff / workspace".to_string(),
        |path| path.display().to_string(),
    );

    eprintln!(
        "Audit d'implementation Codex en cours depuis {} sur {} (timeout: {} secondes)...",
        command.plan_path.display(),
        scope,
        command.timeout.as_secs()
    );

    match codex::run_until_final_message(&command.request, command.timeout) {
        Ok(result) => {
            let Some(message) = result.final_message else {
                if !result.status.success() {
                    print_failure_details(&result.stdout, &result.stderr);
                    process::exit(result.status.code().unwrap_or(1));
                }

                eprintln!("codex n'a pas retourne de message final pour l'audit d'implementation");
                process::exit(1);
            };

            if !result.status.success() {
                eprintln!(
                    "codex a produit un audit d'implementation final mais s'est termine avec le statut {}. Le rapport est conserve.",
                    result.status
                );
                print_failure_details(&result.stdout, &result.stderr);
            }

            match implementation_audit::save_audit(&command.output_dir, &message) {
                Ok(path) => {
                    println!("Audit d'implementation enregistre dans {}", path.display());
                    println!();
                    println!("{message}");
                }
                Err(error) => {
                    eprintln!(
                        "echec lors de l'enregistrement de l'audit d'implementation: {error}"
                    );
                    process::exit(1);
                }
            }
        }
        Err(error) => {
            eprintln!("echec lors de l'appel a codex: {error}");
            process::exit(1);
        }
    }
}

fn run_review(command: &ReviewCommand) {
    eprintln!(
        "Review adversariale Codex en cours ({}) sur {} (timeout: {} secondes)...",
        command.subject,
        command.artifact_path.display(),
        command.timeout.as_secs()
    );

    match codex::run_until_final_message(&command.request, command.timeout) {
        Ok(result) => {
            let Some(message) = result.final_message else {
                if !result.status.success() {
                    print_failure_details(&result.stdout, &result.stderr);
                    process::exit(result.status.code().unwrap_or(1));
                }

                eprintln!("codex n'a pas retourne de message final pour la review");
                process::exit(1);
            };

            if !result.status.success() {
                eprintln!(
                    "codex a produit une review finale mais s'est termine avec le statut {}. La review est conservee.",
                    result.status
                );
                print_failure_details(&result.stdout, &result.stderr);
            }

            match review::save_review(&command.output_dir, &message) {
                Ok(path) => {
                    println!("Review enregistree dans {}", path.display());
                    println!();
                    println!("{message}");
                }
                Err(error) => {
                    eprintln!("echec lors de l'enregistrement de la review: {error}");
                    process::exit(1);
                }
            }
        }
        Err(error) => {
            eprintln!("echec lors de l'appel a codex: {error}");
            process::exit(1);
        }
    }
}

fn run_fix_loop(command: &FixLoopCommand) {
    eprintln!(
        "Boucle review/correction Codex en cours ({}) sur {} (timeout: {} secondes)...",
        command.input_kind,
        command.artifact_path.display(),
        command.timeout.as_secs()
    );

    match codex::run_until_final_message(&command.request, command.timeout) {
        Ok(result) => {
            let Some(message) = result.final_message else {
                if !result.status.success() {
                    print_failure_details(&result.stdout, &result.stderr);
                    process::exit(result.status.code().unwrap_or(1));
                }

                eprintln!("codex n'a pas retourne de message final pour fix-loop");
                process::exit(1);
            };

            if !result.status.success() {
                eprintln!(
                    "codex a produit un rapport final mais s'est termine avec le statut {}. Le rapport est conserve.",
                    result.status
                );
                print_failure_details(&result.stdout, &result.stderr);
            }

            match fix_loop::save_report(&command.output_dir, &message) {
                Ok(path) => {
                    println!("Rapport fix-loop enregistre dans {}", path.display());
                    println!();
                    println!("{message}");
                }
                Err(error) => {
                    eprintln!("echec lors de l'enregistrement du rapport fix-loop: {error}");
                    process::exit(1);
                }
            }
        }
        Err(error) => {
            eprintln!("echec lors de l'appel a codex: {error}");
            process::exit(1);
        }
    }
}

fn parse_timeout(value: &str) -> Result<Duration, ParseOutcome> {
    let seconds = value.parse::<u64>().map_err(|_| {
        ParseOutcome::Error(format!(
            "timeout invalide: {value}. Valeur attendue: nombre de secondes positif"
        ))
    })?;

    if seconds == 0 {
        return Err(ParseOutcome::Error(
            "timeout invalide: 0. Valeur attendue: nombre de secondes positif".to_string(),
        ));
    }

    Ok(Duration::from_secs(seconds))
}

fn build_audit_prompt(workspace_root: &Path, target_dir: &Path, output_dir: &Path) -> String {
    format!(
        concat!(
            "Use $rust-refactor-audit to audit the Rust code located at \"{}\".\n",
            "The audit must use the central Codex skill named rust-refactor-audit, follow its SKILL.md instructions, ",
            "and apply its references/audit-rubric.md rubric.\n",
            "The current local workspace running this command is \"{}\" and the audit report will be saved by the wrapper under \"{}\".\n",
            "The target directory may not be a Git repository; if git commands fail for that reason, mention it briefly and continue.\n",
            "Inspect the current workspace before concluding and produce the final answer as a complete Markdown audit report only.\n",
            "Use this exact section order:\n",
            "1. Scope\n",
            "2. Architecture Snapshot\n",
            "3. Key Findings\n",
            "4. Principle Review (SOLID / DRY / KISS / YAGNI)\n",
            "5. Refactoring Roadmap\n",
            "6. Quick Wins\n",
            "7. Open Questions / Validation Needed\n"
        ),
        target_dir.display(),
        workspace_root.display(),
        output_dir.display()
    )
}

fn resolve_audit_target(path: PathBuf) -> Result<PathBuf, ParseOutcome> {
    let path = if path.is_absolute() {
        path
    } else {
        env::current_dir()
            .map_err(|error| {
                ParseOutcome::Error(format!("impossible de lire le repertoire courant: {error}"))
            })?
            .join(path)
    };

    let metadata = fs::metadata(&path).map_err(|error| {
        ParseOutcome::Error(format!(
            "impossible d'acceder au dossier cible {}: {error}",
            path.display()
        ))
    })?;

    if !metadata.is_dir() {
        return Err(ParseOutcome::Error(format!(
            "le chemin cible doit etre un dossier: {}",
            path.display()
        )));
    }

    fs::canonicalize(&path).map_err(|error| {
        ParseOutcome::Error(format!(
            "impossible de resoudre le dossier cible {}: {error}",
            path.display()
        ))
    })
}

fn save_audit_report(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    artifact::save_timestamped_markdown(output_dir, "audit", content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::ReasoningEffort;

    fn normalize_path(path: &Path) -> String {
        path.display().to_string().replace("\\\\?\\", "")
    }

    fn parse(input: &[&str]) -> Result<CliCommand, ParseOutcome> {
        let args = input
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        parse_args(&args)
    }

    fn request_for(command: &CliCommand) -> &CodexRequest {
        match command {
            CliCommand::Run(request) => request,
            CliCommand::Audit(command) => &command.request,
            CliCommand::Plan(command) => &command.request,
            CliCommand::ImplementationAudit(command) => &command.request,
            CliCommand::Review(command) => &command.request,
            CliCommand::FixLoop(command) => &command.request,
        }
    }

    #[test]
    fn parses_defaults() {
        let command = parse(&[]).expect("la configuration par defaut doit etre valide");

        let CliCommand::Run(request) = command else {
            panic!("la commande par defaut doit etre le mode run");
        };

        assert_eq!(request.model, DEFAULT_MODEL);
        assert_eq!(request.reasoning_effort, DEFAULT_REASONING_EFFORT);
        assert_eq!(request.mode, CodexMode::Interactive);
        assert_eq!(request.prompt, None);
        assert!(!request.verbose);
    }

    #[test]
    fn parses_all_options() {
        let command = parse(&[
            "--model",
            "gpt-5.5",
            "--reasoning",
            "high",
            "--mode",
            "exec",
            "Analyse",
            "ce",
            "repo",
        ])
        .expect("les options doivent etre parsees");

        let CliCommand::Run(request) = command else {
            panic!("la commande attendue est run");
        };

        assert_eq!(request.model, "gpt-5.5");
        assert_eq!(request.reasoning_effort, ReasoningEffort::High);
        assert_eq!(request.mode, CodexMode::Exec);
        assert_eq!(request.prompt.as_deref(), Some("Analyse ce repo"));
        assert!(!request.verbose);
    }

    #[test]
    fn codex_commands_accept_model_and_reasoning_overrides() {
        let cases: &[&[&str]] = &[
            &["--model", "gpt-5.6", "--reasoning", "medium"],
            &["audit", "--model", "gpt-5.6", "--reasoning", "medium"],
            &[
                "plan",
                "Cargo.toml",
                "--model",
                "gpt-5.6",
                "--reasoning",
                "medium",
            ],
            &[
                "implementation-audit",
                "Cargo.toml",
                "--model",
                "gpt-5.6",
                "--reasoning",
                "medium",
            ],
            &[
                "review",
                "audit",
                "Cargo.toml",
                "--model",
                "gpt-5.6",
                "--reasoning",
                "medium",
            ],
            &[
                "fix-loop",
                "plan",
                "Cargo.toml",
                "--model",
                "gpt-5.6",
                "--reasoning",
                "medium",
            ],
        ];

        for case in cases {
            let command = parse(case).expect("la commande doit accepter model et reasoning");
            let request = request_for(&command);

            assert_eq!(request.model, "gpt-5.6");
            assert_eq!(request.reasoning_effort, ReasoningEffort::Medium);
        }
    }

    #[test]
    fn rejects_exec_without_prompt() {
        let error = parse(&["--mode", "exec"]).expect_err("exec sans prompt doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "le mode exec requiert un prompt. Exemple: cargo run -p app -- --mode exec \"Ecris un resume du projet\""
                    .to_string()
            )
        );
    }

    #[test]
    fn keeps_prompt_spacing() {
        let command = parse(&["--mode", "exec", "Resume", "ce", "projet"])
            .expect("le prompt doit etre reconstruit");

        let CliCommand::Run(request) = command else {
            panic!("la commande attendue est run");
        };

        assert_eq!(request.prompt.as_deref(), Some("Resume ce projet"));
    }

    #[test]
    fn parses_verbose_flag() {
        let command = parse(&["--verbose", "--mode", "exec", "Resume"])
            .expect("le flag verbose doit etre parse");

        let CliCommand::Run(request) = command else {
            panic!("la commande attendue est run");
        };

        assert!(request.verbose);
    }

    #[test]
    fn parses_audit_command() {
        let command = parse(&["audit", "--verbose"]).expect("audit doit etre parse");

        let CliCommand::Audit(audit) = command else {
            panic!("la commande attendue est audit");
        };

        assert_eq!(audit.request.mode, CodexMode::Exec);
        assert_eq!(audit.request.model, DEFAULT_MODEL);
        assert_eq!(audit.request.reasoning_effort, DEFAULT_REASONING_EFFORT);
        assert!(audit.request.verbose);
        assert_eq!(
            normalize_path(&audit.target_dir),
            normalize_path(&audit.workspace_root)
        );
        assert!(audit.output_dir.ends_with(".audit"));
        assert_eq!(audit.timeout, Duration::from_secs(900));
        assert!(audit.request.prompt.as_deref().is_some_and(|prompt| {
            prompt.contains("$rust-refactor-audit")
                && prompt.contains("central Codex skill named rust-refactor-audit")
                && !prompt.contains(".agents/skills/rust-refactor-audit")
                && prompt.contains(&audit.target_dir.display().to_string())
                && prompt.contains(&audit.output_dir.display().to_string())
        }));
    }

    #[test]
    fn parses_audit_timeout_argument() {
        let command = parse(&["audit", "--timeout-seconds", "42"])
            .expect("audit doit accepter un timeout configurable");

        let CliCommand::Audit(audit) = command else {
            panic!("la commande attendue est audit");
        };

        assert_eq!(audit.timeout, Duration::from_secs(42));
    }

    #[test]
    fn rejects_zero_audit_timeout() {
        let error = parse(&["audit", "--timeout-seconds", "0"])
            .expect_err("un timeout nul doit etre refuse");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "timeout invalide: 0. Valeur attendue: nombre de secondes positif".to_string()
            )
        );
    }

    #[test]
    fn parses_audit_target_argument() {
        let command =
            parse(&["audit", "--target", "."]).expect("audit doit accepter un dossier cible");

        let CliCommand::Audit(audit) = command else {
            panic!("la commande attendue est audit");
        };

        assert_eq!(
            normalize_path(&audit.target_dir),
            normalize_path(&audit.workspace_root)
        );
    }

    #[test]
    fn parses_plan_command_with_positional_audit_path() {
        let command = parse(&["plan", "Cargo.toml"]).expect("plan doit etre parse");

        let CliCommand::Plan(plan) = command else {
            panic!("la commande attendue est plan");
        };

        assert_eq!(plan.request.mode, CodexMode::Exec);
        assert_eq!(plan.request.model, DEFAULT_MODEL);
        assert_eq!(plan.request.reasoning_effort, DEFAULT_REASONING_EFFORT);
        assert!(plan.output_dir.ends_with(".plan"));
        assert_eq!(plan.timeout, Duration::from_secs(900));
        assert!(plan.request.prompt.as_deref().is_some_and(|prompt| {
            prompt.contains("$refactor-plan-from-audit")
                && prompt.contains("central Codex skill named refactor-plan-from-audit")
                && prompt.contains("complete Markdown implementation handoff plan only")
        }));
    }

    #[test]
    fn parses_implementation_audit_command_with_positional_plan_path() {
        let command = parse(&["implementation-audit", "Cargo.toml"])
            .expect("implementation-audit doit etre parse");

        let CliCommand::ImplementationAudit(audit) = command else {
            panic!("la commande attendue est implementation-audit");
        };

        assert_eq!(audit.request.mode, CodexMode::Exec);
        assert_eq!(audit.request.model, DEFAULT_MODEL);
        assert_eq!(audit.request.reasoning_effort, DEFAULT_REASONING_EFFORT);
        assert!(audit.output_dir.ends_with(".audit"));
        assert_eq!(audit.timeout, Duration::from_secs(900));
        assert!(audit.implementation_path.is_none());
        assert!(audit.request.prompt.as_deref().is_some_and(|prompt| {
            prompt.contains("$rust-implementation-plan-audit")
                && prompt.contains("central Codex skill named rust-implementation-plan-audit")
                && prompt.contains("No explicit implementation path was provided")
                && prompt.contains("complete Markdown Rust Implementation Plan Audit report only")
        }));
    }

    #[test]
    fn parses_implementation_audit_command_with_named_options() {
        let command = parse(&[
            "impl-audit",
            "--plan",
            "Cargo.toml",
            "--implementation",
            ".",
            "--timeout-seconds",
            "42",
            "--verbose",
        ])
        .expect("impl-audit doit accepter les options nommees");

        let CliCommand::ImplementationAudit(audit) = command else {
            panic!("la commande attendue est implementation-audit");
        };

        assert_eq!(audit.timeout, Duration::from_secs(42));
        assert!(audit.request.verbose);
        assert!(
            audit
                .implementation_path
                .as_ref()
                .is_some_and(|path| path.is_dir())
        );
        assert!(
            audit
                .request
                .prompt
                .as_deref()
                .is_some_and(|prompt| { prompt.contains("Review the implementation evidence at") })
        );
    }

    #[test]
    fn parses_review_command_with_positional_type_and_artifact() {
        let command = parse(&["review", "implementation", "."]).expect("review doit etre parse");

        let CliCommand::Review(review) = command else {
            panic!("la commande attendue est review");
        };

        assert_eq!(review.request.mode, CodexMode::Exec);
        assert_eq!(review.request.model, DEFAULT_MODEL);
        assert_eq!(review.request.reasoning_effort, DEFAULT_REASONING_EFFORT);
        assert_eq!(review.subject, ReviewSubject::Implementation);
        assert!(review.output_dir.ends_with(".review"));
        assert_eq!(review.timeout, Duration::from_secs(900));
        assert!(review.request.prompt.as_deref().is_some_and(|prompt| {
            prompt.contains("$adversarial-review")
                && prompt.contains("central Codex skill named adversarial-review")
                && prompt.contains("Review mode: Implementation review")
                && prompt.contains("complete Markdown adversarial review only")
        }));
    }

    #[test]
    fn parses_review_command_with_named_type_and_artifact() {
        let command = parse(&[
            "review",
            "--type",
            "audit",
            "--artifact",
            "Cargo.toml",
            "--timeout-seconds",
            "42",
            "--verbose",
        ])
        .expect("review doit accepter les options nommees");

        let CliCommand::Review(review) = command else {
            panic!("la commande attendue est review");
        };

        assert_eq!(review.subject, ReviewSubject::Audit);
        assert_eq!(review.timeout, Duration::from_secs(42));
        assert!(review.request.verbose);
    }

    #[test]
    fn parses_fix_loop_command_with_positional_type_and_artifact() {
        let command = parse(&["fix-loop", "plan", "Cargo.toml"]).expect("fix-loop doit etre parse");

        let CliCommand::FixLoop(fix_loop) = command else {
            panic!("la commande attendue est fix-loop");
        };

        assert_eq!(fix_loop.request.mode, CodexMode::Exec);
        assert_eq!(fix_loop.request.model, DEFAULT_MODEL);
        assert_eq!(fix_loop.request.reasoning_effort, DEFAULT_REASONING_EFFORT);
        assert_eq!(fix_loop.input_kind, ReviewSubject::Plan);
        assert!(fix_loop.output_dir.ends_with(".fix-loop"));
        assert_eq!(fix_loop.timeout, Duration::from_secs(1800));
        assert!(fix_loop.request.prompt.as_deref().is_some_and(|prompt| {
            prompt.contains("$rust-review-fix-loop")
                && prompt.contains("central Codex skill named rust-review-fix-loop")
                && prompt.contains("Input kind: plan")
                && prompt.contains("rust-dev-solid")
                && prompt.contains("adversarial-review cycles until no actionable findings remain")
        }));
    }

    #[test]
    fn parses_fix_loop_alias_with_named_type_and_artifact() {
        let command = parse(&[
            "loop",
            "--type",
            "implementation",
            "--artifact",
            ".",
            "--timeout-seconds",
            "42",
            "--verbose",
        ])
        .expect("loop doit accepter les options nommees");

        let CliCommand::FixLoop(fix_loop) = command else {
            panic!("la commande attendue est fix-loop");
        };

        assert_eq!(fix_loop.input_kind, ReviewSubject::Implementation);
        assert_eq!(fix_loop.timeout, Duration::from_secs(42));
        assert!(fix_loop.request.verbose);
    }

    #[test]
    fn rejects_fix_loop_without_type() {
        let error = parse(&["fix-loop"]).expect_err("fix-loop sans type doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "la commande fix-loop requiert un type: plan, audit ou implementation".to_string()
            )
        );
    }

    #[test]
    fn rejects_fix_loop_without_artifact() {
        let error = parse(&["fix-loop", "audit"]).expect_err("fix-loop sans artefact doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "la commande fix-loop requiert un artefact. Exemple: cargo run -p app -- fix-loop plan .plan\\plan.md"
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_invalid_fix_loop_input_kind_with_command_specific_error() {
        let error =
            parse(&["fix-loop", "design", "Cargo.toml"]).expect_err("type invalide doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "type d'entree fix-loop invalide: design. Valeurs attendues: plan, audit, implementation"
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_review_without_type() {
        let error = parse(&["review"]).expect_err("review sans type doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "la commande review requiert un type: plan, audit ou implementation".to_string()
            )
        );
    }

    #[test]
    fn rejects_review_without_artifact() {
        let error = parse(&["review", "plan"]).expect_err("review sans artefact doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "la commande review requiert un artefact. Exemple: cargo run -p app -- review plan .plan\\plan.md"
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_duplicate_plan_audit_argument() {
        let error = parse(&["plan", "--audit", "Cargo.toml", "--audit", "README.md"])
            .expect_err("plan ne doit pas accepter deux chemins d'audit");

        assert_eq!(
            error,
            ParseOutcome::Error("l'audit a deja ete fourni pour la commande plan".to_string())
        );
    }

    #[test]
    fn rejects_plan_without_audit_path() {
        let error = parse(&["plan"]).expect_err("plan sans audit doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "la commande plan requiert un chemin d'audit. Exemple: cargo run -p app -- plan .audit\\audit.md"
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_implementation_audit_without_plan_path() {
        let error = parse(&["implementation-audit"])
            .expect_err("implementation-audit sans plan doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "la commande implementation-audit requiert un plan. Exemple: cargo run -p app -- implementation-audit .plan\\plan.md"
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_positional_argument_for_audit() {
        let error =
            parse(&["audit", "foo"]).expect_err("audit ne doit pas accepter d'argument libre");

        assert_eq!(
            error,
            ParseOutcome::Error("argument inattendu pour audit: foo".to_string())
        );
    }
}
