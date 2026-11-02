//! 内嵌终端：一端是 PTY 里的 shell / agent CLI，另一端是前端 xterm.js。
//!
//! 壳层只做搬运工：
//! - `terminal_open` 起进程（Windows 走 ConPTY，mac/Linux 走 forkpty）；
//! - `terminal_write` 把键盘输入写回去，`terminal_resize` 同步窗口尺寸；
//! - 进程输出通过 `terminal://output` 事件推送，退出通过 `terminal://exit`。
//!
//! 这里**不做命令白名单**：用户点的是自己的终端，跑什么由用户决定。
//! 但仍不碰受管目录的写入门禁——终端里的写操作就是用户自己的写操作。

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use agenthub_core::paths;
use agenthub_core::terminal::history::{self, SessionMeta};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::terminal_log::LogHandle;

/// 前端订阅：终端输出（可高频）。
pub const EVENT_OUTPUT: &str = "terminal://output";
/// 前端订阅：PTY 进程结束。
pub const EVENT_EXIT: &str = "terminal://exit";

/// 单次读取的缓冲大小：足够吞掉 TUI 的一次整屏重绘。
const READ_BUF: usize = 16 * 1024;
/// 尺寸上限，挡住前端算出的异常值。
const MAX_DIM: u16 = 500;

struct Session {
    /// 键盘输入交给每个会话自己的写线程，避免持锁等一个可能被卡住的 write
    input: std::sync::mpsc::Sender<Vec<u8>>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    /// 会话输出落盘句柄；用户关掉「记录终端输出」时为 None
    log: Option<LogHandle>,
}

/// index.json 是所有会话共用的：两个标签同时退出时不能互相覆盖。
static INDEX_LOCK: Mutex<()> = Mutex::new(());

/// 新会话入索引，顺手做一次保留策略（不做定时清理，开终端时清就够了）。
fn register_session(meta: SessionMeta) {
    let _guard = INDEX_LOCK.lock();
    let mut entries = history::load();
    history::upsert(&mut entries, meta);
    let (kept, dropped) = history::prune(entries, paths::now_ts());
    for id in dropped {
        history::remove_log(&id);
    }
    let _ = history::save(&kept);
}

/// 更新已存在条目的状态与体积；条目不存在（比如上次关掉了记录）就什么都不做。
fn update_session(id: &str, status: Option<&str>, log: Option<&LogHandle>) {
    let _guard = INDEX_LOCK.lock();
    let mut entries = history::load();
    let Some(slot) = entries.iter_mut().find(|entry| entry.id == id) else {
        return;
    };
    if let Some(status) = status {
        slot.status = status.to_string();
    }
    if let Some(log) = log {
        slot.bytes = log.bytes();
        slot.truncated = log.truncated();
    }
    let _ = history::save(&entries);
}

/// 会话表：key 是前端生成的会话 id（每个终端标签一个）。
#[derive(Default)]
pub struct TerminalState(Mutex<HashMap<String, Session>>);

impl Drop for TerminalState {
    fn drop(&mut self) {
        // 应用退出时别把 shell 留成孤儿进程
        if let Ok(mut map) = self.0.lock() {
            for (_, mut session) in map.drain() {
                let _ = session.child.kill();
            }
        }
    }
}

#[derive(Serialize)]
pub struct TerminalInfo {
    /// 默认 shell 的可执行文件（Windows 是完整路径或程序名）
    pub shell: String,
    /// 展示用名字：PowerShell 7 / zsh …
    pub shell_label: String,
    /// 新终端的默认工作目录
    pub home: String,
    pub platform: String,
    /// 会话日志目录：页脚要如实告诉用户输出落在哪
    pub logs_dir: String,
}

#[derive(Clone, Serialize)]
struct OutputPayload {
    id: String,
    data: String,
}

#[derive(Clone, Serialize)]
struct ExitPayload {
    id: String,
}

