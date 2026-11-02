use agenthub_core::ext::catalog::{AgentKind, CatalogEntry, EnvState, Managed};
use agenthub_core::human_bytes;
use agenthub_core::l10n::{Msg, t};
use agenthub_core::model::change::{ChangeSet, RiskLevel};
use agenthub_core::model::mcp::McpServerView;
use agenthub_core::model::skill::LinkState;
use agenthub_core::scan::ScanReport;

pub fn print_scan(report: &ScanReport) {
    println!("{}", t!(Msg::CliScanDuration, ms = report.duration_ms));
    println!(
        "{}",
        t!(
            Msg::CliScanSkills,
            total = report.stats.total,
            store = report.stats.store_skills,
            agent = report.stats.agent_skills,
            readonly = report.stats.readonly
        )
    );
    println!(
        "{}",
        t!(
            Msg::CliScanLinks,
            linked = report.stats.linked,
            copies = report.stats.copies,
            broken = report.stats.broken,
            shared = report.stats.shared_skills,
            wasted = human_bytes(report.stats.wasted_bytes)
        )
    );

    println!("\n{}", t!(Msg::CliAgentsHeader));
    for agent in &report.agents {
        println!(
            "  {:<12} {:<8} {}",
            agent.display_name,
            if agent.detected {
                t!(Msg::CliInstalled)
            } else {
                t!(Msg::CliNotDetected)
            },
            agent
                .config_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".into())
        );
    }

    if !report.duplicates.is_empty() {
        println!("\n{}", t!(Msg::CliDuplicatesHeader));
        for group in report.duplicates.iter().take(12) {
            println!(
                "{}",
                t!(
                    Msg::CliDuplicateRow,
                    name = format!("{:<28}", group.name),
                    count = group.members.len(),
                    size = format!("{:<10}", human_bytes(group.wasted_bytes)),
                    state = if group.identical {
                        t!(Msg::CliIdentical)
                    } else {
                        t!(Msg::CliDrifting)
                    }
                )
            );
        }
    }

    let broken: Vec<_> = report
        .records
        .iter()
        .filter(|r| matches!(r.link, LinkState::Broken { .. }))
        .collect();
    if !broken.is_empty() {
        println!("\n{}", t!(Msg::CliBrokenLinksHeader));
        for rec in broken {
            println!("  {}", rec.path.display());
        }
    }
}

pub fn print_plan(cs: &ChangeSet) {
    println!(
        "{}",
        t!(
            Msg::CliPlanSummary,
            id = cs.id,
            title = cs.kind.label(),
            count = cs.ops.len()
        )
    );
    for op in &cs.ops {
        println!("  - {}", op.describe());
    }
    for note in &cs.notes {
        println!("  · {note}");
    }
    for risk in &cs.risks {
        let tag = match risk.level {
            RiskLevel::Info => "INFO",
            RiskLevel::Warn => "WARN",
            RiskLevel::Danger => "DANGER",
        };
        println!("  [{tag}] {}", risk.message);
    }
}

/// 常见 Agent 目录：每行一个产品；未就绪的把首条安装命令缩进到下一行。
pub fn print_catalog(entries: &[CatalogEntry]) {
    for entry in entries {
        println!(
            "[{:<9}] {:<16} {:<20} {:<8} {:<8} {}",
            state_label(entry.state),
            entry.key,
            entry.name,
            kind_label(entry.kind),
            managed_label(entry.managed),
            entry_path(entry)
        );
        if let Some(first) = entry.evidence.first() {
            println!("{:>12}· {first}", "");
        }
        if entry.needs_confirm {
            println!("{}", t!(Msg::CliNeedsConfirm, pad = format!("{:>12}", "")));
        }
        if entry.managed == Managed::Blocked
            && let Some(note) = &entry.managed_note
        {
            println!(
                "{}",
                t!(Msg::CliBlocked, pad = format!("{:>12}", ""), note = note)
            );
        }
        if entry.state != EnvState::Ready
            && let Some(step) = entry.steps.first()
        {
            println!("{:>12}→ {}", "", step.command);
        }
    }
}

