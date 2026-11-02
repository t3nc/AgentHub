use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::agent::AgentId;
use crate::l10n::{self, Msg};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionKind {
    /// 全局规则（AGENTS.md / CLAUDE.md 等）
    Rule,
    /// 人格 / 用户画像（SOUL.md、USER.md）
    Persona,
    /// 桥接文件（由 AgentHub 生成）
    Bridge,
}

impl InstructionKind {
    pub fn label(&self) -> &'static str {
        l10n::str_of(match self {
            InstructionKind::Rule => Msg::InstrRule,
            InstructionKind::Persona => Msg::InstrPersona,
            InstructionKind::Bridge => Msg::InstrBridge,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionSpec {
    pub agent: AgentId,
    pub path: PathBuf,
    pub kind: InstructionKind,
    pub exists: bool,
    /// 是否已含 AgentHub 受控区块
    pub managed: bool,
    pub size_bytes: u64,
    /// 相对受管根的展示路径
    pub display: String,
}

/// 某个目录下，某 agent 实际按序加载的指令文件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainEntry {
    pub agent: AgentId,
    pub order: usize,
    pub path: PathBuf,
    pub exists: bool,
    pub managed: bool,
    /// 被更靠前的同优先级文件遮蔽
    pub shadowed: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedBlockInfo {
    pub present: bool,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub body: Option<String>,
}

pub const BLOCK_BEGIN: &str = "<!-- agenthub:begin -->";
pub const BLOCK_END: &str = "<!-- agenthub:end -->";
pub const MAX_INSTRUCTION_BYTES: usize = 1024 * 1024;