fn flat(err: impl std::fmt::Display) -> String {
    err.to_string()
}

fn is_powershell(program: &str) -> bool {
    let name = PathBuf::from(program)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    name == "powershell" || name == "pwsh"
}

fn default_shell() -> (String, String) {
    #[cfg(windows)]
    {
        // PowerShell 7 比 Windows PowerShell 5.1 好用得多；没有就退回系统自带
        if let Some(path) = agenthub_core::adapters::which("pwsh") {
            return (paths::path_str(&path), "PowerShell 7".to_string());
        }
        if let Some(path) = agenthub_core::adapters::which("powershell") {
            return (paths::path_str(&path), "Windows PowerShell".to_string());
        }
        let fallback = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
        (fallback.to_string(), "Windows PowerShell".to_string())
    }
    #[cfg(not(windows))]
    {
        let shell = std::env::var("SHELL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "/bin/zsh".to_string());
        let label = PathBuf::from(&shell)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| shell.clone());
        (shell, label)
    }
}

#[tauri::command]
pub fn terminal_info() -> TerminalInfo {
    let (shell, shell_label) = default_shell();
    TerminalInfo {
        shell,
        shell_label,
        home: paths::path_str(&paths::home_dir()),
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        logs_dir: paths::path_str(&paths::terminal_logs_root()),
    }
}

/// 工作目录：空 / 未给 → 用户主目录；给了就必须是已存在的目录，避免 PTY 起在意外位置。
fn resolve_cwd(cwd: Option<String>) -> Result<PathBuf, String> {
    let Some(raw) = cwd else {
        return Ok(paths::home_dir());
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(paths::home_dir());
    }
    let path = paths::expand(trimmed);
    if !path.is_dir() {
        return Err(format!("工作目录不存在或不是目录：{}", path.display()));
    }
    Ok(path)
}

fn pty_size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.clamp(1, MAX_DIM),
        cols: cols.clamp(1, MAX_DIM),
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// 增量 UTF-8 解码：一次 read 可能把多字节字符劈成两半，
/// 直接把每块当字符串会让中文 / emoji 变成替换符。
fn take_text(pending: &mut Vec<u8>) -> String {
    if pending.is_empty() {
        return String::new();
    }
    match std::str::from_utf8(pending) {
        Ok(text) => {
            let out = text.to_string();
            pending.clear();
            out
        }
        Err(err) => {
            let valid = err.valid_up_to();
            let out = String::from_utf8_lossy(&pending[..valid]).to_string();
            match err.error_len() {
                // 尾部是半个字符：留着等下一块
                None => pending.drain(..valid),
                // 中间有非法字节：连同坏字节一起丢掉，避免卡死
                Some(len) => pending.drain(..valid + len),
            };
            out
        }
    }
}

/// `terminal_open` 的入参。合成一个结构体而不是 8 个裸参数，前端也只需要拼一次。
#[derive(Deserialize)]
pub struct OpenTerminal {
    pub id: String,
    /// 空 / null = 用户主目录
    pub cwd: Option<String>,
    /// 空 / null = 平台默认 shell
    pub shell: Option<String>,
    pub cols: u16,
    pub rows: u16,
    /// 起 shell 后自动敲的一行（agent CLI）
    pub command: Option<String>,
    /// 标签标题，只用于会话索引
    #[serde(default)]
    pub title: Option<String>,
    /// 标签对应的 agent 图标 id，只用于会话索引
    #[serde(default)]
    pub agent: Option<String>,
    /// 是否把输出落盘（用户的「记录终端输出」开关）
    #[serde(default)]
    pub record: bool,
}

/// 一次已经起来的 PTY + shell。`terminal_open`（真实标签）与单测都走 [`spawn_shell`]，
/// 保证「登录 shell / TERM / COLORTERM / 初始尺寸」四件事只有一份实现。
struct Spawned {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
}