fn entry_path(entry: &CatalogEntry) -> String {
    let base = entry
        .cli_path
        .clone()
        .or_else(|| entry.app_path.clone())
        .or_else(|| entry.config_path.clone())
        .unwrap_or_else(|| "-".into());
    match &entry.version {
        Some(version) => format!("{base}  ({version})"),
        None => base,
    }
}

fn kind_label(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::Cli => "cli",
        AgentKind::Desktop => "desktop",
        AgentKind::Ide => "ide",
        AgentKind::Cloud => "cloud",
    }
}

/// 设置总览：仓库、策略、各 agent 的生效路径与覆盖情况。
/// Provider 注册表 + 各 agent 绑定状态（密钥只显示引用形式）。
pub fn print_providers(view: &agenthub_core::api::provider::ProvidersView) {
    if view.providers.is_empty() {
        println!("{}", t!(Msg::CliProvidersEmpty));
    } else {
        println!("{}", t!(Msg::CliProvidersHeader));
        for p in &view.providers {
            // 协议决定网关要不要转换，属于必看信息，放在第一行
            let protocol = p
                .protocol
                .map(|value| value.label().to_string())
                .unwrap_or_else(|| t!(Msg::CliProtocolUndeclared));
            println!(
                "{:<12} {:<16} {:<10} {:<40} {}{}",
                p.id,
                p.name,
                protocol,
                p.base_url,
                p.api_key,
                p.note
                    .as_deref()
                    .map(|n| format!("  （{n}）"))
                    .unwrap_or_default()
            );
            if !p.models.is_empty() {
                let limits = p
                    .limits_summary
                    .as_deref()
                    .map(|text| format!("  [{text}]"))
                    .unwrap_or_default();
                println!(
                    "{}",
                    t!(
                        Msg::CliModelsLine,
                        pad = format!("{:<12}", ""),
                        models = p.models.join(" / "),
                        limits = limits
                    )
                );
            }
        }
    }

    println!("\n{}", t!(Msg::CliBindingsHeader));
    for b in &view.bindings {
        if !b.supported {
            // 把「为什么不能写」直接说出来：只写「暂不支持」等于把问题丢回给用户
            println!(
                "{}",
                t!(
                    Msg::CliBindingUnsupported,
                    name = format!("{:<10}", b.display_name),
                    reason = b
                        .unsupported_reason
                        .clone()
                        .unwrap_or_else(|| t!(Msg::CliConfigFormatMissing))
                )
            );
            continue;
        }
        match (&b.provider_id, &b.model) {
            (Some(provider), Some(model)) => {
                let default = if b.make_default {
                    t!(Msg::CliDefaultBadge)
                } else {
                    String::new()
                };
                let gateway = if b.via_gateway {
                    t!(
                        Msg::CliViaGateway,
                        route = b
                            .route_id
                            .clone()
                            .unwrap_or_else(|| t!(Msg::CliNoRouteSelected))
                    )
                } else {
                    String::new()
                };
                println!(
                    "{:<10} {} · {}{}{}",
                    b.display_name, provider, model, default, gateway
                )
            }
            _ => println!(
                "{}",
                t!(Msg::CliUnbound, name = format!("{:<10}", b.display_name))
            ),
        }
    }
}

