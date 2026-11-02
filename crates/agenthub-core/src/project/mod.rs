use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::l10n::{Msg, t};
use crate::paths;

pub const STATE_DIR: &str = ".agenthub";
pub const STATE_FILE: &str = "project.json";

/// 项目内唯一状态文件：只记「启用哪些技能 / MCP」，不写绝对路径与密钥。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectState {
    pub name: String,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub mcp: Vec<String>,
    #[serde(default)]
    pub agents: Vec<String>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

impl ProjectState {
    pub fn new(name: impl Into<String>) -> Self {
        let now = paths::now_ts();
        Self {
            name: name.into(),
            skills: Vec::new(),
            mcp: Vec::new(),
            agents: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

pub fn state_path(root: &Path) -> PathBuf {
    root.join(STATE_DIR).join(STATE_FILE)
}

pub fn is_project(root: &Path) -> bool {
    state_path(root).is_file()
}

pub fn load(root: &Path) -> Option<ProjectState> {
    let text = std::fs::read_to_string(state_path(root)).ok()?;
    serde_json::from_str(crate::text::strip_bom(&text)).ok()
}

pub fn save(root: &Path, state: &ProjectState) -> Result<()> {
    let path = state_path(root);
    if let Some(parent) = path.parent() {
        paths::ensure_dir(parent)?;
    }
    let text = serde_json::to_string_pretty(state)?;
    paths::write_atomic(&path, text.as_bytes())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub root: PathBuf,
    pub name: String,
    pub branch: Option<String>,
    pub is_git: bool,
    pub has_state: bool,
    pub skill_count: usize,
    pub mcp_count: usize,
    /// 项目内各 agent 的 skills 目录最新状态
    pub agent_dirs: Vec<PathBuf>,
}

pub fn summarize(root: &Path) -> ProjectSummary {
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| root.display().to_string());
    let (is_git, branch) = git_info(root);
    let state = load(root);
    ProjectSummary {
        root: root.to_path_buf(),
        name,
        branch,
        is_git,
        has_state: state.is_some(),
        skill_count: state.as_ref().map(|s| s.skills.len()).unwrap_or(0),
        mcp_count: state.as_ref().map(|s| s.mcp.len()).unwrap_or(0),
        agent_dirs: Vec::new(),
    }
}

fn git_info(root: &Path) -> (bool, Option<String>) {
    let head = root.join(".git/HEAD");
    match std::fs::read_to_string(&head) {
        Ok(text) => {
            let branch = text
                .trim()
                .strip_prefix("ref: refs/heads/")
                .map(|s| s.to_string())
                .or_else(|| Some("detached".into()));
            (true, branch)
        }
        Err(_) => (root.join(".git").is_dir(), None),
    }
}

/// 从 `path` 向上寻找项目根（含 .git 或 .agenthub）。
pub fn discover_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        if dir.join(".git").exists() || dir.join(STATE_DIR).exists() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

pub fn require_root(root: &Path) -> Result<()> {
    if !root.is_absolute() {
        return Err(Error::Other(t!(
            Msg::ErrProjectRootRelative,
            path = root.display()
        )));
    }
    Ok(())
}
