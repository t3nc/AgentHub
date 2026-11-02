//! 常见 Agent 目录：检测、安装指引与「纳入受管」模板。
//!
//! 边界（刻意写死，不要放宽）：
//! - **只读检测**：`which()` 查 PATH、`exists()` 查目录/文件；`probe_versions = false` 时
//!   一个子进程都不会起，所以页面挂载、每次 `refresh()` 都可以安全调用。
//! - **不执行安装**：步骤只是字符串，UI 只负责复制；没有任何「运行」入口。
//! - **不创建目录**：纳入受管只写 `config.custom_agents`，扫描时目录不存在就显示不存在。
//! - **不缓存版本**：升级后缓存会骗人，宁可每次点击重新探测。

mod table;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::adapters::{AgentAdapter, display_path, which};
use crate::config::{AppConfig, CustomAgentDef};
use crate::error::{Error, Result};
use crate::ext::installer::InstallStep;
use crate::l10n::{self, Msg, t};
use crate::model::mcp::McpFormat;
use crate::paths;

pub use table::Spec;
use table::Template;

/// 版本探测上限。
///
/// 实测：Node 系的 `*.cmd` 都在 200 ms 内；Hermes 是 Python 启动 + 更新检查，
/// 本机 1.33 s，所以 1.2 s 会误判为「无法读取版本」，这里放宽到 2 s。
/// 探测只在用户点击时发生，且各条并行，最坏整体仍 < 2.5 s。
const VERSION_TIMEOUT: Duration = Duration::from_millis(2000);
const VERSION_MAX_CHARS: usize = 64;

/// 产品形态：决定 UI 文案（「CLI 就绪」还是「已安装」）与检测依据。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Cli,
    Desktop,
    Ide,
    Cloud,
}

/// 三态。语义固定：
/// - `Ready`：主入口（CLI 或应用包）在；
/// - `ConfigOnly`：只有配置残留，入口不在；
/// - `Missing`：两者都没有。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvState {
    Ready,
    ConfigOnly,
    Missing,
}

/// 受管状态：内置已受管 / 已登记 / 可以登记 / 与内置适配器共用配置 / 不能登记。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Managed {
    Builtin,
    Managed,
    Addable,
    /// 配置落在某个内置适配器的目录里（如 OpenAI 桌面端与 Codex CLI 共用 `~/.codex`），
    /// 已经由那个适配器接管，不需要也不应该重复登记。
    Shared,
    Blocked,
}

/// 静态表里的 MCP 格式描述（`Vec` 不能在 const 里构造，所以先存切片再 build）。
#[derive(Debug, Clone, Copy)]
pub enum McpFormatSpec {
    Json(&'static [&'static str]),
    Toml(&'static str),
    Yaml(&'static str),
}

impl McpFormatSpec {
    fn build(self) -> McpFormat {
        match self {
            McpFormatSpec::Json(pointer) => McpFormat::Json {
                pointer: pointer.iter().map(|s| (*s).to_string()).collect(),
            },
            McpFormatSpec::Toml(table) => McpFormat::Toml {
                table: table.to_string(),
            },
            McpFormatSpec::Yaml(top_key) => McpFormat::Yaml {
                top_key: top_key.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Os {
    Windows,
    Mac,
    Linux,
}

impl Os {
    fn current() -> Self {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Linux
        }
    }
}

impl Spec {
    fn apps(&self, os: Os) -> &'static [&'static str] {
        match os {
            Os::Windows => self.win_apps,
            _ => self.mac_apps,
        }
    }

    fn configs(&self, os: Os) -> &'static [&'static str] {
        match os {
            Os::Windows => self.win_config,
            _ => self.mac_config,
        }
    }

    /// MSIX（Microsoft Store）包家族名；只有 Windows 有这个概念。
    fn msix(&self, os: Os) -> &'static [&'static str] {
        match os {
            Os::Windows => self.win_msix,
            _ => &[],
        }
    }

    fn steps(&self, os: Os) -> &'static [table::Step] {
        match os {
            Os::Windows => self.win_steps,
            _ => self.mac_steps,
        }
    }

    // `specs()` 返回 `&'static [Spec]`，所以这里可以安全地要求 `'static self`，
    // 否则 `as_ref()` 的生命周期会被绑到调用时的借用上。
    fn template(&'static self, os: Os) -> Option<&'static Template> {
        match os {
            Os::Windows => self.win_managed.as_ref(),
            _ => self.mac_managed.as_ref(),
        }
    }
}

/// 「纳入受管」会写进配置的路径，UI 在确认前先列出来。
#[derive(Debug, Clone, Serialize)]
pub struct ManagedTarget {
    pub label: String,
    pub path: String,
    pub exists: bool,
    /// `user` / `project`
    pub scope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogEntry {
    pub key: String,
    pub family: String,
    pub name: String,
    pub vendor: String,
    pub kind: AgentKind,
    pub official: String,
    pub needs_confirm: bool,
    /// 对应内置适配器时给出其 slug（claude / codex / …）
    pub builtin: Option<String>,
    pub state: EnvState,
    pub cli_name: Option<String>,
    pub cli_path: Option<String>,
    pub version: Option<String>,
    pub app_path: Option<String>,
    /// 存在的配置路径；都不存在时是第一个「预期位置」，配合 `config_exists=false`。
    pub config_path: Option<String>,
    pub config_exists: bool,
    /// 判定依据，逐条给用户看，避免「凭感觉说没装」。
    pub evidence: Vec<String>,
    pub managed: Managed,
    /// `managed = shared` 时给出接管它的内置适配器 slug（codex / claude …）
    pub shared_agent: Option<String>,
    pub managed_id: Option<String>,
    pub managed_note: Option<String>,
    pub managed_targets: Vec<ManagedTarget>,
    pub steps: Vec<InstallStep>,
    pub docs: String,
    pub platform: String,
}

/// 目录里所有产品（内置 5 + 常见 8），按表内顺序。
pub fn specs() -> &'static [Spec] {
    table::specs()
}

pub fn find(key: &str) -> Option<&'static Spec> {
    specs().iter().find(|s| s.key == key)
}