pub fn print_settings(view: &agenthub_core::api::settings::SettingsView) {
    println!("{}", t!(Msg::CliDataDir, path = view.data_root));
    println!(
        "{}",
        t!(
            Msg::CliStoreLine,
            path = view.store_root,
            skills = view.store_usage.skills,
            size = human_bytes(view.store_usage.bytes)
        )
    );
    let default = match view.default_strategy {
        Some(strategy) => t!(Msg::CliStrategyManual, label = strategy.label()),
        None => t!(
            Msg::CliStrategyDefault,
            label = view.platform_strategy.label()
        ),
    };
    println!("{}", t!(Msg::CliStrategyLine, value = default));
    println!("{}", t!(Msg::CliBackupKeep, count = view.backups_keep));
    println!(
        "{}",
        t!(
            Msg::CliDedupeMode,
            mode = if view.use_trash_for_dedupe {
                t!(Msg::CliUseTrash)
            } else {
                t!(Msg::CliUseBackupDir)
            },
            suffix = if view.backup_config_files {
                t!(Msg::CliBackupConfigSuffix)
            } else {
                String::new()
            }
        )
    );
    if !view.excluded_paths.is_empty() {
        println!(
            "{}",
            t!(Msg::CliExcluded, paths = view.excluded_paths.join(", "))
        );
    }

    println!("\n{}", t!(Msg::CliAgentPathsHeader));
    for agent in &view.agents {
        println!(
            "{:<10} {}{}",
            agent.id,
            agent.display_name,
            if agent.overrides.is_empty() {
                String::new()
            } else {
                t!(Msg::CliOverrideBadge)
            }
        );
        print_path(
            &t!(Msg::CliConfigDir),
            agent.effective.config_dir.as_deref(),
            agent.presence.config_dir,
        );
        print_path(
            &t!(Msg::CliSkillsDir),
            agent.effective.skills_dir.as_deref(),
            agent.presence.skills_dir,
        );
        for (path, exists) in agent
            .effective
            .extra_skills_dirs
            .iter()
            .zip(agent.presence.extra_skills_dirs.iter())
        {
            print_path(&t!(Msg::CliCompatDir), Some(path.as_str()), *exists);
        }
        print_path(
            &t!(Msg::CliMcpFile),
            agent.effective.mcp_file.as_deref(),
            agent.presence.mcp_file,
        );
        for (path, exists) in agent
            .effective
            .instructions
            .iter()
            .zip(agent.presence.instructions.iter())
        {
            print_path(&t!(Msg::CliInstructionsFile), Some(path.as_str()), *exists);
        }
    }

    if !view.custom_agents.is_empty() {
        println!("\n{}", t!(Msg::CliCustomAgentsHeader));
        for def in &view.custom_agents {
            println!("  {} ({})", def.display_name, def.id);
        }
    }
}

fn print_path(label: &str, path: Option<&str>, exists: bool) {
    let Some(path) = path else {
        return;
    };
    println!(
        "{label}  {path}{}",
        if exists {
            String::new()
        } else {
            t!(Msg::CliPathMissing)
        }
    );
}

fn state_label(state: EnvState) -> &'static str {
    match state {
        EnvState::Ready => "ready",
        EnvState::ConfigOnly => "config",
        EnvState::Missing => "missing",
    }
}

fn managed_label(managed: Managed) -> &'static str {
    match managed {
        Managed::Builtin => "builtin",
        Managed::Managed => "managed",
        Managed::Addable => "addable",
        Managed::Shared => "shared",
        Managed::Blocked => "blocked",
    }
}

pub fn print_mcp(views: &[McpServerView]) {
    if views.is_empty() {
        println!("{}", t!(Msg::CliNoMcpServers));
        return;
    }
    for view in views {
        println!(
            "{}  [{}]  {}{}",
            view.name,
            view.transport.kind(),
            view.transport.summary(),
            if view.enabled {
                String::new()
            } else {
                t!(Msg::CliDisabledBadge)
            }
        );
        for target in &view.targets {
            println!(
                "    {} {} {}",
                if target.present { "✓" } else { "-" },
                target.agent.display_name(),
                target.reason.clone().unwrap_or_default()
            );
        }
    }
}

/// token 数量的紧凑写法：12.3M / 890K。原始数字在 `--json` 里仍是精确值。
fn compact_tokens(value: u64) -> String {
    const UNITS: [(f64, &str); 4] = [
        (1_000_000_000_000.0, "T"),
        (1_000_000_000.0, "B"),
        (1_000_000.0, "M"),
        (1_000.0, "K"),
    ];
    let raw = value as f64;
    for (scale, suffix) in UNITS {
        if raw >= scale {
            return format!("{:.1}{suffix}", raw / scale);
        }
    }
    value.to_string()
}