/// 按平台约定起 shell：Windows 不传 `-l`（PowerShell 不认），mac/Linux 用登录 shell。
fn spawn_shell(
    program: &str,
    cwd: &std::path::Path,
    cols: u16,
    rows: u16,
) -> std::result::Result<Spawned, String> {
    let mut cmd = CommandBuilder::new(program);
    if is_powershell(program) {
        cmd.arg("-NoLogo");
    }
    #[cfg(not(windows))]
    cmd.arg("-l");
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.cwd(cwd);

    let pair = native_pty_system()
        .openpty(pty_size(cols, rows))
        .map_err(|err| format!("无法创建 PTY：{err}"))?;
    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|err| format!("无法启动 {program}：{err}"))?;
    // slave 必须尽早丢掉，否则 Reader 永远等不到 EOF
    drop(pair.slave);

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|err| format!("无法读取 PTY：{err}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|err| format!("无法写入 PTY：{err}"))?;
    Ok(Spawned {
        master: pair.master,
        child,
        reader,
        writer,
    })
}

#[tauri::command]
pub fn terminal_open(
    app: AppHandle,
    state: State<'_, TerminalState>,
    request: OpenTerminal,
) -> Result<(), String> {
    let OpenTerminal {
        id,
        cwd,
        shell,
        cols,
        rows,
        command,
        title,
        agent,
        record,
    } = request;

    let mut map = state.0.lock().map_err(flat)?;
    if map.contains_key(&id) {
        return Err(format!("终端会话已存在：{id}"));
    }

    let program = shell
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_shell().0);
    let cwd = resolve_cwd(cwd)?;
    // 索引里要存字符串；cwd 随后会被 cmd 吃掉
    let cwd_text = paths::path_str(&cwd);
    let command_meta = command.clone();

    let Spawned {
        master,
        child,
        mut reader,
        mut writer,
    } = spawn_shell(&program, &cwd, cols, rows)?;

    // 键盘输入不进命令线程：某个 CLI 不读 stdin 把 PTY 缓冲写满时，
    // 只会卡住它自己的写线程，不会连带冻住其它标签和 IPC。
    let (input, pending_input) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        for chunk in pending_input {
            if writer.write_all(&chunk).is_err() {
                break;
            }
            let _ = writer.flush();
        }
    });

    // agent CLI 的启动命令当成「在 shell 里敲一行」：进程退出后标签还能继续用
    if let Some(line) = command.map(|value| value.trim().to_string())
        && !line.is_empty()
    {
        let mut bytes = line.into_bytes();
        bytes.push(b'\r');
        input
            .send(bytes)
            .map_err(|_| "写入启动命令失败：写端已关闭".to_string())?;
    }

    // 输出落盘：真正的写盘在独立线程里，这里只建句柄
    let log = if record {
        let header = format!(
            "# AgentHub 终端会话 · {} · {}\n",
            paths::ts_label(),
            cwd_text
        );
        LogHandle::spawn(history::log_path(&id), &header)
    } else {
        None
    };

    map.insert(
        id.clone(),
        Session {
            input,
            master,
            child,
            log: log.clone(),
        },
    );
    drop(map);

    // 索引写盘放在 map 锁之外：它要走一次 read + write_atomic，别挡着其它标签
    if let Some(handle) = &log {
        register_session(SessionMeta {
            id: id.clone(),
            title: title.unwrap_or_else(|| "终端".to_string()),
            command: command_meta,
            cwd: cwd_text,
            agent,
            started_at: paths::now_ts(),
            status: "running".to_string(),
            bytes: handle.bytes(),
            truncated: false,
        });
    }

    let app = app.clone();
    let reader_log = log;
    std::thread::spawn(move || {
        let mut pending: Vec<u8> = Vec::new();
        let mut buf = vec![0u8; READ_BUF];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    pending.extend_from_slice(&buf[..n]);
                    let data = take_text(&mut pending);
                    if data.is_empty() {
                        continue;
                    }
                    if let Some(log) = &reader_log {
                        log.write(data.as_bytes().to_vec());
                    }
                    if app
                        .emit(
                            EVENT_OUTPUT,
                            OutputPayload {
                                id: id.clone(),
                                data,
                            },
                        )
                        .is_err()
                    {
                        break;
                    }
                }
                // Windows 上关闭 ConPTY 会返回 ERROR_BROKEN_PIPE，当作正常退出
                Err(_) => break,
            }
        }
        if !pending.is_empty() {
            let data = String::from_utf8_lossy(&pending).to_string();
            if let Some(log) = &reader_log {
                log.write(data.as_bytes().to_vec());
            }
            let _ = app.emit(
                EVENT_OUTPUT,
                OutputPayload {
                    id: id.clone(),
                    data,
                },
            );
        }
        // 进程结束：状态与体积回写索引（此时日志线程可能还在 flush，字节数会略滞后）
        if let Some(log) = &reader_log {
            update_session(&id, Some("exited"), Some(log));
        }
        let _ = app.emit(EVENT_EXIT, ExitPayload { id });
    });

    Ok(())
}

