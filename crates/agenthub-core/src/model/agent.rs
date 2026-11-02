use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::l10n::{self, Msg, t};

/// agent 身份。内置 5 个 + 用户自定义平台。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentId {
    Claude,
    Codex,
    CodeBuddy,
    Hermes,
    Gemini,
    /// OpenCode：自己的 skills 目录通常为空，实际从 `.agents/skills` 与
    /// `.claude/skills` 兼容读取。
    Opencode,
    Custom(String),
}

impl Serialize for AgentId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.slug())
    }
}

impl<'de> Deserialize<'de> for AgentId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Ok(AgentId::from_slug(&raw))
    }
}

impl AgentId {
    pub fn from_slug(raw: &str) -> AgentId {
        match raw {
            "claude" => AgentId::Claude,
            "codex" => AgentId::Codex,
            "codebuddy" | "code_buddy" => AgentId::CodeBuddy,
            "hermes" => AgentId::Hermes,
            "gemini" => AgentId::Gemini,
            "opencode" => AgentId::Opencode,
            other => AgentId::Custom(other.trim_start_matches("custom:").to_string()),
        }
    }

    pub fn slug(&self) -> String {
        match self {
            AgentId::Claude => "claude".into(),
            AgentId::Codex => "codex".into(),
            AgentId::CodeBuddy => "codebuddy".into(),
            AgentId::Hermes => "hermes".into(),
            AgentId::Gemini => "gemini".into(),
            AgentId::Opencode => "opencode".into(),
            AgentId::Custom(name) => format!("custom:{name}"),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            AgentId::Claude => "Claude Code".into(),
            AgentId::Codex => "Codex".into(),
            AgentId::CodeBuddy => "CodeBuddy".into(),
            AgentId::Hermes => "Hermes".into(),
            AgentId::Gemini => "Gemini CLI".into(),
            AgentId::Opencode => "OpenCode".into(),
            AgentId::Custom(name) => name.clone(),
        }
    }

    pub fn short(&self) -> &'static str {
        match self {
            AgentId::Claude => "CC",
            AgentId::Codex => "CX",
            AgentId::CodeBuddy => "CB",
            AgentId::Hermes => "HM",
            AgentId::Gemini => "GM",
            AgentId::Opencode => "OC",
            AgentId::Custom(_) => "CU",
        }
    }

    pub fn builtin() -> Vec<AgentId> {
        vec![
            AgentId::Claude,
            AgentId::Codex,
            AgentId::CodeBuddy,
            AgentId::Hermes,
            AgentId::Gemini,
            AgentId::Opencode,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub skills: bool,
    pub mcp: bool,
    pub instructions: bool,
    pub project_scope: bool,
    pub links: bool,
}

impl Capabilities {
    pub fn full() -> Self {
        Self {
            skills: true,
            mcp: true,
            instructions: true,
            project_scope: true,
            links: true,
        }
    }

    pub fn skills_only() -> Self {
        Self {
            mcp: false,
            instructions: false,
            project_scope: false,
            ..Self::full()
        }
    }
}

/// 用户级 / 项目级作用域。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    User,
    Project { root: PathBuf },
}

impl Scope {
    pub fn is_user(&self) -> bool {
        matches!(self, Scope::User)
    }

    pub fn root(&self) -> Option<&PathBuf> {
        match self {
            Scope::User => None,
            Scope::Project { root } => Some(root),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Scope::User => l10n::str_of(Msg::ScopeGlobal).to_string(),
            Scope::Project { root } => t!(
                Msg::ScopeProject,
                name = root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallSource {
    Builtin,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInstall {
    pub id: AgentId,
    pub display_name: String,
    pub config_dir: Option<PathBuf>,
    pub cli_path: Option<PathBuf>,
    pub capabilities: Capabilities,
    pub source: InstallSource,
    pub detected: bool,
}

/// 一个可写入的技能目标目录（某 agent 的某个 skills 目录）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTarget {
    pub agent: AgentId,
    pub dir: PathBuf,
    pub readonly: bool,
    pub scope: Scope,
    pub exists: bool,
}

impl SkillTarget {
    pub fn new(agent: AgentId, dir: PathBuf, scope: Scope) -> Self {
        let exists = dir.is_dir();
        Self {
            agent,
            dir,
            readonly: false,
            scope,
            exists,
        }
    }

    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }
}
