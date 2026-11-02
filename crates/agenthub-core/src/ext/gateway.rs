//! 网关的纯决策函数：模型匹配、候选排序、URL 拼接、模型池合并、配置校验。
//!
//! 这里不碰网络也不碰磁盘 —— 服务层（`agenthub-gateway`）只负责把请求搬来搬去，
//! 「下一个该试谁」全部由本模块决定，因此可以脱离 tokio 做完整单测。

use crate::l10n::{Msg, t};
use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::error::{Error, Result};
use crate::model::gateway::{GatewayProtocol, GatewayRoute, RouteMember, RouteMode};

/// 模型名匹配：精确，或尾部通配 `claude-*` / `*sonnet`。大小写不敏感。
///
/// 只支持「头尾通配各一个」，不做正则 —— 正则写错会让整条路由静默失效，
/// 而模型名的形态无非就是前缀区分（`claude-opus-*`、`gpt-*`）。
pub fn model_matches(pattern: &str, model: &str) -> bool {
    let pattern = pattern.trim().to_ascii_lowercase();
    let model = model.trim().to_ascii_lowercase();
    if pattern.is_empty() {
        return true;
    }
    if pattern == "*" {
        return true;
    }
    let head = pattern.starts_with('*');
    let tail = pattern.ends_with('*');
    let core = pattern.trim_matches('*');
    if core.is_empty() {
        return true;
    }
    match (head, tail) {
        (true, true) => model.contains(core),
        (true, false) => model.ends_with(core),
        (false, true) => model.starts_with(core),
        (false, false) => pattern == model,
    }
}

/// 本路由的候选顺序。空表示没有可用上游（服务层直接返回 502）。
///
/// 规则：
/// 1. 成员必须 enabled，且模型在白名单内；
/// 2. 命中 `rules` 里第一条模型规则时，只在规则列出的成员里选；
/// 3. 主选 = 优先级最小的那一组；`Aggregate` 在组内按权重轮询，
///    `Route` / `Single` 组内按声明顺序；
/// 4. 其余按优先级升序接在后面作为故障转移尾部（`Single` 不留尾部）。
pub fn route_candidates(route: &GatewayRoute, model: Option<&str>, tick: u64) -> Vec<RouteMember> {
    if !route.enabled {
        return Vec::new();
    }

    let mut eligible: Vec<&RouteMember> = route
        .members
        .iter()
        .filter(|member| match model {
            Some(name) => member.accepts(name),
            None => member.enabled,
        })
        .collect();

    if let Some(name) = model
        && let Some(rule) = route
            .rules
            .iter()
            .find(|rule| !rule.members.is_empty() && model_matches(&rule.pattern, name))
    {
        eligible.retain(|member| rule.members.iter().any(|id| id == &member.provider_id));
    }

    if eligible.is_empty() {
        return Vec::new();
    }

    let best = eligible.iter().map(|m| m.priority).min().unwrap_or(0);
    let (primary, mut rest): (Vec<&RouteMember>, Vec<&RouteMember>) =
        eligible.into_iter().partition(|m| m.priority == best);
    rest.sort_by_key(|m| m.priority);

    let mut ordered: Vec<RouteMember> = match route.mode {
        RouteMode::Single => primary.iter().take(1).map(|m| (*m).clone()).collect(),
        RouteMode::Route => primary.iter().map(|m| (*m).clone()).collect(),
        RouteMode::Aggregate => weighted_order(&primary, tick),
    };
    if route.mode != RouteMode::Single {
        ordered.extend(rest.into_iter().cloned());
    }
    ordered
}

/// 按权重把主选组展开成轮转序列，并从 `tick` 处开始。
///
/// `[A(2), B(1)]` 展开成 `[A, A, B]`：tick=0 → `A,B`，tick=1 → `A,B`，tick=2 → `B,A`，
/// 三拍里 A 拿到两次，符合 2:1 的权重要求。单成员权重上限 10，避免有人填 1000 撑爆内存。
fn weighted_order(members: &[&RouteMember], tick: u64) -> Vec<RouteMember> {
    let mut slots: Vec<&RouteMember> = Vec::new();
    for member in members {
        for _ in 0..member.weight_or_one().min(10) {
            slots.push(member);
        }
    }
    if slots.is_empty() {
        return Vec::new();
    }
    let total = slots.len();
    let start = (tick % total as u64) as usize;
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = Vec::with_capacity(members.len());
    for offset in 0..total {
        let member = slots[(start + offset) % total];
        if seen.insert(member.provider_id.as_str()) {
            out.push(member.clone());
        }
    }
    out
}

