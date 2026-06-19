# Rust Implementation Plan Audit

## Scope
- Plan reviewed: `C:\dev\rust_agent\.plan\plan-1781906268.md`
- Implementation reviewed: current workspace changes inferred from `git status --short` and `git diff --stat`, limited to `crates/app/src/main.rs`, `crates/app/src/automate.rs`, and new `crates/app/src/command_registry.rs`
- Commands run: `git status --short` -> modified `crates/app/src/automate.rs`, `crates/app/src/main.rs`; untracked `.plan/plan-1781906268.md`, `.audit/audit-1781906077.md`, `.fix-loop/`, and `crates/app/src/command_registry.rs`
- Commands run: `git diff --stat` -> 2 tracked files changed, 138 insertions and 250 deletions; no manifest or workflow diff reported
- Commands run: `git diff -- crates/app/src/main.rs crates/app/src/automate.rs crates/app/src/command_registry.rs` -> added a central registry module, switched parsing/automation helpers to consult it, and extracted shared `run_codex_report` handling
- Commands run: `cargo metadata --no-deps --format-version 1` -> single-package workspace `app`; no package-boundary changes
- Commands run: `cargo test` -> passed, `61 passed; 0 failed`
- Commands run: `rg -n "run_workflow|audit_step|current_exe|enregistre dans|No actionable deviations found" crates/app/src/automate.rs crates/app/src/main.rs workflows -S` -> workflow still respawns current executable, still parses `" enregistre dans "`, and still uses `loop_policy.audit_step` plus text markers

## Summary
- 6 actionable deviations found.
- Highest-risk gap: workflow orchestration still depends on subprocess stdout scraping and Markdown clean markers, so the plan’s contract-hardening and structured-result phases are not implemented.

## Deviations

### D1 - Phase 1 workflow contract tests were not added
- Category: Untested
- Severity: High
- Plan requirement: `P1-T1` required focused workflow contract tests for artifact chaining, failure propagation, and clean-stop behavior around `run_workflow`.
- Evidence: `crates/app/src/automate.rs:151-222` contains the workflow logic being refactored, but the test block at `crates/app/src/automate.rs:406-520` only covers parsing, placeholder expansion, artifact extraction, and Codex option injection; there is no `run_workflow` contract test. `cargo test` passed with 61 tests, but none exercise multi-step workflow execution semantics.
- Impact: The plan explicitly sequenced refactoring behind boundary tests. Without those tests, regressions in artifact propagation, loop termination, or failure reporting can slip through while Phase 2+ changes land.
- Correction needed: Add focused tests that exercise `run_workflow` behavior for multi-step artifact chaining, step failure propagation, and clean-stop evaluation against `loop_policy.audit_step` without requiring live Codex execution.

### D2 - No execution seam was introduced for workflow testing
- Category: Missing
- Severity: High
- Plan requirement: `P1-T2` required a narrow execution seam so workflow tests could simulate step outputs while production still used the current executable.
- Evidence: `crates/app/src/automate.rs:255-269` still hard-codes `env::current_exe()` and `Command::new(current_exe)` inside `run_step`, with no injected executor, private seam, or `cfg(test)` override path visible nearby.
- Impact: The missing seam is why the Phase 1 contract tests are still absent; workflow behavior remains coupled to real process spawning, blocking the testability the plan called for before larger refactors.
- Correction needed: Extract `run_step` behind a small private helper or test-only injectable executor so tests can simulate step outputs and failure cases deterministically.

### D3 - Command registry work is only partially complete
- Category: Partial
- Severity: Medium
- Plan requirement: `P2-T1` and `P2-T2` required a single authoritative subcommand registry, with top-level recognition and help coverage driven from the same source.
- Evidence: `crates/app/src/command_registry.rs:1-102` adds central command metadata and helper functions, `crates/app/src/main.rs:73-90` now consults `command_registry::canonical_name`, and `crates/app/src/automate.rs:327-334` uses registry-backed command classification. But help output remains a hand-written string in `crates/app/src/main.rs:821-855`, and top-level dispatch still requires a hard-coded `match` over canonical names in `crates/app/src/main.rs:74-87`.
- Impact: The registry removes some drift, but command definitions are still mirrored in help text and dispatch structure. The plan’s “same command source” goal is not fully met.
- Correction needed: Drive help command listings and alias coverage from the registry, and reduce remaining manual subcommand mirroring so command metadata changes do not require parallel edits.

