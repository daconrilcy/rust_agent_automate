use std::fmt;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexRequest {
    pub model: String,
    pub reasoning_effort: ReasoningEffort,
    pub mode: CodexMode,
    pub prompt: Option<String>,
    pub verbose: bool,
    pub resume_last: bool,
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
        }
    }

    pub fn with_resume_last(mut self, resume_last: bool) -> Self {
        self.resume_last = resume_last;
        self
    }
}
