# Railguard

## Purpose And Scope

This document defines guardrails for the `rust_agent` Cargo workspace. It applies to the `crates/app` binary crate, the default automation workflow in `workflows/refactor.json`, and the repository-level scripts documented in `README.md`.

## Project Map

- `crates/app/src/cli.rs`: top-level CLI parsing, help rendering, command execution, and process exit behavior.
- `crates/app/src/service_command/`: shared service-command parsing, prompt preparation, dispatch, execution, and artifact persistence.
- `crates/app/src/automate/`: JSON workflow parsing, placeholder expansion, multi-step execution, loop control, and artifact reuse.
- `crates/app/src/codex/`: Codex CLI discovery, request modeling, command construction, and exec output capture.
- `crates/app/src/reporting/` and `crates/app/src/artifact.rs`: structured command outcomes, final report detection, and timestamped Markdown artifacts.
- `workflows/refactor.json`: embedded default refactor workflow, included by `crates/app/src/automate/workflow_model.rs`.
- `crates/app/tests/`: behavior-level CLI, parser, service, and workflow integration tests.

## Non-Negotiable Invariants

- Registered service commands must be added through `ServiceCommandSpec` and `SERVICE_COMMAND_SPECS`; CLI routing depends on `command_registry` delegating to those specs. Evidence: `crates/app/src/service_command/spec.rs`, `crates/app/src/command_registry.rs`, `crates/app/tests/service_parser.rs`.
- Service commands that produce artifacts must return a structured `CommandOutcome` so automation can persist and reuse artifact paths. Evidence: `crates/app/src/reporting/`, `crates/app/src/automate/step_outcome.rs`, `crates/app/tests/workflow_chain.rs`.
- `{artifact:<step>}` placeholders may only reference earlier structured service steps, never direct Codex runs. Evidence: `crates/app/src/automate/workflow_model.rs`, `crates/app/tests/workflow_model.rs`.
- `refactor-automate` writes reports to the launch workspace, not into a nested target directory. Evidence: `crates/app/src/cli.rs`, `crates/app/src/automate/step_outcome.rs`, `crates/app/tests/workflow_chain.rs`.
- The default refactor workflow must make plan, implementation, and review/correction phases consult and update a railguard document for the target workspace. Evidence: `workflows/refactor.json`, `crates/app/src/plan.rs`, `crates/app/src/fix_loop.rs`, `crates/app/tests/workflow_model.rs`, `crates/app/tests/plan_cli.rs`, `crates/app/tests/fix_loop_cli.rs`.

## Architecture Boundaries

- Keep top-level option parsing in `cli.rs`; subcommand-specific parsing belongs in `service_command` or the relevant command module. Evidence: `crates/app/src/cli.rs`, `crates/app/src/service_command/args.rs`.
- Keep workflow JSON validation in `automate/workflow_model.rs`; runtime command construction belongs in `automate/step_args.rs` and `automate/step_outcome.rs`. Evidence: `crates/app/src/automate/workflow_model.rs`, `crates/app/src/automate/step_args.rs`.
- Keep Codex process mechanics isolated in `codex/`; service modules should build prompts and options, not spawn processes directly. Evidence: `crates/app/src/codex.rs`, `crates/app/src/service_command/exec.rs`.
- Do not duplicate timestamped artifact writing outside `artifact.rs` and `service_command::save_markdown_artifact`. Evidence: `crates/app/src/artifact.rs`, `crates/app/src/service_command/exec.rs`.

## Rust Rules

- This workspace is currently a single binary crate under a Cargo workspace; preserve the workspace shape unless a refactor plan explicitly changes it. Evidence: `Cargo.toml`, `crates/app/Cargo.toml`.
- Avoid `unsafe` unless the user explicitly approves it and the safety contract is documented next to the code. Evidence: no current unsafe policy or unsafe code is documented in `README.md`.
- Keep public re-exports in `crates/app/src/lib.rs` minimal and test-driven; the README states they are not a stable external API contract.
- Preserve the existing explicit error enums and user-facing French error messages for CLI/workflow failures. Evidence: `crates/app/src/automate/parse.rs`, `crates/app/src/service_paths.rs`, `crates/app/tests/*_cli.rs`.

## Testing And Verification

Run focused checks for changed behavior first, for example:

```powershell
cargo test --test plan_cli
cargo test --test workflow_model
cargo test --test workflow_chain
```

Before finalizing broader changes, run the repository verification script:

```powershell
.\verify.ps1
```

`verify.ps1` sets `CARGO_TARGET_DIR` to `.target-verify`, then runs `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, selected integration tests, and the full test suite.

## Change Protocol

Before editing workflow or command behavior:

- inspect `workflows/refactor.json`
- inspect the corresponding command module in `crates/app/src/`
- inspect the behavior-level tests under `crates/app/tests/`

During editing:

- keep workflow semantics observable through tests
- preserve unrelated user changes and generated audit/plan artifacts
- update this file when a new invariant or workflow boundary is introduced

Before handing off:

- run the smallest relevant tests
- report skipped checks with the reason
- list any unresolved assumptions about Codex CLI behavior or external skill prompts

## Known Risks And Open Questions

- Direct Codex workflow steps do not produce structured artifacts, so railguard updates are enforced by prompt contract rather than an artifact path recorded in `AutomateReport`.
- The railguard skill can create or update files in the target workspace only when the active Codex permissions allow writing there.
