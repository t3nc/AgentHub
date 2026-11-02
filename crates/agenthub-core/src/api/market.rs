//! 技能市场的门面：查询 + 安装计划。
//!
//! 只有用户显式点「搜索 / 详情 / 安装」才会走到这里，任何自动流程都不该调它。
//! 网络与解包细节在 `crate::market`，这里只负责「把结果拼成一份可审阅的 ChangeSet」。

use crate::l10n::{Msg, t};
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::market::{self, Registry, install};
use crate::model::agent::Scope;
use crate::model::change::{ChangeKind, ChangeSet, FileOp, RiskLevel};
use crate::model::market::{
    MarketAgent, MarketInstallInput, MarketPage, MarketSkillDetail, MarketSort,
};
use crate::paths;

use super::AgentHub;

impl AgentHub {
    /// 本机可安装目标。
    ///
    /// `available` 的判定是「adapter 检测到（CLI 在 PATH 或配置目录存在）**且**有可写技能目录」，
    /// 比前端从扫描结果里猜更准 —— OpenCode 这类自己目录为空、实际读兼容目录的也能覆盖到。
    pub fn market_agents(&self) -> Vec<MarketAgent> {
        let mut out: Vec<MarketAgent> = self
            .adapters
            .iter()
            .map(|adapter| {
                let install = adapter.detect();
                let target = adapter
                    .skill_targets(&Scope::User)
                    .into_iter()
                    .find(|t| !t.readonly);
                let skills_dir = target.as_ref().map(|t| t.dir.clone());
                let available = install.detected && skills_dir.is_some();
                let reason = if !install.detected {
                    Some(t!(Msg::MarketAgentNotDetected))
                } else if skills_dir.is_none() {
                    Some(t!(Msg::MarketAgentNoSkillsDir))
                } else {
                    None
                };
                MarketAgent {
                    id: install.id.clone(),
                    display_name: install.display_name.clone(),
                    skills_dir,
                    detected: install.detected,
                    available,
                    reason,
                }
            })
            .collect();
        out.sort_by(|a, b| {
            b.available
                .cmp(&a.available)
                .then_with(|| a.display_name.cmp(&b.display_name))
        });
        out
    }

