//! 壳层的网关生命周期与请求日志。
//!
//! 这一层只做三件事：启停、热重载、把网关推来的元数据攒进环形缓冲。
//! **不碰 DB、不碰 `AgentHub` 的锁** —— 请求热路径上任何一次加锁都会变成吞吐上限。
//!
//! 请求日志刻意只放内存（默认 500 条）：正文可能含代码与密钥，不落盘是安全边界的一部分；
//! 重启后清空属于预期行为。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use agenthub_core::config::AppConfig;
use agenthub_gateway::mpsc::{self, UnboundedReceiver};
use agenthub_gateway::{GatewayHandle, RequestLog, Snapshot};
use serde::Serialize;

#[derive(Default)]
struct Inner {
    handle: Option<GatewayHandle>,
    /// 当前监听端口；配置改端口后需要重启才能生效
    port: Option<u16>,
    last_error: Option<String>,
}

#[derive(Default)]
pub struct GatewayState {
    inner: Mutex<Inner>,
    logs: Arc<Mutex<VecDeque<RequestLog>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GatewayStatusView {
    pub running: bool,
    pub host: String,
    pub port: u16,
    /// 已编译进快照的路由数（未运行时按配置里启用的条数算）
    pub routes: usize,
    pub auth: bool,
    pub notes: Vec<String>,
    /// 上一次启动失败的原始错误，界面直接展示
    pub last_error: Option<String>,
}

/// 热重载的结果。端口变了就必须重启，不能假装热更生效了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadOutcome {
    NotRunning,
    Reloaded,
    NeedsRestart,
}

impl GatewayState {
    pub async fn start(&self, config: &AppConfig) -> Result<(), String> {
        {
            let inner = self.lock_inner();
            if inner.handle.is_some() {
                return Ok(());
            }
        }

        let host = config.gateway.listen_host().to_string();
        let port = config.gateway.port;
        let (tx, rx) = mpsc::unbounded_channel();
        let snapshot = Snapshot::compile(config);
        let notes = snapshot.notes().to_vec();
        let handle = GatewayHandle::start(&host, port, snapshot, tx)
            .await
            .inspect_err(|err| {
                let mut inner = self.lock_inner();
                inner.last_error = Some(err.clone());
            })?;

        self.spawn_consumer(rx, config.gateway.max_log_entries.clamp(50, 5000));
        {
            let mut inner = self.lock_inner();
            inner.port = Some(handle.addr().port());
            inner.handle = Some(handle);
            inner.last_error = None;
        }
        if !notes.is_empty() {
            eprintln!("[gateway] 配置提示：{}", notes.join("；"));
        }
        Ok(())
    }

    pub fn stop(&self) {
        let mut inner = self.lock_inner();
        if let Some(mut handle) = inner.handle.take() {
            handle.stop();
        }
        inner.port = None;
    }

    /// 配置改动后调用：端口没变就热重载快照，变了就告诉调用方需要重启。
    pub fn reload(&self, config: &AppConfig) -> ReloadOutcome {
        let inner = self.lock_inner();
        let Some(handle) = inner.handle.as_ref() else {
            return ReloadOutcome::NotRunning;
        };
        if inner.port != Some(config.gateway.port) {
            return ReloadOutcome::NeedsRestart;
        }
        handle.reload(Snapshot::compile(config));
        ReloadOutcome::Reloaded
    }

    pub fn status(&self, config: &AppConfig) -> GatewayStatusView {
        let inner = self.lock_inner();
        match inner.handle.as_ref() {
            Some(handle) => {
                let status = handle.status();
                GatewayStatusView {
                    running: status.running,
                    host: status.host,
                    port: status.port,
                    routes: status.routes,
                    auth: status.auth,
                    notes: status.notes,
                    last_error: None,
                }
            }
            None => GatewayStatusView {
                running: false,
                host: config.gateway.listen_host().to_string(),
                port: config.gateway.port,
                routes: config
                    .gateway_routes
                    .iter()
                    .filter(|route| route.enabled)
                    .count(),
                auth: config
                    .gateway
                    .auth_token
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|token| !token.is_empty()),
                notes: Vec::new(),
                last_error: inner.last_error.clone(),
            },
        }
    }

    pub fn logs(&self, limit: usize) -> Vec<RequestLog> {
        let guard = match self.logs.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let skip = guard.len().saturating_sub(limit);
        guard.iter().skip(skip).cloned().collect()
    }

    pub fn clear_logs(&self) {
        match self.logs.lock() {
            Ok(mut guard) => guard.clear(),
            Err(poisoned) => poisoned.into_inner().clear(),
        }
    }

    /// 网关推日志 → 环形缓冲。队列满了丢最旧的，不阻塞转发。
    fn spawn_consumer(&self, mut rx: UnboundedReceiver<RequestLog>, cap: usize) {
        let logs = self.logs.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(log) = rx.recv().await {
                let mut guard = match logs.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                guard.push_back(log);
                while guard.len() > cap {
                    guard.pop_front();
                }
            }
        });
    }

    fn lock_inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}
