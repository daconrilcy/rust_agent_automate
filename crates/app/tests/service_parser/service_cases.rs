use super::*;

#[test]
fn service_commands_preserve_named_output_dirs() {
    let cases = [
        ("audit", parse(&["audit", "--output-dir", ".audit-custom"])),
        (
            "plan",
            parse(&["plan", "Cargo.toml", "--output-dir", ".plan-custom"]),
        ),
        (
            "implementation-audit",
            parse(&[
                "implementation-audit",
                "Cargo.toml",
                "--output-dir",
                ".audit-custom",
            ]),
        ),
        (
            "review",
            parse(&[
                "review",
                "implementation",
                ".",
                "--output-dir",
                ".review-custom",
            ]),
        ),
        (
            "fix-loop",
            parse(&[
                "fix-loop",
                "implementation",
                ".",
                "--output-dir",
                ".fix-loop-custom",
            ]),
        ),
    ];

    for (name, result) in cases {
        let command = result.expect("parse service command");
        let dispatch = command
            .service()
            .unwrap_or_else(|| panic!("{name} doit etre une commande service"));
        let output_dir = dispatch.output_dir();

        assert!(
            output_dir.to_string_lossy().contains("-custom"),
            "le dossier de sortie de {name} doit reprendre la valeur nommee"
        );
    }
}

#[test]
fn service_commands_preserve_timeout_seconds() {
    let cases = [
        parse(&["audit", "--timeout-seconds", "41"]),
        parse(&["plan", "Cargo.toml", "--timeout-seconds", "42"]),
        parse(&[
            "implementation-audit",
            "Cargo.toml",
            "--timeout-seconds",
            "43",
        ]),
        parse(&["review", "implementation", ".", "--timeout-seconds", "44"]),
        parse(&["fix-loop", "implementation", ".", "--timeout-seconds", "45"]),
    ];

    for (expected_timeout, result) in [41_u64, 42, 43, 44, 45].into_iter().zip(cases) {
        let command = result.expect("parse service command");
        let dispatch = command
            .service()
            .expect("la commande doit etre une commande service");
        let timeout = dispatch.timeout();

        assert_eq!(timeout, Duration::from_secs(expected_timeout));
    }
}

#[test]
fn every_registered_subcommand_routes_help() {
    for spec in app::registered_commands() {
        let command = vec![spec.name, "--help"];
        let result = parse(&command);
        assert_eq!(
            result,
            Err(ParseOutcome::Help),
            "route help manquante pour {}",
            spec.name
        );

        for alias in spec.aliases {
            let alias_command = vec![*alias, "--help"];
            let result = parse(&alias_command);
            assert_eq!(
                result,
                Err(ParseOutcome::Help),
                "route help manquante pour {}",
                alias
            );
        }
    }
}

#[test]
fn registered_aliases_resolve_to_expected_command_variants() {
    let impl_alias = parse(&["impl-audit", "Cargo.toml"]).expect("alias impl-audit");
    assert!(impl_alias.as_implementation_audit().is_some());

    let loop_alias = parse(&["loop", "implementation", "."]).expect("alias loop");
    assert!(loop_alias.as_fix_loop().is_some());
}

#[test]
fn every_registered_subcommand_parses_to_its_expected_variant() {
    let cases = [
        ("audit", vec!["audit".to_string()]),
        ("plan", vec!["plan".to_string(), "Cargo.toml".to_string()]),
        (
            "implementation-audit",
            vec!["implementation-audit".to_string(), "Cargo.toml".to_string()],
        ),
        (
            "review",
            vec![
                "review".to_string(),
                "implementation".to_string(),
                ".".to_string(),
            ],
        ),
        (
            "fix-loop",
            vec![
                "fix-loop".to_string(),
                "implementation".to_string(),
                ".".to_string(),
            ],
        ),
    ];

    for (expected, args) in cases {
        let command = parse(&args.iter().map(String::as_str).collect::<Vec<_>>())
            .expect("commande enregistree");
        assert_eq!(command_kind(&command), expected);
    }
}

#[test]
fn rejects_duplicate_plan_output_dir_option() {
    let error = parse(&[
        "plan",
        "Cargo.toml",
        "--output-dir",
        ".plan",
        "--output-dir",
        ".other-plan",
    ])
    .expect_err("plan ne doit pas accepter deux dossiers de sortie");

    assert_eq!(
        error,
        ParseOutcome::Error("l'option --output-dir a deja ete fournie".to_string())
    );
}

#[test]
fn rejects_duplicate_implementation_audit_timeout_option() {
    let error = parse(&[
        "implementation-audit",
        "Cargo.toml",
        "--timeout-seconds",
        "42",
        "--timeout-seconds",
        "84",
    ])
    .expect_err("implementation-audit ne doit pas accepter deux timeouts");

    assert_eq!(
        error,
        ParseOutcome::Error("l'option --timeout-seconds a deja ete fournie".to_string())
    );
}

#[test]
fn parses_review_with_named_type_and_positional_artifact() {
    let command = parse(&["review", "--type", "implementation", "."])
        .expect("review doit accepter type nomme et artefact positionnel");
    let review = command
        .as_review()
        .expect("la commande attendue est review");

    assert_eq!(review.subject, app::ReviewSubject::Implementation);
    assert_eq!(
        normalize_path(&review.artifact_path),
        normalize_path(&review.workspace_root)
    );
}

#[test]
fn rejects_unknown_option_for_fix_loop() {
    let error = parse(&["fix-loop", "audit", "Cargo.toml", "--bogus"])
        .expect_err("fix-loop doit rejeter les options inconnues");

    assert_eq!(
        error,
        ParseOutcome::Error("option inconnue: --bogus".to_string())
    );
}

#[test]
fn rejects_duplicate_fix_loop_type_argument() {
    let error = parse(&[
        "fix-loop",
        "--type",
        "plan",
        "--type",
        "audit",
        "Cargo.toml",
    ])
    .expect_err("fix-loop ne doit pas accepter deux types");

    assert_eq!(
        error,
        ParseOutcome::Error("le type de fix-loop a deja ete fourni".to_string())
    );
}

#[test]
fn rejects_duplicate_fix_loop_artifact_argument() {
    let error = parse(&[
        "fix-loop",
        "--type",
        "plan",
        "--artifact",
        "Cargo.toml",
        "--artifact",
        "README.md",
    ])
    .expect_err("fix-loop ne doit pas accepter deux artefacts");

    assert_eq!(
        error,
        ParseOutcome::Error("l'artefact de fix-loop a deja ete fourni".to_string())
    );
}

#[test]
fn rejects_invalid_fix_loop_input_kind_with_command_specific_error() {
    let error = parse(&["fix-loop", "design", "Cargo.toml"]).expect_err("type invalide");

    assert_eq!(
        error,
        ParseOutcome::Error(
            "type d'entree fix-loop invalide: design. Valeurs attendues: plan, audit, implementation"
                .to_string()
        )
    );
}
