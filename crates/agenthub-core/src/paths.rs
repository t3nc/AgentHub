use crate::l10n::{Msg, t};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{Error, Result};
use crate::platform;

static HOME_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();
static DATA_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// 测试 / 沙箱场景用：进程启动时把 home 与数据目录钉死，避免触碰真实环境。
/// 只能生效一次（OnceLock），生产代码不应调用。
pub fn init_overrides(home: impl Into<PathBuf>, data: impl Into<PathBuf>) {
    let _ = HOME_OVERRIDE.set(home.into());
    let _ = DATA_OVERRIDE.set(data.into());
}

/// 用户主目录。设置 `AGENTHUB_HOME` 可把全部 agent 目录重定向到沙箱，
/// 便于集成测试与演示，不需要动真实环境。
///
/// Windows 上优先 `USERPROFILE`：Git Bash / MSYS2 常把 `HOME` 设成 `/c/Users/x`，
/// 直接拼出来是非法盘符路径。
pub fn home_dir() -> PathBuf {
    if let Some(v) = HOME_OVERRIDE.get() {
        return v.clone();
    }
    if let Some(v) = env_nonempty("AGENTHUB_HOME") {
        return PathBuf::from(v);
    }
    #[cfg(windows)]
    {
        if let Some(v) = env_nonempty("USERPROFILE") {
            return PathBuf::from(v);
        }
        if let Some(v) = env_nonempty("HOME")
            && looks_like_windows_absolute(&v)
        {
            return PathBuf::from(v);
        }
        if let Some(drive) = env_nonempty("HOMEDRIVE")
            && let Some(rest) = env_nonempty("HOMEPATH")
        {
            return PathBuf::from(format!("{drive}{rest}"));
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(v) = env_nonempty("HOME") {
            return PathBuf::from(v);
        }
    }
    if let Some(v) = env_nonempty("USERPROFILE") {
        return PathBuf::from(v);
    }
    PathBuf::from(".")
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

/// 家目录是否被重定向（测试 `init_overrides` 或 `AGENTHUB_HOME`）。
///
/// 有些数据根不在家目录下（CodeBuddy IDE 在 `%LOCALAPPDATA%`）；沙箱化时必须一起
/// 改道，否则会在隔离环境里读到真实用量，把测试与文档里的数字污染掉。
pub fn home_is_overridden() -> bool {
    HOME_OVERRIDE.get().is_some() || env_nonempty("AGENTHUB_HOME").is_some()
}

/// `C:\Users\x` 或 `\\server\share`，排除 Git Bash 的 `/c/Users/x`。
#[cfg(windows)]
fn looks_like_windows_absolute(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    if raw.starts_with(r"\\") {
        return true;
    }
    bytes.len() >= 3 && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/')
}

/// AgentHub 自身数据目录（索引库、备份、日志、配置）。
pub fn data_root() -> PathBuf {
    if let Some(v) = DATA_OVERRIDE.get() {
        return v.clone();
    }
    if let Ok(v) = std::env::var("AGENTHUB_DATA_DIR")
        && !v.trim().is_empty()
    {
        return PathBuf::from(v);
    }
    home_dir().join(".agenthub")
}

pub fn default_store_root() -> PathBuf {
    data_root().join("store")
}

pub fn backups_root() -> PathBuf {
    data_root().join("backups")
}

pub fn logs_root() -> PathBuf {
    data_root().join("logs")
}

/// 终端会话日志：`logs/terminal/<id>.log` + `index.json`。
///
/// 输出可能含密钥，所以这里只是目录约定；是否记录由前端开关决定，
/// 保留上限（20 个 / 7 天 / 单文件 2MB）见 `terminal::history`。
pub fn terminal_logs_root() -> PathBuf {
    logs_root().join("terminal")
}

/// 技能市场下载缓存：`~/.agenthub/market/cache/<技能名>-<批次>/`。
///
/// 放在数据根下（受管根之一），下载物才能被 `MoveIntoStore` 移动进仓库；
/// 安装计划应用后由 `market::install::discard` 清掉。
pub fn market_cache_root() -> PathBuf {
    data_root().join("market").join("cache")
}

pub fn db_path() -> PathBuf {
    data_root().join("index.db")
}

pub fn config_path() -> PathBuf {
    data_root().join("config.json")
}

/// 支持 `~`、`~/x`、`~\x`、`$HOME/x`、`${HOME}/x`，Windows 额外支持开头的 `%VAR%`。
///
/// 变量不存在时原样返回，由调用方按「路径不存在」报错，不做静默猜测。
pub fn expand(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return PathBuf::new();
    }
    if trimmed == "~" {
        return home_dir();
    }
    if let Some(rest) = trimmed
        .strip_prefix("~/")
        .or_else(|| trimmed.strip_prefix("~\\"))
    {
        return join_rel(&home_dir(), rest);
    }
    if let Some(rest) = trimmed
        .strip_prefix("$HOME/")
        .or_else(|| trimmed.strip_prefix("${HOME}/"))
    {
        return join_rel(&home_dir(), rest);
    }
    #[cfg(windows)]
    if let Some(expanded) = expand_env_prefix(trimmed) {
        return expanded;
    }
    PathBuf::from(trimmed)
}

/// 把 `/` 或 `\` 分隔的相对路径按组件拼到 `base` 之后。
///
/// `Path::join("a/b")` 会把 `a/b` 当成单个组件保留，Windows 上就出现
/// `C:\Users\x/.claude/skills` 这种混合分隔符；这里逐段 push 让 `PathBuf` 自己加分隔符。
pub fn join_rel(base: &Path, rel: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    for part in rel.split(['/', '\\']).filter(|s| !s.is_empty()) {
        out.push(part);
    }
    out
}

/// 展开开头的 `%VAR%`（Windows 习惯写法）；变量缺失或格式不对时返回 None。
#[cfg(windows)]
fn expand_env_prefix(raw: &str) -> Option<PathBuf> {
    let rest = raw.strip_prefix('%')?;
    let close = rest.find('%')?;
    let (var, tail) = (&rest[..close], &rest[close + 1..]);
    if var.is_empty() {
        return None;
    }
    let value = std::env::var(var).ok()?;
    let tail = tail.trim_start_matches(['\\', '/']);
    Some(if tail.is_empty() {
        PathBuf::from(value)
    } else {
        join_rel(Path::new(&value), tail)
    })
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(platform::long_path(path)).map_err(|e| Error::io(path, e))
}

pub fn path_str(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

/// 纯词法规范化：处理 `.`/`..`，不触碰文件系统（目标可能尚不存在）。
pub fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                let popped = out.pop();
                if !popped {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// 路径的等价比较键：词法规范化 + 分隔符统一为 `/`；Windows 额外做大小写折叠。
///
/// 用作 SQLite 主键与「是不是同一个路径」的唯一判据——Windows 文件系统大小写
/// 不敏感，直接比 `to_string_lossy()` 会产生同一目录两条记录。
pub fn path_key(path: &Path) -> String {
    let key = normalize_lexical(path).to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key
    }
}

/// 两个路径是否等价（按 [`path_key`] 的语义）。
pub fn paths_eq(a: &Path, b: &Path) -> bool {
    path_key(a) == path_key(b)
}

/// target 是否位于 root 之内（含相等）。
pub fn is_within(root: &Path, target: &Path) -> bool {
    let root_key = path_key(root);
    if root_key.is_empty() {
        // 空 root（`.` 或纯相对路径的退化形态）视为容纳一切，与旧行为一致
        return true;
    }
    let target_key = path_key(target);
    if target_key == root_key {
        return true;
    }
    let mut prefix = root_key;
    if !prefix.ends_with('/') {
        prefix.push('/');
    }
    target_key.starts_with(&prefix)
}

/// 所有写操作必须先过这里：目标必须落在某个受管根内。
pub fn ensure_within(roots: &[PathBuf], target: &Path) -> Result<()> {
    let target = normalize_lexical(target);
    if roots.iter().any(|r| is_within(r, &target)) {
        return Ok(());
    }
    Err(Error::UnsafePath(t!(
        Msg::ErrPathOutsideRoots,
        path = target.display(),
        roots = roots
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

/// 目录占用（字节数, 文件数），跳过符号链接避免死循环。
pub fn dir_usage(root: &Path) -> (u64, usize) {
    let mut bytes = 0u64;
    let mut files = 0usize;
    let walker = walkdir::WalkDir::new(platform::long_path(root))
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok());
    for entry in walker {
        if entry.file_type().is_file() {
            files += 1;
            if let Ok(meta) = entry.metadata() {
                bytes += meta.len();
            }
        }
    }
    (bytes, files)
}

pub fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn ts_label() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

static SEQ: AtomicU64 = AtomicU64::new(0);

/// 秒级时间戳 + 毫秒 + 进程内自增序号：同一秒内多次生成也不会撞车。
pub fn unique_id() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_millis())
        .unwrap_or(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!(
        "{:04}-{:03}-{:03}",
        std::process::id() % 10000,
        millis,
        seq % 1000
    )
}

pub fn fmt_ts(ts: i64) -> String {
    match chrono::DateTime::from_timestamp(ts, 0) {
        Some(dt) => dt
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        None => ts.to_string(),
    }
}

/// 同目录临时文件 + fsync + rename，保证目标文件不会出现半写状态。
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::UnsafePath(t!(Msg::ErrPathNoParent, path = path.display())))?;
    ensure_dir(parent)?;
    let long_parent = platform::long_path(parent);
    let mut tmp =
        tempfile::NamedTempFile::new_in(&long_parent).map_err(|e| Error::io(parent, e))?;
    tmp.write_all(bytes).map_err(|e| Error::io(path, e))?;
    tmp.as_file().sync_all().map_err(|e| Error::io(path, e))?;
    replace_file(tmp.path(), path)
}

/// rename 覆盖；Windows 上目标存在时 rename 会失败，先删再改名。
pub fn replace_file(tmp: &Path, path: &Path) -> Result<()> {
    let from = platform::long_path(tmp);
    let to = platform::long_path(path);
    match fs::rename(&from, &to) {
        Ok(()) => Ok(()),
        Err(first) => {
            if to.exists() {
                fs::remove_file(&to).map_err(|e| Error::io(path, e))?;
                fs::rename(&from, &to).map_err(|e| Error::io(path, e))
            } else {
                Err(Error::io(path, first))
            }
        }
    }
}

pub fn read_to_string_limited(path: &Path, max_bytes: usize) -> Result<String> {
    let long = platform::long_path(path);
    let meta = fs::metadata(&long).map_err(|e| Error::io(path, e))?;
    if meta.len() as usize > max_bytes {
        return Err(Error::Other(t!(
            Msg::ErrFileTooLarge,
            path = path.display(),
            limit = max_bytes
        )));
    }
    let bytes = fs::read(&long).map_err(|e| Error::io(path, e))?;
    String::from_utf8(bytes).map_err(|_| Error::Other(t!(Msg::ErrNotUtf8, path = path.display())))
}
