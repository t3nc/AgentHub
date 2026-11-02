//! 网关路由的编排：CRUD、绑定、视图与「回退到直连」计划。
//!
//! 分工与 provider 一致：纯决策在 `ext::gateway`，这里只做校验、写 `config.json`
//! 与拼 `ChangeSet`。改 agent 配置文件永远走 `apply()`，本模块不直接写盘。

use crate::l10n::{Msg, t};
use serde::{Deserialize, Serialize};

use crate::api::AgentHub;
use crate::error::{Error, Result};
use crate::ext::gateway as route_logic;
use crate::ext::gateway::gateway_base_url;
use crate::model::agent::AgentId;
use crate::model::change::{ChangeKind, ChangeSet};
use crate::model::gateway::{FeatureKey, GatewayConfig, GatewayRoute};

/// 网关令牌写进 Codex `env_key` 时用的变量名。
///
/// Codex 的 `config.toml` 只能写变量名，所以本地令牌必须通过环境变量注入；
/// 界面在绑定行给出 `setx` / `export` 命令。
pub const GATEWAY_TOKEN_ENV: &str = "AGENTHUB_GATEWAY_TOKEN";

/// 未配置鉴权时写进 agent 配置的占位令牌。
///
/// 部分客户端要求认证头非空，网关在关闭鉴权时也不校验内容；
/// 用一个固定串比写空串更容易在日志里认出来源。
pub const PLACEHOLDER_TOKEN: &str = "agenthub-local";

/// 网关视图：配置 + 路由 + 一致性问题。
#[derive(Debug, Clone, Serialize)]
pub struct GatewayView {
    pub config: GatewayConfig,
    pub routes: Vec<GatewayRoute>,
    /// 配置里发现的问题（成员引用了不存在的 provider、绑定指向已删除的路由…）
    pub notes: Vec<String>,
    /// 路由端口的对外地址模板（含 `{route}` 占位），界面用来生成接入片段
    pub base_url_template: String,
}

/// 保存网关全局配置时可改的字段。`host` 故意不在列表里：只允许回环地址。
#[derive(Debug, Clone, Deserialize)]
pub struct GatewayConfigInput {
    pub port: u16,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub log_requests: bool,
    #[serde(default)]
    pub max_log_entries: usize,
    /// 空串 = 关闭鉴权
    #[serde(default)]
    pub auth_token: String,
}

/// 删除路由的结果：哪些 agent 被退回直连。
#[derive(Debug, Clone, Serialize)]
pub struct RouteDeleteResult {
    pub view: GatewayView,
    pub reverted: Vec<String>,
}

impl AgentHub {
    /// 写进 agent 配置的本地令牌。
    ///
    /// 鉴权关闭时返回占位串（客户端要求非空），但网关不会校验它。
    pub fn gateway_client_token(&self) -> String {
        self.config
            .gateway
            .auth_token
            .as_deref()
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .unwrap_or(PLACEHOLDER_TOKEN)
            .to_string()
    }

    pub fn gateway_view(&self) -> GatewayView {
        let mut notes = Vec::new();
        let provider_ids: Vec<String> =
            self.config.providers.iter().map(|p| p.id.clone()).collect();

        for route in &self.config.gateway_routes {
            if let Err(err) = route_logic::validate_route(route, &provider_ids) {
                notes.push(t!(
                    Msg::GatewayRouteCheckFailed,
                    route = route.id,
                    error = err
                ));
            }
        }
        for (agent, binding) in &self.config.provider_bindings {
            if !binding.via_gateway {
                continue;
            }
            match binding.route_id.as_deref() {
                Some(id) if self.config.gateway_routes.iter().any(|r| r.id == id) => {}
                Some(id) => notes.push(t!(
                    Msg::GatewayBindingMissingRoute,
                    agent = agent,
                    route = id
                )),
                None => notes.push(t!(Msg::GatewayBindingNoRoute, agent = agent)),
            }
        }

        let port = self.config.gateway.port;
        GatewayView {
            config: self.config.gateway.clone(),
            routes: self.config.gateway_routes.clone(),
            notes,
            base_url_template: format!("http://127.0.0.1:{port}/gw/{{route}}"),
        }
    }

    pub fn save_gateway_config(&mut self, input: &GatewayConfigInput) -> Result<GatewayView> {
        self.require_feature(FeatureKey::Provider)?;
        if input.port < 1024 {
            return Err(Error::msg(t!(Msg::ErrGatewayPortReserved)));
        }
        let max_log_entries = input.max_log_entries.clamp(50, 5000);
        self.config.gateway.port = input.port;
        self.config.gateway.auto_start = input.auto_start;
        self.config.gateway.log_requests = input.log_requests;
        self.config.gateway.max_log_entries = max_log_entries;
        self.config.gateway.auth_token =
            Some(input.auth_token.trim().to_string()).filter(|token| !token.is_empty());
        self.config.save()?;
        Ok(self.gateway_view())
    }

    /// 新增或整条替换一条路由（按 id）。
    pub fn save_gateway_route(&mut self, route: GatewayRoute) -> Result<GatewayView> {
        self.require_feature(FeatureKey::Provider)?;
        let provider_ids: Vec<String> =
            self.config.providers.iter().map(|p| p.id.clone()).collect();
        route_logic::validate_route(&route, &provider_ids)?;
        // 协议不兼容的成员在这里就挡住：等请求进来才 502，用户根本不知道是哪条配置的问题
        self.check_route_protocols(&route)?;

        match self
            .config
            .gateway_routes
            .iter_mut()
            .find(|existing| existing.id == route.id)
        {
            Some(existing) => *existing = route,
            None => self.config.gateway_routes.push(route),
        }
        self.config.gateway_routes.sort_by(|a, b| a.id.cmp(&b.id));
        self.config.save()?;
        Ok(self.gateway_view())
    }

