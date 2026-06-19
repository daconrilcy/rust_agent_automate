# rust_agent

Workspace Cargo minimal pour demarrer un programme Rust.

Le crate `app` contient un premier module Rust capable de lancer `codex` en terminal en configurant:
- le modele via `--model`
- le niveau de raisonnement via `--reasoning` (`low`, `medium`, `high`)
- le mode via `--mode` (`interactive` ou `exec`)
- la verbosite via `--verbose` pour voir la sortie brute de `codex exec`
- un audit Rust via le skill Codex central `rust-refactor-audit`, avec `--target` pour choisir le dossier a auditer et sauvegarde du rapport dans `.audit`
- un plan d'integration depuis un audit via le skill Codex central `refactor-plan-from-audit`, avec sauvegarde du plan dans `.plan`
- un audit d'implementation depuis un plan via le skill Codex central `rust-implementation-plan-audit`, avec sauvegarde du rapport dans `.audit`
- une review adversariale via le skill Codex central `adversarial-review`, en precisant si l'entree est un `plan`, un `audit` ou une `implementation`, avec sauvegarde dans `.review`
- une boucle review/correction via le skill Codex central `rust-review-fix-loop`, en partant d'un `audit`, d'un `plan` ou d'une `implementation`, avec sauvegarde du rapport dans `.fix-loop`

## Commandes utiles

```powershell
cargo run -p app
cargo check
cargo test
cargo run -q -p app -- audit
cargo run -q -p app -- audit --target ..\mon-projet
cargo run -q -p app -- audit --timeout-seconds 120
cargo run -q -p app -- plan .audit\audit-1781887189.md
cargo run -q -p app -- implementation-audit .plan\plan-1781894465.md
cargo run -q -p app -- implementation-audit --plan .plan\plan-1781894465.md --implementation crates\app
cargo run -q -p app -- impl-audit .plan\plan-1781894465.md --implementation crates\app
cargo run -q -p app -- review plan .plan\plan-1781894465.md
cargo run -q -p app -- review audit .audit\audit-1781887189.md
cargo run -q -p app -- review implementation crates\app
cargo run -q -p app -- fix-loop plan .plan\plan-1781894465.md
cargo run -q -p app -- fix-loop audit .audit\audit-1781887189.md
cargo run -q -p app -- fix-loop implementation crates\app
```

## Exemples

```powershell
cargo run -q -p app -- --model gpt-5.4 --reasoning high
cargo run -q -p app -- --mode exec --model gpt-5.4 --reasoning low "Resume ce projet"
cargo run -q -p app -- --mode exec --verbose --model gpt-5.4 --reasoning low "Resume ce projet"
cargo run -q -p app -- audit
cargo run -q -p app -- audit --target ..\mon-projet
cargo run -q -p app -- audit --verbose
cargo run -q -p app -- audit --timeout-seconds 120
cargo run -q -p app -- plan .audit\audit-1781887189.md
cargo run -q -p app -- plan --audit .audit\audit-1781887189.md --output-dir .plan
cargo run -q -p app -- implementation-audit .plan\plan-1781894465.md --timeout-seconds 900
cargo run -q -p app -- impl-audit --plan .plan\plan-1781894465.md --implementation crates\app --output-dir .audit
cargo run -q -p app -- review plan .plan\plan-1781894465.md
cargo run -q -p app -- review --type audit --artifact .audit\audit-1781887189.md --output-dir .review
cargo run -q -p app -- review implementation crates\app --timeout-seconds 120
cargo run -q -p app -- fix-loop plan .plan\plan-1781894465.md --timeout-seconds 1800
cargo run -q -p app -- loop --type implementation --artifact crates\app --output-dir .fix-loop
```
