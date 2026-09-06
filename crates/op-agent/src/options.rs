use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{session::SessionId, usage::Usage};

#[derive(Debug, Clone)]
pub struct SessionOptions {
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub effort: Option<Effort>,
    pub instructions: Option<String>,
    pub permissions: Permissions,
    pub mcp: McpPolicy,
    pub persistence: Persistence,
    pub resume: Option<SessionId>,
    pub budget: Budget,
}

impl SessionOptions {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            model: None,
            effort: None,
            instructions: None,
            permissions: Permissions::default(),
            mcp: McpPolicy::default(),
            persistence: Persistence::default(),
            resume: None,
            budget: Budget::default(),
        }
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn effort(mut self, effort: Effort) -> Self {
        self.effort = Some(effort);
        self
    }

    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    pub fn permissions(mut self, permissions: Permissions) -> Self {
        self.permissions = permissions;
        self
    }

    pub fn mcp(mut self, mcp: McpPolicy) -> Self {
        self.mcp = mcp;
        self
    }

    pub fn persistence(mut self, persistence: Persistence) -> Self {
        self.persistence = persistence;
        self
    }

    pub fn resume(mut self, session: SessionId) -> Self {
        self.resume = Some(session);
        self
    }

    pub fn budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
    Low,
    Medium,
    High,
}

impl Effort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permissions {
    #[default]
    Ask,
    AcceptEdits,
    Full,
}

// Every MCP server puts its tool schemas in the prompt of every request of the session, so a
// session that needs none is markedly cheaper with them off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpPolicy {
    #[default]
    Inherit,
    Disabled,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Persistence {
    #[default]
    Stored,
    Ephemeral,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    pub max_cost_usd: Option<f64>,
    pub max_tokens: Option<u64>,
}

impl Budget {
    pub fn max_cost_usd(mut self, limit: f64) -> Self {
        self.max_cost_usd = Some(limit);
        self
    }

    pub fn max_tokens(mut self, limit: u64) -> Self {
        self.max_tokens = Some(limit);
        self
    }

    pub fn exhausted_by(&self, usage: &Usage) -> bool {
        let over_cost = matches!((self.max_cost_usd, usage.cost_usd), (Some(limit), Some(spent)) if spent >= limit);
        let over_tokens = matches!(self.max_tokens, Some(limit) if usage.total_tokens() >= limit);
        over_cost || over_tokens
    }
}