### D4 - Command lifecycle ownership was not moved out of `main.rs`
- Category: Missing
- Severity: High
- Plan requirement: `P3-T1` required per-command modules to own parse, validate, run, and save behavior, with `main.rs` reduced to thin dispatch.
- Evidence: `crates/app/src/main.rs:892-1049` still owns `run_audit`, `run_plan`, `run_implementation_audit`, `run_review`, `run_fix_loop`, `run_automate`, and `run_refactor_automate`. `crates/app/src/main.rs:73-700` still contains top-level parse logic for all commands. No new `audit` module was added; the only new module is `crates/app/src/command_registry.rs`.
- Impact: The central god-module concern identified in the plan remains substantially unresolved. Future command changes still have a broad blast radius in `main.rs`.
- Correction needed: Move command-specific parse/validate/run/save logic into their respective modules, add the planned audit ownership module, and leave `main.rs` as argument collection plus dispatch.

### D5 - Structured internal command outcomes were not introduced
- Category: Missing
- Severity: High
- Plan requirement: `P4-T1` required service commands to return a structured internal outcome so workflow automation could stop scraping human-facing output.
- Evidence: `crates/app/src/main.rs:1050-1095` centralizes human report saving/printing but returns no machine-readable command result. `crates/app/src/automate.rs:355-361` still extracts artifacts by splitting stdout on `" enregistre dans "`.
- Impact: Workflow correctness still depends on localized presentation text instead of an internal API boundary, which the plan explicitly called out as brittle.
- Correction needed: Introduce an internal command outcome containing at least command name, status, final-message presence, and artifact path; update workflow execution to consume that outcome instead of parsing display strings.

### D6 - Loop-stop logic still depends on audit prose and the workflow still respawns the binary
- Category: Contradiction
- Severity: High
- Plan requirement: `P4-T2` required explicit audit-clean status for loop termination and, conditionally, removal of recursive self-spawn once structured outcomes existed.
- Evidence: `crates/app/src/automate.rs:364-376` still reads the saved audit file and searches for marker text to decide whether the workflow is clean. `crates/app/src/automate.rs:255-269` still executes each workflow step by spawning `env::current_exe()`. `workflows/refactor.json:67` still depends on `audit_step` naming for loop-stop evaluation.
- Impact: The highest-risk architectural coupling from the plan remains in place: clean-stop behavior is tied to Markdown wording, and workflow execution remains process-coupled.
- Correction needed: Add explicit clean/not-clean status to the relevant audit outcome, make loop termination consume that status, and only then evaluate whether subprocess recursion can be safely removed.

## Plan Coverage

| Plan item | Status | Evidence |
| --- | --- | --- |
| `P1-T1` Add focused workflow contract tests for artifact chaining and loop-stop behavior | Untested | `crates/app/src/automate.rs:406-520` contains no `run_workflow` contract tests; `cargo test` passes without workflow-boundary coverage |
| `P1-T2` Introduce a narrow execution seam for workflow testing | Missing | `crates/app/src/automate.rs:255-269` still hard-codes `env::current_exe()` and `Command::new(...)` |
| `P2-T1` Create a single authoritative subcommand registry | Partial | `crates/app/src/command_registry.rs:1-102`; consumed by `crates/app/src/main.rs:73-90` and `crates/app/src/automate.rs:327-334`, but not the only source of command-facing data |
| `P2-T2` Derive help and top-level dispatch checks from the registry | Partial | Parse recognition now uses `command_registry::canonical_name` at `crates/app/src/main.rs:73-90`; help remains manual at `crates/app/src/main.rs:821-855` |
| `P3-T1` Extract command lifecycle ownership into per-command modules | Missing | `crates/app/src/main.rs:892-1049` still owns service-command run lifecycles; parse logic remains centralized above |
| `P3-T2` Consolidate repeated Codex final-message handling into shared command execution helpers | Implemented | Shared `run_codex_report` added at `crates/app/src/main.rs:1050-1095`; service commands delegate to it at `crates/app/src/main.rs:898-986` |
| `P4-T1` Introduce a structured internal command outcome for service commands | Missing | No internal result type added; `crates/app/src/automate.rs:355-361` still parses stdout for artifact paths |
| `P4-T2` Replace marker-based workflow loop-stop logic with explicit audit status, then optionally eliminate recursive self-spawn | Missing | Marker-based clean check remains at `crates/app/src/automate.rs:364-376`; subprocess recursion remains at `crates/app/src/automate.rs:255-269` |

## Residual Risk
- The audit scope was inferred from the dirty worktree because no explicit diff range or commit range was provided.
- `cargo test` passed, but no targeted workflow boundary tests exist yet, so the most brittle logic remains weakly verified.
- I did not run `cargo run -q -p app -- --help`; help coverage was judged from source at `crates/app/src/main.rs:821-855`.
- I did not run live Codex-backed service commands, so behavior depending on external Codex execution remains locally unverifiable beyond static inspection.