/// 检测全部产品。`adapters` 用于内置 5 个（路径以适配器为准，避免两处漂移）。
pub fn detect(
    adapters: &[Box<dyn AgentAdapter>],
    config: &AppConfig,
    probe_versions: bool,
) -> Vec<CatalogEntry> {
    let os = Os::current();
    let packages = msix_packages();
    let mut out: Vec<CatalogEntry> = specs()
        .iter()
        .map(|spec| detect_one(spec, adapters, config, os, &packages))
        .collect();
    if probe_versions {
        probe_all(&mut out);
    }
    out
}

/// `%LOCALAPPDATA%\Packages` 下的目录名，用于 MSIX（Store）检测。
///
/// 一次 `read_dir` 取全部，避免每个条目各扫一遍。包目录名形如 `<Name>_<PublisherId>`，
/// 所以表里只写 `<Name>`（或完整 PackageFamilyName）都能命中——不同发布者 hash
/// 因机器而异，写死完整 PFN 会漏检。
fn msix_packages() -> Vec<String> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let root = paths::expand("%LOCALAPPDATA%/Packages");
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

/// 在已安装的包目录里找匹配项：完整 PFN 精确匹配，短名按 `<Name>_` 前缀匹配。
fn msix_match(packages: &[String], wanted: &str) -> Option<String> {
    let prefix = format!("{wanted}_");
    packages
        .iter()
        .find(|name| name.as_str() == wanted || name.starts_with(&prefix))
        .cloned()
}

