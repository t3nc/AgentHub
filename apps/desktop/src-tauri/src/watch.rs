//! 壳层的文件监听：把「外部工具动了 agent 目录 / 中央仓库 / 配置」翻译成一条前端事件。
//!
//! 只做非递归监听 + 去抖，回调里再用 `agenthub_core::watch` 的快照做一次差异判定，
//! 避免临时文件、自身写入造成的抖动被当成「用户变更」。
//!
//! 监听目标带白名单（见 `agenthub_core::watch::WatchRoot`）：技能目录看全部一级子项，
//! agent 配置目录只看真正的配置文件——否则 CLI 常驻进程写的 `history.jsonl` /
//! `*.sqlite-wal` 会让「外部有变更」提示永远消不掉。

use std::time::Duration;

use agenthub_core::watch::{self, WatchRoot};
use notify_debouncer_full::new_debouncer;
use notify_debouncer_full::notify::RecursiveMode;
use tauri::{AppHandle, Emitter};

/// 前端订阅的事件名。
pub const EVENT: &str = "hub://fs-changed";

/// 去抖窗口：编辑器保存常常连续触发多次事件。
const DEBOUNCE: Duration = Duration::from_millis(400);

pub fn spawn(handle: AppHandle, roots: Vec<WatchRoot>) {
    if roots.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut debouncer = match new_debouncer(DEBOUNCE, None, tx) {
            Ok(debouncer) => debouncer,
            Err(err) => {
                eprintln!("[watch] 初始化失败，将只能手动重新扫描: {err}");
                return;
            }
        };

        let mut watched = 0usize;
        for root in &roots {
            match debouncer.watch(&root.path, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    watched += 1;
                    // 只打初始化日志：逐事件打印会被 CLI 的写入刷爆
                    match &root.only {
                        Some(names) => {
                            eprintln!("[watch] {} · whitelist({names:?})", root.path.display())
                        }
                        None => eprintln!("[watch] {} · all", root.path.display()),
                    }
                }
                // 网络盘 / 权限不足：记一条日志，不影响其它目录
                Err(err) => eprintln!("[watch] 无法监听 {}: {err}", root.path.display()),
            }
        }
        if watched == 0 {
            return;
        }

        let mut prev = watch::snapshot_roots(&roots);
        for result in rx {
            // 单个事件批量出错时跳过，等下一次
            if result.is_err() {
                continue;
            }
            let next = watch::snapshot_roots(&roots);
            let delta = watch::diff(&prev, &next);
            if delta.is_empty() {
                continue;
            }
            if handle.emit(EVENT, &delta).is_err() {
                break;
            }
            prev = next;
        }
    });
}
