//! AgentHub 核心领域库。
//!
//! 设计原则：零 Tauri 依赖，GUI 与 CLI 都是它的薄壳；
//! 所有写操作先 `plan()` 出 `ChangeSet`，再由 `apply()` 事务化落盘。

pub mod adapters;
pub mod api;
pub mod apply;
pub mod config;
pub mod error;
pub mod ext;
pub mod instructions;
pub mod l10n;
pub mod link;
pub mod market;
pub mod mcp;
pub mod model;
pub mod paths;
pub mod platform;
pub mod project;
pub mod scan;
pub mod store;
pub mod terminal;
pub mod text;
pub mod watch;

pub use api::{AgentHub, human_bytes};
pub use config::AppConfig;
pub use error::{Error, Result};
pub use model::agent::Scope;
pub use scan::ScanReport;