#[tauri::command]
pub fn terminal_write(
    state: State<'_, TerminalState>,
    id: String,
    data: String,
) -> Result<(), String> {
    let map = state.0.lock().map_err(flat)?;
    let session = map
        .get(&id)
        .ok_or_else(|| format!("终端会话不存在：{id}"))?;
    session
        .input
        .send(data.into_bytes())
        .map_err(|_| format!("终端写端已关闭：{id}"))
}

#[tauri::command]
pub fn terminal_resize(
    state: State<'_, TerminalState>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let map = state.0.lock().map_err(flat)?;
    let session = map
        .get(&id)
        .ok_or_else(|| format!("终端会话不存在：{id}"))?;
    session
        .master
        .resize(pty_size(cols, rows))
        .map_err(|err| format!("调整窗口大小失败：{err}"))
}

#[tauri::command]
pub fn terminal_close(state: State<'_, TerminalState>, id: String) -> Result<(), String> {
    // 只在移除这一步持锁：杀进程与写索引都不该挡住别的标签
    let removed = {
        let mut map = state.0.lock().map_err(flat)?;
        map.remove(&id)
    };
    if let Some(mut session) = removed {
        let _ = session.child.kill();
        update_session(&id, Some("exited"), session.log.as_ref());
    }
    Ok(())
}

/// 上次会话列表（新的在前）。只恢复「在这干过什么」，不恢复进程。
///
/// 顺手做两件收尾：
/// - 索引里还没有字节数（每次 flush 都回写索引不划算），这里用文件大小补；
/// - 应用被强杀过的会话会一直挂着 `running`，这里按「不在当前会话表里」纠正成 `exited`。
#[tauri::command]
pub fn terminal_history(state: State<'_, TerminalState>) -> Vec<SessionMeta> {
    let active: std::collections::HashSet<String> = state
        .0
        .lock()
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default();

    let mut entries = history::load();
    let mut dirty = false;
    for entry in &mut entries {
        if let Ok(meta) = std::fs::metadata(history::log_path(&entry.id))
            && meta.len() > entry.bytes
        {
            entry.bytes = meta.len();
            dirty = true;
        }
        if entry.status == "running" && !active.contains(&entry.id) {
            entry.status = "exited".to_string();
            dirty = true;
        }
    }
    if dirty && let Ok(_guard) = INDEX_LOCK.lock() {
        let _ = history::save(&entries);
    }
    entries
}

/// 导出当前标签的纯文本输出，返回落盘路径。
#[tauri::command]
pub fn terminal_export(text: String, title: String) -> Result<String, String> {
    let name = format!(
        "export-{}-{}-{}.txt",
        paths::ts_label(),
        paths::unique_id(),
        history::sanitize(&title)
    );
    let path = history::dir().join(name);
    paths::write_atomic(&path, text.as_bytes()).map_err(flat)?;
    Ok(paths::path_str(&path))
}

