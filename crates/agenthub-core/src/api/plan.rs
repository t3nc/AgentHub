use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use crate::api::AgentHub;
use crate::error::{Error, Result};
use crate::instructions::{bridge, managed_block};
use crate::l10n::{self, Msg, t};
use crate::mcp;
use crate::model::agent::{AgentId, Scope};
use crate::model::change::{ChangeKind, ChangeSet, DeleteOptions, FileOp, RiskLevel};
use crate::model::instruction::InstructionKind;
use crate::model::mcp::{McpServer, McpServerView, McpTargetState};
use crate::model::skill::{LinkState, SkillRecord, SkillSource};
use crate::paths;
use crate::project;

impl AgentHub {
    /// 去重接管：把同名多份副本收敛为「中央仓库一份 + 各 agent 链接」。
    ///
    /// `baseline` 指定「以哪一份为准」：必须是本组成员且是实体目录副本；
    /// 传 None 时沿旧策略（体积最大的实体副本）。指定值不可用时回退并在 note 里说明。
    pub fn plan_dedupe(
        &self,
        names: Option<&[String]>,
        baseline: Option<&Path>,
    ) -> Result<ChangeSet> {
        let report = self.scan(&Scope::User)?;
        let store = self.config.store_root_path();
        let trash = self.config.adoption.use_trash_for_dedupe;
        let mut cs = ChangeSet::new(ChangeKind::Dedupe, t!(Msg::PlanTitleDedupe));

        for group in &report.duplicates {
            if let Some(filter) = names
                && !filter.iter().any(|n| n == &group.name)
            {
                continue;
            }
            // agent 独有技能（computer-use 这类）各家的实现互不通用，收敛只会把
            // 一家的实现塞给另一家。扫描阶段已不进重复组，这里是第二道防线：
            // 万一有旧扫描结果 / 手改配置绕过来，也不会生成搬运或建链操作。
            if self.config.is_agent_local(&group.name) {
                continue;
            }
            if group.members.iter().all(|m| m.readonly) {
                continue;
            }
            let store_target = store.join(&group.name);
            let store_ready = group.members.iter().any(|m| m.source == SkillSource::Store)
                || store_target.exists();

            // 指定基线只对「包含它的那一组」生效，别组的计划不受影响
            let preferred = baseline.and_then(|wanted| {
                group
                    .members
                    .iter()
                    .find(|m| paths::paths_eq(&m.path, wanted))
            });

            let mut moved_source: Option<PathBuf> = None;
            if !store_ready {
                let (chosen, by_user) = match preferred {
                    Some(member) if is_usable_baseline(member) => (Some(member), true),
                    Some(member) => {
                        cs.note(t!(
                            Msg::PlanBaselineUnusable,
                            at = paths::path_str(&member.path),
                            name = group.name
                        ));
                        (pick_baseline(&group.members), false)
                    }
                    None => (pick_baseline(&group.members), false),
                };
                let Some(source) = chosen else {
                    cs.note(t!(Msg::PlanSkipNoBaseline, name = group.name));
                    continue;
                };
                cs.push(FileOp::move_into_store(
                    source.path.clone(),
                    store_target.clone(),
                ));
                moved_source = Some(source.path.clone());
                let at = paths::path_str(&source.path);
                cs.note(if by_user {
                    t!(Msg::PlanBaselineChosenByUser, at = at, name = group.name)
                } else {
                    t!(Msg::PlanBaselineChosen, at = at, name = group.name)
                });
                if !group.identical {
                    cs.risk(
                        RiskLevel::Warn,
                        if trash {
                            t!(Msg::PlanCopiesDifferTrash, name = group.name)
                        } else {
                            t!(Msg::PlanCopiesDifferBackup, name = group.name)
                        },
                    );
                }
            } else if preferred.is_some() {
                cs.note(t!(Msg::PlanBaselineIgnored, name = group.name));
            }

            for member in &group.members {
                if member.source == SkillSource::Store || member.readonly {
                    continue;
                }
                let strategy = self.strategy_for_skill(
                    &group.name,
                    member.agent.as_ref().unwrap_or(&AgentId::Claude),
                );
                let is_moving_source = moved_source
                    .as_ref()
                    .map(|p| paths::paths_eq(p, &member.path))
                    .unwrap_or(false);
                // 基线本身已经被搬进仓库，这里必须补回指回仓库的链接
                if is_moving_source {
                    cs.push(FileOp::create_link(
                        member.path.clone(),
                        store_target.clone(),
                        strategy,
                    ));
                    continue;
                }
                if matches!(&member.link, LinkState::Foreign { .. }) {
                    cs.note(t!(
                        Msg::PlanSkipForeignLink,
                        at = paths::path_str(&member.path)
                    ));
                    continue;
                }
                if let Some(target) = member.link.target()
                    && paths::paths_eq(target, &store_target)
                    && target.exists()
                {
                    continue;
                }
                cs.push(FileOp::remove_path(member.path.clone(), trash));
                cs.push(FileOp::create_link(
                    member.path.clone(),
                    store_target.clone(),
                    strategy,
                ));
            }
        }

        // 用户显式点名要收敛的技能被标记为 agent 独有时必须解释原因：
        // 静默跳过会让「勾了它却什么都没发生」看起来像 bug。
        if let Some(filter) = names {
            let mut reported: BTreeSet<&str> = BTreeSet::new();
            for name in filter {
                if self.config.is_agent_local(name) && reported.insert(name.as_str()) {
                    cs.note(t!(Msg::PlanSkipAgentLocal, name = name));
                }
            }
        }

        if cs.is_empty() {
            cs.note(t!(Msg::PlanNothingToDedupe));
        }
        Ok(cs)
    }

