use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::agent::AgentId;

/// 密钥引用：绝不把明文写进配置或数据库。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SecretRef {
    /// 从环境变量读取
    Env { var: String },
    /// 从系统钥匙串读取
    Keychain { service: String, account: String },
    /// 明文（仅当目标 agent 不支持插值时由用户显式确认）
    Literal { value: String, acknowledged: bool },
}

impl SecretRef {
    pub fn masked(&self) -> String {
        match self {
            SecretRef::Env { var } => format!("env:{var}"),
            SecretRef::Keychain { service, account } => format!("keychain:{service}/{account}"),
            SecretRef::Literal { .. } => "literal:********".into(),
        }
    }

    /// 落盘时写入的值：已解析成 Literal 时写真值，否则退化为引用形式（不应发生）。
    pub fn render_value(&self) -> String {
        match self {
            SecretRef::Literal { value, .. } => value.clone(),
            other => other.masked(),
        }
    }

    /// `parse()` 的逆：把引用还原成可以再次喂给 `parse()` 的字符串。
    ///
    /// 只用于「内部需要在进程内传一遍引用」的场景（例如模型探测要把已保存的
    /// 密钥引用交给 HTTP 层）；**不要**拿它做展示，展示一律用 `masked()`。
    pub fn to_reference(&self) -> String {
        match self {
            SecretRef::Env { var } => format!("env:{var}"),
            SecretRef::Keychain { service, account } => format!("keychain:{service}/{account}"),
            SecretRef::Literal { value, .. } => value.clone(),
        }
    }

    pub fn parse(raw: &str) -> Self {
        if let Some(rest) = raw.strip_prefix("env:") {
            return SecretRef::Env {
                var: rest.to_string(),
            };
        }
        if let Some(rest) = raw.strip_prefix("keychain:") {
            let mut parts = rest.splitn(2, '/');
            let service = parts.next().unwrap_or_default().to_string();
            let account = parts.next().unwrap_or_default().to_string();
            return SecretRef::Keychain { service, account };
        }
        SecretRef::Literal {
            value: raw.to_string(),
            acknowledged: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Transport {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, SecretRef>,
        #[serde(default)]
        cwd: Option<String>,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, SecretRef>,
    },
}

impl Transport {
    pub fn summary(&self) -> String {
        match self {
            Transport::Stdio { command, args, .. } => {
                if args.is_empty() {
                    command.clone()
                } else {
                    format!("{command} {}", args.join(" "))
                }
            }
            Transport::Http { url, .. } => url.clone(),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Transport::Stdio { .. } => "stdio",
            Transport::Http { .. } => "http",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServer {
    pub name: String,
    pub transport: Transport,
    pub enabled: bool,
    #[serde(default)]
    pub timeout_secs: Option<u32>,
    #[serde(default)]
    pub note: Option<String>,
}

/// 各 agent 配置文件的格式族。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "format", rename_all = "snake_case")]
pub enum McpFormat {
    /// JSON，pointer 是定位到 mcp 映射的键路径，如 ["mcpServers"]
    Json { pointer: Vec<String> },
    /// TOML 表名，如 "mcp_servers"
    Toml { table: String },
    /// YAML 顶层键，如 "mcp_servers"
    Yaml { top_key: String },
}

impl McpFormat {
    pub fn label(&self) -> &'static str {
        match self {
            McpFormat::Json { .. } => "JSON",
            McpFormat::Toml { .. } => "TOML",
            McpFormat::Yaml { .. } => "YAML",
        }
    }
}

/// 某个 agent 的 MCP 配置文件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpFile {
    pub agent: AgentId,
    pub path: PathBuf,
    pub format: McpFormat,
    pub exists: bool,
}

/// UI 用的聚合视图：一个 server 分发到了哪些 agent。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerView {
    pub name: String,
    pub transport: Transport,
    pub enabled: bool,
    pub targets: Vec<McpTargetState>,
    pub timeout_secs: Option<u32>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTargetState {
    pub agent: AgentId,
    pub file: PathBuf,
    pub format: String,
    /// 该 agent 配置里是否已有同名条目
    pub present: bool,
    /// 该 agent 是否支持写入
    pub writable: bool,
    pub reason: Option<String>,
}
