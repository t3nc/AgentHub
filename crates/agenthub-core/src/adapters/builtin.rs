use std::path::{Path, PathBuf};

use super::{AgentAdapter, instruction_spec};
use crate::config::AgentPathOverride;
use crate::model::agent::{AgentId, Capabilities, Scope, SkillTarget};
use crate::model::instruction::{InstructionKind, InstructionSpec};
use crate::model::mcp::{McpFile, McpFormat};
use crate::paths;

/// 数据驱动的内置适配器：5 个 agent 的差异只有目录、配置格式与指令文件，
/// 用同一结构描述，避免 5 份重复实现。
pub struct BuiltinAdapter {
    pub agent: AgentId,
    pub caps: Capabilities,
    pub config_dir: PathBuf,
    pub skills_dir: Option<PathBuf>,
    /// 额外的技能目录（OpenCode 会兼容读 `.agents/skills` / `.claude/skills`）。
    /// 这些是内置固定值，不参与设置页的路径覆盖。
    pub extra_skills_dirs: Vec<PathBuf>,
    pub project_skills_rel: Option<&'static str>,
    pub mcp_user: Option<(PathBuf, McpFormat)>,
    pub mcp_project: Option<(&'static str, McpFormat)>,
    pub instructions_user: Vec<(PathBuf, InstructionKind)>,
    pub instructions_project: Vec<(&'static str, InstructionKind)>,
    pub cli: Vec<&'static str>,
}

impl AgentAdapter for BuiltinAdapter {
    fn id(&self) -> AgentId {
        self.agent.clone()
    }

    fn capabilities(&self) -> Capabilities {
        self.caps
    }

    fn config_dir(&self) -> Option<PathBuf> {
        Some(self.config_dir.clone())
    }

    /// 键路径全部集中在 `ext::provider::spec_for`，这里只负责把适配器的
    /// `config_dir`（可能已被设置页覆盖）传进去。
    fn provider_spec(&self) -> Option<crate::ext::provider::ProviderTarget> {
        crate::ext::provider::spec_for(&self.agent, Some(&self.config_dir))
    }

    fn cli_names(&self) -> Vec<String> {
        self.cli.iter().map(|s| s.to_string()).collect()
    }

    fn skill_targets(&self, scope: &Scope) -> Vec<SkillTarget> {
        let mut out = Vec::new();
        match scope {
            Scope::User => {
                if let Some(dir) = &self.skills_dir {
                    out.push(SkillTarget::new(
                        self.agent.clone(),
                        dir.clone(),
                        Scope::User,
                    ));
                }
                for dir in &self.extra_skills_dirs {
                    out.push(SkillTarget::new(
                        self.agent.clone(),
                        dir.clone(),
                        Scope::User,
                    ));
                }
            }
            Scope::Project { root } => {
                if let Some(rel) = self.project_skills_rel {
                    out.push(SkillTarget::new(
                        self.agent.clone(),
                        root.join(rel),
                        scope.clone(),
                    ));
                }
            }
        }
        out
    }

    fn mcp_files(&self, scope: &Scope) -> Vec<McpFile> {
        let mut out = Vec::new();
        match scope {
            Scope::User => {
                if let Some((path, format)) = &self.mcp_user {
                    out.push(McpFile {
                        agent: self.agent.clone(),
                        exists: path.is_file(),
                        path: path.clone(),
                        format: format.clone(),
                    });
                }
            }
            Scope::Project { root } => {
                if let Some((rel, format)) = &self.mcp_project {
                    let path = root.join(rel);
                    out.push(McpFile {
                        agent: self.agent.clone(),
                        exists: path.is_file(),
                        path,
                        format: format.clone(),
                    });
                }
            }
        }
        out
    }

    fn instruction_specs(&self, scope: &Scope) -> Vec<InstructionSpec> {
        let pairs: Vec<(PathBuf, InstructionKind)> = match scope {
            Scope::User => self.instructions_user.clone(),
            Scope::Project { root } => self
                .instructions_project
                .iter()
                .map(|(rel, kind)| (root.join(rel), *kind))
                .collect(),
        };
        pairs
            .into_iter()
            .map(|(path, kind)| instruction_spec(self.agent.clone(), path, kind))
            .collect()
    }
}

/// 用组件拼路径。
///
/// 不能直接 `home.join(".claude/skills")`：`join` 只会补一个分隔符，字符串里剩下的
/// `/` 会原样留下，Windows 上就出现 `...\home\.claude/skills\cloudflare` 这种混合形态。
fn dir(home: &Path, rel: &str) -> PathBuf {
    let mut out = home.to_path_buf();
    for part in rel.split(['/', '\\']).filter(|s| !s.is_empty()) {
        out.push(part);
    }
    out
}

fn json(pointer: &[&str]) -> McpFormat {
    McpFormat::Json {
        pointer: pointer.iter().map(|s| s.to_string()).collect(),
    }
}

fn toml(table: &str) -> McpFormat {
    McpFormat::Toml {
        table: table.to_string(),
    }
}

fn yaml(top_key: &str) -> McpFormat {
    McpFormat::Yaml {
        top_key: top_key.to_string(),
    }
}

pub fn all(home: &Path) -> Vec<BuiltinAdapter> {
    // Hermes 的 Windows 安装器把数据放在 `%LOCALAPPDATA%\hermes`，而 `~/.hermes`
    // 可能存在却是空的（本机实测）。候选根在这里解析一次，后续扫描/链接/MCP 全用结果。
    let hermes = hermes_root(home);
    vec![
        BuiltinAdapter {
            agent: AgentId::Claude,
            caps: Capabilities::full(),
            config_dir: dir(home, ".claude"),
            skills_dir: Some(dir(home, ".claude/skills")),
            project_skills_rel: Some(".claude/skills"),
            extra_skills_dirs: Vec::new(),
            mcp_user: Some((home.join(".claude.json"), json(&["mcpServers"]))),
            mcp_project: Some((".mcp.json", json(&["mcpServers"]))),
            instructions_user: vec![(dir(home, ".claude/CLAUDE.md"), InstructionKind::Rule)],
            instructions_project: vec![("CLAUDE.md", InstructionKind::Rule)],
            cli: vec!["claude"],
        },
        BuiltinAdapter {
            agent: AgentId::Codex,
            caps: Capabilities::full(),
            config_dir: dir(home, ".codex"),
            skills_dir: Some(dir(home, ".codex/skills")),
            project_skills_rel: Some(".codex/skills"),
            extra_skills_dirs: Vec::new(),
            mcp_user: Some((dir(home, ".codex/config.toml"), toml("mcp_servers"))),
            mcp_project: Some((".codex/config.toml", toml("mcp_servers"))),
            instructions_user: vec![(dir(home, ".codex/AGENTS.md"), InstructionKind::Rule)],
            instructions_project: vec![("AGENTS.md", InstructionKind::Rule)],
            cli: vec!["codex"],
        },
        BuiltinAdapter {
            agent: AgentId::CodeBuddy,
            caps: Capabilities::full(),
            config_dir: dir(home, ".codebuddy"),
            skills_dir: Some(dir(home, ".codebuddy/skills")),
            project_skills_rel: Some(".codebuddy/skills"),
            extra_skills_dirs: Vec::new(),
            mcp_user: Some((dir(home, ".codebuddy/mcp.json"), json(&["mcpServers"]))),
            mcp_project: Some((".codebuddy/mcp.json", json(&["mcpServers"]))),
            instructions_user: vec![(dir(home, ".codebuddy/AGENTS.md"), InstructionKind::Rule)],
            instructions_project: vec![("AGENTS.md", InstructionKind::Rule)],
            cli: vec!["codebuddy"],
        },
        BuiltinAdapter {
            agent: AgentId::Hermes,
            caps: Capabilities::full(),
            config_dir: hermes.clone(),
            skills_dir: Some(hermes.join("skills")),
            project_skills_rel: Some(".hermes/skills"),
            extra_skills_dirs: Vec::new(),
            mcp_user: Some((hermes.join("config.yaml"), yaml("mcp_servers"))),
            mcp_project: None,
            instructions_user: vec![
                (hermes.join("AGENTS.md"), InstructionKind::Rule),
                (hermes.join("SOUL.md"), InstructionKind::Persona),
                (hermes.join("USER.md"), InstructionKind::Persona),
            ],
            instructions_project: vec![("AGENTS.md", InstructionKind::Rule)],
            cli: vec!["hermes"],
        },
        BuiltinAdapter {
            agent: AgentId::Gemini,
            caps: Capabilities::full(),
            config_dir: dir(home, ".gemini"),
            skills_dir: Some(dir(home, ".gemini/skills")),
            project_skills_rel: Some(".gemini/skills"),
            extra_skills_dirs: Vec::new(),
            mcp_user: Some((dir(home, ".gemini/settings.json"), json(&["mcpServers"]))),
            mcp_project: Some((".gemini/settings.json", json(&["mcpServers"]))),
            instructions_user: vec![(dir(home, ".gemini/GEMINI.md"), InstructionKind::Rule)],
            instructions_project: vec![("GEMINI.md", InstructionKind::Rule)],
            cli: vec!["gemini"],
        },
        BuiltinAdapter {
            agent: AgentId::Opencode,
            // MCP 不接管：`opencode.json` 的 `mcp` 是 `{type, command: []}` 数组形态，
            // 与标准 `mcpServers` 不兼容，强写会破坏它的配置
            caps: Capabilities {
                mcp: false,
                ..Capabilities::full()
            },
            config_dir: dir(home, ".config/opencode"),
            // 本机实测：自己的 skills 目录通常不存在，技能实际来自两个兼容目录
            skills_dir: Some(dir(home, ".config/opencode/skills")),
            extra_skills_dirs: vec![dir(home, ".agents/skills"), dir(home, ".claude/skills")],
            project_skills_rel: Some(".opencode/skills"),
            mcp_user: None,
            mcp_project: None,
            instructions_user: vec![(
                dir(home, ".config/opencode/AGENTS.md"),
                InstructionKind::Rule,
            )],
            instructions_project: vec![("AGENTS.md", InstructionKind::Rule)],
            cli: vec!["opencode"],
        },
    ]
}

/// Hermes 的数据根：`~/.hermes` 优先，Windows 安装器的 `%LOCALAPPDATA%\hermes` 兜底。
///
/// 顺序不能反：本机 `~/.hermes` 是空目录（早期版本留下的），真正有 `skills/` 的是
/// `%LOCALAPPDATA%\hermes`；`pick_root` 会跳过空目录，所以先放 `~/.hermes` 既能让
/// 沙箱测试隔离，也能让「用户自己的目录」优先。
fn hermes_root(home: &Path) -> PathBuf {
    let primary = dir(home, ".hermes");
    // 候选表按平台整块构造，而不是 `Vec::new()` 后逐个 push：
    // 非 Windows 只有一项，`push` 写法会触发 `clippy::vec_init_then_push`（macOS/Linux 专属红）。
    #[cfg(windows)]
    let candidates = vec![primary, paths::expand("%LOCALAPPDATA%/hermes")];
    #[cfg(not(windows))]
    let candidates = vec![primary];
    pick_root(&candidates)
}

/// 候选根里挑一个：优先「有 `config.yaml` 或 `skills/`」的（空目录不算），
/// 其次「存在的」，最后回退首选。
///
/// 只看「目录存在」会选错：本机 `~/.hermes` 空目录存在，真实数据在
/// `%LOCALAPPDATA%\hermes`。
fn pick_root(candidates: &[PathBuf]) -> PathBuf {
    for candidate in candidates {
        if candidate.join("config.yaml").exists() || candidate.join("skills").is_dir() {
            return candidate.clone();
        }
    }
    if let Some(existing) = candidates.iter().find(|c| c.exists()) {
        return existing.clone();
    }
    candidates.first().cloned().unwrap_or_default()
}

/// 把用户的路径覆盖写回适配器字段。
///
/// 空字符串视为「跟随自动检测」；填了就原样使用、即使不存在——UI 会显示「不存在」，
/// 静默回退成默认值会让用户以为设置没生效。
pub fn apply_override(adapter: &mut BuiltinAdapter, overrides: &AgentPathOverride) {
    let pick = |raw: Option<&String>| -> Option<PathBuf> {
        raw.map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(paths::expand)
    };

    if let Some(path) = pick(overrides.config_dir.as_ref()) {
        adapter.config_dir = path;
    }
    if let Some(path) = pick(overrides.skills_dir.as_ref()) {
        adapter.skills_dir = Some(path);
    }
    if let Some(path) = pick(overrides.mcp_file.as_ref()) {
        // 只换路径，格式沿用该 agent 的默认写法（JSON / TOML / YAML）
        let format = adapter
            .mcp_user
            .as_ref()
            .map(|(_, format)| format.clone())
            .unwrap_or(McpFormat::Json {
                pointer: vec!["mcpServers".to_string()],
            });
        adapter.mcp_user = Some((path, format));
    }
    if !overrides.instructions.is_empty() {
        adapter.instructions_user = overrides
            .instructions
            .iter()
            .map(|raw| raw.trim())
            .filter(|raw| !raw.is_empty())
            .map(|raw| (paths::expand(raw), instruction_kind_of(raw)))
            .collect();
    }
}

/// 覆盖的指令文件按文件名猜语义：SOUL / USER 是人格，其余是规则。
fn instruction_kind_of(raw: &str) -> InstructionKind {
    let name = raw
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(raw)
        .to_ascii_uppercase();
    if name.contains("SOUL") || name.contains("USER") {
        InstructionKind::Persona
    } else {
        InstructionKind::Rule
    }
}
