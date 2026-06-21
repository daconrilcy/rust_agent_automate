# rust_agent

Workspace Cargo minimal pour demarrer un programme Rust.

Le crate `app` contient un premier module Rust capable de lancer `codex` en terminal en configurant:
- le modele via `--model` (`gpt-5.4` par defaut)
- le niveau de raisonnement via `--reasoning` (`low` par defaut, valeurs possibles: `low`, `medium`, `high`)
- le mode via `--mode` (`interactive` ou `exec`)
- la verbosite via `--verbose` pour voir la sortie brute de `codex exec`
- les droits d'execution de l'agent Codex via `--sandbox`, `--ask-for-approval`, `--add-dir` et, en dernier recours seulement, `--dangerously-bypass-approvals-and-sandbox`
- le contexte agentique par defaut: developpement solo, local, Windows-only, sans objectif de portabilite implicite
- des extensions de contexte via `--team`, `--portable`/`--portability`, et `--docker` quand une commande doit raisonner pour une equipe, une cible portable ou une execution conteneurisee
- un audit Rust via le skill Codex central `rust-refactor-audit`, avec `--target` pour choisir le dossier a auditer et sauvegarde du rapport dans `.audit`
- un plan d'integration depuis un audit via le skill Codex central `refactor-plan-from-audit`, avec sauvegarde du plan dans `.plan`
- un audit d'implementation depuis un plan via le skill Codex central `rust-implementation-plan-audit`, avec sauvegarde du rapport dans `.audit`
- une review adversariale via le skill Codex central `adversarial-review`, en precisant si l'entree est un `plan`, un `audit` ou une `implementation`, avec sauvegarde dans `.review`
- une boucle review/correction via le skill Codex central `rust-review-fix-loop`, en partant d'un `audit`, d'un `plan` ou d'une `implementation`, avec sauvegarde du rapport dans `.fix-loop`
- un automate JSON via `automate`, capable d'enchainer les services Rust du binaire courant
- un automate de refactoring via `refactor-automate`, qui cible un dossier donne ou le workspace local par defaut

## Commandes utiles

```powershell
cargo run -p app
cargo check
.\verify.ps1
.\install.ps1
cargo run -q -p app -- audit
cargo run -q -p app -- audit --target ..\mon-projet
cargo run -q -p app -- audit --timeout-seconds 120
cargo run -q -p app -- audit --team --portable
cargo run -q -p app -- refactor-automate --target crates\app --docker "Durcir le workflow local"
cargo run -q -p app -- refactor-automate --target ..\astral_calculator --sandbox danger-full-access --ask-for-approval never "Refactorer sans blocage sandbox local"
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
cargo run -q -p app -- automate .\workflow.json "Objectif initial"
cargo run -q -p app -- refactor-automate --target crates\app "Refactoring SOLID/KISS/DRY"
cargo run -q -p app -- refactor-automate --target ..\mon-projet --sandbox danger-full-access --ask-for-approval never
```

## Verification locale

L'application charge automatiquement un fichier `.env` depuis le repertoire de lancement. Copie `.env.example` vers `.env` pour configurer les chemins locaux utiles a l'application, par exemple `CODEX_CLI_PATH`.

## Droits de l'agent Codex

Les commandes qui lancent Codex acceptent les options de droits suivantes et les transmettent au CLI Codex:

```powershell
rust_agent audit --target ..\mon-projet --sandbox workspace-write --ask-for-approval on-request
rust_agent fix-loop audit .audit\implementation-audit.md --sandbox danger-full-access --ask-for-approval never
rust_agent refactor-automate --target ..\mon-projet --sandbox danger-full-access --ask-for-approval never
rust_agent refactor-automate --target ..\mon-projet --add-dir C:\dev\shared
```

Valeurs supportees:
- `--sandbox read-only|workspace-write|danger-full-access`
- `--ask-for-approval untrusted|on-failure|on-request|never`
- `--add-dir <chemin>` repetable pour rendre d'autres dossiers accessibles en ecriture
- `--dangerously-bypass-approvals-and-sandbox` pour executer sans prompts ni sandbox, uniquement si l'environnement externe est deja isole

Pour un refactor local autonome sur Windows, le profil pratique est:

```powershell
rust_agent refactor-automate --target ..\mon-projet --sandbox danger-full-access --ask-for-approval never
```

