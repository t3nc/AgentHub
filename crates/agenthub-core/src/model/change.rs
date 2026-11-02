use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::l10n::{self, Msg, t};
use crate::link::LinkStrategy;
use crate::paths;

/// 一次变更计划。apply 的唯一输入，也是 UI 差异预览的数据源。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSet {
    pub id: String,
    pub kind: ChangeKind,
    pub title: String,
    pub ops: Vec<FileOp>,
    pub notes: Vec<String>,
    pub risks: Vec<Risk>,
}

impl ChangeSet {
    pub fn new(kind: ChangeKind, title: impl Into<String>) -> Self {
        Self {
            id: format!("cs-{}-{}", paths::ts_label(), paths::unique_id()),
            kind,
            title: title.into(),
            ops: Vec::new(),
            notes: Vec::new(),
            risks: Vec::new(),
        }
    }

    pub fn push(&mut self, mut op: FileOp) -> &mut Self {
        op.id = self.ops.len();
        self.ops.push(op);
        self
    }

    pub fn note(&mut self, text: impl Into<String>) -> &mut Self {
        self.notes.push(text.into());
        self
    }

    pub fn risk(&mut self, level: RiskLevel, text: impl Into<String>) -> &mut Self {
        self.risks.push(Risk {
            level,
            message: text.into(),
        });
        self
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn op_count(&self) -> usize {
        self.ops.len()
    }

    pub fn highest_risk(&self) -> Option<RiskLevel> {
        self.risks.iter().map(|r| r.level).max()
    }
}

/// 删除选项：由确认弹窗的两个开关决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteOptions {
    /// 移入回收站；false = 直接删除（只能靠批次备份恢复）
    pub use_trash: bool,
    /// 删仓库真身时，连带删除各 agent 指向它的链接（避免留断链）
    pub with_links: bool,
}

impl DeleteOptions {
    /// 默认最安全的组合：进回收站 + 清理链接。
    pub fn safe() -> Self {
        Self {
            use_trash: true,
            with_links: true,
        }
    }
}

impl Default for DeleteOptions {
    fn default() -> Self {
        Self::safe()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Info,
    Warn,
    Danger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Risk {
    pub level: RiskLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Dedupe,
    Adopt,
    Repair,
    McpWrite,
    Instruction,
    ProjectInit,
    /// Phase 3：把 provider（中转站 / 官方 API）写进 agent 的模型配置
    Provider,
    /// 删除技能（可连带删除指向它的链接）
    Delete,
    /// 从技能市场安装到中央仓库并按 agent 建链接
    MarketInstall,
    Rollback,
}

impl ChangeKind {
    /// 稳定标识，落库与前端判断用它，不随界面语言变。
    ///
    /// 批次表里曾经存的是 `label()`（已本地化），所以老数据里是当时的自然语言；
    /// 前端按 slug 判断、认不出来就当普通条目显示。
    pub fn slug(&self) -> &'static str {
        match self {
            ChangeKind::Dedupe => "dedupe",
            ChangeKind::Adopt => "adopt",
            ChangeKind::Repair => "repair",
            ChangeKind::McpWrite => "mcp_write",
            ChangeKind::Instruction => "instruction",
            ChangeKind::ProjectInit => "project_init",
            ChangeKind::Provider => "provider",
            ChangeKind::Delete => "delete",
            ChangeKind::MarketInstall => "market_install",
            ChangeKind::Rollback => "rollback",
        }
    }

    pub fn label(&self) -> &'static str {
        l10n::str_of(match self {
            ChangeKind::Dedupe => Msg::KindDedupe,
            ChangeKind::Adopt => Msg::KindAdopt,
            ChangeKind::Repair => Msg::KindRepair,
            ChangeKind::McpWrite => Msg::KindMcpWrite,
            ChangeKind::Instruction => Msg::KindInstruction,
            ChangeKind::ProjectInit => Msg::KindProjectInit,
            ChangeKind::Provider => Msg::KindProvider,
            ChangeKind::Delete => Msg::KindDelete,
            ChangeKind::MarketInstall => Msg::KindMarketInstall,
            ChangeKind::Rollback => Msg::KindRollback,
        })
    }
}