    /// 修复破损 / 漂移的链接：重新指向中央仓库。
    pub fn plan_repair(&self) -> Result<ChangeSet> {
        let report = self.scan(&Scope::User)?;
        let store = self.config.store_root_path();
        let mut cs = ChangeSet::new(ChangeKind::Repair, t!(Msg::PlanTitleRepair));
        // 白名单技能不能"修复"成指向仓库的链接——那正是用户要避免的收敛。
        // 名字去重后汇总成一条 note，别按 agent 目录刷一屏。
        let mut skipped_local: BTreeSet<String> = BTreeSet::new();

        for rec in &report.records {
            let Some(agent) = &rec.agent else { continue };
            if rec.readonly {
                continue;
            }
            if rec.agent_local {
                skipped_local.insert(rec.name.clone());
                continue;
            }
            let target = store.join(&rec.name);
            let needs = match &rec.link {
                LinkState::Broken { .. } => true,
                LinkState::Symlink { target: t } | LinkState::Junction { target: t } => {
                    !paths::paths_eq(t, &target)
                }
                _ => false,
            };
            if needs {
                if !target.exists() {
                    cs.risk(
                        RiskLevel::Danger,
                        t!(
                            Msg::PlanRepairNoTarget,
                            name = rec.name,
                            target = paths::path_str(&target)
                        ),
                    );
                    continue;
                }
                cs.push(FileOp::remove_path(rec.path.clone(), false));
                cs.push(FileOp::create_link(
                    rec.path.clone(),
                    target,
                    self.strategy_for_skill(&rec.name, agent),
                ));
            }
        }
        if cs.is_empty() {
            cs.note(t!(Msg::PlanRepairAllHealthy));
        }
        if !skipped_local.is_empty() {
            let names = skipped_local
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(l10n::str_of(Msg::PlanListSeparator));
            cs.note(t!(
                Msg::PlanRepairSkippedLocal,
                count = skipped_local.len(),
                names = names
            ));
        }
        Ok(cs)
    }