    /// 搜索 / 浏览一页。关键词为空时退化成「按排序浏览」。
    ///
    /// `cursor` 原样透传给适配器（SkillHub 是页码，ClawHub 是 `nextCursor` /
    /// 搜索用的 offset），这里不做任何解释。
    pub fn market_search(
        &self,
        query: &str,
        sort: MarketSort,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<MarketPage> {
        let registry = market::build(&self.config)?;
        self.market_search_with(registry.as_ref(), query, sort, cursor, limit)
    }

    /// 供测试注入数据源。
    pub fn market_search_with(
        &self,
        registry: &dyn Registry,
        query: &str,
        sort: MarketSort,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<MarketPage> {
        // SkillHub 的 pageSize 上限就是 100，再大得自己拼多页；这里统一收口
        let limit = limit.clamp(1, market::PAGE_SIZE_MAX);
        if query.trim().is_empty() {
            registry.list_page(sort, cursor, limit)
        } else {
            registry.search_page(query, cursor, limit)
        }
    }

    pub fn market_detail(&self, slug: &str) -> Result<MarketSkillDetail> {
        let registry = market::build(&self.config)?;
        registry.detail(slug)
    }

    /// 生成安装计划（真实网络）。
    pub fn plan_market_install(&self, input: &MarketInstallInput) -> Result<ChangeSet> {
        let registry = market::build(&self.config)?;
        self.plan_market_install_with(registry.as_ref(), input)
    }

    /// 生成安装计划：下载到缓存 → 落仓库 → 给所选 agent 建链接。
    ///
    /// 落盘仍然走 Plan/Apply：这里只产出 `RemovePath`（覆盖旧版）+ `MoveIntoStore`
    /// + `CreateLink`，由用户在执行前逐条审阅。
    pub fn plan_market_install_with(
        &self,
        registry: &dyn Registry,
        input: &MarketInstallInput,
    ) -> Result<ChangeSet> {
        if input.agents.is_empty() {
            return Err(Error::Other(t!(Msg::ErrMarketSelectAgent)));
        }
        let slug = input.slug.trim();
        if slug.is_empty() {
            return Err(Error::Other(t!(Msg::ErrSkillSlugEmpty)));
        }

        let detail = registry.detail(slug)?;
        if let Some(reason) = detail.blocked_reason() {
            return Err(Error::Unsupported(reason));
        }

        let name = detail.skill.local_name();
        let store = self.config.store_root_path();
        let store_target = store.join(&name);

        // 目标 agent：只认「检测到 + 有可写技能目录」的，其余记 note 跳过
        let available = self.market_agents();
        let mut targets: Vec<(crate::model::agent::AgentId, PathBuf)> = Vec::new();
        let mut cs = ChangeSet::new(
            ChangeKind::MarketInstall,
            t!(
                Msg::PlanTitleMarketInstall,
                name = detail.skill.display_name
            ),
        );

        for agent in &input.agents {
            let Some(entry) = available.iter().find(|a| &a.id == agent) else {
                continue;
            };
            if !entry.available {
                cs.note(t!(
                    Msg::PlanSkipAgent,
                    agent = entry.display_name,
                    reason = entry
                        .reason
                        .clone()
                        .unwrap_or_else(|| t!(Msg::MarketAgentUnavailable))
                ));
                continue;
            }
            if let Some(dir) = &entry.skills_dir {
                targets.push((agent.clone(), dir.clone()));
            }
        }
        if targets.is_empty() {
            return Err(Error::Other(t!(Msg::ErrMarketNoTargets)));
        }

        // 下载 + 解包到缓存目录（受管根内，后续 MoveIntoStore 才过得了门禁）
        let prepared = install::prepare(
            registry,
            slug,
            &name,
            input.version.as_deref().or(detail.skill.version.as_deref()),
        )?;

        if store_target.exists() {
            cs.push(FileOp::remove_path(store_target.clone(), true));
            cs.note(t!(Msg::PlanStoreReplaced, name = name));
        }
        cs.push(FileOp::move_into_store(
            prepared.dir.clone(),
            store_target.clone(),
        ));

        for (agent, dir) in &targets {
            let at = dir.join(&name);
            if at.exists() || std::fs::symlink_metadata(crate::platform::long_path(&at)).is_ok() {
                cs.note(t!(Msg::PlanLinkSkipped, path = paths::path_str(&at)));
                continue;
            }
            let strategy = self.strategy_for_skill(&name, agent);
            cs.push(FileOp::create_link(at, store_target.clone(), strategy));
        }

        cs.note(t!(
            Msg::PlanPackageSummary,
            files = prepared.summary.files,
            size = super::human_bytes(prepared.summary.bytes),
            mode = if prepared.fallback_files {
                t!(Msg::PlanModePerFile)
            } else {
                "ZIP".to_string()
            }
        ));
        if let Some(version) = input.version.as_deref().or(detail.skill.version.as_deref()) {
            cs.note(t!(Msg::PlanVersion, version = version));
        }
        if !prepared.summary.skipped.is_empty() {
            cs.note(t!(
                Msg::PlanSkippedFiles,
                count = prepared.summary.skipped.len(),
                list = prepared
                    .summary
                    .skipped
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" / ")
            ));
        }
        if detail.skill.suspicious {
            cs.risk(
                RiskLevel::Warn,
                t!(
                    Msg::RiskMarketSuspicious,
                    name = detail.skill.display_name,
                    summary = detail
                        .moderation_summary
                        .clone()
                        .unwrap_or_else(|| t!(Msg::MarketNoDescription))
                ),
            );
        }

        Ok(cs)
    }
}