fn compact_cost(report: &agenthub_core::api::usage::UsageReport) -> String {
    match report.totals.cost {
        Some(cost) => {
            let suffix = if report.totals.unpriced_calls > 0 {
                t!(Msg::CliPartiallyUnpriced)
            } else {
                String::new()
            };
            format!("{:.2} {}{suffix}", cost, report.totals.currency)
        }
        None => t!(Msg::CliUnpriced),
    }
}

fn ts_label(epoch: i64) -> String {
    agenthub_core::paths::fmt_ts(epoch)
        .split(' ')
        .next()
        .unwrap_or_default()
        .to_string()
}

/// 桶的时间标签：小时档到点，天档到日。`fmt_ts` 固定是 `%Y-%m-%d %H:%M:%S`。
fn bucket_label(epoch: i64, hourly: bool) -> String {
    let full = agenthub_core::paths::fmt_ts(epoch);
    if hourly {
        format!("{}:00", full.chars().take(13).collect::<String>())
    } else {
        full.chars().take(10).collect()
    }
}

/// 用量总览：汇总、按天、按 agent、按模型、数据源状态与口径尾注。
pub fn print_usage(report: &agenthub_core::api::usage::UsageReport) {
    use std::collections::BTreeMap;

    let range_text = match report.start {
        Some(start) => t!(
            Msg::CliRangeFrom,
            label = report.range_label,
            start = ts_label(start)
        ),
        None => t!(Msg::CliRangeEarliest, label = report.range_label),
    };
    println!("{}", t!(Msg::CliUsageHeader, range = range_text));
    if report.totals.calls == 0 {
        println!("{}", t!(Msg::CliUsageEmpty));
    } else {
        println!(
            "{}",
            t!(
                Msg::CliUsageTotalsHead,
                input = format!("{:<10}", compact_tokens(report.totals.input)),
                cache_read = format!("{:<10}", compact_tokens(report.totals.cache_read)),
                cache_write = format!("{:<10}", compact_tokens(report.totals.cache_write)),
                output = format!("{:<10}", compact_tokens(report.totals.output))
            )
        );
        println!(
            "{}",
            t!(
                Msg::CliUsageTotalsRow,
                total = format!("{:<10}", compact_tokens(report.totals.total)),
                calls = format!("{:<8}", report.totals.calls),
                cost = compact_cost(report)
            )
        );
    }

    // 按天（或小时）——cells 已按当前粒度聚合，这里只做跨模型求和
    let mut timeline: BTreeMap<i64, agenthub_core::api::usage::UsageTotals> = BTreeMap::new();
    for cell in &report.cells {
        let entry = timeline.entry(cell.bucket).or_default();
        let bucket = agenthub_core::ext::usage::UsageBucket {
            hour_utc: cell.bucket,
            model: cell.model.clone(),
            provider: cell.provider.clone(),
            input: cell.input,
            cache_read: cell.cache_read,
            cache_write: cell.cache_write,
            output: cell.output,
            calls: cell.calls.min(u32::MAX as u64) as u32,
        };
        entry.add_bucket(&bucket);
    }
    if !timeline.is_empty() {
        println!(
            "\n{}",
            t!(
                Msg::CliUsageGroupHeader,
                kind = if matches!(
                    report.granularity,
                    agenthub_core::api::usage::Granularity::Hour
                ) {
                    t!(Msg::CliGranularityHour)
                } else {
                    t!(Msg::CliGranularityDay)
                }
            )
        );
        let hourly = matches!(
            report.granularity,
            agenthub_core::api::usage::Granularity::Hour
        );
        for (bucket, totals) in &timeline {
            println!(
                "{}",
                t!(
                    Msg::CliUsageTrendRow,
                    label = bucket_label(*bucket, hourly),
                    input = format!("{:<9}", compact_tokens(totals.input)),
                    cache = format!("{:<9}", compact_tokens(totals.cache_read)),
                    write = format!("{:<9}", compact_tokens(totals.cache_write)),
                    output = format!("{:<9}", compact_tokens(totals.output)),
                    total = format!("{:<9}", compact_tokens(totals.total))
                )
            );
        }
    }

    if !report.by_agent.is_empty() {
        println!("\n{}", t!(Msg::CliByAgent));
        for group in &report.by_agent {
            print_group(group);
        }
    }
    if !report.by_provider.is_empty() {
        println!("\n{}", t!(Msg::CliByProvider));
        for group in &report.by_provider {
            print_group(group);
        }
    }
    if !report.by_model.is_empty() {
        println!("\n{}", t!(Msg::CliByModel));
        for group in &report.by_model {
            print_group(group);
        }
    }

    println!("\n{}", t!(Msg::CliSourcesHeader));
    for stats in &report.sources {
        if let Some(reason) = &stats.source.unsupported {
            println!(
                "{}",
                t!(
                    Msg::CliSourceNoUsage,
                    label = format!("{:<14}", stats.source.label),
                    reason = reason
                )
            );
            continue;
        }
        if !stats.source.exists {
            println!(
                "{}",
                t!(
                    Msg::CliSourceNotDetected,
                    label = format!("{:<14}", stats.source.label)
                )
            );
            continue;
        }
        println!(
            "{}",
            t!(
                Msg::CliSourceStats,
                label = format!("{:<14}", stats.source.label),
                files = stats.files,
                records = stats.records,
                bad = stats.bad_lines,
                extra = stats
                    .source
                    .roots
                    .first()
                    .map(|root| format!("  {}", root.display()))
                    .unwrap_or_default()
            )
        );
    }

    println!(
        "\n{}",
        t!(
            Msg::CliScanStats,
            files = report.stats.files_total,
            cached = report.stats.files_cached,
            ms = report.stats.elapsed_ms,
            pruned = report.stats.pruned
        )
    );
    println!("{}", t!(Msg::CliUsageNote, note = report.note));
}