fn detect_one(
    spec: &'static Spec,
    adapters: &[Box<dyn AgentAdapter>],
    config: &AppConfig,
    os: Os,
    packages: &[String],
) -> CatalogEntry {
    let mut evidence: Vec<String> = Vec::new();
    let mut cli_name: Option<String> = spec.cli.first().map(|s| (*s).to_string());
    let mut cli_path: Option<PathBuf> = None;
    let mut app_path: Option<PathBuf> = None;
    let mut config_path: Option<PathBuf> = None;

    if let Some(id) = &spec.builtin
        && let Some(adapter) = adapters.iter().find(|a| &a.id() == id)
    {
        let install = adapter.detect();
        cli_name = adapter.cli_names().into_iter().next();
        cli_path = install.cli_path;
        config_path = adapter.config_dir();
        if let Some(path) = &cli_path {
            evidence.push(t!(Msg::CatalogEvidencePathHit, value = display_path(path)));
        }
        if let Some(path) = &config_path
            && path.exists()
        {
            evidence.push(t!(
                Msg::CatalogEvidenceConfigDirExists,
                path = display_path(path)
            ));
        }
    } else {
        for name in spec.cli {
            if let Some(path) = which(name) {
                evidence.push(t!(Msg::CatalogEvidencePathHit, value = name));
                cli_name = Some((*name).to_string());
                cli_path = Some(path);
                break;
            }
        }
        // 传统安装目录优先：Electron 应用（如 WorkBuddy）同时有 Programs 目录和
        // MSIX Share Target 包，展示主程序目录更有意义。
        for raw in spec.apps(os) {
            let path = paths::expand(raw);
            if path.exists() {
                evidence.push(t!(
                    Msg::CatalogEvidenceAppExists,
                    path = display_path(&path)
                ));
                app_path = Some(path);
                break;
            }
        }
        // Store（MSIX）版：装在 `%LOCALAPPDATA%\Packages\<PFN>`，既没有 CLI
        // 也不在 Programs 下；这里只做目录存在性判断，不调 Get-AppxPackage。
        if app_path.is_none() {
            for pfn in spec.msix(os) {
                let Some(dir) = msix_match(packages, pfn) else {
                    continue;
                };
                let path = paths::expand(&format!("%LOCALAPPDATA%/Packages/{dir}"));
                if path.exists() {
                    evidence.push(t!(Msg::CatalogEvidenceStoreInstalled, name = dir));
                    app_path = Some(path);
                    break;
                }
            }
        }
        // 配置：优先记录真实存在的；都没有时保留第一个预期位置，让 UI 能显示「应该在哪」。
        let mut hint: Option<PathBuf> = None;
        for raw in spec.configs(os) {
            let path = paths::expand(raw);
            if hint.is_none() {
                hint = Some(path.clone());
            }
            if path.exists() {
                evidence.push(t!(
                    Msg::CatalogEvidenceConfigExists,
                    path = display_path(&path)
                ));
                config_path = Some(path);
                break;
            }
        }
        if config_path.is_none() {
            config_path = hint;
        }
    }

    let has_entry = cli_path.is_some() || app_path.is_some();
    let config_exists = config_path.as_ref().map(|p| p.exists()).unwrap_or(false);
    let state = if has_entry {
        EnvState::Ready
    } else if config_exists {
        EnvState::ConfigOnly
    } else {
        EnvState::Missing
    };

    let (managed, managed_id, managed_note, managed_targets) = managed_state(spec, config, os);
    let shared_agent = spec.shared_agent.as_ref().map(|id| id.slug());

    CatalogEntry {
        key: spec.key.to_string(),
        family: spec.family.to_string(),
        name: spec.name.resolve(),
        vendor: spec.vendor.resolve(),
        kind: spec.kind,
        official: spec.official.to_string(),
        needs_confirm: spec.needs_confirm,
        builtin: spec.builtin.as_ref().map(|id| id.slug()),
        state,
        cli_name,
        cli_path: cli_path.map(|p| display_path(&p)),
        version: None,
        app_path: app_path.map(|p| display_path(&p)),
        config_path: config_path.map(|p| display_path(&p)),
        config_exists,
        evidence,
        managed,
        shared_agent,
        managed_id,
        managed_note,
        managed_targets,
        steps: steps_for(spec, os),
        docs: spec.official.to_string(),
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    }
}

fn steps_for(spec: &Spec, os: Os) -> Vec<InstallStep> {
    // 静态表只存 `Msg`，成品文案在这里按当前界面语言取：这样切语言后重新拉一次就变。
    spec.steps(os)
        .iter()
        .map(|s| InstallStep {
            title: l10n::str_of(s.title).to_string(),
            command: s.command.resolve(),
            risk: l10n::str_of(s.risk).to_string(),
            url: s.url.map(|u| u.to_string()),
        })
        .collect()
}

/// 某条目的当前平台安装步骤（`installer::plan_for` 与测试共用）。
pub fn plan_steps(spec: &'static Spec) -> Vec<InstallStep> {
    steps_for(spec, Os::current())
}

fn managed_state(
    spec: &'static Spec,
    config: &AppConfig,
    os: Os,
) -> (Managed, Option<String>, Option<String>, Vec<ManagedTarget>) {
    if spec.builtin.is_some() {
        return (Managed::Builtin, None, None, Vec::new());
    }
    // 共用内置配置的优先判：有 `shared_agent` 就不该落到 Blocked
    if let Some(shared) = &spec.shared_agent {
        return (
            Managed::Shared,
            None,
            Some(t!(Msg::CatalogNoteShared, agent = shared.display_name())),
            Vec::new(),
        );
    }
    let Some(tpl) = spec.template(os) else {
        let reason = Some(match spec.blocked {
            Some(msg) => l10n::str_of(msg).to_string(),
            None => t!(Msg::ErrCatalogUnsupported),
        });
        return (Managed::Blocked, None, reason, Vec::new());
    };
    let targets = targets_of(tpl);
    let note = Some(l10n::str_of(tpl.note).to_string());
    if config.custom_agents.iter().any(|d| d.id == tpl.id) {
        return (Managed::Managed, Some(tpl.id.to_string()), note, targets);
    }
    (Managed::Addable, Some(tpl.id.to_string()), note, targets)
}

