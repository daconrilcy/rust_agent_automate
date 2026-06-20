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

pub fn command_failure_exit_code(error: &crate::reporting::ReportFailure) -> i32 {
    match error {
        crate::reporting::ReportFailure::MissingFinalMessage { status_code, .. } => {
            crate::codex::process_exit_code(Some(*status_code))
        }
        _ => 1,
    }
}

pub fn print_completed_report(
    report: &crate::reporting::CompletedReport,
    spec: &crate::reporting::ReportSpec<'_>,
) {
    if report.status_code != 0 {
        eprintln!(
            "codex a produit un {} mais s'est termine avec le statut {}. Le {} est conserve.",
            spec.final_label, report.status_code, spec.saved_label
        );
        print_failure_details(&report.stdout, &report.stderr);
    }

    println!(
        "{} enregistre dans {}",
        capitalize(spec.saved_label),
        report.saved_path.display()
    );
    println!();
    println!("{}", report.message);
}

pub fn print_report_failure(
    failure: &crate::reporting::ReportFailure,
    spec: &crate::reporting::ReportSpec<'_>,
) {
    eprintln!("{}", render_report_failure(failure, spec));
}

pub fn render_report_failure(
    failure: &crate::reporting::ReportFailure,
    spec: &crate::reporting::ReportSpec<'_>,
) -> String {
    match failure {
        crate::reporting::ReportFailure::CodexCall(error) => {
            format!("echec lors de l'appel a codex: {error}")
        }
        crate::reporting::ReportFailure::MissingFinalMessage {
            status_code,
            stdout,
            stderr,
        } => {
            if *status_code != 0 {
                let stderr = stderr.trim();
                let stdout = stdout.trim();
                if !stderr.is_empty() {
                    stderr.to_string()
                } else if !stdout.is_empty() {
                    stdout.to_string()
                } else {
                    String::new()
                }
            } else {
                format!(
                    "codex n'a pas retourne de message final pour {}",
                    spec.missing_message_label
                )
            }
        }
        crate::reporting::ReportFailure::Save { error, .. } => {
            format!(
                "echec lors de l'enregistrement du {}: {error}",
                spec.saved_label
            )
        }
    }
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