fn print_group(group: &agenthub_core::api::usage::UsageGroup) {
    let cost = match group.totals.cost {
        Some(value) => {
            if group.totals.unpriced_calls > 0 {
                t!(Msg::CliCostPartial, value = format!("{value:.2}"))
            } else {
                format!("{value:.2}")
            }
        }
        None => t!(Msg::CliUnpriced),
    };
    println!(
        "{}",
        t!(
            Msg::CliPriceRow,
            model = format!("{:<26}", group.label),
            input = format!("{:<9}", compact_tokens(group.totals.input)),
            cache = format!("{:<9}", compact_tokens(group.totals.cache_read)),
            write = format!("{:<9}", compact_tokens(group.totals.cache_write)),
            output = format!("{:<9}", compact_tokens(group.totals.output)),
            total = format!("{:<9}", compact_tokens(group.totals.total)),
            calls = format!("{:<7}", group.totals.calls),
            source = cost
        )
    );
}

/// 价格表：每 100 万 token 的单价。
pub fn print_pricing(view: &agenthub_core::api::pricing::PricingView) {
    if view.prices.is_empty() {
        println!("{}", t!(Msg::CliPricingEmpty));
    } else {
        println!("{}", t!(Msg::CliPricingHeader));
        for price in &view.prices {
            println!(
                "{}",
                t!(
                    Msg::CliPriceTableRow,
                    model = format!("{:<34}", price.model),
                    provider = format!(
                        "{:<14}",
                        price.provider.clone().unwrap_or_else(|| "-".into())
                    ),
                    input = format!("{:<9.4}", price.input),
                    output = format!("{:<9.4}", price.output),
                    cache_read = format!("{:<9.4}", price.cache_read),
                    cache_write = format!("{:<9.4}", price.cache_write),
                    source = price.currency
                )
            );
        }
    }
    println!("\n{}", view.note);
}