/// 对外模型名 → 上游模型名。没有映射时原样返回。
pub fn aliased_model<'a>(member: &'a RouteMember, model: &'a str) -> &'a str {
    member
        .model_map
        .get(model)
        .map(|value| value.as_str())
        .unwrap_or(model)
}

/// agent 配置里该写的 base_url（老形态，按路径段指定路由）：`http://127.0.0.1:<port>/gw/<route>`
pub fn gateway_base_url(port: u16, route_id: &str) -> String {
    format!("http://127.0.0.1:{port}/gw/{}", route_id.trim())
}

/// 统一入口：所有 agent 共用同一个地址，靠令牌区分供应商池。
pub fn unified_base_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/v1")
}

/// 网关模式下 Claude 的别名映射：官方名 → 上游模型名。
///
/// 同时生成带 `[1M]` 后缀的键 —— Claude Code 的别名 pin 值可能带这个后缀
/// （本机旧配置里出现过 `claude-sonnet-5[1M]`），而网关是精确匹配，
/// 不多写一份就会漏改写。
pub fn alias_map_for(
    slots: &std::collections::BTreeMap<String, String>,
    default_model: &str,
) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    for (slot, official) in crate::ext::provider::OFFICIAL_ALIAS {
        let target = slots
            .get(slot)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(default_model);
        if target.is_empty() {
            continue;
        }
        map.insert(official.to_string(), target.to_string());
        map.insert(format!("{official}[1M]"), target.to_string());
        map.insert(format!("{official}[1m]"), target.to_string());
    }
    map
}

/// 把入站路径拼到上游 base_url 上，并做 `/v1` 去重。
///
/// 中转站填 `https://relay.example.com/v1`，agent 打过来的是 `/v1/messages`；
/// 直接拼会变成 `/v1/v1/messages`，所以两边都以 `/v1` 开头时只保留一份。
pub fn join_upstream_url(base_url: &str, path: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    let mut path = path.trim().to_string();
    if path.is_empty() {
        return base.to_string();
    }
    if !path.starts_with('/') {
        path.insert(0, '/');
    }
    if let Some(rest) = base.strip_suffix("/v1")
        && (path == "/v1" || path.starts_with("/v1/"))
    {
        return format!("{rest}{path}");
    }
    format!("{base}{path}")
}

/// 合并成员模型成对外模型池（去重、保持首次出现顺序）。
///
/// 成员 `models` 为空表示「接受全部」：此时用该 provider 自己登记的模型列表兜底；
/// 两者都空则不贡献条目（绝不编造模型名）。
pub fn merged_models(
    route: &GatewayRoute,
    provider_models: &BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out: Vec<String> = Vec::new();
    for member in route.members.iter().filter(|m| m.enabled) {
        for alias in member.model_map.keys() {
            if seen.insert(alias.clone()) {
                out.push(alias.clone());
            }
        }
        let list: Vec<String> = if member.models.is_empty() {
            provider_models
                .get(&member.provider_id)
                .cloned()
                .unwrap_or_default()
        } else {
            member.models.clone()
        };
        for model in list {
            if seen.insert(model.clone()) {
                out.push(model);
            }
        }
    }
    out
}

/// 路由 id 只允许小写字母、数字、`-`、`_`，长度 1..=32。
///
/// 它要进 URL 路径并且会和 `/gw/` 拼接，放宽字符集只会制造转义问题。
pub fn route_id_valid(id: &str) -> bool {
    let id = id.trim();
    !id.is_empty()
        && id.len() <= 32
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// 上游协议与下游协议的兼容结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compatibility {
    /// 同协议：字节级透传，最省事也最不容易出问题
    PassThrough,
    /// Anthropic ⇄ Chat Completions：网关做结构化转换
    Convert,
    /// 本期不支持，附可读原因（界面与 CLI 直接展示）
    Unsupported(String),
}