/// 选区落成 md 文件，供「交给 agent 分析」引用路径（不把长文本拼进命令行）。
#[tauri::command]
pub fn terminal_save_selection(text: String) -> Result<String, String> {
    let path = history::dir().join(format!(
        "selection-{}-{}.md",
        paths::ts_label(),
        paths::unique_id()
    ));
    paths::write_atomic(&path, text.as_bytes()).map_err(flat)?;
    Ok(paths::path_str(&path))
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::time::Duration;

    use super::*;

    /// 冒烟测试：真的开一个 PTY 起 shell、把一行命令写进去、再从读端收回显。
    /// 走的就是 `terminal_open` / `terminal_write` 的同一条路径（去掉 Tauri 事件层）。
    ///
    /// 特意不用 `cmd /C echo` 这种「起进程立刻退出」的写法：ConPTY 在进程
    /// 瞬间结束时可能来不及把首屏刷进管道，测出来是假阴性。
    #[test]
    fn pty_roundtrip_returns_output() {
        let pair = native_pty_system()
            .openpty(pty_size(80, 24))
            .expect("openpty 失败");
        let cmd = if cfg!(windows) {
            CommandBuilder::new("cmd.exe")
        } else {
            CommandBuilder::new("/bin/sh")
        };

        let mut child = pair.slave.spawn_command(cmd).expect("spawn 失败");
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().expect("读端获取失败");
        let mut writer = pair.master.take_writer().expect("写端获取失败");

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        writer
            .write_all(b"echo agenthub-pty\r")
            .expect("写命令失败");
        writer.flush().ok();

        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        let mut acc: Vec<u8> = Vec::new();
        while std::time::Instant::now() < deadline {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match rx.recv_timeout(left) {
                Ok(chunk) => {
                    // ConPTY 启动时先发 DSR（ESC[6n）问光标位置，不回它就刷不出首屏。
                    // 真实界面里这一步由 xterm.js 自动应答，测试里手动补上。
                    if chunk.windows(4).any(|window| window == b"\x1b[6n") {
                        writer.write_all(b"\x1b[1;1R").ok();
                        writer.flush().ok();
                    }
                    acc.extend_from_slice(&chunk);
                    if acc.windows(12).any(|window| window == b"agenthub-pty") {
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        let text = String::from_utf8_lossy(&acc);
        assert!(text.contains("agenthub-pty"), "实际输出: {text:?}");
        let _ = child.kill();
    }

    #[test]
    fn splits_multibyte_across_chunks() {
        let mut pending = Vec::new();
        // 「终」= E7 BB 88，先喂前两个字节
        pending.extend_from_slice(&[0xE7, 0xBB]);
        assert_eq!(take_text(&mut pending), "");
        assert_eq!(pending.len(), 2);

        pending.push(0x88);
        assert_eq!(take_text(&mut pending), "终");
        assert!(pending.is_empty());
    }

    #[test]
    fn drops_invalid_bytes_and_keeps_going() {
        let mut pending = vec![b'a', 0xFF, b'b'];
        assert_eq!(take_text(&mut pending), "a");
        assert_eq!(take_text(&mut pending), "b");
        assert!(pending.is_empty());
    }

    #[test]
    fn plain_ascii_passes_through() {
        let mut pending = b"hello".to_vec();
        assert_eq!(take_text(&mut pending), "hello");
        assert!(pending.is_empty());
    }

    /// 关闭标签必须真的把 shell 收掉。
    ///
    /// `terminal_close` 做两件事：把 `Session` 从表里移除（丢掉 `master` = 关掉 PTY）再
    /// `child.kill()`。这里按同样的顺序做一遍，断言进程在 5 秒内退出且不是僵尸。
    #[cfg(not(windows))]
    #[test]
    fn closing_session_reclaims_the_shell_process() {
        let (program, _) = default_shell();
        let cwd = std::env::temp_dir();
        let Spawned {
            master, mut child, ..
        } = spawn_shell(&program, &cwd, 80, 24).expect("起默认 shell 失败");
        let pid = child.process_id().expect("应该能拿到 shell 的 pid");

        drop(master);
        let _ = child.kill();

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                _ => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "关闭标签后 5 秒内进程没退出（pid {pid}）"
                    );
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }

        // 已经 wait 过就不是僵尸；再用 ps 确认进程真的不在了
        let status = std::process::Command::new("ps")
            .args(["-p", &pid.to_string()])
            .status()
            .expect("调用 ps 失败");
        assert!(!status.success(), "pid {pid} 仍然存在");
    }

    /// 默认 shell 与真实标签同一条路径：`$SHELL`、`TERM` / `COLORTERM`、初始 PTY 尺寸。
    ///
    /// 2026-11-02 macOS 真机用例。以前的 PTY 冒烟测试自己写死 `/bin/sh`，与
    /// `terminal_open` 的实现是两条路——把环境变量或 `-l` 改坏了也测不出来。
    #[cfg(not(windows))]
    #[test]
    fn default_shell_gets_term_and_size() {
        let (program, label) = default_shell();
        assert!(
            std::path::Path::new(&program).exists(),
            "默认 shell 不存在：{program}"
        );
        let expected = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        if std::path::Path::new(&expected).exists() && !expected.trim().is_empty() {
            assert_eq!(program, expected, "$SHELL 存在时必须用它");
        }
        assert!(!label.trim().is_empty(), "shell 标签不能为空");

        let cwd = std::env::temp_dir();
        // `_master` 必须活到用例结束：丢掉它等于关掉 PTY，读端会立刻 EOF
        let Spawned {
            master: _master,
            mut child,
            mut reader,
            mut writer,
        } = spawn_shell(&program, &cwd, 80, 24).expect("起默认 shell 失败");

        // 一次问三件事：环境变量两个 + 当前 PTY 尺寸
        const PROBE: &[u8] = b"echo AGENTHUB_ENV=$TERM/$COLORTERM; stty size; echo AGENTHUB_DONE\r";

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        let mut acc: Vec<u8> = Vec::new();
        let mut next_probe = std::time::Instant::now();
        // 用户的 `.zshrc` 可能有 instant-prompt / 主题重绘，首屏要等它画完才吃键盘输入：
        // 没看到标记就每 800ms 重发一次探测命令，避免和主题的初始化时序耦合。
        while std::time::Instant::now() < deadline {
            if std::time::Instant::now() >= next_probe {
                writer.write_all(PROBE).ok();
                writer.flush().ok();
                next_probe = std::time::Instant::now() + Duration::from_millis(800);
            }
            match rx.recv_timeout(Duration::from_millis(120)) {
                Ok(chunk) => {
                    // 主题/instant-prompt 会发 DSR（ESC[6n）问光标位置，不回它就继续等
                    if chunk.windows(4).any(|window| window == b"\x1b[6n") {
                        writer.write_all(b"\x1b[1;1R").ok();
                        writer.flush().ok();
                    }
                    acc.extend_from_slice(&chunk);
                    let text = String::from_utf8_lossy(&acc);
                    // 注意不能用 `AGENTHUB_ENV=` 当标记：终端会把我们敲的命令原样回显，
                    // 那一行本身就带着这两个词，只会立刻「命中」。
                    if text.contains("AGENTHUB_ENV=xterm-256color/truecolor")
                        && text.contains("24 80")
                    {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        let text = String::from_utf8_lossy(&acc);
        assert!(
            text.contains("AGENTHUB_ENV=xterm-256color/truecolor"),
            "TERM / COLORTERM 没传进去：{text:?}"
        );
        assert!(
            text.contains("24 80"),
            "初始 PTY 尺寸没生效（期望 `stty size` 输出 24 80）：{text:?}"
        );
        let _ = child.kill();
    }
}