/// 候选路径里第一个存在的下标；都不存在时回退 0。
///
/// 返回下标而不是路径，是为了让调用方保留「可移植写法」（`%LOCALAPPDATA%/...`、`~/...`）
/// 写进配置，而不是展开后的绝对路径——换平台后配置仍能解析。
pub fn pick_existing_index(candidates: &[PathBuf]) -> (usize, bool) {
    for (idx, path) in candidates.iter().enumerate() {
        if path.exists() {
            return (idx, true);
        }
    }
    (0, false)
}

/// 该模板实际要写的 MCP 文件：Store 虚拟目录 / Roaming / LocalAppData 里挑存在的那个。
fn resolve_mcp_file(tpl: &Template) -> Option<String> {
    let mut raws: Vec<&'static str> = Vec::new();
    if let Some(v) = tpl.mcp_file {
        raws.push(v);
    }
    raws.extend(tpl.mcp_file_alternates.iter().copied());
    if raws.is_empty() {
        return None;
    }
    let expanded: Vec<PathBuf> = raws.iter().map(|raw| paths::expand(raw)).collect();
    let (idx, _exists) = pick_existing_index(&expanded);
    Some(raws[idx].to_string())
}

fn targets_of(tpl: &Template) -> Vec<ManagedTarget> {
    let mut out = Vec::new();
    let mut add = |label: Msg, raw: &str, scope: &str| {
        let path = paths::expand(raw);
        out.push(ManagedTarget {
            label: l10n::str_of(label).to_string(),
            path: display_path(&path),
            exists: path.exists(),
            scope: scope.to_string(),
        });
    };
    if let Some(v) = tpl.skills_dir {
        add(Msg::CatalogTargetSkills, v, "user");
    }
    if let Some(v) = tpl.project_skills_dir {
        add(Msg::CatalogTargetProjectSkills, v, "project");
    }
    if let Some(v) = resolve_mcp_file(tpl) {
        add(Msg::CatalogTargetMcpFile, &v, "user");
    }
    if let Some(v) = tpl.project_mcp_file {
        add(Msg::CatalogTargetProjectMcpFile, v, "project");
    }
    for v in tpl.instructions {
        add(Msg::CatalogTargetInstructions, v, "user");
    }
    out
}

/// 把目录条目转成 `CustomAgentDef`（只做内存对象，落盘由 `AgentHub` 负责）。
pub fn managed_def(key: &str) -> Result<CustomAgentDef> {
    let spec = find(key).ok_or_else(|| Error::NotFound(t!(Msg::ErrCatalogNoEntry, key = key)))?;
    if spec.builtin.is_some() {
        return Err(Error::Conflict(t!(
            Msg::ErrCatalogBuiltinManaged,
            name = spec.name.resolve()
        )));
    }
    if let Some(shared) = &spec.shared_agent {
        return Err(Error::Conflict(t!(
            Msg::ErrCatalogShared,
            name = spec.name.resolve(),
            agent = shared.display_name()
        )));
    }
    let tpl = spec.template(Os::current()).ok_or_else(|| {
        Error::Unsupported(match spec.blocked {
            Some(msg) => l10n::str_of(msg).to_string(),
            None => t!(Msg::ErrCatalogUnsupported),
        })
    })?;
    let def = CustomAgentDef {
        id: tpl.id.to_string(),
        display_name: tpl.display_name.to_string(),
        skills_dir: tpl.skills_dir.map(str::to_string),
        project_skills_dir: tpl.project_skills_dir.map(str::to_string),
        mcp_file: resolve_mcp_file(tpl),
        mcp_format: tpl.mcp_format.map(|f| f.build()),
        project_mcp_file: tpl.project_mcp_file.map(str::to_string),
        instructions: tpl.instructions.iter().map(|s| (*s).to_string()).collect(),
    };
    if def.skills_dir.is_none()
        && def.project_skills_dir.is_none()
        && def.mcp_file.is_none()
        && def.project_mcp_file.is_none()
        && def.instructions.is_empty()
    {
        return Err(Error::Unsupported(t!(
            Msg::ErrCatalogNoPaths,
            name = spec.name.resolve()
        )));
    }
    Ok(def)
}