/// 判断一组「下游协议（路由）/ 上游协议（供应商）」能不能通。
///
/// 只放行同协议与 Anthropic⇄Chat 两种情形。Responses 的事件模型与另两者差异太大，
/// 硬做转换会拖长战线且很难验证，所以在**保存配置时**就拒绝，而不是等请求进来才 502。
pub fn protocol_compatible(
    downstream: GatewayProtocol,
    upstream: GatewayProtocol,
) -> Compatibility {
    use GatewayProtocol::{Anthropic, ChatCompletions, Responses};
    if downstream == upstream {
        return Compatibility::PassThrough;
    }
    match (downstream, upstream) {
        (Anthropic, ChatCompletions) | (ChatCompletions, Anthropic) => Compatibility::Convert,
        (Anthropic, Responses) | (Responses, Anthropic) => {
            Compatibility::Unsupported(t!(Msg::ErrConvertResponsesAnthropic))
        }
        (ChatCompletions, Responses) | (Responses, ChatCompletions) => {
            Compatibility::Unsupported(t!(Msg::ErrConvertResponsesChat))
        }
        // 上面的分支已覆盖全部跨协议组合；留一条兜底避免将来加协议时静默放行
        _ => Compatibility::Unsupported(t!(Msg::ErrProtocolComboUnsupported)),
    }
}

/// 实际生效的上游协议：供应商没声明时跟随路由的下游协议（旧行为）。
pub fn upstream_protocol(
    downstream: GatewayProtocol,
    declared: Option<GatewayProtocol>,
) -> GatewayProtocol {
    declared.unwrap_or(downstream)
}

