mod render;

use std::io::Write;
use std::path::PathBuf;

use agenthub_core::AgentHub;
use agenthub_core::l10n::{self, Locale, Msg, t};
use agenthub_core::model::agent::{AgentId, Scope};
use agenthub_core::model::change::ChangeSet;
use agenthub_core::model::gateway::GatewayProtocol;
use agenthub_core::model::mcp::{McpServer, Transport};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agenthub", version)]
struct Cli {
    /// 输出语言：auto / en / zh-Hans / zh-Hant
    ///
    /// 默认 auto：读 `AGENTHUB_LANG`、再读 `config.json` 的 `locale`。
    /// 这一次性覆盖不会写回配置（想永久切换请改设置页）。
    #[arg(long, global = true, value_name = "LANG")]
    lang: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 扫描本机各 agent 的技能目录
    Scan {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// 只看重复技能
    Duplicates,
    /// 生成去重接管计划（默认只打印，不落盘）
    PlanDedupe {
        /// 只处理这些技能名（可重复）
        #[arg(long)]
        only: Vec<String>,
        /// 指定以哪一份副本为准（实体目录路径）；缺省按体积最大者
        #[arg(long)]
        baseline: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// 生成链接修复计划
    PlanRepair {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// 执行计划文件
    Apply {
        plan: PathBuf,
        /// 计划是在项目作用域下生成的（写入路径落在该项目内）
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        yes: bool,
    },
    /// 回滚某个批次
    Rollback { batch: String },
    /// 最近的写入批次
    Batches,
    /// 体检
    Doctor,
    /// Agent 环境：检测 + 安装指引
    Agents {
        /// 探测 CLI 版本（会短暂启动子进程）
        #[arg(long)]
        probe: bool,
        #[arg(long)]
        json: bool,
    },
    /// 把常见目录里的产品登记为受管平台（只写配置）
    AgentAdd { key: String },
    /// 移出受管（只删配置，不动目录与链接）
    AgentRemove { key: String },
    /// 打印当前设置（仓库、链接策略、各 agent 生效路径与覆盖）
    Settings {
        #[arg(long)]
        json: bool,
    },
    /// MCP 相关
    Mcp {
        #[command(subcommand)]
        cmd: McpCommand,
    },
    /// 指令文件链路
    Instructions {
        #[command(subcommand)]
        cmd: InstructionCommand,
    },
    /// 项目作用域
    Project {
        #[command(subcommand)]
        cmd: ProjectCommand,
    },
    /// Provider（API 中转站）登记与写入
    Provider {
        #[command(subcommand)]
        cmd: ProviderCommand,
    },
    /// 本机 token 用量统计（只读解析会话日志）
    Usage {
        /// today / 24h / 7d / 15d / 30d / all
        #[arg(long, default_value = "7d")]
        range: String,
        /// 只看某个来源（claude / codex / codebuddy / codebuddy-ide / workbuddy …）
        #[arg(long)]
        agent: Option<String>,
        /// 只看某个模型（大小写不敏感的精确匹配）
        #[arg(long)]
        model: Option<String>,
        /// 只看某个 provider（含「未登记」）
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        json: bool,
        /// 丢弃按文件缓存，强制重新解析
        #[arg(long)]
        refresh: bool,
    },
    /// 模型价格登记（每 100 万 token 单价）
    Pricing {
        #[command(subcommand)]
        cmd: PricingCommand,
    },
    /// 功能开关（Provider / Token 统计）
    Feature {
        #[command(subcommand)]
        cmd: FeatureCommand,
    },
    /// Provider 网关（只读：看配置、路由与接入地址，不启动服务）
    Gateway {
        #[command(subcommand)]
        cmd: GatewayCommand,
    },
}

#[derive(Subcommand)]
enum FeatureCommand {
    /// 列出功能开关状态
    List {
        #[arg(long)]
        json: bool,
    },
    /// 启用一个功能模块
    Enable { key: String },
    /// 禁用一个功能模块（只隐藏入口，数据保留）
    Disable { key: String },
}

#[derive(Subcommand)]
enum GatewayCommand {
    /// 网关配置与路由
    List {
        #[arg(long)]
        json: bool,
    },
    /// 某条路由的接入地址
    Url { route: String },
}

#[derive(Subcommand)]
enum PricingCommand {
    /// 列出现有价格
    List {
        #[arg(long)]
        json: bool,
    },
    /// 手填/覆盖一条模型价格
    Set {
        model: String,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long, default_value_t = 0.0)]
        input: f64,
        #[arg(long, default_value_t = 0.0)]
        output: f64,
        #[arg(long = "cache-read", default_value_t = 0.0)]
        cache_read: f64,
        #[arg(long = "cache-write", default_value_t = 0.0)]
        cache_write: f64,
        #[arg(long, default_value = "USD")]
        currency: String,
    },
    /// 从公开源导入（models_dev / openrouter），默认只预览
    Import {
        #[arg(long, default_value = "models_dev")]
        source: String,
        /// 确认写入（不加则只打印会导入多少条）
        #[arg(long)]
        yes: bool,
    },
    /// 清空价格表
    Clear {
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum McpCommand {
    /// 列出所有 MCP 定义及其分发情况
    List {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// 把 JSON 里的 mcpServers 写入指定 agent
    Import {
        file: PathBuf,
        #[arg(long, value_delimiter = ',', default_value = "claude")]
        agents: Vec<String>,
        /// 写进项目级 MCP 文件（与 `apply --project` 配套使用）
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum ProviderCommand {
    /// 列出已登记的 provider 与各 agent 的绑定
    List {
        #[arg(long)]
        json: bool,
    },
    /// 登记或更新一个 provider（只写 config.json，不碰 agent 配置）
    Add {
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        base_url: String,
        /// env:VAR / keychain:service/account / 明文（明文必须加 --yes 确认）
        #[arg(long)]
        key: String,
        #[arg(long)]
        model: Vec<String>,
        #[arg(long)]
        note: Option<String>,
        /// 上游协议：anthropic / chat_completions / responses（不填 = 未声明，按路由协议透传）
        #[arg(long)]
        protocol: Option<String>,
        /// 确认「明文密钥会写进目标配置」的风险
        #[arg(long)]
        yes: bool,
    },
    /// 从上游拉模型列表（含上下文 / 最大输出，拿得到才有）
    Models { id: String },
    /// 删除 provider（同时清掉指向它的绑定）
    Delete { id: String },
    /// 把 provider 绑到某个 agent，并生成写入计划
    Use {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        provider: String,
        #[arg(long)]
        model: Option<String>,
        /// 设为该 agent 的默认模型
        #[arg(long = "default")]
        make_default: bool,
        /// 经本地网关写入（配合 --route；不给则直连上游）
        #[arg(long = "via-gateway")]
        via_gateway: bool,
        /// 经网关时走哪条路由
        #[arg(long)]
        route: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
        /// 直接执行（默认只打印计划）
        #[arg(long)]
        yes: bool,
    },
    /// 按已保存的绑定生成计划
    Plan {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum InstructionCommand {
    /// 列出各 agent 的指令文件
    List {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// 把一段共享内容注入某 agent 的规则文件（受控区块）
    Inject {
        #[arg(long, default_value = "claude")]
        agent: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        yes: bool,
    },
    /// 生成 AGENTS.md -> CLAUDE.md / GEMINI.md 桥接
    Bridge {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum ProjectCommand {
    /// 在目录内初始化 .agenthub/project.json
    Init { dir: PathBuf },
    /// 给项目启用中央仓库里的技能
    Enable {
        dir: PathBuf,
        #[arg(long, value_delimiter = ',')]
        skills: Vec<String>,
        #[arg(long)]
        yes: bool,
    },
    /// 列出最近项目
    List,
}

/// `--lang` 是全局参数，帮助文本却要在解析之前定好语言：先扫一遍原始 argv。
fn pre_set_locale_from_args() {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let raw = if let Some(value) = arg.strip_prefix("--lang=") {
            Some(value.to_string())
        } else if arg == "--lang" {
            args.next()
        } else {
            None
        };
        if let Some(raw) = raw
            && !raw.eq_ignore_ascii_case("auto")
            && let Some(locale) = Locale::parse(&raw)
        {
            l10n::set_current(locale);
        }
    }
}

/// 根命令的 `about` 换成词典里的那句话。
///
/// clap 的 `#[command(about = "...")]` 只接受字面量，所以放到运行时设。
/// 子命令仍用 `///` doc comment（中文，给维护者看）——没条目的项回落到它，
/// 不会出现空白帮助。
fn localize_help(cmd: &mut clap::Command) {
    *cmd = std::mem::take(cmd).about(t!(Msg::CliAbout));
}

/// 先本地化帮助，再解析参数（`Cli::parse()` 内部就是这两步，但不给我们改文案的机会）。
fn parse_cli() -> Result<Cli, clap::Error> {
    let mut command = Cli::command();
    localize_help(&mut command);
    let matches = command.try_get_matches()?;
    Cli::from_arg_matches(&matches)
}

fn main() {
    pre_set_locale_from_args();
    let cli = match parse_cli() {
        Ok(cli) => cli,
        Err(err) => err.exit(),
    };
    let mut hub = match AgentHub::bootstrap() {
        Ok(h) => h,
        Err(e) => {
            // bootstrap 失败时配置还没读到，只能按环境变量选语言
            l10n::set_current(l10n::resolve(None, None));
            eprintln!("{}", t!(Msg::CliInitFailed, error = e));
            std::process::exit(1);
        }
    };

    // `--lang` 只影响本次调用：不写 config.json，免得一条命令改掉用户的长期选择
    if let Some(raw) = cli.lang.as_deref()
        && !raw.eq_ignore_ascii_case("auto")
    {
        match Locale::parse(raw) {
            Some(locale) => l10n::set_current(locale),
            None => {
                eprintln!("{}", t!(Msg::ErrUnknownLocale, value = raw));
                std::process::exit(2);
            }
        }
    }

    let result = match cli.command {
        Command::Scan { project, json } => scan(&hub, project, json),
        Command::Duplicates => duplicates(&hub),
        Command::PlanDedupe {
            only,
            baseline,
            out,
        } => plan_dedupe(&mut hub, only, baseline, out),
        Command::PlanRepair { out } => plan_repair(&hub, out),
        Command::Apply { plan, project, yes } => apply_plan(&hub, plan, project, yes),
        Command::Rollback { batch } => rollback(&hub, batch),
        Command::Batches => batches(&hub),
        Command::Doctor => doctor(&hub),
        Command::Agents { probe, json } => agents(&hub, probe, json),
        Command::AgentAdd { key } => agent_add(&mut hub, key),
        Command::AgentRemove { key } => agent_remove(&mut hub, key),
        Command::Settings { json } => settings(&hub, json),
        Command::Mcp { cmd } => mcp(&hub, cmd),
        Command::Instructions { cmd } => instructions(&hub, cmd),
        Command::Project { cmd } => project(&mut hub, cmd),
        Command::Provider { cmd } => provider(&mut hub, cmd),
        Command::Usage {
            range,
            agent,
            model,
            provider,
            json,
            refresh,
        } => usage(&hub, range, agent, model, provider, json, refresh),
        Command::Pricing { cmd } => pricing(&mut hub, cmd),
        Command::Feature { cmd } => feature(&mut hub, cmd),
        Command::Gateway { cmd } => gateway(&hub, cmd),
    };

    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn scan(hub: &AgentHub, project: Option<PathBuf>, json: bool) -> agenthub_core::Result<()> {
    let scope = scope_of(project);
    let report = hub.scan(&scope)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        render::print_scan(&report);
    }
    Ok(())
}

fn duplicates(hub: &AgentHub) -> agenthub_core::Result<()> {
    let report = hub.scan(&Scope::User)?;
    if report.duplicates.is_empty() {
        println!("{}", t!(Msg::CliNoDuplicates));
        return Ok(());
    }
    for group in &report.duplicates {
        println!(
            "{}",
            t!(
                Msg::CliDuplicateLine,
                name = group.name,
                count = group.members.len(),
                size = agenthub_core::human_bytes(group.wasted_bytes),
                members = group.reason
            )
        );
        for member in &group.members {
            println!(
                "    {:<10} {:<8} {}",
                member.agent_label(),
                member.link.label(),
                member.path.display()
            );
        }
    }
    Ok(())
}

fn plan_dedupe(
    hub: &mut AgentHub,
    only: Vec<String>,
    baseline: Option<PathBuf>,
    out: Option<PathBuf>,
) -> agenthub_core::Result<()> {
    let filter = if only.is_empty() {
        None
    } else {
        Some(only.as_slice())
    };
    let cs = hub.plan_dedupe(filter, baseline.as_deref())?;
    finish_plan(&cs, out)
}

fn plan_repair(hub: &AgentHub, out: Option<PathBuf>) -> agenthub_core::Result<()> {
    let cs = hub.plan_repair()?;
    finish_plan(&cs, out)
}

fn finish_plan(cs: &ChangeSet, out: Option<PathBuf>) -> agenthub_core::Result<()> {
    render::print_plan(cs);
    if let Some(path) = out {
        std::fs::write(&path, serde_json::to_string_pretty(cs)?)
            .map_err(|e| agenthub_core::Error::io(&path, e))?;
        println!("{}", t!(Msg::CliPlanWritten, path = path.display()));
    }
    Ok(())
}

fn apply_plan(
    hub: &AgentHub,
    plan: PathBuf,
    project: Option<PathBuf>,
    yes: bool,
) -> agenthub_core::Result<()> {
    let raw = std::fs::read_to_string(&plan).map_err(|e| agenthub_core::Error::io(&plan, e))?;
    let cs: ChangeSet = serde_json::from_str(agenthub_core::text::strip_bom(&raw))?;
    confirm_or_exit(&cs, yes)?;
    // 项目作用域的计划必须用同一作用域 apply，否则项目路径不在受管根内会被拒绝
    let scope = scope_of(project);
    let result = hub.apply(&cs, &scope)?;
    println!(
        "{}",
        t!(
            Msg::CliApplied,
            count = result.applied,
            batch = result.batch_id,
            backup = result
                .backup_dir
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        )
    );
    Ok(())
}

fn rollback(hub: &AgentHub, batch: String) -> agenthub_core::Result<()> {
    for line in hub.rollback(&batch)? {
        println!("{line}");
    }
    Ok(())
}

fn batches(hub: &AgentHub) -> agenthub_core::Result<()> {
    for batch in hub.batches(10)? {
        println!(
            "{}",
            t!(
                Msg::CliBatchRow,
                id = batch.batch_id,
                kind = format!("{:<10}", batch.kind),
                when = format!("{:<20}", batch.title),
                count = batch.op_count,
                undone = if batch.undone {
                    t!(Msg::CliBatchUndone)
                } else {
                    String::new()
                }
            )
        );
    }
    Ok(())
}

fn doctor(hub: &AgentHub) -> agenthub_core::Result<()> {
    let issues = hub.doctor()?;
    if issues.is_empty() {
        println!("{}", t!(Msg::CliDoctorOk));
    }
    for issue in issues {
        println!(
            "[{}] {} — {}",
            issue.level,
            issue.path.display(),
            issue.message
        );
    }
    Ok(())
}

fn agents(hub: &AgentHub, probe: bool, json: bool) -> agenthub_core::Result<()> {
    let entries = hub.agent_catalog(probe);
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        render::print_catalog(&entries);
    }
    Ok(())
}

fn settings(hub: &AgentHub, json: bool) -> agenthub_core::Result<()> {
    let view = hub.settings_view();
    if json {
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        render::print_settings(&view);
    }
    Ok(())
}

fn agent_add(hub: &mut AgentHub, key: String) -> agenthub_core::Result<()> {
    let entries = hub.add_managed_agent(&key)?;
    report_managed(&entries, &key, true);
    Ok(())
}

fn agent_remove(hub: &mut AgentHub, key: String) -> agenthub_core::Result<()> {
    let entries = hub.remove_managed_agent(&key)?;
    report_managed(&entries, &key, false);
    Ok(())
}

fn report_managed(entries: &[agenthub_core::ext::catalog::CatalogEntry], key: &str, added: bool) {
    let name = entries
        .iter()
        .find(|e| e.key == key)
        .map(|e| e.name.as_str())
        .unwrap_or(key);
    if added {
        println!("{}", t!(Msg::CliManagedAdded, name = name));
        println!("{}", t!(Msg::CliManagedAddedHint));
    } else {
        println!("{}", t!(Msg::CliManagedRemoved, name = name));
    }
}

fn mcp(hub: &AgentHub, cmd: McpCommand) -> agenthub_core::Result<()> {
    match cmd {
        McpCommand::List { project } => {
            let scope = scope_of(project);
            render::print_mcp(&hub.mcp_overview(&scope)?);
            Ok(())
        }
        McpCommand::Import {
            file,
            agents,
            project,
            yes,
        } => {
            let raw =
                std::fs::read_to_string(&file).map_err(|e| agenthub_core::Error::io(&file, e))?;
            // Windows 上很多编辑器/`Set-Content` 会写 UTF-8 BOM，serde_json 不接受
            let value: serde_json::Value =
                serde_json::from_str(agenthub_core::text::strip_bom(&raw))?;
            let map = value
                .get("mcpServers")
                .and_then(|v| v.as_object())
                .ok_or_else(|| agenthub_core::Error::msg(t!(Msg::ErrMcpServersMissing)))?;
            let mut servers = Vec::new();
            for (name, def) in map {
                let command = def.get("command").and_then(|v| v.as_str()).unwrap_or("");
                let args = def
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                servers.push(McpServer {
                    name: name.clone(),
                    transport: Transport::Stdio {
                        command: command.to_string(),
                        args,
                        env: Default::default(),
                        cwd: None,
                    },
                    enabled: true,
                    timeout_secs: None,
                    note: None,
                });
            }
            let ids: Vec<AgentId> = agents.iter().map(|a| parse_agent(a)).collect();
            let scope = scope_of(project);
            let cs = hub.plan_mcp_write(&servers, &ids, &scope)?;
            confirm_or_exit(&cs, yes)?;
            let result = hub.apply(&cs, &scope)?;
            println!(
                "{}",
                t!(
                    Msg::CliMcpWritten,
                    count = result.applied,
                    batch = result.batch_id
                )
            );
            Ok(())
        }
    }
}

fn instructions(hub: &AgentHub, cmd: InstructionCommand) -> agenthub_core::Result<()> {
    match cmd {
        InstructionCommand::List { project } => {
            let scope = scope_of(project);
            let (specs, chain) = hub.instructions_overview(&scope);
            for spec in &specs {
                println!(
                    "{:<12} {:<6} {} {}",
                    spec.agent.display_name(),
                    spec.kind.label(),
                    if spec.exists {
                        t!(Msg::CliExists)
                    } else {
                        t!(Msg::CliMissing)
                    },
                    spec.display
                );
            }
            if !chain.is_empty() {
                println!("\n{}", t!(Msg::CliChainHeader));
                for entry in &chain {
                    println!(
                        "  {}. {:<12} {}",
                        entry.order,
                        entry.agent.display_name(),
                        entry.path.display()
                    );
                }
            }
            Ok(())
        }
        InstructionCommand::Inject { agent, text, yes } => {
            let id = parse_agent(&agent);
            let cs = hub.plan_instruction_inject(&id, &Scope::User, &text)?;
            confirm_or_exit(&cs, yes)?;
            let result = hub.apply(&cs, &Scope::User)?;
            println!(
                "{}",
                t!(Msg::CliInstructionInjected, batch = result.batch_id)
            );
            Ok(())
        }
        InstructionCommand::Bridge { project, yes } => {
            let scope = scope_of(project);
            let cs = hub.plan_bridge(&scope)?;
            confirm_or_exit(&cs, yes)?;
            let result = hub.apply(&cs, &scope)?;
            println!("{}", t!(Msg::CliBridgeDone, batch = result.batch_id));
            Ok(())
        }
    }
}

fn project(hub: &mut AgentHub, cmd: ProjectCommand) -> agenthub_core::Result<()> {
    match cmd {
        ProjectCommand::Init { dir } => {
            let result = hub.init_project(&dir)?;
            println!("{}", t!(Msg::CliProjectInit, batch = result.batch_id));
            Ok(())
        }
        ProjectCommand::Enable { dir, skills, yes } => {
            let cs = hub.enable_project_skills(&dir, &skills)?;
            confirm_or_exit(&cs, yes)?;
            let scope = Scope::Project { root: dir.clone() };
            let result = hub.apply(&cs, &scope)?;
            println!(
                "{}",
                t!(
                    Msg::CliProjectSkillsEnabled,
                    count = skills.len(),
                    batch = result.batch_id
                )
            );
            Ok(())
        }
        ProjectCommand::List => {
            for summary in hub.projects() {
                println!(
                    "{}  git={} branch={} skills={} mcp={}",
                    summary.root.display(),
                    summary.is_git,
                    summary.branch.unwrap_or_else(|| "-".into()),
                    summary.skill_count,
                    summary.mcp_count
                );
            }
            Ok(())
        }
    }
}

fn provider(hub: &mut AgentHub, cmd: ProviderCommand) -> agenthub_core::Result<()> {
    use agenthub_core::api::provider::{BindInput, ProviderInput};

    match cmd {
        ProviderCommand::List { json } => {
            let view = hub.providers_view();
            if json {
                println!("{}", serde_json::to_string_pretty(&view)?);
            } else {
                render::print_providers(&view);
            }
            Ok(())
        }
        ProviderCommand::Add {
            id,
            name,
            base_url,
            key,
            model,
            note,
            protocol,
            yes,
        } => {
            let view = hub.upsert_provider(ProviderInput {
                id,
                name,
                base_url,
                api_key: key,
                models: model,
                note,
                acknowledged: yes,
                protocol: protocol
                    .as_deref()
                    .map(|raw| {
                        GatewayProtocol::parse(raw).ok_or_else(|| {
                            agenthub_core::Error::msg(t!(Msg::ErrUnknownProtocol, value = raw))
                        })
                    })
                    .transpose()?,
                model_limits: Default::default(),
                catalog_id: None,
            })?;
            println!("{}", t!(Msg::CliProviderSaved));
            render::print_providers(&view);
            Ok(())
        }
        ProviderCommand::Models { id } => {
            use agenthub_core::api::provider::{ProbeInput, probe_models};
            use agenthub_core::ext::provider::format_tokens;

            let provider = hub
                .config
                .providers
                .iter()
                .find(|p| p.id == id)
                .ok_or_else(|| {
                    agenthub_core::Error::NotFound(t!(Msg::ErrProviderMissing, id = id))
                })?
                .clone();
            let result = probe_models(&ProbeInput {
                base_url: provider.base_url.clone(),
                api_key: provider.api_key.to_reference(),
                protocol: provider.protocol,
            });
            if let Some(error) = &result.error {
                eprintln!("{error}");
                eprintln!("{}", t!(Msg::CliProbeEndpoint, endpoint = result.endpoint));
                return Err(agenthub_core::Error::Other(t!(Msg::CliProbeFailed)));
            }
            println!(
                "{}",
                t!(
                    Msg::CliProbeOk,
                    count = result.models.len(),
                    endpoint = result.endpoint
                )
            );
            for model in &result.models {
                let limits = match (model.context, model.max_output) {
                    (None, None) => String::new(),
                    (context, output) => format!(
                        "  {}{}",
                        context
                            .map(|value| t!(
                                Msg::ProviderContextSize,
                                tokens = format_tokens(value)
                            ))
                            .unwrap_or_default(),
                        output
                            .map(|value| format!(
                                " · {}",
                                t!(Msg::ProviderMaxOutput, tokens = format_tokens(value))
                            ))
                            .unwrap_or_default()
                    ),
                };
                println!("  {}{limits}", model.id);
            }
            Ok(())
        }
        ProviderCommand::Delete { id } => {
            let view = hub.delete_provider(&id)?;
            println!("{}", t!(Msg::CliProviderDeleted, id = id));
            render::print_providers(&view);
            Ok(())
        }
        ProviderCommand::Use {
            agent,
            provider,
            model,
            make_default,
            via_gateway,
            route,
            out,
            yes,
        } => {
            let view = hub.bind_provider(BindInput {
                agent: agent.clone(),
                provider_id: provider,
                model,
                make_default,
                via_gateway,
                route_id: route,
                // CLI 不做槽位映射：那是 Claude Code 专属的精细配置，走桌面端
                ..Default::default()
            })?;
            render::print_providers(&view);
            let cs = hub.plan_provider_apply(&agent)?;
            if yes {
                confirm_or_exit(&cs, true)?;
                if cs.ops.is_empty() {
                    return Ok(());
                }
                let result = hub.apply(&cs, &Scope::User)?;
                println!(
                    "{}",
                    t!(
                        Msg::CliApplied,
                        count = result.applied,
                        batch = result.batch_id,
                        backup = result
                            .backup_dir
                            .map(|p| p.display().to_string())
                            .unwrap_or_default()
                    )
                );
                Ok(())
            } else {
                finish_plan(&cs, out)
            }
        }
        ProviderCommand::Plan { agent, out } => {
            let cs = hub.plan_provider_apply(&agent)?;
            finish_plan(&cs, out)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn usage(
    hub: &AgentHub,
    range: String,
    agent: Option<String>,
    model: Option<String>,
    provider: Option<String>,
    json: bool,
    refresh: bool,
) -> agenthub_core::Result<()> {
    use agenthub_core::api::usage::{UsageRange, narrow};

    let report = hub.usage_report(UsageRange::parse(&range), refresh)?;
    let report = narrow(
        &report,
        agent.as_deref(),
        model.as_deref(),
        provider.as_deref(),
    );
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        render::print_usage(&report);
    }
    Ok(())
}

fn feature(hub: &mut AgentHub, cmd: FeatureCommand) -> agenthub_core::Result<()> {
    match cmd {
        FeatureCommand::List { json } => {
            if json {
                println!("{}", serde_json::to_string_pretty(&hub.config.features)?);
            } else {
                for meta in hub.feature_catalog() {
                    println!(
                        "{}  {:<12} {}",
                        if meta.enabled {
                            t!(Msg::CliFeatureOn)
                        } else {
                            t!(Msg::CliFeatureOff)
                        },
                        meta.display_name,
                        meta.description
                    );
                }
            }
            Ok(())
        }
        FeatureCommand::Enable { key } => {
            hub.set_feature(&key, true)?;
            println!("{}", t!(Msg::CliFeatureEnabled, key = key));
            Ok(())
        }
        FeatureCommand::Disable { key } => {
            hub.set_feature(&key, false)?;
            println!("{}", t!(Msg::CliFeatureDisabled, key = key));
            Ok(())
        }
    }
}

/// 网关只读视图。CLI **不启动**服务：常驻代理属于桌面壳的职责，
/// 这里只回答「配置长什么样、某条路由的接入地址是什么」。
fn gateway(hub: &AgentHub, cmd: GatewayCommand) -> agenthub_core::Result<()> {
    let view = hub.gateway_view();
    match cmd {
        GatewayCommand::List { json } => {
            if json {
                println!("{}", serde_json::to_string_pretty(&view)?);
                return Ok(());
            }
            println!(
                "{}",
                t!(
                    Msg::CliGatewayListen,
                    port = view.config.port,
                    auth = if view.config.auth_token.is_some() {
                        t!(Msg::CliGatewayNeedToken)
                    } else {
                        t!(Msg::CliGatewayNoToken)
                    }
                )
            );
            if view.routes.is_empty() {
                println!("{}", t!(Msg::CliNoRoutes));
            }
            for route in &view.routes {
                println!(
                    "\n{}",
                    t!(
                        Msg::CliRouteLine,
                        id = route.id,
                        protocol = route.mode.label(),
                        name = route.name,
                        members = route.members.len(),
                        rules = route.rules.len(),
                        disabled = if route.enabled {
                            String::new()
                        } else {
                            t!(Msg::CliRouteDisabled)
                        }
                    )
                );
                println!(
                    "{}",
                    t!(Msg::CliRouteUrl, url = hub.gateway_route_url(&route.id))
                );
                for member in &route.members {
                    let name = hub
                        .config
                        .providers
                        .iter()
                        .find(|p| p.id == member.provider_id)
                        .map(|p| p.name.as_str())
                        .unwrap_or(member.provider_id.as_str());
                    println!(
                        "{}",
                        t!(
                            Msg::CliRouteMember,
                            id = name,
                            weight = member.weight.max(1),
                            priority = member.priority
                        )
                    );
                }
            }
            for note in &view.notes {
                println!("\n{}", t!(Msg::CliGatewayHint, note = note));
            }
            Ok(())
        }
        GatewayCommand::Url { route } => {
            if !view.routes.iter().any(|item| item.id == route) {
                return Err(agenthub_core::Error::NotFound(t!(
                    Msg::ErrGatewayRouteMissing,
                    route = route
                )));
            }
            println!("{}", hub.gateway_route_url(&route));
            Ok(())
        }
    }
}

fn pricing(hub: &mut AgentHub, cmd: PricingCommand) -> agenthub_core::Result<()> {
    use agenthub_core::api::pricing::ModelPriceInput;

    match cmd {
        PricingCommand::List { json } => {
            let view = hub.usage_pricing();
            if json {
                println!("{}", serde_json::to_string_pretty(&view)?);
            } else {
                render::print_pricing(&view);
            }
            Ok(())
        }
        PricingCommand::Set {
            model,
            provider,
            input,
            output,
            cache_read,
            cache_write,
            currency,
        } => {
            let view = hub.upsert_usage_price(&ModelPriceInput {
                model,
                provider,
                input,
                output,
                cache_read,
                cache_write,
                currency: Some(currency),
            })?;
            render::print_pricing(&view);
            Ok(())
        }
        PricingCommand::Import { source, yes } => {
            let rows = hub.preview_usage_pricing(&source)?;
            if !yes {
                println!(
                    "{}",
                    t!(Msg::CliPricingPreview, source = source, count = rows.len())
                );
                for row in rows.iter().take(10) {
                    println!(
                        "{}",
                        t!(
                            Msg::CliPricingPreviewRow,
                            model = format!("{:<34}", row.model),
                            provider = format!(
                                "{:<12}",
                                row.provider.clone().unwrap_or_else(|| "-".into())
                            ),
                            input = format!("{:.4}", row.input),
                            output = format!("{:.4}", row.output)
                        )
                    );
                }
                if rows.len() > 10 {
                    println!("{}", t!(Msg::CliPricingMore, count = rows.len()));
                }
                return Ok(());
            }
            let view = hub.apply_usage_pricing(&rows)?;
            println!("{}", t!(Msg::CliPricingImported, count = rows.len()));
            render::print_pricing(&view);
            Ok(())
        }
        PricingCommand::Clear { yes } => {
            if !yes {
                println!("{}", t!(Msg::CliPricingClearHint));
                return Ok(());
            }
            let view = hub.clear_usage_pricing()?;
            render::print_pricing(&view);
            Ok(())
        }
    }
}

fn scope_of(project: Option<PathBuf>) -> Scope {
    match project {
        Some(root) => Scope::Project {
            root: agenthub_core::paths::normalize_lexical(&root),
        },
        None => Scope::User,
    }
}

fn parse_agent(raw: &str) -> AgentId {
    match raw.to_ascii_lowercase().as_str() {
        "claude" | "cc" => AgentId::Claude,
        "codex" | "cx" => AgentId::Codex,
        "codebuddy" | "cb" => AgentId::CodeBuddy,
        "hermes" | "hm" => AgentId::Hermes,
        "gemini" | "gm" => AgentId::Gemini,
        other => AgentId::Custom(other.to_string()),
    }
}

fn confirm_or_exit(cs: &ChangeSet, yes: bool) -> agenthub_core::Result<()> {
    render::print_plan(cs);
    if cs.ops.is_empty() {
        return Ok(());
    }
    if yes {
        return Ok(());
    }
    print!("\n{}", t!(Msg::CliConfirmApply, count = cs.ops.len()));
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    if !matches!(input.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        return Err(agenthub_core::Error::msg(t!(Msg::CliCancelled)));
    }
    Ok(())
}