    /// 删除技能（多选）。
    ///
    /// 语义（与 UI 的确认弹窗一致）：
    /// - 只按**当前扫描结果**定位，前端传来的路径必须命中一条记录，
    ///   否则记 note 跳过 —— 不接受「随便给个路径就删」；
    /// - 只读内置技能永不被删（`.system` / marketplace）；
    /// - 删仓库真身时，`with_links` 为真则连带删除所有指向它的链接，避免留断链；
    /// - `use_trash` 透传给 `RemovePath`：回收站失败会自动退回批次备份。
    ///
    /// 顺序固定为「先链接、后副本、最后真身」，中途失败时 apply 会回滚已完成步骤。
    pub fn plan_delete(&self, targets: &[PathBuf], opts: DeleteOptions) -> Result<ChangeSet> {
        if targets.is_empty() {
            return Err(Error::Other(t!(Msg::ErrNoSkillSelected)));
        }
        let report = self.scan(&Scope::User)?;
        let store_root = self.config.store_root_path();
        let mut cs = ChangeSet::new(
            ChangeKind::Delete,
            t!(Msg::PlanTitleDelete, count = targets.len()),
        );

        let mut by_key: BTreeMap<String, &SkillRecord> = BTreeMap::new();
        for rec in &report.records {
            by_key.insert(paths::path_key(&rec.path), rec);
        }

        // 选中的记录（去重保序）
        let mut picked: Vec<&SkillRecord> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for wanted in targets {
            let key = paths::path_key(wanted);
            let Some(rec) = by_key.get(&key).copied() else {
                cs.note(t!(Msg::PlanSkipNotScanned, at = paths::path_str(wanted)));
                continue;
            };
            if !seen.insert(key) {
                continue;
            }
            if rec.readonly {
                cs.note(t!(Msg::PlanSkipReadonly, at = paths::path_str(&rec.path)));
                continue;
            }
            picked.push(rec);
        }

        let bodies: Vec<&SkillRecord> = picked
            .iter()
            .copied()
            .filter(|rec| self.is_store_body(rec, &store_root))
            .collect();
        let mut links: Vec<PathBuf> = picked
            .iter()
            .copied()
            .filter(|rec| !self.is_store_body(rec, &store_root) && rec.link.target().is_some())
            .map(|rec| rec.path.clone())
            .collect();
        let copies: Vec<PathBuf> = picked
            .iter()
            .copied()
            .filter(|rec| !self.is_store_body(rec, &store_root) && rec.link.target().is_none())
            .map(|rec| rec.path.clone())
            .collect();

        // 连带清理：指向被删真身的链接（包括已失效、指向该仓库目录的 broken 链接）
        let mut dangling = 0usize;
        if opts.with_links {
            for body in &bodies {
                let body_key = paths::path_key(&body.path);
                for rec in &report.records {
                    if paths::path_key(&rec.path) == body_key {
                        continue;
                    }
                    let Some(target) = rec.link.target() else {
                        continue;
                    };
                    if paths::path_key(target) != body_key && !paths::paths_eq(target, &body.path) {
                        continue;
                    }
                    if seen.insert(paths::path_key(&rec.path)) {
                        links.push(rec.path.clone());
                        dangling += 1;
                    }
                }
            }
        }

        let mut removed_links = 0usize;
        for path in &links {
            cs.push(FileOp::remove_path(path.clone(), opts.use_trash));
            removed_links += 1;
        }
        for path in &copies {
            cs.push(FileOp::remove_path(path.clone(), opts.use_trash));
        }
        for body in &bodies {
            cs.push(FileOp::remove_path(body.path.clone(), opts.use_trash));
        }

        if !cs.is_empty() {
            cs.note(if opts.use_trash {
                t!(
                    Msg::PlanDeleteSummaryTrash,
                    count = cs.op_count(),
                    bodies = bodies.len(),
                    copies = copies.len(),
                    links = removed_links
                )
            } else {
                t!(
                    Msg::PlanDeleteSummaryPermanent,
                    count = cs.op_count(),
                    bodies = bodies.len(),
                    copies = copies.len(),
                    links = removed_links
                )
            });
            if dangling > 0 && opts.with_links {
                cs.note(t!(Msg::PlanDeleteDangling, count = dangling));
            }
        } else {
            cs.note(t!(Msg::PlanDeleteNothing));
        }

        if !opts.use_trash {
            cs.risk(RiskLevel::Warn, t!(Msg::RiskPermanentDelete));
        }
        if !opts.with_links && !bodies.is_empty() {
            cs.risk(
                RiskLevel::Warn,
                t!(Msg::RiskKeptDanglingLinks, count = bodies.len()),
            );
        }
        if bodies.len() > 1 {
            cs.risk(
                RiskLevel::Warn,
                t!(Msg::RiskManyBodies, count = bodies.len()),
            );
        }

        Ok(cs)
    }

    /// 该记录是不是「中央仓库里的真身」（而不是 agent 目录下的条目）。
    fn is_store_body(&self, rec: &SkillRecord, store_root: &Path) -> bool {
        rec.source == SkillSource::Store || paths::is_within(store_root, &rec.path)
    }