/// 校验路由配置（纯函数，便于单测与 CLI 复用）。
pub fn validate_route(route: &GatewayRoute, known_providers: &[String]) -> Result<()> {
    if !route_id_valid(&route.id) {
        return Err(Error::msg(t!(Msg::ErrRouteIdChars)));
    }
    if route.name.trim().is_empty() {
        return Err(Error::msg(t!(Msg::ErrRouteNameEmpty)));
    }
    if route.members.is_empty() {
        return Err(Error::msg(t!(Msg::ErrRouteNoMembers)));
    }

    let mut seen: HashSet<&str> = HashSet::new();
    for member in &route.members {
        if !known_providers.iter().any(|id| id == &member.provider_id) {
            return Err(Error::msg(t!(
                Msg::ErrRouteMemberMissingProvider,
                ids = member.provider_id
            )));
        }
        if !seen.insert(member.provider_id.as_str()) {
            return Err(Error::msg(t!(
                Msg::ErrRouteDuplicateProvider,
                ids = member.provider_id
            )));
        }
    }

    if route.mode == RouteMode::Single && route.members.len() != 1 {
        return Err(Error::msg(t!(Msg::ErrRouteSingleMember)));
    }

    for rule in &route.rules {
        if rule.pattern.trim().is_empty() {
            return Err(Error::msg(t!(Msg::ErrRuleMatchEmpty)));
        }
        for id in &rule.members {
            if !seen.contains(id.as_str()) {
                return Err(Error::msg(t!(Msg::ErrRuleMissingProvider, id = id)));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::gateway::{GatewayProtocol, RetryPolicy, RouteRule};

    fn member(id: &str, weight: u32, priority: u32) -> RouteMember {
        RouteMember {
            provider_id: id.into(),
            weight,
            priority,
            models: Vec::new(),
            model_map: BTreeMap::new(),
            enabled: true,
        }
    }

    fn route(mode: RouteMode, members: Vec<RouteMember>) -> GatewayRoute {
        GatewayRoute {
            id: "main".into(),
            name: "主路由".into(),
            protocol: GatewayProtocol::Anthropic,
            mode,
            members,
            rules: Vec::new(),
            retry: RetryPolicy::default(),
            enabled: true,
        }
    }

    #[test]
    fn model_patterns() {
        assert!(model_matches("claude-sonnet-4-5", "claude-sonnet-4-5"));
        assert!(model_matches("claude-*", "claude-opus-4"));
        assert!(model_matches("*sonnet*", "claude-sonnet-4-5"));
        assert!(!model_matches("claude-*", "gpt-6.1"));
        assert!(model_matches("", "anything"));
        assert!(model_matches("*", "anything"));
    }

    #[test]
    fn priority_groups_decide_failover_order() {
        let r = route(
            RouteMode::Route,
            vec![
                member("backup", 1, 200),
                member("primary", 1, 10),
                member("second", 1, 100),
            ],
        );
        let ids: Vec<String> = route_candidates(&r, Some("m"), 0)
            .into_iter()
            .map(|m| m.provider_id)
            .collect();
        assert_eq!(ids, ["primary", "second", "backup"]);
    }

    #[test]
    fn weight_round_robin_rotates() {
        let r = route(
            RouteMode::Aggregate,
            vec![member("a", 2, 10), member("b", 1, 10)],
        );
        let first: Vec<String> = route_candidates(&r, Some("m"), 0)
            .into_iter()
            .map(|m| m.provider_id)
            .collect();
        let third: Vec<String> = route_candidates(&r, Some("m"), 2)
            .into_iter()
            .map(|m| m.provider_id)
            .collect();
        assert_eq!(first, ["a", "b"]);
        assert_eq!(third, ["b", "a"], "第三拍应该轮到 b 打头");
    }

    #[test]
    fn rules_restrict_members() {
        let mut r = route(
            RouteMode::Aggregate,
            vec![member("a", 1, 10), member("b", 1, 10)],
        );
        r.rules.push(RouteRule {
            pattern: "claude-*".into(),
            members: vec!["b".into()],
        });
        let ids: Vec<String> = route_candidates(&r, Some("claude-opus-4"), 0)
            .into_iter()
            .map(|m| m.provider_id)
            .collect();
        assert_eq!(ids, ["b"]);
        // 未命中规则时两个都在
        assert_eq!(route_candidates(&r, Some("gpt-6.1"), 0).len(), 2);
    }

    #[test]
    fn single_mode_keeps_one_candidate() {
        let r = route(
            RouteMode::Single,
            vec![member("a", 1, 10), member("b", 1, 20)],
        );
        assert_eq!(route_candidates(&r, Some("m"), 0).len(), 1);
    }

    #[test]
    fn disabled_member_is_skipped() {
        let mut r = route(RouteMode::Route, vec![member("a", 1, 10)]);
        r.members[0].enabled = false;
        assert!(route_candidates(&r, Some("m"), 0).is_empty());
    }

    #[test]
    fn upstream_url_dedupes_v1() {
        assert_eq!(
            join_upstream_url("https://relay.example.com/v1", "/v1/messages"),
            "https://relay.example.com/v1/messages"
        );
        assert_eq!(
            join_upstream_url("https://relay.example.com/v1/", "/v1/messages"),
            "https://relay.example.com/v1/messages"
        );
        assert_eq!(
            join_upstream_url("https://api.openai.com", "/v1/chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            join_upstream_url("https://x.dev/v1", "/v1"),
            "https://x.dev/v1"
        );
    }

    #[test]
    fn merged_models_dedupes_and_falls_back_to_provider_models() {
        let mut provider_models: BTreeMap<String, Vec<String>> = BTreeMap::new();
        provider_models.insert("a".into(), vec!["m1".into(), "m2".into()]);
        provider_models.insert("b".into(), vec!["m2".into(), "m3".into()]);
        let r = route(
            RouteMode::Aggregate,
            vec![member("a", 1, 10), member("b", 1, 10)],
        );
        assert_eq!(merged_models(&r, &provider_models), ["m1", "m2", "m3"]);
    }

    #[test]
    fn validate_rejects_unknown_provider_and_duplicates() {
        let r = route(RouteMode::Route, vec![member("a", 1, 10)]);
        assert!(validate_route(&r, &["a".into()]).is_ok());
        assert!(validate_route(&r, &["b".into()]).is_err());

        let dup = route(
            RouteMode::Route,
            vec![member("a", 1, 10), member("a", 1, 20)],
        );
        assert!(validate_route(&dup, &["a".into()]).is_err());

        let single = route(
            RouteMode::Single,
            vec![member("a", 1, 10), member("b", 1, 10)],
        );
        assert!(validate_route(&single, &["a".into(), "b".into()]).is_err());
    }

    #[test]
    fn base_url_points_at_loopback() {
        assert_eq!(
            gateway_base_url(43117, "main"),
            "http://127.0.0.1:43117/gw/main"
        );
    }
}