/// 某个目录条目登记后的自定义平台 id（移出受管时用）。
pub fn managed_id(key: &str) -> Result<String> {
    let spec = find(key).ok_or_else(|| Error::NotFound(t!(Msg::ErrCatalogNoEntry, key = key)))?;
    if spec.builtin.is_some() {
        return Err(Error::Conflict(t!(
            Msg::ErrCatalogBuiltinNoRemove,
            name = spec.name.resolve()
        )));
    }
    spec.template(Os::current())
        .map(|t| t.id.to_string())
        .ok_or_else(|| {
            Error::Unsupported(match spec.blocked {
                Some(msg) => l10n::str_of(msg).to_string(),
                None => t!(Msg::ErrCatalogUnsupported),
            })
        })
}

// ---- 版本探测 ----------------------------------------------------------

fn probe_all(entries: &mut [CatalogEntry]) {
    let (tx, rx) = mpsc::channel::<(usize, Option<String>)>();
    let mut pending = 0usize;
    for (idx, entry) in entries.iter().enumerate() {
        if entry.cli_path.is_none() || entry.version.is_some() {
            continue;
        }
        // `cli_path` 是展示用的 `~/x` 缩略路径，不能直接拿去 spawn；
        // 用 CLI 名重新在 PATH 里解析一次真实路径。
        let Some(name) = entry.cli_name.clone() else {
            continue;
        };
        let Some(path) = which(&name) else {
            continue;
        };
        let tx = tx.clone();
        pending += 1;
        std::thread::spawn(move || {
            let version = probe_version(&path);
            let _ = tx.send((idx, version));
        });
    }
    drop(tx);
    for _ in 0..pending {
        match rx.recv_timeout(VERSION_TIMEOUT + Duration::from_millis(300)) {
            Ok((idx, Some(version))) => entries[idx].version = Some(version),
            // 超时或失败：保持「无法读取版本」，不报错、不重试
            Ok((_, None)) => {}
            Err(_) => break,
        }
    }
}

/// 执行 `<cli> --version` 并清洗输出。
///
/// 超时不 kill 子进程：`--version` 秒退，强杀反而可能抢句柄；线程结束后自行回收。
fn probe_version(path: &Path) -> Option<String> {
    let mut command = version_command(path);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let child = command.spawn().ok()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let captured = child
            .wait_with_output()
            .ok()
            .map(|out| String::from_utf8_lossy(&out.stdout).to_string());
        let _ = tx.send(captured);
    });
    let raw = rx.recv_timeout(VERSION_TIMEOUT).ok()??;
    clean_version(&raw)
}

/// Windows 上不能直接 spawn `.cmd` / `.ps1`（os error 193 / 无法执行）。
fn version_command(path: &Path) -> Command {
    #[cfg(windows)]
    {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase());
        match ext.as_deref() {
            Some("cmd") | Some("bat") => {
                let mut cmd = Command::new("cmd");
                cmd.arg("/C").arg(path);
                return cmd;
            }
            Some("ps1") => {
                let mut cmd = Command::new("powershell");
                cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
                    .arg(path);
                return cmd;
            }
            _ => {}
        }
    }
    Command::new(path)
}

/// 取第一行非空内容，去 BOM / ANSI / CR，截断到 64 字符。
pub fn clean_version(raw: &str) -> Option<String> {
    let text = raw.trim_start_matches('\u{feff}');
    for line in text.lines() {
        let cleaned = strip_ansi(line);
        let trimmed = cleaned.trim();
        if trimmed.is_empty() {
            continue;
        }
        return Some(truncate_chars(trimmed, VERSION_MAX_CHARS));
    }
    None
}

fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}

fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::{clean_version, truncate_chars};

    #[test]
    fn version_output_is_cleaned() {
        // Windows 上常见：BOM + 回车 + 彩色控制码 + 后续日志行
        let raw = "\u{feff}1.2.3\r\n\u{1b}[32mdebug\u{1b}[0m 启动中";
        assert_eq!(clean_version(raw).as_deref(), Some("1.2.3"));
    }

    #[test]
    fn version_output_skips_blank_lines() {
        assert_eq!(
            clean_version("\n\n  \ncodex-cli 0.9.0\n").as_deref(),
            Some("codex-cli 0.9.0")
        );
    }

    #[test]
    fn empty_output_has_no_version() {
        assert!(clean_version("   \r\n").is_none());
    }

    #[test]
    fn long_version_is_truncated() {
        let long = "v".repeat(200);
        let out = truncate_chars(&long, 8);
        assert_eq!(out.chars().count(), 9);
        assert!(out.ends_with('…'));
    }
}