    /// 逐成员检查「路由下游协议 × 供应商上游协议」是否可通。
    fn check_route_protocols(&self, route: &GatewayRoute) -> Result<()> {
        for member in &route.members {
            let Some(provider) = self
                .config
                .providers
                .iter()
                .find(|p| p.id == member.provider_id)
            else {
                continue;
            };
            let upstream = route_logic::upstream_protocol(route.protocol, provider.protocol);
            if let route_logic::Compatibility::Unsupported(reason) =
                route_logic::protocol_compatible(route.protocol, upstream)
            {
                return Err(Error::Unsupported(t!(
                    Msg::ErrSupplierReason,
                    name = provider.name,
                    reason = reason
                )));
            }
        }
        Ok(())
    }

    /// 删路由，并把指向它的绑定退回直连（不留悬空状态）。
    pub fn delete_gateway_route(&mut self, id: &str) -> Result<RouteDeleteResult> {
        self.require_feature(FeatureKey::Provider)?;
        let id = id.trim();
        let before = self.config.gateway_routes.len();
        self.config.gateway_routes.retain(|route| route.id != id);
        if self.config.gateway_routes.len() == before {
            return Err(Error::NotFound(t!(Msg::ErrGatewayRouteMissing, route = id)));
        }

        let mut reverted = Vec::new();
        for (agent, binding) in self.config.provider_bindings.iter_mut() {
            if binding.route_id.as_deref() == Some(id) {
                // `via_gateway` 是 `mode` 的派生值，两个一起改，否则
                // `effective_mode()` 还会认为它走网关
                binding.via_gateway = false;
                binding.mode = crate::model::gateway::PoolMode::Direct;
                binding.route_id = None;
                reverted.push(agent.clone());
            }
        }
        self.config.save()?;
        Ok(RouteDeleteResult {
            view: self.gateway_view(),
            reverted,
        })
    }

    /// 把一批 agent 从「经网关」退回「直连」的写入计划。
    ///
    /// 只出计划、不改绑定：应用成功后由调用方（或用户）再改绑定，
    /// 否则会出现「绑定说直连、文件还指向网关」的不一致。
    pub fn plan_revert_to_direct(&self, agents: &[String]) -> Result<ChangeSet> {
        self.require_feature(FeatureKey::Provider)?;
        let mut cs = ChangeSet::new(ChangeKind::Provider, t!(Msg::PlanTitleGatewayToDirect));

        for slug in agents {
            let agent = AgentId::from_slug(slug.trim());
            let Some(binding) = self.config.provider_bindings.get(&agent.slug()) else {
                continue;
            };
            if !binding.effective_mode().uses_gateway() {
                continue;
            }
            let mut direct = binding.clone();
            direct.via_gateway = false;
            direct.mode = crate::model::gateway::PoolMode::Direct;
            direct.route_id = None;
            let plan = self.plan_for(&agent, &direct)?;
            for note in &plan.notes {
                cs.note(t!(
                    Msg::GatewayNoteLine,
                    agent = agent.display_name(),
                    note = note
                ));
            }
            for op in plan.ops {
                cs.push(op);
            }
        }

        if cs.ops.is_empty() {
            cs.note(t!(Msg::PlanGatewayNothingToRevert));
        }
        Ok(cs)
    }

    /// 某个路由对外暴露的 base_url（不带末尾斜杠）。
    pub fn gateway_route_url(&self, route_id: &str) -> String {
        gateway_base_url(self.config.gateway.port, route_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Db;

    /// 不碰磁盘的 AgentHub：内存索引库 + 默认配置。
    fn hub_with(config: crate::config::AppConfig) -> AgentHub {
        let adapters = crate::adapters::build_adapters(&config);
        AgentHub {
            config,
            db: Db::open_memory().expect("in-memory db"),
            adapters,
        }
    }

    #[test]
    fn token_falls_back_to_placeholder() {
        let mut config = crate::config::AppConfig::default();
        config.gateway.auth_token = Some("   ".into());
        let mut hub = hub_with(config);
        assert_eq!(hub.gateway_client_token(), PLACEHOLDER_TOKEN);

        hub.config.gateway.auth_token = Some(" tk ".into());
        assert_eq!(hub.gateway_client_token(), "tk");
    }

    #[test]
    fn view_reports_dangling_binding() {
        let mut config = crate::config::AppConfig::default();
        config.provider_bindings.insert(
            "claude".into(),
            crate::ext::provider::ProviderBinding {
                agent: AgentId::Claude,
                provider_id: "gone".into(),
                model: "m".into(),
                make_default: false,
                via_gateway: true,
                route_id: Some("missing".into()),
                mode: crate::model::gateway::PoolMode::Route,
                model_slots: Default::default(),
            },
        );
        let hub = hub_with(config);
        let view = hub.gateway_view();
        assert!(
            view.notes.iter().any(|note| note.contains("不存在的路由")),
            "悬空绑定必须被报出来：{:?}",
            view.notes
        );
    }
}
