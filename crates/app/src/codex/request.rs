use std::fmt;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

impl<'de> Deserialize<'de> for ReasoningEffort {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

pub const DEFAULT_MODEL: &str = "gpt-5.4";
pub const DEFAULT_REASONING_EFFORT: ReasoningEffort = ReasoningEffort::Low;

impl ReasoningEffort {
    pub fn as_config_value(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl fmt::Display for ReasoningEffort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_config_value())
    }
}

impl std::str::FromStr for ReasoningEffort {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => Err(format!(
                "niveau de raisonnement invalide: {value}. Valeurs attendues: low, medium, high"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexMode {
    Interactive,
    Exec,
}

impl std::str::FromStr for CodexMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "interactive" => Ok(Self::Interactive),
            "exec" => Ok(Self::Exec),
            _ => Err(format!(
                "mode invalide: {value}. Valeurs attendues: interactive, exec"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl SandboxMode {
    pub fn as_cli_value(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
        }
    }
}

impl fmt::Display for SandboxMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_cli_value())
    }
}

impl std::str::FromStr for SandboxMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "read-only" => Ok(Self::ReadOnly),
            "workspace-write" => Ok(Self::WorkspaceWrite),
            "danger-full-access" => Ok(Self::DangerFullAccess),
            _ => Err(format!(
                "mode sandbox invalide: {value}. Valeurs attendues: read-only, workspace-write, danger-full-access"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalPolicy {
    Untrusted,
    OnFailure,
    OnRequest,
    Never,
}

impl ApprovalPolicy {
    pub fn as_cli_value(self) -> &'static str {
        match self {
            Self::Untrusted => "untrusted",
            Self::OnFailure => "on-failure",
            Self::OnRequest => "on-request",
            Self::Never => "never",
        }
    }
}

impl fmt::Display for ApprovalPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_cli_value())
    }
}

impl std::str::FromStr for ApprovalPolicy {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "untrusted" => Ok(Self::Untrusted),
            "on-failure" => Ok(Self::OnFailure),
            "on-request" => Ok(Self::OnRequest),
            "never" => Ok(Self::Never),
            _ => Err(format!(
                "politique d'approbation invalide: {value}. Valeurs attendues: untrusted, on-failure, on-request, never"
            )),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentPermissions {
    pub sandbox: Option<SandboxMode>,
    pub approval_policy: Option<ApprovalPolicy>,
    pub additional_writable_dirs: Vec<PathBuf>,
    pub bypass_approvals_and_sandbox: bool,
}

impl AgentPermissions {
    pub fn cli_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        if self.bypass_approvals_and_sandbox {
            args.push("--dangerously-bypass-approvals-and-sandbox".to_string());
            return args;
        }

        if let Some(sandbox) = self.sandbox {
            args.push("--sandbox".to_string());
            args.push(sandbox.to_string());
        }

        if let Some(policy) = self.approval_policy {
            args.push("--ask-for-approval".to_string());
            args.push(policy.to_string());
        }

        for dir in &self.additional_writable_dirs {
            args.push("--add-dir".to_string());
            args.push(dir.display().to_string());
        }

        args
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentContext {
    pub solo: bool,
    pub windows_only: bool,
    pub portability: bool,
    pub docker: bool,
}

impl Default for AgentContext {
    fn default() -> Self {
        Self {
            solo: true,
            windows_only: true,
            portability: false,
            docker: false,
        }
    }
}

impl AgentContext {
    pub fn enable_solo(&mut self) {
        self.solo = true;
    }

    pub fn enable_team(&mut self) {
        self.solo = false;
    }

    pub fn enable_windows_only(&mut self) {
        self.windows_only = true;
        self.portability = false;
    }

    pub fn enable_portability(&mut self) {
        self.windows_only = false;
        self.portability = true;
    }

    pub fn enable_docker(&mut self) {
        self.docker = true;
    }

    pub fn apply_cli_flag(&mut self, value: &str) -> bool {
        match value {
            "--solo" | "-solo" => {
                self.enable_solo();
                true
            }
            "--team" | "-team" => {
                self.enable_team();
                true
            }
            "--windows-only" | "-windows-only" => {
                self.enable_windows_only();
                true
            }
            "--portable" | "-portable" | "--portability" | "-portability" => {
                self.enable_portability();
                true
            }
            "--docker" | "-docker" => {
                self.enable_docker();
                true
            }
            _ => false,
        }
    }

    pub fn cli_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if !self.solo {
            args.push("--team".to_string());
        }
        if self.portability || !self.windows_only {
            args.push("--portable".to_string());
        }
        if self.docker {
            args.push("--docker".to_string());
        }
        args
    }

    pub fn apply_to_prompt(&self, prompt: &str) -> String {
        format!("{}\n\n{}", self.prompt_context(), prompt)
    }

    fn prompt_context(&self) -> String {
        let collaboration = if self.solo {
            "developpement solo avec agents; ne pas supposer une equipe de developpeurs, une gouvernance de PR, ou une CI distante sauf demande explicite"
        } else {
            "developpement en equipe; prendre en compte collaboration, revues, CI partagee et contrats plus explicites"
        };
        let platform = if self.windows_only {
            "application locale Windows-only; ne pas traiter la portabilite Linux/macOS comme un objectif par defaut"
        } else {
            "portabilite demandee; prendre en compte Windows, Linux/macOS si pertinent, et eviter les hypotheses Windows-only"
        };
        let docker = if self.docker {
            "\n- Docker/containerisation demandee; inclure les contraintes de build, execution et verification en conteneur."
        } else {
            ""
        };

        format!("Contexte d'execution a respecter:\n- {collaboration}.\n- {platform}.{docker}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexRequest {
    pub model: String,
    pub reasoning_effort: ReasoningEffort,
    pub mode: CodexMode,
    pub prompt: Option<String>,
    pub verbose: bool,
    pub resume_last: bool,
    pub working_dir: Option<PathBuf>,
    pub agent_context: AgentContext,
    pub permissions: AgentPermissions,
}

impl CodexRequest {
    pub fn new(
        model: impl Into<String>,
        reasoning_effort: ReasoningEffort,
        mode: CodexMode,
        prompt: Option<String>,
        verbose: bool,
    ) -> Self {
        Self {
            model: model.into(),
            reasoning_effort,
            mode,
            prompt,
            verbose,
            resume_last: false,
            working_dir: None,
            agent_context: AgentContext::default(),
            permissions: AgentPermissions::default(),
        }
    }

    pub fn with_resume_last(mut self, resume_last: bool) -> Self {
        self.resume_last = resume_last;
        self
    }

    pub fn with_working_dir(mut self, working_dir: PathBuf) -> Self {
        self.working_dir = Some(working_dir);
        self
    }

    pub fn with_agent_context(mut self, agent_context: AgentContext) -> Self {
        self.agent_context = agent_context;
        self
    }

    pub fn with_permissions(mut self, permissions: AgentPermissions) -> Self {
        self.permissions = permissions;
        self
    }

    pub fn prompt_with_agent_context(&self) -> Option<String> {
        self.prompt
            .as_deref()
            .map(|prompt| self.agent_context.apply_to_prompt(prompt))
    }
}
