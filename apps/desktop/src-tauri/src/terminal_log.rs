//! 终端输出的落盘线程。
//!
//! 读线程每拿到一块 PTY 输出就 `emit` 给前端，同时把同一份数据投进这里；
//! 真的写盘在本线程里做完，保证「写磁盘慢」拖不住终端渲染。
//!
//! 三个硬约束：
//! - 单文件超过 `MAX_LOG_BYTES` 就停止追加（并写一行截断标记）；
//! - 每 200ms 或攒够 32KB flush 一次，进程被强杀时最多丢 200ms 的输出；
//! - 会话关闭时 sender 被丢弃，线程 flush 后自然退出。

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use agenthub_core::terminal::history::MAX_LOG_BYTES;

/// 攒够这么多就先落盘
const FLUSH_BYTES: u64 = 32 * 1024;
/// 没有新数据也要定期 flush，否则长时间静默的会话会丢最后一段
const FLUSH_INTERVAL: Duration = Duration::from_millis(200);

const TRUNCATED_NOTE: &[u8] =
    b"\r\n[AgentHub] \xe8\xbe\x93\xe5\x87\xba\xe8\xb6\x85\xe8\xbf\x87 2MB\xef\xbc\x8c\xe5\xb7\xb2\xe5\x81\x9c\xe6\xad\xa2\xe8\xae\xb0\xe5\xbd\x95\r\n";

/// 写端句柄。`bytes` / `truncated` 让索引能如实反映文件状态。
///
/// 可 Clone：会话表持一份（关标签时读字节数），读线程持一份（持续追加）。
#[derive(Clone)]
pub struct LogHandle {
    tx: Sender<Vec<u8>>,
    bytes: Arc<AtomicU64>,
    truncated: Arc<AtomicBool>,
}

impl LogHandle {
    /// 建日志文件并起写线程。路径不可写时返回 None——不能因为记日志失败就开不了终端。
    ///
    /// `header` 会立刻落盘：日志开头写明这是哪次会话，也让「文件是空」这种故障
    /// 一眼能分辨是「没建起来」还是「没有输出」。
    pub fn spawn(path: PathBuf, header: &str) -> Option<Self> {
        if let Some(parent) = path.parent()
            && agenthub_core::paths::ensure_dir(parent).is_err()
        {
            return None;
        }
        let mut file = File::create(&path).ok()?;
        let _ = file.write_all(header.as_bytes());
        let _ = file.flush();

        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        let bytes = Arc::new(AtomicU64::new(header.len() as u64));
        let truncated = Arc::new(AtomicBool::new(false));

        let thread_bytes = Arc::clone(&bytes);
        let thread_truncated = Arc::clone(&truncated);
        std::thread::spawn(move || {
            run(file, rx, thread_bytes, thread_truncated);
        });

        Some(Self {
            tx,
            bytes,
            truncated,
        })
    }

    pub fn write(&self, data: Vec<u8>) {
        // 写线程已退出（超限或磁盘出错）时 send 会失败，这里静默即可
        let _ = self.tx.send(data);
    }

    pub fn bytes(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }

    pub fn truncated(&self) -> bool {
        self.truncated.load(Ordering::Relaxed)
    }
}

fn run(file: File, rx: Receiver<Vec<u8>>, bytes: Arc<AtomicU64>, truncated: Arc<AtomicBool>) {
    let mut writer = BufWriter::with_capacity(64 * 1024, file);
    let mut unflushed: u64 = 0;
    let mut last_flush = std::time::Instant::now();

    loop {
        match rx.recv_timeout(FLUSH_INTERVAL) {
            Ok(chunk) => {
                let written = bytes.load(Ordering::Relaxed);
                if written >= MAX_LOG_BYTES {
                    break;
                }
                if writer.write_all(&chunk).is_err() {
                    break;
                }
                let added = chunk.len() as u64;
                bytes.fetch_add(added, Ordering::Relaxed);
                unflushed += added;

                if bytes.load(Ordering::Relaxed) >= MAX_LOG_BYTES {
                    let _ = writer.write_all(TRUNCATED_NOTE);
                    truncated.store(true, Ordering::Relaxed);
                    let _ = writer.flush();
                    break;
                }
                // 时间到了就落一次盘：TUI / 计时提示符会持续吐字节，
                // 只按「静默 200ms」判断的话缓冲可能一直不刷
                if unflushed >= FLUSH_BYTES || last_flush.elapsed() >= FLUSH_INTERVAL {
                    if writer.flush().is_err() {
                        break;
                    }
                    unflushed = 0;
                    last_flush = std::time::Instant::now();
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if unflushed > 0 && writer.flush().is_err() {
                    break;
                }
                unflushed = 0;
                last_flush = std::time::Instant::now();
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    let _ = writer.flush();
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    fn write_all(handle: &LogHandle, chunks: &[&[u8]]) {
        for chunk in chunks {
            handle.write(chunk.to_vec());
        }
    }

    fn drain(path: PathBuf, handle: LogHandle) -> String {
        drop(handle);
        // 给写线程一点时间把缓冲刷完
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(20));
            if let Ok(text) = std::fs::read_to_string(&path)
                && !text.is_empty()
            {
                return text;
            }
        }
        let mut text = String::new();
        File::open(&path)
            .and_then(|mut file| file.read_to_string(&mut text))
            .ok();
        text
    }

    #[test]
    fn writes_and_flushes_on_close() {
        let dir = std::env::temp_dir().join(format!("agenthub-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let path = dir.join("basic.log");
        let handle = LogHandle::spawn(path.clone(), "# header\n").expect("建日志");
        write_all(&handle, &[b"hello ", b"world"]);
        std::thread::sleep(Duration::from_millis(300));
        let text = drain(path.clone(), handle);
        assert!(text.starts_with("# header"), "头部应立刻落盘：{text:?}");
        assert!(text.ends_with("hello world"), "正文应完整：{text:?}");
        assert!(handle_bytes(&path) > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn handle_bytes(path: &PathBuf) -> u64 {
        std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
    }
}