    /// 聚合 MCP 视图：一份定义分发到哪些 agent、哪些还没写。
    pub fn mcp_overview(&self, scope: &Scope) -> Result<Vec<McpServerView>> {
        let mut map: BTreeMap<String, McpServerView> = BTreeMap::new();
        let files = self.mcp_files(scope);

        for file in &files {
            let caps = self.capabilities(&file.agent).unwrap_or_default_caps();
            for server in mcp::read_servers(file) {
                let entry = map
                    .entry(server.name.clone())
                    .or_insert_with(|| McpServerView {
                        name: server.name.clone(),
                        transport: server.transport.clone(),
                        enabled: server.enabled,
                        timeout_secs: server.timeout_secs,
                        note: server.note.clone(),
                        targets: Vec::new(),
                    });
                entry.targets.push(McpTargetState {
                    agent: file.agent.clone(),
                    file: file.path.clone(),
                    format: file.format.label().to_string(),
                    present: true,
                    writable: caps.mcp,
                    reason: if caps.mcp {
                        None
                    } else {
                        Some(t!(Msg::McpUnsupportedAgent))
                    },
                });
            }
        }

        for view in map.values_mut() {
            for file in &files {
                if view.targets.iter().any(|t| t.agent == file.agent) {
                    continue;
                }
                let caps = self.capabilities(&file.agent).unwrap_or_default_caps();
                view.targets.push(McpTargetState {
                    agent: file.agent.clone(),
                    file: file.path.clone(),
                    format: file.format.label().to_string(),
                    present: false,
                    writable: caps.mcp,
                    reason: None,
                });
            }
        }

        // 出站前统一把密钥脱敏：UI 与 IPC 都不该看到明文
        for view in map.values_mut() {
            view.transport = redact_transport(&view.transport);
        }

        Ok(map.into_values().collect())
    }

    /// 把一组 MCP 定义写入指定 agent 的配置文件（plan 阶段完成合并与密钥解析）。
    pub fn plan_mcp_write(
        &self,
        servers: &[McpServer],
        agents: &[AgentId],
        scope: &Scope,
    ) -> Result<ChangeSet> {
        if servers.is_empty() {
            return Err(Error::Other(t!(Msg::ErrNoMcpToWrite)));
        }
        let resolved = mcp::resolve_secrets(servers)?;
        let mut cs = ChangeSet::new(
            ChangeKind::McpWrite,
            t!(Msg::PlanTitleMcpWrite, count = servers.len()),
        );

        // 同一条路径可能被多个平台声明：Trae 与 Trae CN 都写项目级 `.trae/mcp.json`，
        // 两版都登记时不能生成两个写操作（备份与回滚会各做两份）。按规范化路径去重。
        let mut seen: HashSet<String> = HashSet::new();

        for file in self.mcp_files(scope) {
            if !agents.iter().any(|a| a == &file.agent) {
                continue;
            }
            let caps = self.capabilities(&file.agent).unwrap_or_default_caps();
            if !caps.mcp {
                cs.note(t!(
                    Msg::PlanMcpSkipUnsupported,
                    agent = file.agent.display_name()
                ));
                continue;
            }
            if !seen.insert(paths::path_key(&file.path)) {
                cs.note(t!(
                    Msg::PlanMcpSkipDuplicate,
                    at = paths::path_str(&file.path),
                    agent = file.agent.display_name()
                ));
                continue;
            }
            let existing = mcp::read_text(&file.path)?;
            let merged = mcp::upsert(existing.as_deref(), &file.format, &resolved)?;
            if existing.as_deref() == Some(merged.as_str()) {
                cs.note(t!(Msg::PlanMcpUpToDate, at = paths::path_str(&file.path)));
                continue;
            }
            if existing.is_none() {
                cs.note(t!(
                    Msg::PlanMcpCreate,
                    at = paths::path_str(&file.path),
                    format = file.format.label()
                ));
            }
            cs.push(FileOp::write_file(file.path.clone(), merged, true));
        }

        if cs.is_empty() {
            cs.note(t!(Msg::PlanMcpAllCurrent));
        }
        Ok(cs)
    }

    /// 把共享片段注入某 agent 的指令文件（受控区块，不动用户内容）。
    pub fn plan_instruction_inject(
        &self,
        agent: &AgentId,
        scope: &Scope,
        body: &str,
    ) -> Result<ChangeSet> {
        let adapter = self
            .adapter(agent)
            .ok_or_else(|| Error::NotFound(t!(Msg::ErrUnknownAgent, slug = agent.slug())))?;
        let specs: Vec<_> = adapter
            .instruction_specs(scope)
            .into_iter()
            .filter(|s| s.kind == InstructionKind::Rule)
            .collect();
        let spec = specs
            .first()
            .ok_or_else(|| Error::Other(t!(Msg::ErrNoRuleFile, agent = agent.display_name())))?;

        let existing = if spec.path.exists() {
            Some(paths::read_to_string_limited(
                &spec.path,
                crate::model::instruction::MAX_INSTRUCTION_BYTES,
            )?)
        } else {
            None
        };
        let merged = managed_block::inject(existing.as_deref(), body)?;
        let mut cs = ChangeSet::new(ChangeKind::Instruction, t!(Msg::PlanTitleInstruction));
        cs.note(t!(Msg::PlanInstructionWrite, at = spec.display));
        cs.push(FileOp::write_file(spec.path.clone(), merged, true));
        Ok(cs)
    }