/// 单条文件操作。合并内容在 plan 阶段就算好，apply 只做原子落盘。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOp {
    pub id: usize,
    pub label: String,
    pub action: OpAction,
}

impl FileOp {
    pub fn create_link(at: PathBuf, target: PathBuf, strategy: LinkStrategy) -> Self {
        Self {
            id: 0,
            label: t!(Msg::OpCreateLink, strategy = strategy.label()),
            action: OpAction::CreateLink {
                at,
                target,
                strategy,
            },
        }
    }

    pub fn replace_with_link(at: PathBuf, target: PathBuf, strategy: LinkStrategy) -> Self {
        Self {
            id: 0,
            label: t!(Msg::OpReplaceLink, strategy = strategy.label()),
            action: OpAction::ReplaceWithLink {
                at,
                target,
                strategy,
            },
        }
    }

    pub fn move_into_store(from: PathBuf, to: PathBuf) -> Self {
        Self {
            id: 0,
            label: t!(Msg::OpMoveIntoStore),
            action: OpAction::MoveIntoStore { from, to },
        }
    }

    pub fn remove_path(at: PathBuf, use_trash: bool) -> Self {
        Self {
            id: 0,
            label: if use_trash {
                t!(Msg::OpRemoveTrash)
            } else {
                t!(Msg::OpRemove)
            },
            action: OpAction::RemovePath { at, use_trash },
        }
    }

    pub fn write_file(at: PathBuf, content: String, backup: bool) -> Self {
        Self {
            id: 0,
            label: t!(Msg::OpWriteFile),
            action: OpAction::WriteFile {
                at,
                content,
                backup,
            },
        }
    }

    /// 人类可读的一行说明，用于 CLI 与 diff 视图标题。
    pub fn describe(&self) -> String {
        match &self.action {
            OpAction::CreateLink { at, target, .. } => t!(
                Msg::DescCreateLink,
                at = paths::path_str(at),
                target = paths::path_str(target)
            ),
            OpAction::ReplaceWithLink { at, target, .. } => t!(
                Msg::DescReplaceLink,
                at = paths::path_str(at),
                target = paths::path_str(target)
            ),
            OpAction::MoveIntoStore { from, to } => t!(
                Msg::DescMove,
                from = paths::path_str(from),
                to = paths::path_str(to)
            ),
            OpAction::RemovePath { at, use_trash } => {
                if *use_trash {
                    t!(Msg::DescRemoveTrash, at = paths::path_str(at))
                } else {
                    t!(Msg::DescRemove, at = paths::path_str(at))
                }
            }
            OpAction::WriteFile { at, content, .. } => t!(
                Msg::DescWriteFile,
                at = paths::path_str(at),
                bytes = content.len()
            ),
        }
    }

    /// 该操作涉及的主路径，用于 diff 面板分组。
    pub fn primary_path(&self) -> PathBuf {
        match &self.action {
            OpAction::CreateLink { at, .. }
            | OpAction::ReplaceWithLink { at, .. }
            | OpAction::RemovePath { at, .. }
            | OpAction::WriteFile { at, .. } => at.clone(),
            OpAction::MoveIntoStore { from, .. } => from.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum OpAction {
    CreateLink {
        at: PathBuf,
        target: PathBuf,
        strategy: LinkStrategy,
    },
    ReplaceWithLink {
        at: PathBuf,
        target: PathBuf,
        strategy: LinkStrategy,
    },
    MoveIntoStore {
        from: PathBuf,
        to: PathBuf,
    },
    RemovePath {
        at: PathBuf,
        use_trash: bool,
    },
    WriteFile {
        at: PathBuf,
        content: String,
        backup: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub batch_id: String,
    pub applied: usize,
    pub skipped: usize,
    pub backup_dir: Option<PathBuf>,
    pub log: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRecord {
    pub batch_id: String,
    pub kind: String,
    pub title: String,
    pub created_at: i64,
    pub undone: bool,
    pub op_count: usize,
}
