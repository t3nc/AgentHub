//! 把 `config.json` 里的路由与 provider 编译成运行时快照。
//!
//! 编译期做三件在请求热路径上不该做的事：
//! 1. 解析密钥引用（`env:` / `keychain:`）—— 钥匙串实现会起子进程或调系统 API，
//!    每个请求都解析一次会把首字节拖到几百毫秒；
//! 2. 把 provider 索引成 `BTreeMap`，请求时按 id 直取；
//! 3. 算出每个成员的实际启用状态与失败原因，界面可以直接展示。
//!
//! 解析失败的成员不会让整条路由失效：它被标记出来并在转发时跳过，
//! 用户还有其它上游可用。

use agenthub_core::l10n::{Msg, t};
use std::collections::BTreeMap;
use std::sync::Arc;

use agenthub_core::config::AppConfig;
use agenthub_core::ext::gateway as route_logic;
use agenthub_core::mcp::secrets;
use agenthub_core::model::gateway::{GatewayProtocol, GatewayRoute, RouteMember};

/// 请求体上限：32 MB。再大的 prompt 也不该把网关内存吃穿。
pub const DEFAULT_MAX_BODY: usize = 32 * 1024 * 1024;

/// 一个已就绪的上游成员。
#[derive(Debug, Clone)]
pub struct RuntimeMember {
    pub provider_id: String,
    pub name: String,
    pub base_url: String,
    /// 解析后的密钥。`None` 一定伴随 `error`，转发时跳过该成员。
    pub api_key: Option<String>,
    pub error: Option<String>,
    pub weight: u32,
    pub priority: u32,
    pub enabled: bool,
    /// provider 自己登记的模型列表，成员白名单为空时用它兜底
    pub models: Vec<String>,
    /// 对外模型名 → 上游模型名
    pub model_map: BTreeMap<String, String>,
    /// 供应商声明的上游协议；None = 跟随路由的下游协议（旧行为，字节级透传）
    pub protocol: Option<GatewayProtocol>,
}

impl RuntimeMember {
    /// 能真正发起请求（有地址、有密钥、没被禁用）。
    pub fn usable(&self) -> bool {
        self.error.is_none() && self.enabled && !self.base_url.trim().is_empty()
    }
}

/// 一条可服务的路由。
#[derive(Debug, Clone)]
pub struct RuntimeRoute {
    pub config: GatewayRoute,
    pub members: BTreeMap<String, RuntimeMember>,
}

impl RuntimeRoute {
    pub fn member(&self, provider_id: &str) -> Option<&RuntimeMember> {
        self.members.get(provider_id)
    }

    /// 转成本地 `/v1/models` 用的模型池。
    pub fn model_pool(&self) -> Vec<String> {
        let by_provider: BTreeMap<String, Vec<String>> = self
            .members
            .values()
            .map(|member| (member.provider_id.clone(), member.models.clone()))
            .collect();
        route_logic::merged_models(&self.config, &by_provider)
    }
}

/// 一次编译的结果。变更后整体替换，请求即时生效。
#[derive(Debug, Clone)]
pub struct Snapshot {
    routes: BTreeMap<String, RuntimeRoute>,
    auth_token: Option<String>,
    max_body_bytes: usize,
    /// 编译时被跳过的配置（给界面与日志用）
    notes: Vec<String>,
}

impl Snapshot {
    pub fn compile(config: &AppConfig) -> Self {
        let mut routes = BTreeMap::new();
        let mut notes = Vec::new();

        for route in &config.gateway_routes {
            if !route_logic::route_id_valid(&route.id) {
                notes.push(t!(Msg::GatewayRouteIdInvalid, route = route.id));
                continue;
            }
            if routes.contains_key(&route.id) {
                notes.push(t!(Msg::GatewayRouteDuplicate, route = route.id));
                continue;
            }

            let mut members = BTreeMap::new();
            for member in &route.members {
                let Some(provider) = config.providers.iter().find(|p| p.id == member.provider_id)
                else {
                    notes.push(t!(
                        Msg::GatewayRouteMissingProvider,
                        route = route.id,
                        provider = member.provider_id
                    ));
                    continue;
                };
                let (api_key, error) = match secrets::resolve(&provider.api_key) {
                    Ok(key) => (Some(key), None),
                    Err(err) => (
                        None,
                        Some(t!(
                            Msg::GatewayKeyResolveFailed,
                            masked = provider.api_key.masked(),
                            error = err
                        )),
                    ),
                };
                members.insert(
                    member.provider_id.clone(),
                    RuntimeMember {
                        provider_id: member.provider_id.clone(),
                        name: provider.name.clone(),
                        base_url: provider.base_url.clone(),
                        api_key,
                        error,
                        weight: member.weight_or_one(),
                        priority: member.priority,
                        enabled: member.enabled,
                        models: provider.models.clone(),
                        model_map: member.model_map.clone(),
                        protocol: provider.protocol,
                    },
                );
            }

            if members.is_empty() {
                notes.push(t!(Msg::GatewayRouteNoMembers, route = route.id));
                continue;
            }

            routes.insert(
                route.id.clone(),
                RuntimeRoute {
                    config: route.clone(),
                    members,
                },
            );
        }

        let auth_token = config
            .gateway
            .auth_token
            .as_ref()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());

        Self {
            routes,
            auth_token,
            max_body_bytes: DEFAULT_MAX_BODY,
            notes,
        }
    }

    pub fn route(&self, id: &str) -> Option<&RuntimeRoute> {
        self.routes.get(id)
    }

    pub fn routes(&self) -> impl Iterator<Item = &RuntimeRoute> {
        self.routes.values()
    }

    /// 统一入口用：令牌即路由 id（每个 agent 一条池路由），这里只做存在性判断。
    pub fn has_route(&self, route_id: &str) -> bool {
        self.routes.contains_key(route_id)
    }

    /// 未知令牌时回给客户端，方便用户看出自己该填哪个。
    pub fn route_ids(&self) -> Vec<String> {
        self.routes.keys().cloned().collect()
    }

    pub fn auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }

    pub fn max_body_bytes(&self) -> usize {
        self.max_body_bytes
    }

    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// 候选顺序。`tick` 用于聚合模式的加权轮询。
    pub fn candidates(
        &self,
        route: &RuntimeRoute,
        model: Option<&str>,
        tick: u64,
    ) -> Vec<RuntimeMember> {
        route_logic::route_candidates(&route.config, model, tick)
            .into_iter()
            .filter_map(|candidate: RouteMember| route.member(&candidate.provider_id).cloned())
            .filter(RuntimeMember::usable)
            .collect()
    }
}

/// `RwLock<Arc<Snapshot>>` 的读侧克隆：热重载时整体替换，读者不会看到半成品。
pub type SharedSnapshot = std::sync::RwLock<Arc<Snapshot>>;