    /// 桥接：项目里只有 AGENTS.md 时，给其他 agent 补一行 import。
    pub fn plan_bridge(&self, scope: &Scope) -> Result<ChangeSet> {
        let root = match scope {
            Scope::Project { root } => root.clone(),
            Scope::User => paths::home_dir(),
        };
        let source = root.join("AGENTS.md");
        if !source.exists() {
            return Err(Error::NotFound(t!(
                Msg::ErrBridgeNoSource,
                at = paths::path_str(&source)
            )));
        }
        let mut cs = ChangeSet::new(ChangeKind::Instruction, t!(Msg::PlanTitleBridge));
        for (agent, rel) in [
            (AgentId::Claude, "CLAUDE.md"),
            (AgentId::Gemini, "GEMINI.md"),
        ] {
            let target = root.join(rel);
            if target == source {
                continue;
            }
            let existing = std::fs::read_to_string(&target).ok();
            if bridge::has_import(existing.as_deref(), "AGENTS.md") {
                continue;
            }
            let merged = bridge::ensure_import(existing.as_deref(), "AGENTS.md");
            cs.push(FileOp::write_file(target.clone(), merged, true));
            cs.note(t!(
                Msg::PlanBridgeImport,
                at = paths::path_str(&target),
                agent = agent.display_name()
            ));
        }
        if cs.is_empty() {
            cs.note(t!(Msg::PlanBridgeExists));
        }
        Ok(cs)
    }

    /// 初始化项目作用域：只创建一个受控的 .agenthub/project.json。
    pub fn plan_project_init(&self, root: &Path) -> Result<ChangeSet> {
        project::require_root(root)?;
        let state_path = project::state_path(root);
        let mut cs = ChangeSet::new(ChangeKind::ProjectInit, t!(Msg::PlanTitleProjectInit));
        if state_path.exists() {
            cs.note(t!(Msg::PlanProjectAlreadyInit));
            return Ok(cs);
        }
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".into());
        let state = project::ProjectState::new(name);
        let content = serde_json::to_string_pretty(&state)?;
        cs.push(FileOp::write_file(state_path.clone(), content, false));
        cs.note(t!(
            Msg::PlanProjectCreate,
            at = paths::path_str(&state_path)
        ));
        Ok(cs)
    }
}

/// 读取出来的明文密钥在渲染成视图前全部替换为掩码。
fn redact_transport(transport: &crate::model::mcp::Transport) -> crate::model::mcp::Transport {
    use crate::model::mcp::{SecretRef, Transport};
    let mask = |map: &BTreeMap<String, SecretRef>| -> BTreeMap<String, SecretRef> {
        map.iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    SecretRef::Literal {
                        value: v.masked(),
                        acknowledged: true,
                    },
                )
            })
            .collect()
    };
    match transport {
        Transport::Stdio {
            command,
            args,
            env,
            cwd,
        } => Transport::Stdio {
            command: command.clone(),
            args: args.clone(),
            env: mask(env),
            cwd: cwd.clone(),
        },
        Transport::Http { url, headers } => Transport::Http {
            url: url.clone(),
            headers: mask(headers),
        },
    }
}

/// 只有「实体目录副本」才能当基线：链接（含外部链接）搬进仓库会变成坏链或指向外部。
fn is_usable_baseline(record: &SkillRecord) -> bool {
    !record.readonly
        && record.source != SkillSource::Store
        && matches!(record.link, LinkState::RealDir)
}

fn pick_baseline(members: &[SkillRecord]) -> Option<&SkillRecord> {
    members
        .iter()
        .filter(|m| is_usable_baseline(m))
        .max_by_key(|m| (m.size_bytes, m.modified_at))
}

trait CapsFallback {
    fn unwrap_or_default_caps(self) -> crate::model::agent::Capabilities;
}

impl CapsFallback for Option<crate::model::agent::Capabilities> {
    fn unwrap_or_default_caps(self) -> crate::model::agent::Capabilities {
        self.unwrap_or(crate::model::agent::Capabilities {
            skills: false,
            mcp: false,
            instructions: false,
            project_scope: false,
            links: false,
        })
    }
}
