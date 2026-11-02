pub mod builtin;
pub mod custom;

use std::path::{Path, PathBuf};

pub use builtin::BuiltinAdapter;

use crate::config::AppConfig;
use crate::ext::provider::ProviderTarget;
use crate::model::agent::{AgentId, AgentInstall, Capabilities, InstallSource, Scope, SkillTarget};
use crate::model::instruction::{BLOCK_BEGIN, InstructionKind, InstructionSpec};
use crate::model::mcp::McpFile;
use crate::paths;
use crate::platform;

/// 每个 agent 一个实现；所有平台差异收敛在此。
pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> AgentId;

    fn capabilities(&self) -> Capabilities;

    fn config_dir(&self) -> Option<PathBuf>;

    fn cli_names(&self) -> Vec<String>;

    /// 用户级 / 项目级的技能目标目录。
    fn skill_targets(&self, scope: &Scope) -> Vec<SkillTarget>;

    /// 该 agent 的 MCP 配置文件。
    fn mcp_files(&self, scope: &Scope) -> Vec<McpFile>;

    /// 指令文件（AGENTS.md / CLAUDE.md / SOUL.md …）。
    fn instruction_specs(&self, scope: &Scope) -> Vec<InstructionSpec>;

    /// provider（上游 API）配置的落点。
    ///
    /// 默认 `None`：格式没核实过的 agent 一律不写。键路径只能来自实证，
    /// 「看起来像」的猜测会写坏用户的配置文件。
    fn provider_spec(&self) -> Option<ProviderTarget> {
        None
    }

    /// 配置目录里值得监听的文件（只用于「外部有变更」的监听白名单，不参与扫描）。
    ///
    /// 配置目录是整个 agent 的家目录（`~/.claude`、`~/.codex`），里面混着大量
    /// CLI 运行时文件。整目录监听会让每次会话写入都报一次「外部有变更」，
    /// 所以监听只认这些文件名；不存在的名字不会进快照，多列几个没有副作用。
    fn config_watch_files(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        if let Some(dir) = self.config_dir() {
            for name in [
                // Provider 写盘的目标（Claude settings.json / Codex config.toml）
                "settings.json",
                "settings.local.json",
                "config.toml",
                "config.json",
                // OpenCode 的 provider / MCP 都在这里；不列进来，外部改动就看不见
                "opencode.json",
                "hooks.json",
                // 指令文件
                "CLAUDE.md",
                "AGENTS.md",
                "SOUL.md",
            ] {
                out.push(dir.join(name));
            }
        }
        out.extend(
            self.mcp_files(&Scope::User)
                .into_iter()
                .map(|file| file.path),
        );
        out.extend(
            self.instruction_specs(&Scope::User)
                .into_iter()
                .map(|spec| spec.path),
        );
        out
    }

    fn detect(&self) -> AgentInstall {
        let cli = self.cli_names().iter().find_map(|name| which(name));
        let dir = self.config_dir();
        let detected = cli.is_some() || dir.as_ref().map(|d| d.exists()).unwrap_or(false);
        AgentInstall {
            display_name: self.id().display_name(),
            id: self.id(),
            config_dir: dir.filter(|d| d.exists()),
            cli_path: cli,
            capabilities: self.capabilities(),
            source: InstallSource::Builtin,
            detected,
        }
    }
}

/// PATH 中查找可执行文件。
///
/// Windows 上按真实的 `PATHEXT` 逐个后缀尝试（`claude` → `claude.exe`），而不是固定
/// `exe/cmd/bat`；Unix 上只看可执行位。
pub fn which(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let exts = exec_extensions();
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        for ext in &exts {
            let c = dir.join(format!("{name}{ext}"));
            if is_executable(&c) {
                return Some(c);
            }
        }
    }
    None
}

/// 平台可执行后缀：Windows 读 `PATHEXT`（小写、带点），其他平台为空。
fn exec_extensions() -> Vec<String> {
    #[cfg(windows)]
    {
        let raw = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        let mut out: Vec<String> = raw
            .split(';')
            .map(|e| e.trim().to_ascii_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
        if out.is_empty() {
            out = vec![".exe".into(), ".cmd".into(), ".bat".into()];
        }
        out
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        // Windows 上「可执行」= 扩展名在 PATHEXT 里；否则同名普通文件会被误判成 CLI
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => {
                let ext = format!(".{}", ext.to_ascii_lowercase());
                exec_extensions().contains(&ext)
            }
            None => false,
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        true
    }
}

/// 构造 InstructionSpec，顺带探测「存在」与「已含受控区块」。
pub fn instruction_spec(agent: AgentId, path: PathBuf, kind: InstructionKind) -> InstructionSpec {
    let meta = std::fs::metadata(&path).ok();
    let managed = std::fs::read_to_string(&path)
        .map(|t| t.contains(BLOCK_BEGIN))
        .unwrap_or(false);
    InstructionSpec {
        display: display_path(&path),
        agent,
        exists: meta.is_some(),
        managed,
        size_bytes: meta.map(|m| m.len()).unwrap_or(0),
        path,
        kind,
    }
}

/// 展示用：把主目录前缀缩成 `~`，分隔符统一成 `/`。
///
/// 旧实现直接 `format!("~/{}", rest.display())`，Windows 上会产出 `~/foo\bar`
/// 这种混合形态，前端再按 `/` 切分就取不到文件名。
pub fn display_path(path: &Path) -> String {
    let home = paths::home_dir();
    if let Ok(rest) = path.strip_prefix(&home) {
        return format!("~/{}", platform::display(rest));
    }
    if paths::is_within(&home, path) && !paths::paths_eq(&home, path) {
        // 仅大小写不同（Windows）时 strip_prefix 会失败，退回朴素截断
        let raw = platform::display(path);
        let home_raw = platform::display(&home);
        if let Some(rest) = raw.strip_prefix(&home_raw) {
            return format!("~/{}", rest.trim_start_matches('/'));
        }
        return raw;
    }
    platform::display(path)
}

/// 内置适配器 + 用户自定义平台。
pub fn build_adapters(config: &AppConfig) -> Vec<Box<dyn AgentAdapter>> {
    let mut out: Vec<Box<dyn AgentAdapter>> = builtin::all(&paths::home_dir())
        .into_iter()
        .map(|mut adapter| {
            // 用户覆盖优先于候选根：设置里填了就照办，哪怕路径不存在
            if let Some(overrides) = config.agent_paths.get(&adapter.agent.slug()) {
                builtin::apply_override(&mut adapter, overrides);
            }
            Box::new(adapter) as Box<dyn AgentAdapter>
        })
        .collect();
    for def in &config.custom_agents {
        out.push(Box::new(custom::CustomAdapter::new(def.clone())));
    }
    out
}
