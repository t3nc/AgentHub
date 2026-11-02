use crate::l10n::{Msg, t};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::agent::AgentId;

/// 一个技能在某个 agent 目录下的链接状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LinkState {
    /// 指向 store 的符号链接
    Symlink { target: PathBuf },
    /// Windows 目录联接
    Junction { target: PathBuf },
    /// 与 store 中的同一 inode（硬链接）
    Hardlink { target: PathBuf },
    /// 独立副本（占空间）
    RealDir,
    /// 链接存在但目标丢失
    Broken { target: PathBuf },
    /// 指向受管根之外的链接，AgentHub 不处理
    Foreign { target: PathBuf },
}

impl LinkState {
    pub fn is_linked(&self) -> bool {
        matches!(
            self,
            LinkState::Symlink { .. } | LinkState::Junction { .. } | LinkState::Hardlink { .. }
        )
    }

    pub fn is_broken(&self) -> bool {
        matches!(self, LinkState::Broken { .. })
    }

    pub fn label(&self) -> String {
        match self {
            LinkState::Symlink { .. } => t!(Msg::LabelLinkSymlink),
            LinkState::Junction { .. } => t!(Msg::LabelLinkJunction),
            LinkState::Hardlink { .. } => t!(Msg::LabelLinkHardlink),
            LinkState::RealDir => t!(Msg::LabelLinkRealDir),
            LinkState::Broken { .. } => t!(Msg::LabelLinkBroken),
            LinkState::Foreign { .. } => t!(Msg::LabelLinkForeign),
        }
    }

    pub fn target(&self) -> Option<&PathBuf> {
        match self {
            LinkState::Symlink { target }
            | LinkState::Junction { target }
            | LinkState::Hardlink { target }
            | LinkState::Broken { target }
            | LinkState::Foreign { target } => Some(target),
            LinkState::RealDir => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSource {
    /// 中央仓库唯一真身
    Store,
    /// 某个 agent 目录下的实体副本
    AgentCopy,
    /// 系统内置（.system / marketplace 等）
    Readonly,
}

impl SkillSource {
    pub fn label(&self) -> String {
        match self {
            SkillSource::Store => t!(Msg::LabelSourceStore),
            SkillSource::AgentCopy => t!(Msg::LabelSourceAgentCopy),
            SkillSource::Readonly => t!(Msg::LabelSourceReadonly),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRecord {
    pub name: String,
    /// 该记录所在路径（agent 目录下的条目，或 store 中的目录）
    pub path: PathBuf,
    pub agent: Option<AgentId>,
    pub scope_label: String,
    pub source: SkillSource,
    pub readonly: bool,
    pub description: Option<String>,
    pub tree_hash: Option<String>,
    pub size_bytes: u64,
    pub file_count: usize,
    pub modified_at: i64,
    pub link: LinkState,
    /// 解引用之后的真实路径（链接则指向 store）
    pub canonical: PathBuf,
    pub never_upload: bool,
    /// 该技能名在「agent 独有」白名单里：不参与去重接管 / 链接修复 / 重复统计。
    ///
    /// `serde(default)` 是兼容性要求：老壳 / 老 CLI 读新扫描结果时不能因此解析失败。
    #[serde(default)]
    pub agent_local: bool,
}

impl SkillRecord {
    pub fn hash_short(&self) -> String {
        self.tree_hash
            .as_deref()
            .map(|h| h.chars().take(8).collect())
            .unwrap_or_else(|| "--------".into())
    }

    pub fn agent_label(&self) -> String {
        self.agent
            .as_ref()
            .map(|a| a.short().to_string())
            .unwrap_or_else(|| "ST".into())
    }
}

/// 同名技能的多份拷贝。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub name: String,
    pub members: Vec<SkillRecord>,
    /// 各成员内容是否完全一致
    pub identical: bool,
    /// 历史上每次重复安装浪费的字节数
    pub wasted_bytes: u64,
    pub suggested_source: PathBuf,
    pub reason: String,
}

impl DuplicateGroup {
    pub fn agent_count(&self) -> usize {
        self.members.iter().filter(|m| m.agent.is_some()).count()
    }
}