Sur cette machine, `cargo test` peut echouer en cible par defaut si `target\debug\app.exe` reste verrouille. La procedure fiable est donc versionnee dans [verify.ps1](verify.ps1): le script fixe `CARGO_TARGET_DIR` vers `.target-verify`, puis lance la sequence Windows complete.

```powershell
.\verify.ps1
```

Sequence executee par le script:

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --test service_parser
cargo test --test workflow_chain
cargo test
```

## Installation locale

Le script [install.ps1](install.ps1) compile le binaire en release, copie l'executable dans `%LOCALAPPDATA%\Programs\rust_agent`, puis ajoute ce dossier au `PATH` utilisateur.

```powershell
.\install.ps1
rust_agent --help
```

Par defaut, Cargo produit `app.exe`; le script l'installe comme `rust_agent.exe`. Pour changer le nom de commande ou le dossier d'installation:

```powershell
.\install.ps1 -CommandName app -InstallRoot "$env:LOCALAPPDATA\Programs"
```

## Note d'architecture

Le crate `app` reste volontairement mono-crate pour cette passe, avec des frontieres de responsabilite explicites:
- `cli` / interface: parsing top-level, aide et codes de sortie
- `service_command` / application: definitions de commandes de service, options communes, preparation des prompts et dispatch
- `automate` / application: orchestration des workflows et boucle multi-etapes
- `codex`, `reporting`, `artifact`, `service_paths` / infra: execution externe, transport de rapports, persistance et resolution de chemins

Les modules exportes par `src/lib.rs` servent d'abord au binaire et aux tests d'integration. Ils ne doivent pas etre traites comme une promesse d'API publique stable tant qu'un contrat plus strict n'est pas documente.

## Politique de tests inline

Les tests de scenario, CLI et workflow doivent vivre sous `crates/app/tests/`. Les `#[cfg(test)]` conserves dans `src/` sont des exceptions reservees a de petites invariantes privees, par exemple:
- conversions de statuts ou mappages d'erreurs locaux
- helpers de rendu de prompt ou de transport difficiles a verifier uniquement via l'API publique
- petites verifications de parsing prive quand exposer un detail n'est pas justifie

Si un test inline commence a couvrir un flux multi-etapes, un comportement CLI observable, une resolution de chemins, ou un cas d'integration Codex, il doit etre deplace vers `crates/app/tests/`.

## Workflow JSON d'automate

Un workflow definit des etapes qui lancent les commandes Rust du binaire courant.

```json
{
  "defaults": {
    "model": null,
    "reasoning": null,
    "timeout_seconds": 1800
  },
  "steps": [
    {
      "name": "audit",
      "rust_command": ["audit", "--target", "{target}"],
      "model": null,
      "reasoning": null,
      "fresh_codex_call": true
    },
    {
      "name": "plan",
      "rust_command": ["plan", "{artifact:audit}"],
      "fresh_codex_call": true
    }
  ]
}
```

Champs d'etape:
- `rust_command`: arguments passes au binaire courant, par exemple `["audit", "--target", "{target}"]`
- `model`: modele Codex de l'etape, ou `null` pour le modele par defaut
- `reasoning`: `low`, `medium`, `high`, ou `null` pour le reasoning par defaut
- `fresh_codex_call`: indique si l'etape repart d'un appel Codex vierge ou depend du contexte precedent

Placeholders disponibles: `{initial_prompt}`, `{target}`, `{cycle}`, `{last_artifact}`, `{last_output}`, `{artifact:<nom_etape>}`. Les references `{artifact:<nom_etape>}` doivent viser une etape precedente qui produit un resultat structure.

`refactor-automate` embarque le workflow [workflows/refactor.json](workflows/refactor.json): audit, plan, implementation, review/corrections adversariales par `fix-loop`, audit d'alignement avec le plan initial, corrections, puis commit/push. Le cycle peut se repeter uniquement si l'etape `alignment_audit` ne publie pas un statut structure `clean: true`.

## Exemples

```powershell
cargo run -q -p app -- --model gpt-5.4 --reasoning low
cargo run -q -p app -- --mode exec --model gpt-5.4 --reasoning low "Resume ce projet"
cargo run -q -p app -- --mode exec --verbose --model gpt-5.4 --reasoning low "Resume ce projet"
cargo run -q -p app -- --mode exec --sandbox danger-full-access --ask-for-approval never "Resume ce projet"
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
