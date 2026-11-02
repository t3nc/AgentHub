//! AgentHub 本地 Provider 网关。
//!
//! 职责边界：
//! - 本 crate 只做 HTTP：解析、选路、转发、回写、记元数据；
//! - 路由怎么选由 `agenthub_core::ext::gateway` 的纯函数决定；
//! - 配置怎么改、计划怎么落盘由 `agenthub_core::api` 决定；
//! - 进程生命周期与请求日志的持久化由桌面壳（`apps/desktop/src-tauri`）决定。
//!
//! 这样 CLI 依旧不依赖 tokio，网关也能脱离 Tauri 做集成测试。

pub mod config;
mod convert;
mod models;
mod protocol;
mod proxy;
mod router;
mod upstream;

use agenthub_core::l10n::{Msg, t};
use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};

use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

pub use config::{RuntimeMember, RuntimeRoute, Snapshot};
pub use protocol::CanonicalRequest;
pub use proxy::RequestLog;
/// 重导出日志通道类型：壳层不该为了一个 `Sender` 再单独依赖 tokio。
pub use tokio::sync::mpsc;

/// 服务运行期共享状态。请求热路径只读快照，不碰 DB、不碰 `AgentHub` 的锁。
pub struct State {
    snapshot: RwLock<Arc<Snapshot>>,
    pub client: reqwest::Client,
    /// 加权轮询的全局计数（无锁）
    pub tick: AtomicU64,
    pub logs: UnboundedSender<RequestLog>,
}

impl State {
    fn new(snapshot: Snapshot, logs: UnboundedSender<RequestLog>) -> Self {
        Self {
            snapshot: RwLock::new(Arc::new(snapshot)),
            client: upstream::build_client(),
            tick: AtomicU64::new(0),
            logs,
        }
    }

    /// 拿一份快照。热重载整体替换 `Arc`，读者不会看到半成品。
    pub fn snapshot(&self) -> Arc<Snapshot> {
        match self.snapshot.read() {
            Ok(guard) => guard.clone(),
            // 写侧 panic 过也不能让网关停摆：退回上一次快照
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

/// 一台已经在跑的网关。
pub struct GatewayHandle {
    addr: SocketAddr,
    state: Arc<State>,
    shutdown: Option<oneshot::Sender<()>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GatewayStatus {
    pub running: bool,
    pub host: String,
    pub port: u16,
    /// 已编译进快照的路由数
    pub routes: usize,
    /// 是否启用本地令牌
    pub auth: bool,
    /// 编译期被跳过或需要提醒的配置
    pub notes: Vec<String>,
}

impl GatewayHandle {
    /// 启动监听。只接受回环地址之外的调用会被 core 的 `listen_host()` 挡掉。
    pub async fn start(
        host: &str,
        port: u16,
        snapshot: Snapshot,
        logs: UnboundedSender<RequestLog>,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind((host, port)).await.map_err(|err| {
            t!(
                Msg::GatewayListenFailed,
                host = host,
                port = port,
                error = err
            )
        })?;
        let addr = listener
            .local_addr()
            .map_err(|err| t!(Msg::GatewayLocalAddrFailed, error = err))?;

        let state = Arc::new(State::new(snapshot, logs));
        let app = router::build(state.clone());
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async {
                let _ = rx.await;
            });
            if let Err(err) = server.await {
                eprintln!("{}", t!(Msg::GatewayExited, error = err));
            }
        });

        Ok(Self {
            addr,
            state,
            shutdown: Some(tx),
        })
    }

    /// 热重载：路由 / 成员 / 令牌变更后立刻生效，端口不变就不用重启。
    pub fn reload(&self, snapshot: Snapshot) {
        let mut guard = match self.state.snapshot.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = Arc::new(snapshot);
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn is_running(&self) -> bool {
        self.shutdown.is_some()
    }

    pub fn status(&self) -> GatewayStatus {
        let snapshot = self.state.snapshot();
        GatewayStatus {
            running: self.is_running(),
            host: self.addr.ip().to_string(),
            port: self.addr.port(),
            routes: snapshot.routes().count(),
            auth: snapshot.auth_token().is_some(),
            notes: snapshot.notes().to_vec(),
        }
    }
}

impl Drop for GatewayHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 供壳层构造快照用的便捷入口。
pub fn compile(config: &agenthub_core::config::AppConfig) -> Snapshot {
    Snapshot::compile(config)
}

/// 健康检查用的最小探活：不经 TCP，只判断句柄是否还持有停止通道。
pub fn alive(handle: &GatewayHandle) -> bool {
    handle.is_running() && handle.state.snapshot.read().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_of_empty_config_has_no_routes() {
        let config = agenthub_core::config::AppConfig::default();
        let snapshot = Snapshot::compile(&config);
        assert_eq!(snapshot.routes().count(), 0);
        assert!(snapshot.auth_token().is_none());
    }

    #[test]
    fn auth_token_whitespace_is_ignored() {
        let mut config = agenthub_core::config::AppConfig::default();
        config.gateway.auth_token = Some("   ".into());
        assert!(Snapshot::compile(&config).auth_token().is_none());

        config.gateway.auth_token = Some(" tk ".into());
        assert_eq!(Snapshot::compile(&config).auth_token(), Some("tk"));
    }
}
