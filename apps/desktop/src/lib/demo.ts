import { basename, tailPath } from "./format";
import { tr } from "./i18n/runtime";
import { RANGE_OPTIONS, groupCells, providerKey, totalsOf } from "./usage";
import type {
  AgentKind,
  AgentPathOverride,
  AgentPathsView,
  ApplyReport,
  ApplyResult,
  BatchRecord,
  BindProviderInput,
  CatalogEntry,
  CustomAgentDef,
  ChainEntry,
  ChangeSet,
  DeleteOptions,
  DuplicateGroup,
  EnvState,
  FeatureFlags,
  FeatureKey,
  FeatureMeta,
  GatewayConfigInput,
  GatewayConfigView,
  GatewayRequestLog,
  GatewayRouteView,
  GatewayStatusView,
  GatewayView,
  InstallStep,
  InstructionOverview,
  InstructionSpec,
  LinkStrategyName,
  LocaleModeKey,
  LocaleView,
  ManagedState,
  ManagedTarget,
  MarketAgent,
  MarketInstallInput,
  MarketPage,
  MarketSkill,
  MarketSkillDetail,
  MarketSort,
  ModelLimits,
  ModelSlots,
  PoolMode,
  MarketSourceKey,
  McpServerView,
  ModelPrice,
  ModelPriceInput,
  Overview,
  PricingView,
  ProjectSummary,
  ProviderInput,
  ProviderView,
  ProvidersView,
  Risk,
  RouteDeleteResult,
  ProbeInput,
  ProbeReport,
  ProbeResult,
  ScanReport,
  SettingsInput,
  SupplierDef,
  SettingsView,
  SkillRecord,
  StrategyConfig,
  UsageCell,
  UsageRange,
  UsageReport,
  UsageSourceStats,
} from "./types";

const now = () => Math.floor(Date.now() / 1000);
const HOUR = 3600;

// 演示数据按运行平台生成：Windows 上不该看到 /Users/... 与 /opt/homebrew/...
//
// `?platform=windows` / `?platform=macos` 可以强制另一端的演示数据：无头浏览器跑 UI 断言时
// 一台机器就能覆盖两端（脚本见 `scripts/ui-smoke.mjs`）。
const FORCED_PLATFORM =
  typeof location !== "undefined"
    ? new URLSearchParams(location.search).get("platform")
    : null;
const IS_WINDOWS = FORCED_PLATFORM
  ? FORCED_PLATFORM.toLowerCase() === "windows"
  : typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);
const HOME = IS_WINDOWS ? "C:\\Users\\demo" : "/Users/demo";
const BIN_DIR = IS_WINDOWS ? `${HOME}\\AppData\\Local\\Programs` : "/opt/homebrew/bin";
const PLATFORM_LABEL = IS_WINDOWS ? "Windows (x64)" : "macOS (arm64)";
const LINK_STRATEGY: LinkStrategyName = IS_WINDOWS ? "junction" : "symlink";
const LINK_LABEL = IS_WINDOWS ? "目录联接" : "符号链接";
/** 演示模式下的「单条策略覆盖」，切换页面时不丢（刷新会重置，属预期）。 */
let demoOverrides: Record<string, LinkStrategyName> = {};

/** 功能开关：默认全开，与后端一致。 */
let demoFeatures: FeatureFlags = { provider: true, usage: true };

/** 演示模式的界面语言：只存在内存里，刷新就回到「跟随系统」。 */
let demoLocaleMode: LocaleModeKey = "system";

function demoLocale(): LocaleView {
  // 演示数据本身是中文，实际生效语言固定报简体
  return { mode: demoLocaleMode, effective: "zh-Hans", tag: "zh-CN" };
}

function demoSetLocale(mode: LocaleModeKey, _effective: string): LocaleView {
  demoLocaleMode = mode;
  return demoLocale();
}

/** 演示模式下的「agent 独有」白名单，初值与后端 `DEFAULT_AGENT_LOCAL_SKILLS` 一致。 */
let demoAgentLocal = new Set<string>([
  "computer-use",
  "computer_use",
  "computeruse",
  "computer-control",
  "browser-use",
  "browser_use",
]);

/** 与后端 `AppConfig::is_agent_local` 同义：trim + 大小写不敏感。 */
function isAgentLocalName(name: string): boolean {
  const key = name.trim().toLowerCase();
  return key.length > 0 && demoAgentLocal.has(key);
}

function strategyConfig(): StrategyConfig {
  return {
    default: LINK_STRATEGY,
    platform: LINK_STRATEGY,
    overrides: { ...demoOverrides },
  };
}

const NODE_REPL = IS_WINDOWS
  ? `${BIN_DIR}\\ChatGPT\\cua_node\\node_repl.exe`
  : "/Applications/ChatGPT.app/Contents/Resources/cua_node/bin/node_repl";

// 桌面端的应用包与配置文件位置两平台完全不同（Windows 在 Programs / MSIX Packages 下，
// macOS 在 /Applications 与 ~/Library/Application Support），演示数据必须跟着平台走，
// 否则 macOS 上会看到 `/Users/demo/AppData/...` 这种不存在的路径。
const CLAUDE_DESKTOP_APP = IS_WINDOWS
  ? `${HOME}/AppData/Local/Packages/Claude_pzs8sxrjxfjjc`
  : "/Applications/Claude.app";
const CLAUDE_DESKTOP_CFG = IS_WINDOWS
  ? `${HOME}/AppData/Local/Packages/Claude_pzs8sxrjxfjjc/LocalCache/Roaming/Claude/claude_desktop_config.json`
  : `${HOME}/Library/Application Support/Claude/claude_desktop_config.json`;
const OPENAI_DESKTOP_APP = IS_WINDOWS
  ? `${HOME}/AppData/Local/Packages/OpenAI.Codex_2p2nqsd0c76g0`
  : "/Applications/ChatGPT.app";
const WORKBUDDY_APP = IS_WINDOWS
  ? `${HOME}/AppData/Local/Programs/WorkBuddy`
  : "/Applications/WorkBuddy.app";
/// 国际版是同代码换 dataFolderName 的另一客户端：exe 名 WorkBuddyAI、数据根 ~/.workbuddy-ai
const WORKBUDDY_AI_APP = IS_WINDOWS
  ? `${HOME}/AppData/Local/Programs/WorkBuddyAI`
  : "/Applications/WorkBuddy AI.app";
/// CodeBuddy IDE 国内版是独立客户端（目录带 CN 后缀），国际版沿用 ~/.codebuddy
const CODEBUDDY_IDE_APP = IS_WINDOWS
  ? `${HOME}/AppData/Local/Programs/CodeBuddy CN`
  : "/Applications/CodeBuddy CN.app";
const TRAE_APP = IS_WINDOWS
  ? `${HOME}/AppData/Local/Programs/Trae`
  : "/Applications/Trae.app";
const TRAE_CN_APP = IS_WINDOWS
  ? `${HOME}/AppData/Local/Programs/Trae CN`
  : "/Applications/Trae CN.app";
/// Hermes 的数据根：Windows 安装器放 `%LOCALAPPDATA%\hermes`，macOS 就是 `~/.hermes`
/// （两平台均已实测）。
const HERMES_ROOT = IS_WINDOWS ? `${HOME}/AppData/Local/hermes` : `${HOME}/.hermes`;

/** 把 `/` 拼出的演示路径转成当前平台形态（Windows → 反斜杠）。 */
function p(raw: string): string {
  return IS_WINDOWS ? raw.replace(/\//g, "\\") : raw;
}

const STORE = p(`${HOME}/.agenthub/store`);

function store(name: string, size: number, description: string): SkillRecord {
  return {
    name,
    path: `${STORE}/${name}`,
    agent: null,
    scope_label: "全局",
    source: "store",
    readonly: false,
    description,
    tree_hash: `b3-${name}-9f2c41d8`,
    size_bytes: size,
    file_count: Math.max(3, Math.round(size / 9000)),
    modified_at: now() - HOUR * 30,
    link: { kind: "real_dir" },
    canonical: `${STORE}/${name}`,
    never_upload: false,
    agent_local: isAgentLocalName(name),
  };
}

function copy(
  agent: string,
  name: string,
  size: number,
  description: string,
  link: SkillRecord["link"],
  readonly = false
): SkillRecord {
  const base = agent === "codex" ? ".codex" : `.${agent}`;
  const at = p(`${HOME}/${base}/skills/${name}`);
  return {
    name,
    path: at,
    agent,
    scope_label: "全局",
    source: readonly ? "readonly" : "agent_copy",
    readonly,
    description,
    tree_hash: `b3-${name}-9f2c41d8`,
    size_bytes: size,
    file_count: Math.max(3, Math.round(size / 9000)),
    modified_at: now() - HOUR * (12 + name.length),
    link,
    canonical:
      link.kind === "symlink" || link.kind === "junction" || link.kind === "hardlink"
        ? link.target
        : at,
    never_upload: false,
    agent_local: isAgentLocalName(name),
  };
}

const D = {
  cloudflare: "Cloudflare Workers / Pages / D1 / R2 全栈开发参考：路由、绑定、限流与可观测性。",
  agentsSdk: "在 Workers 上构建有状态 Agent：Agent 类、持久化、RPC、WebSocket 与工作流。",
  grill: "对设计文档做对抗式评审，先问清边界条件再写代码。",
  projectMgmt: "项目计划、PRD 撰写与任务拆解，输出可执行里程碑。",
  playwright: "浏览器自动化与截图验收，用于端到端回归。",
  imagegen: "Codex 内置图像生成工具（系统只读）。",
};

function buildRecords(applied: boolean): SkillRecord[] {
  const storeSkill = applied
    ? { kind: "symlink" as const, target: `${STORE}/cloudflare` }
    : { kind: "real_dir" as const };
  return [
    store("cloudflare", 626_688, D.cloudflare),
    store("agents-sdk", 1_468_006, D.agentsSdk),
    store("grill-with-docs", 90_112, D.grill),
    store("project-management", 24_576, D.projectMgmt),

    copy("claude", "cloudflare", 641_024, D.cloudflare, storeSkill),
    copy("claude", "agents-sdk", 1_468_006, D.agentsSdk, { kind: "real_dir" }),
    copy("claude", "grill-with-docs", 90_112, D.grill, {
      kind: "symlink",
      target: `${STORE}/grill-with-docs`,
    }),
    copy("claude", "project-management", 24_576, D.projectMgmt, {
      kind: "symlink",
      target: `${STORE}/project-management`,
    }),

    copy("codex", "cloudflare", applied ? 626_688 : 638_976, D.cloudflare, storeSkill),
    copy("codex", "agents-sdk", 1_468_006, D.agentsSdk, { kind: "real_dir" }),
    copy("codex", "imagegen", 210_944, D.imagegen, { kind: "real_dir" }, true),

    copy("gemini", "cloudflare", 638_976, D.cloudflare, storeSkill),
    copy("gemini", "agents-sdk", 1_468_006, D.agentsSdk, { kind: "real_dir" }),

    copy("hermes", "cloudflare", 640_000, D.cloudflare, storeSkill),
    copy("hermes", "agents-sdk", 1_468_006, D.agentsSdk, { kind: "real_dir" }),
    copy("hermes", "playwright-skill", 3_276_800, D.playwright, { kind: "real_dir" }),

    copy("codebuddy", "cloudflare", 0, D.cloudflare, applied ? storeSkill : {
      kind: "broken",
      target: `${STORE}/cloudflare`,
    }),
  ];
}

function buildDuplicates(records: SkillRecord[], applied: boolean): DuplicateGroup[] {
  if (applied) return [];
  // 与后端一致：agent 独有技能不进重复组
  const pick = (name: string) =>
    isAgentLocalName(name) ? [] : records.filter((r) => r.name === name);
  const cloudflare = pick("cloudflare");
  const agents = pick("agents-sdk");
  return [
    {
      name: "agents-sdk",
      members: agents,
      identical: true,
      wasted_bytes: agents
        .filter((m) => m.source === "agent_copy")
        .reduce((sum, m) => sum + m.size_bytes, 0),
      suggested_source: `${STORE}/agents-sdk`,
      reason: "4 份内容完全一致，收敛为单份即可释放约 4.2 MB",
    },
    {
      name: "cloudflare",
      members: cloudflare,
      identical: true,
      wasted_bytes: 1_916_000,
      suggested_source: `${STORE}/cloudflare`,
      reason: "中央仓库已有版本，可将其余副本统一为链接",
    },
  ];
}

function buildOverview(applied: boolean): Overview {
  const records = buildRecords(applied);
  const duplicates = buildDuplicates(records, applied);
  const agentCopies = records.filter((r) => r.agent);
  const linked = agentCopies.filter((r) =>
    ["symlink", "junction", "hardlink"].includes(r.link.kind)
  ).length;
  const copies = agentCopies.filter((r) => r.link.kind === "real_dir" && !r.readonly).length;
  const broken = agentCopies.filter((r) => r.link.kind === "broken").length;

  const report: ScanReport = {
    generated_at: now(),
    scope_label: "全局",
    agents: [
      {
        id: "claude",
        display_name: "Claude Code",
        config_dir: p(`${HOME}/.claude`),
        cli_path: p(`${BIN_DIR}/claude`),
        capabilities: { skills: true, mcp: true, instructions: true, project_scope: true, links: true },
        source: "builtin",
        detected: true,
      },
      {
        id: "codex",
        display_name: "Codex",
        config_dir: p(`${HOME}/.codex`),
        cli_path: p(`${BIN_DIR}/codex`),
        capabilities: { skills: true, mcp: true, instructions: true, project_scope: true, links: true },
        source: "builtin",
        detected: true,
      },
      {
        id: "codebuddy",
        display_name: "CodeBuddy",
        config_dir: p(`${HOME}/.codebuddy`),
        cli_path: p(`${BIN_DIR}/codebuddy`),
        capabilities: { skills: true, mcp: true, instructions: true, project_scope: true, links: true },
        source: "builtin",
        detected: true,
      },
      {
        id: "hermes",
        display_name: "Hermes",
        config_dir: p(`${HOME}/.hermes`),
        cli_path: p(`${HOME}/.local/bin/hermes`),
        capabilities: { skills: true, mcp: true, instructions: true, project_scope: true, links: true },
        source: "builtin",
        detected: true,
      },
      {
        id: "gemini",
        display_name: "Gemini CLI",
        config_dir: p(`${HOME}/.gemini`),
        cli_path: null,
        capabilities: { skills: true, mcp: true, instructions: true, project_scope: true, links: true },
        source: "builtin",
        detected: true,
      },
    ],
    targets: [],
    records,
    duplicates,
    stats: {
      total: records.length,
      store_skills: records.filter((r) => r.source === "store").length,
      agent_skills: agentCopies.length,
      linked,
      copies,
      broken,
      readonly: records.filter((r) => r.source === "readonly").length,
      store_bytes: records.filter((r) => r.source === "store").reduce((s, r) => s + r.size_bytes, 0),
      wasted_bytes: duplicates.reduce((s, d) => s + d.wasted_bytes, 0),
      shared_skills: linked > 0 ? 2 : 0,
    },
    store_root: STORE,
    duration_ms: 268,
  };

  return {
    report,
    mcp: mcpViews(),
    batches: batches(applied),
    doctor: doctor(duplicates),
    data_root: p(`${HOME}/.agenthub`),
    platform: PLATFORM_LABEL,
    strategies: strategyConfig(),
    demo: true,
  };
}

function mcpViews(): McpServerView[] {
  const t = (agent: string, file: string, format: string, present: boolean) => ({
    agent,
    file,
    format,
    present,
    writable: true,
    reason: null,
  });
  return [
    {
      name: "chrome-devtools",
      transport: {
        type: "stdio",
        command: "npx",
        args: ["-y", "chrome-devtools-mcp@latest", "--headless"],
        env: { CHROME_DEVTOOLS_TOKEN: { kind: "env", var: "CDT_TOKEN" } },
        cwd: null,
      },
      enabled: true,
      timeout_secs: 180,
      note: "浏览器调试与截图，Hermes 与 Codex 都在用",
      targets: [
        t("claude", p(`${HOME}/.claude.json`), "JSON", true),
        t("codex", p(`${HOME}/.codex/config.toml`), "TOML", true),
        t("hermes", p(`${HOME}/.hermes/config.yaml`), "YAML", true),
        t("gemini", p(`${HOME}/.gemini/settings.json`), "JSON", false),
        t("codebuddy", p(`${HOME}/.codebuddy/mcp.json`), "JSON", false),
      ],
    },
    {
      name: "node_repl",
      transport: {
        type: "stdio",
        command: NODE_REPL,
        args: [],
        env: { NODE_REPL_NATIVE_PIPE_CONNECT_TIMEOUT_MS: { kind: "literal", value: "1000", acknowledged: true } },
        cwd: p(`${HOME}/.codex`),
      },
      enabled: true,
      timeout_secs: 120,
      note: "Codex 自带的 Node REPL 桥",
      targets: [
        t("codex", p(`${HOME}/.codex/config.toml`), "TOML", true),
        t("claude", p(`${HOME}/.claude.json`), "JSON", false),
        t("hermes", p(`${HOME}/.hermes/config.yaml`), "YAML", false),
        t("gemini", p(`${HOME}/.gemini/settings.json`), "JSON", false),
        t("codebuddy", p(`${HOME}/.codebuddy/mcp.json`), "JSON", false),
      ],
    },
    {
      name: "filesystem",
      transport: {
        type: "stdio",
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-filesystem", p(`${HOME}/Dev`)],
        env: {},
        cwd: null,
      },
      enabled: false,
      timeout_secs: null,
      note: "受限目录访问，当前处于禁用状态",
      targets: [
        t("claude", p(`${HOME}/.claude.json`), "JSON", false),
        t("codex", p(`${HOME}/.codex/config.toml`), "TOML", false),
        t("hermes", p(`${HOME}/.hermes/config.yaml`), "YAML", false),
        t("gemini", p(`${HOME}/.gemini/settings.json`), "JSON", false),
        t("codebuddy", p(`${HOME}/.codebuddy/mcp.json`), "JSON", false),
      ],
    },
  ];
}

/** 各 agent 的项目级 MCP 文件（Hermes 没有项目级入口）。 */
const PROJECT_MCP: Record<string, string | null> = {
  claude: ".mcp.json",
  codex: ".codex/config.toml",
  codebuddy: ".codebuddy/mcp.json",
  hermes: null,
  gemini: ".gemini/settings.json",
};

function instructionSpecs(): InstructionSpec[] {
  const spec = (
    agent: string,
    path: string,
    display: string,
    exists: boolean,
    managed: boolean,
    kind: InstructionSpec["kind"] = "rule",
    size = 0
  ): InstructionSpec => ({
    agent,
    path,
    kind,
    exists,
    managed,
    size_bytes: size,
    display,
  });
  return [
    spec("claude", p(`${HOME}/.claude/CLAUDE.md`), "~/.claude/CLAUDE.md", true, true, "rule", 4_820),
    spec("codex", p(`${HOME}/.codex/AGENTS.md`), "~/.codex/AGENTS.md", true, false, "rule", 2_914),
    spec("codebuddy", p(`${HOME}/.codebuddy/AGENTS.md`), "~/.codebuddy/AGENTS.md", false, false, "rule"),
    spec("hermes", p(`${HOME}/.hermes/AGENTS.md`), "~/.hermes/AGENTS.md", true, true, "rule", 3_640),
    spec("hermes", p(`${HOME}/.hermes/SOUL.md`), "~/.hermes/SOUL.md", true, false, "persona", 1_204),
    spec("hermes", p(`${HOME}/.hermes/USER.md`), "~/.hermes/USER.md", true, false, "persona", 876),
    spec("gemini", p(`${HOME}/.gemini/GEMINI.md`), "~/.gemini/GEMINI.md", false, false, "rule"),
  ];
}

function instructionChain(): ChainEntry[] {
  const specs = instructionSpecs();
  return specs.map((s, idx) => ({
    agent: s.agent,
    order: idx + 1,
    path: s.path,
    exists: s.exists,
    managed: s.managed,
    shadowed: false,
    note: s.exists ? (s.managed ? "含受控区块，由 AgentHub 维护" : "用户自有内容") : "文件不存在，可一键创建",
  }));
}

function projects(): ProjectSummary[] {
  return [
    {
      root: p(`${HOME}/Dev/AgentHub`),
      name: "AgentHub",
      branch: "main",
      is_git: true,
      has_state: true,
      skill_count: 3,
      mcp_count: 2,
      agent_dirs: [
        p(`${HOME}/Dev/AgentHub/.claude/skills`),
        p(`${HOME}/Dev/AgentHub/.codex/skills`),
      ],
    },
    {
      root: p(`${HOME}/Dev/acme-web`),
      name: "acme-web",
      branch: "main",
      is_git: true,
      has_state: true,
      skill_count: 6,
      mcp_count: 1,
      agent_dirs: [p(`${HOME}/Dev/acme-web/.claude/skills`)],
    },
    {
      root: p(`${HOME}/Dev/legacy-cms`),
      name: "legacy-cms",
      branch: "main",
      is_git: true,
      has_state: false,
      skill_count: 0,
      mcp_count: 0,
      agent_dirs: [],
    },
  ];
}

function batches(applied: boolean): BatchRecord[] {
  const base: BatchRecord[] = [
    {
      batch_id: "cs-20261027-141806-0000-118-001",
      kind: "去重接管",
      title: "技能去重接管",
      created_at: now() - HOUR * 26,
      undone: false,
      op_count: 3,
    },
    {
      batch_id: "cs-20261024-093311-0000-402-002",
      kind: "MCP 写入",
      title: "写入 2 个 MCP 定义",
      created_at: now() - HOUR * 80,
      undone: false,
      op_count: 4,
    },
  ];
  if (applied) {
    base.unshift({
      batch_id: "cs-demo-applied",
      kind: "去重接管",
      title: "技能去重接管（演示）",
      created_at: now() - 60,
      undone: false,
      op_count: 7,
    });
  }
  return base;
}

function doctor(duplicates: DuplicateGroup[]) {
  return [
    {
      level: "danger",
      path: p(`${HOME}/.codebuddy/skills/cloudflare`),
      message: "链接失效（目标不存在），可一键修复",
      kind: "broken_link",
      subject: "cloudflare",
    },
    ...duplicates.map((d) => ({
      level: "warn",
      path: d.suggested_source,
      message: `技能 ${d.name} 有 ${d.members.length} 份拷贝，可释放约 ${(d.wasted_bytes / 1024 / 1024).toFixed(1)} MB`,
      kind: "duplicate_copy",
      subject: d.name,
    })),
    {
      level: "info",
      path: p(`${HOME}/.codex/skills/.system`),
      message: "系统内置技能（imagegen / skill-creator）保持只读",
      kind: "info_note",
      subject: null,
    },
  ];
}

function dedupePlan(applied: boolean): ChangeSet {
  const records = buildRecords(applied);
  const group = buildDuplicates(records, applied)[0];
  const ops: ChangeSet["ops"] = [];
  if (group) {
    const target = `${STORE}/${group.name}`;
    ops.push({
      id: 0,
      label: "移入中央仓库",
      action: {
        op: "move_into_store",
        from: p(`${HOME}/.hermes/skills/${group.name}`),
        to: target,
      },
    });
    for (const member of group.members) {
      if (member.source === "store" || member.readonly) continue;
      if (member.link.kind === LINK_STRATEGY) continue;
      ops.push({
        id: ops.length,
        label: `替换为${LINK_LABEL}`,
        action: { op: "replace_with_link", at: member.path, target, strategy: LINK_STRATEGY },
      });
    }
  }
  return {
    id: `cs-demo-${Math.floor(Date.now() / 1000)}`,
    kind: "dedupe",
    title: "技能去重接管",
    ops,
    notes: [
      `以 ${group?.suggested_source ?? STORE} 作为基线`,
      "原有副本会先复制进 ~/.agenthub/backups 再删除，可一键回滚",
    ],
    risks: [
      { level: "warn", message: "接管后 4 个 agent 将共享同一份内容，改一处全生效" },
      { level: "info", message: "Codex 的 skills/.system 为只读，不会被改写" },
    ],
  };
}

// ---- Agent 环境（演示态：三种状态都有样本） ----

/** 演示模式下的「已登记受管」集合，和真实后端一样只影响配置，不碰目录。 */
const demoManaged = new Set<string>();

interface CatalogSeed {
  key: string;
  family: string;
  name: string;
  vendor: string;
  kind: AgentKind;
  official: string;
  builtin: string | null;
  state: EnvState;
  cli?: string;
  version?: string;
  app?: string;
  config?: string;
  steps: InstallStep[];
  targets?: ManagedTarget[];
  note?: string;
  blocked?: string;
  needsConfirm?: boolean;
  /** 配置与某个内置适配器共用（如 OpenAI 桌面端 → codex） */
  shared?: string;
  /** Store（MSIX）包家族名，命中时作为首要证据 */
  msix?: string;
}

const step = (
  title: string,
  command: string,
  risk: string,
  url: string | null = null
): InstallStep => ({ title, command, risk, url });

const NPM_RISK = "需要 Node.js 18+；全局安装可能要管理员权限";

const CATALOG_SEED: CatalogSeed[] = [
  {
    key: "claude-code",
    family: "claude",
    name: "Claude Code",
    vendor: "Anthropic",
    kind: "cli",
    official: "https://docs.claude.com/en/docs/claude-code/setup",
    builtin: "claude",
    state: "ready",
    cli: "claude",
    version: "2.1.306",
    config: `${HOME}/.claude`,
    steps: [
      step("安装 Claude Code", "npm install -g @anthropic-ai/claude-code", NPM_RISK),
      step("验证", "claude --version", "只读验证，不会改动任何文件"),
    ],
  },
  {
    key: "claude-desktop",
    family: "claude",
    name: "Claude Desktop",
    vendor: "Anthropic",
    kind: "desktop",
    official: "https://claude.ai/download",
    builtin: null,
    state: "ready",
    // macOS 没有 Store（MSIX）概念，这里不填，检测依据回落到「应用存在」
    msix: IS_WINDOWS ? "Claude_pzs8sxrjxfjjc" : undefined,
    app: CLAUDE_DESKTOP_APP,
    config: CLAUDE_DESKTOP_CFG,
    steps: [
      step("下载 Claude Desktop", "https://claude.ai/download", "官网下载；Store 版同样可用"),
      step("重启应用", "配置文件由 AgentHub 写入后需完全退出并重开", "MCP 配置只在启动时读取"),
    ],
    targets: [
      {
        label: "MCP 文件",
        path: p(CLAUDE_DESKTOP_CFG),
        exists: true,
        scope: "user",
      },
    ],
    note: "只接管 MCP：Claude Desktop 没有技能目录约定",
  },
  {
    key: "codex",
    family: "codex",
    name: "Codex CLI",
    vendor: "OpenAI",
    kind: "cli",
    official: "https://github.com/openai/codex",
    builtin: "codex",
    state: "ready",
    cli: "codex",
    version: "0.163.0",
    config: `${HOME}/.codex`,
    steps: [
      step("安装 Codex CLI", "npm install -g @openai/codex", NPM_RISK),
      step("验证", "codex --version", "只读验证，不会改动任何文件"),
    ],
  },
  {
    key: "chatgpt-desktop",
    family: "codex",
    name: "ChatGPT / Codex 桌面",
    vendor: "OpenAI",
    kind: "desktop",
    official: "https://chatgpt.com/download",
    builtin: null,
    state: "ready",
    msix: IS_WINDOWS ? "OpenAI.Codex_2p2nqsd0c76g0" : undefined,
    app: OPENAI_DESKTOP_APP,
    config: `${HOME}/.codex/config.toml`,
    steps: [
      step("下载 ChatGPT 桌面端", "https://chatgpt.com/download", "Store 里包名为 OpenAI.Codex"),
      step("MCP 配置位置", "~/.codex/config.toml（与 Codex CLI 共用）", "改这里等于改 Codex 的配置"),
    ],
    shared: "codex",
  },
  {
    key: "codebuddy",
    family: "tencent",
    name: "CodeBuddy Code",
    vendor: "腾讯云",
    kind: "cli",
    official: "https://www.codebuddy.cn/docs/cli/quickstart",
    builtin: "codebuddy",
    state: "ready",
    cli: "codebuddy",
    version: "2.164.0",
    config: `${HOME}/.codebuddy`,
    steps: [
      step("安装 CodeBuddy Code", "npm install -g @tencent-ai/codebuddy-code", NPM_RISK),
      step(
        "登录站点选择",
        "codebuddy 首次启动时选 Chinese Site / International Site",
        "国内站与国际站账号、额度独立；CLI 两版共用 ~/.codebuddy"
      ),
      step("验证", "codebuddy --version", "只读验证，不会改动任何文件"),
    ],
  },
  {
    key: "codebuddy-ide",
    family: "tencent",
    name: "CodeBuddy IDE",
    vendor: "腾讯云",
    kind: "ide",
    official: "https://www.codebuddy.ai/ide",
    builtin: null,
    state: "ready",
    app: CODEBUDDY_IDE_APP,
    config: `${HOME}/.codebuddycn`,
    steps: [
      step("下载 CodeBuddy IDE（国际版）", "https://www.codebuddy.ai/ide", "国际版配置目录 ~/.codebuddy，与 CLI 共用"),
      step("下载 CodeBuddy IDE（国内版）", "https://www.codebuddy.cn/ide/", "国内版配置目录 ~/.codebuddycn"),
    ],
    blocked: "IDE 的 MCP / 技能配置位置未实测，先只做检测",
    needsConfirm: true,
  },
  {
    key: "workbuddy",
    family: "tencent",
    name: "WorkBuddy（国内版）",
    vendor: "腾讯云",
    kind: "desktop",
    official: "https://www.workbuddy.cn/",
    builtin: null,
    state: "ready",
    app: WORKBUDDY_APP,
    config: `${HOME}/.workbuddy/mcp.json`,
    steps: [
      step("下载 WorkBuddy（国内版）", "https://www.workbuddy.cn/", "国内版数据根 ~/.workbuddy"),
      step("国际版是另一个客户端", "https://www.workbuddy.ai/", "国际版 WorkBuddy AI 数据根 ~/.workbuddy-ai，可同机共存"),
      step("配置位置", "~/.workbuddy/mcp.json（顶层 mcpServers）", "技能在 ~/.workbuddy/skills"),
    ],
    targets: [
      {
        label: "技能目录",
        path: p(`${HOME}/.workbuddy/skills`),
        exists: true,
        scope: "user",
      },
      {
        label: "MCP 文件",
        path: p(`${HOME}/.workbuddy/mcp.json`),
        exists: true,
        scope: "user",
      },
      {
        label: "指令文件",
        path: p(`${HOME}/.workbuddy/SOUL.md`),
        exists: true,
        scope: "user",
      },
      {
        label: "指令文件",
        path: p(`${HOME}/.workbuddy/USER.md`),
        exists: true,
        scope: "user",
      },
    ],
    note: "接管 ~/.workbuddy 下的技能、MCP 与人格文件；新写入的 MCP server 可能还要在 WorkBuddy 内批准一次",
  },
  {
    key: "workbuddy-ai",
    family: "tencent",
    name: "WorkBuddy AI（国际版）",
    vendor: "腾讯云",
    kind: "desktop",
    official: "https://www.workbuddy.ai/",
    builtin: null,
    state: "missing",
    app: WORKBUDDY_AI_APP,
    config: `${HOME}/.workbuddy-ai/mcp.json`,
    steps: [
      step("下载 WorkBuddy AI（国际版）", "https://www.workbuddy.ai/", "Windows 装在 Programs/WorkBuddyAI"),
      step("国内版是另一个客户端", "https://www.workbuddy.cn/", "国内版数据根 ~/.workbuddy，两版互不覆盖"),
    ],
    targets: [
      {
        label: "技能目录",
        path: p(`${HOME}/.workbuddy-ai/skills`),
        exists: false,
        scope: "user",
      },
      {
        label: "MCP 文件",
        path: p(`${HOME}/.workbuddy-ai/mcp.json`),
        exists: false,
        scope: "user",
      },
    ],
    note: "接管 ~/.workbuddy-ai 下的技能与 MCP；国际版写入的第三方 server 要在连接器中心手动信任一次",
    needsConfirm: true,
  },
  {
    key: "trae",
    family: "bytedance",
    name: "Trae（国际版）",
    vendor: "字节跳动",
    kind: "ide",
    official: "https://www.trae.ai/",
    builtin: null,
    state: "missing",
    app: TRAE_APP,
    steps: [
      step("下载 Trae（国际版）", "https://www.trae.ai/", "配置目录 %APPDATA%/Trae、数据根 ~/.trae"),
      step("国内版是另一个安装包", "https://www.trae.cn/", "国内版目录带 CN 后缀（%APPDATA%/Trae CN、~/.trae-cn）"),
      step("项目级 MCP 位置", ".trae/mcp.json（顶层 mcpServers）", "Trae 只能按项目接管"),
    ],
    targets: [
      { label: "项目 MCP 文件", path: ".trae/mcp.json", exists: false, scope: "project" },
    ],
    note: "只接管项目级 MCP（.trae/mcp.json），对已登记的项目生效",
    needsConfirm: true,
  },
  {
    key: "trae-cn",
    family: "bytedance",
    name: "Trae CN（国内版）",
    vendor: "字节跳动",
    kind: "ide",
    official: "https://www.trae.cn/",
    builtin: null,
    state: "missing",
    app: TRAE_CN_APP,
    steps: [
      step("下载 Trae CN（国内版）", "https://www.trae.cn/", "配置目录 %APPDATA%/Trae CN、数据根 ~/.trae-cn"),
      step("国际版是另一个安装包", "https://www.trae.ai/", "国际版目录不带 CN 后缀（%APPDATA%/Trae、~/.trae）"),
      step("项目级 MCP 位置", ".trae/mcp.json（顶层 mcpServers）", "两版是否共用同名项目文件未核实"),
    ],
    targets: [
      { label: "项目 MCP 文件", path: ".trae/mcp.json", exists: false, scope: "project" },
    ],
    note: "只接管项目级 MCP（.trae/mcp.json）；与 Trae 国际版同时登记时同一个文件只写一次",
    needsConfirm: true,
  },
  {
    key: "opencode",
    family: "opensource",
    name: "OpenCode",
    vendor: "SST",
    kind: "cli",
    official: "https://opencode.ai/docs/",
    // 内置：技能来自 .agents/skills 与 .claude/skills，自己的 skills 目录通常为空
    builtin: "opencode",
    state: "ready",
    cli: "opencode",
    version: "1.19.3",
    config: `${HOME}/.config/opencode`,
    steps: [
      step("安装 OpenCode", "npm install -g opencode-ai", NPM_RISK),
      step("验证", "opencode --version", "只读验证，不会改动任何文件"),
    ],
  },
  {
    key: "pi",
    family: "opensource",
    name: "Pi",
    vendor: "Earendil Works",
    kind: "cli",
    official: "https://pi.dev/",
    builtin: null,
    // 演示里留一个「只有配置残留」的样本
    state: "config_only",
    cli: "pi",
    config: `${HOME}/.pi`,
    steps: [
      step(
        "安装 Pi",
        "npm install -g --ignore-scripts @earendil-works/pi-coding-agent",
        "官方文档要求 Node.js 22.19+"
      ),
    ],
    blocked: "配置目录与技能目录约定未确证，先只展示安装与验证命令",
    needsConfirm: true,
  },
  {
    key: "openwork",
    family: "opensource",
    name: "OpenWork",
    vendor: "Different AI",
    kind: "cli",
    official: "https://github.com/different-ai/openwork",
    builtin: null,
    state: "missing",
    steps: [
      step(
        "确认包名后再安装",
        "npm install -g @different-ai/openwork",
        "包名来自第三方教程，装前先核对仓库"
      ),
    ],
    blocked: "包名与配置目录均未获官方确证，先只给仓库入口",
    needsConfirm: true,
  },
  {
    key: "hermes",
    family: "other",
    name: "Hermes",
    vendor: "内部",
    kind: "cli",
    official: "",
    builtin: "hermes",
    state: "ready",
    cli: "hermes",
    version: "v0.22.2",
    config: `${HOME}/.hermes`,
    steps: [step("确认安装", "hermes --version", "按团队内部分发文档安装")],
    needsConfirm: true,
  },
  {
    key: "gemini-cli",
    family: "other",
    name: "Gemini CLI",
    vendor: "Google",
    kind: "cli",
    official: "https://github.com/google-gemini/gemini-cli",
    builtin: "gemini",
    state: "missing",
    cli: "gemini",
    config: `${HOME}/.gemini`,
    steps: [
      step("安装 Gemini CLI", "npm install -g @google/gemini-cli", NPM_RISK),
      step("验证", "gemini --version", "只读验证，不会改动任何文件"),
    ],
  },
  {
    key: "manus",
    family: "other",
    name: "Manus",
    vendor: "Manus",
    kind: "cloud",
    official: "https://manus.im/",
    builtin: null,
    state: "missing",
    steps: [step("下载 Manus 桌面端", "https://manus.im/", "云端 agent，没有本地配置约定")],
    blocked: "云端 agent，没有本地技能或 MCP 配置文件约定，只能展示安装入口",
    needsConfirm: true,
  },
];

/** 演示模式的目录：状态、受管、版本都按 seed 现算，纳入/移出只改内存集合。 */
function buildCatalog(probe: boolean): CatalogEntry[] {
  return CATALOG_SEED.map((seed) => {
    const managed: ManagedState = seed.builtin
      ? "builtin"
      : seed.shared
        ? "shared"
        : demoManaged.has(seed.key)
          ? "managed"
          : seed.blocked
            ? "blocked"
            : "addable";
    return {
      key: seed.key,
      family: seed.family,
      name: seed.name,
      vendor: seed.vendor,
      kind: seed.kind,
      official: seed.official,
      needs_confirm: Boolean(seed.needsConfirm),
      builtin: seed.builtin,
      state: seed.state,
      cli_name: seed.cli ?? null,
      cli_path: seed.cli ? p(`${BIN_DIR}/${seed.cli}`) : null,
      version: probe ? (seed.version ?? null) : null,
      app_path: seed.app ? p(seed.app) : null,
      config_path: seed.config ? p(seed.config) : null,
      config_exists: Boolean(seed.config),
      evidence: seed.msix
        ? [`Store 包 ${seed.msix} 已安装`]
        : seed.cli
          ? [`PATH 命中 ${p(`${BIN_DIR}/${seed.cli}`)}`]
          : seed.app
            ? [`应用存在 ${p(seed.app)}`]
            : [],
      managed,
      shared_agent: seed.shared ?? null,
      managed_id: seed.builtin ? null : seed.key,
      managed_note: seed.blocked ?? seed.note ?? null,
      managed_targets: seed.targets ?? [],
      steps: seed.steps,
      docs: seed.official,
      platform: PLATFORM_LABEL,
    };
  });
}

// ---- 设置页（演示态：Hermes 用 Windows 安装器的真实布局） ----

const emptyOverride = (): AgentPathOverride => ({
  config_dir: null,
  skills_dir: null,
  mcp_file: null,
  instructions: [],
});

let demoStoreRoot = p(`${HOME}/.agenthub/store`);
let demoBackupsKeep = 10;
/** 演示默认走 SkillHub（与后端内置默认一致） */
let demoMarketSource: MarketSourceKey = "skillhub";
/** 空串 = 跟随所选数据源的官方地址 */
let demoMarketRegistry = "";

const MARKET_SOURCE_LABELS: Record<MarketSourceKey, string> = {
  skillhub: "SkillHub",
  clawhub: "ClawHub",
};

const MARKET_SOURCE_BASES: Record<MarketSourceKey, string> = {
  skillhub: "https://api.skillhub.cn",
  clawhub: "https://clawhub.ai",
};
let demoDefaultStrategy: LinkStrategyName | null = null;
let demoExcluded: string[] = [];
let demoTrash = false;
let demoBackupConfig = true;
let demoAgentPaths: Record<string, AgentPathOverride> = {};
let demoCustomAgents: CustomAgentDef[] = [
  {
    id: "workbuddy",
    display_name: "WorkBuddy",
    skills_dir: "~/.workbuddy/skills",
    project_skills_dir: null,
    mcp_file: "~/.workbuddy/mcp.json",
    mcp_format: { format: "json", pointer: ["mcpServers"] },
    project_mcp_file: null,
    instructions: ["~/.workbuddy/SOUL.md", "~/.workbuddy/USER.md"],
  },
];

// ---- Provider（Phase 3）演示数据 ----

const PROVIDER_AGENTS: {
  agent: string;
  display_name: string;
  supported: boolean;
  reason: string | null;
}[] = [
  { agent: "claude", display_name: "Claude Code", supported: true, reason: null },
  { agent: "codex", display_name: "Codex", supported: true, reason: null },
  {
    agent: "opencode",
    display_name: "OpenCode",
    supported: true,
    reason: "只写 base_url 与密钥；模型选择尚未接入该格式",
  },
  {
    agent: "codebuddy",
    display_name: "CodeBuddy",
    supported: false,
    reason: "模型配置是 models[] 数组（每条自带 url / apiKey），没有单一的「当前 provider」概念",
  },
  {
    agent: "hermes",
    display_name: "Hermes",
    supported: false,
    reason: "config.yaml 需要块级 YAML 改写，且 base_url / api、model / default_model 存在别名，待核实",
  },
  {
    agent: "gemini",
    display_name: "Gemini CLI",
    supported: false,
    reason: "模型配置键名尚未核实（本机无 ~/.gemini，仓库里也没有键名证据）",
  },
];

/** 网关全局配置（演示）。 */
let demoGatewayConfig: GatewayConfigView = {
  auto_start: false,
  host: "127.0.0.1",
  port: 43117,
  auth_token: null,
  log_requests: false,
  max_log_entries: 500,
};
let demoGatewayRunning = false;

let demoRoutes: GatewayRouteView[] = [
  {
    id: "claude-main",
    name: "Claude 主链路",
    protocol: "anthropic",
    mode: "aggregate",
    members: [
      { provider_id: "relay", weight: 2, priority: 1, models: [], model_map: {}, enabled: true },
      {
        provider_id: "official",
        weight: 1,
        priority: 2,
        models: ["claude-sonnet-4-5", "claude-opus-4-1"],
        model_map: {},
        enabled: true,
      },
    ],
    rules: [{ pattern: "claude-opus*", provider_ids: ["official"] }],
    retry: { max_attempts: 3, first_byte_ms: 120_000, idle_ms: 180_000, retry_status: [429, 500, 502, 503, 504] },
    enabled: true,
  },
  {
    id: "codex-fast",
    name: "Codex 快速通道",
    protocol: "chat_completions",
    mode: "route",
    members: [
      { provider_id: "relay", weight: 1, priority: 1, models: ["gpt-6.1", "gpt-6.1-mini"], model_map: {}, enabled: true },
    ],
    rules: [{ pattern: "gpt-6.1-mini*", provider_ids: ["relay"] }],
    retry: { max_attempts: 2, first_byte_ms: 60_000, idle_ms: 120_000, retry_status: [429, 503] },
    enabled: true,
  },
];

let demoGatewayLogs: GatewayRequestLog[] = [
  {
    at: now() - 12,
    route_id: "claude-main",
    provider_id: "relay",
    provider_name: "中转站（示例）",
    model: "claude-sonnet-4-5",
    protocol: "Anthropic",
    agent: "claude",
    status: 200,
    attempts: 1,
    first_byte_ms: 620,
    total_ms: 4210,
    bytes: 18_240,
    error: null,
  },
  {
    at: now() - 95,
    route_id: "claude-main",
    provider_id: "official",
    provider_name: "Anthropic 官方",
    model: "claude-opus-4-1",
    protocol: "Anthropic",
    agent: "claude",
    status: 200,
    attempts: 2,
    first_byte_ms: 1_180,
    total_ms: 9_620,
    bytes: 42_880,
    error: null,
  },
  {
    at: now() - 240,
    route_id: "codex-fast",
    provider_id: null,
    provider_name: null,
    model: "gpt-6.1",
    protocol: "OpenAI",
    agent: "codex",
    status: 503,
    attempts: 2,
    first_byte_ms: 0,
    total_ms: 1_540,
    bytes: 0,
    error: "中转站（示例）返回 503 Service Unavailable，切下一个上游",
  },
];

let demoProviders: ProviderView[] = [
  {
    id: "official",
    name: "Anthropic 官方",
    base_url: "https://api.anthropic.com",
    api_key: "env:ANTHROPIC_API_KEY",
    models: ["claude-sonnet-4-5", "claude-opus-4-1"],
    note: "直连官方，按量计费",
    protocol: "anthropic",
    model_limits: {
      "claude-sonnet-4-5": { context: 200_000, max_output: 64_000, source: "auto" },
      "claude-opus-4-1": { context: 200_000, max_output: 32_000, source: "auto" },
    },
    catalog_id: "anthropic",
    limits_summary: "200K 上下文 · 64K 输出",
  },
  {
    id: "relay",
    name: "中转站（示例）",
    base_url: "https://relay.example.com/v1",
    api_key: "keychain:agenthub/relay",
    models: ["claude-sonnet-4-5", "gpt-6.1"],
    note: "团队共享额度",
    // 上游只会 Chat Completions：网关据此对 Claude Code 做协议转换
    protocol: "chat_completions",
    model_limits: {
      "gpt-6.1": { context: 400_000, max_output: 128_000, source: "manual" },
    },
    catalog_id: null,
    limits_summary: "400K 上下文 · 128K 输出",
  },
];

/** 演示用的内置渠道（真实目录由后端提供，这里只取常用的几家）。 */
const DEMO_SUPPLIERS: SupplierDef[] = [
  {
    id: "anthropic",
    name: "Anthropic",
    vendor: "Anthropic",
    category: "official",
    website: "https://www.anthropic.com",
    base_url: "https://api.anthropic.com",
    protocol: "anthropic",
    default_model: "claude-sonnet-4-5",
    models: ["claude-sonnet-4-5", "claude-opus-4-1"],
    source_url: "https://docs.anthropic.com/en/api/models-list",
    verified: true,
  },
  {
    id: "openai",
    name: "OpenAI",
    vendor: "OpenAI",
    category: "official",
    website: "https://platform.openai.com",
    base_url: "https://api.openai.com/v1",
    protocol: "chat_completions",
    default_model: "gpt-6.1",
    models: ["gpt-6.1"],
    source_url: "https://platform.openai.com/docs/api-reference/models",
    verified: true,
  },
  {
    id: "deepseek",
    name: "DeepSeek",
    vendor: "DeepSeek",
    category: "model_vendor",
    website: "https://platform.deepseek.com",
    base_url: "https://api.deepseek.com/v1",
    protocol: "chat_completions",
    default_model: "deepseek-chat",
    models: ["deepseek-chat", "deepseek-reasoner"],
    source_url: "https://api-docs.deepseek.com/",
    verified: true,
  },
  {
    id: "kimi",
    name: "Kimi（Moonshot）",
    vendor: "Moonshot",
    category: "model_vendor",
    website: "https://platform.moonshot.cn",
    base_url: "https://api.moonshot.cn/v1",
    protocol: "chat_completions",
    default_model: "kimi-k2-turbo-preview",
    models: ["kimi-k2-turbo-preview"],
    source_url: "https://platform.moonshot.cn/docs/api/chat",
    verified: true,
  },
  {
    id: "zhipu",
    name: "智谱 GLM",
    vendor: "Zhipu",
    category: "model_vendor",
    website: "https://open.bigmodel.cn",
    base_url: "https://open.bigmodel.cn/api/paas/v4",
    protocol: "chat_completions",
    default_model: "glm-5",
    models: ["glm-5", "glm-5-flash"],
    source_url: "https://docs.bigmodel.cn/cn/guide/develop/http/introduction",
    verified: true,
  },
  {
    id: "qwen",
    name: "通义千问 / 阿里百炼",
    vendor: "Alibaba",
    category: "model_vendor",
    website: "https://bailian.console.aliyun.com",
    base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    protocol: "chat_completions",
    default_model: "qwen3-max",
    models: ["qwen3-max", "qwen3-coder-plus"],
    source_url:
      "https://help.aliyun.com/zh/model-studio/compatibility-of-openai-with-dashscope",
    verified: true,
  },
  {
    id: "openrouter",
    name: "OpenRouter",
    vendor: "OpenRouter",
    category: "aggregator",
    website: "https://openrouter.ai",
    base_url: "https://openrouter.ai/api/v1",
    protocol: "chat_completions",
    default_model: "anthropic/claude-sonnet-4.5",
    models: [],
    source_url: "https://openrouter.ai/docs/quickstart",
    verified: true,
  },
  {
    id: "longcat",
    name: "LongCat",
    vendor: "Meituan",
    category: "model_vendor",
    website: "https://longcat.chat",
    base_url: "https://api.longcat.chat/openai/v1",
    protocol: "chat_completions",
    default_model: "LongCat-Flash-Chat",
    models: [],
    source_url: "https://longcat.chat/platform/docs",
    verified: false,
  },
];

/** key = agent slug */
let demoBindings: Record<
  string,
  {
    provider_id: string;
    model: string;
    make_default: boolean;
    mode: PoolMode;
    /** 该 agent 订阅的供应商（顺序即界面顺序） */
    pool: string[];
    model_slots: ModelSlots;
    /** 网关路由 id（经网关时才有）；与后端 `ProviderBindingView.route_id` 同义 */
    route_id?: string;
  }
> = {
  claude: {
    provider_id: "relay",
    model: "claude-sonnet-4-5",
    make_default: true,
    mode: "aggregate",
    pool: ["relay", "official"],
    model_slots: { sonnet: "claude-sonnet-4-5", opus: "gpt-6.1" },
    route_id: "claude-main",
  },
  // Codex 只能写 env_key：官方（env:ANTHROPIC_API_KEY）+ 中转站（keychain:）正好覆盖
  // 「变量名可导出」与「keychain 形式 Codex 读不到」两种提示
  codex: {
    provider_id: "official",
    model: "claude-sonnet-4-5",
    make_default: false,
    mode: "route",
    pool: ["official", "relay"],
    model_slots: {},
    route_id: "codex-fast",
  },
};

/** 与后端 `ext::provider::format_tokens` 保持同一套格式化规则。 */
function formatTokens(tokens: number): string {
  if (tokens >= 1_000_000) {
    const value = tokens / 1_000_000;
    return Math.abs(value - Math.round(value)) < 0.05
      ? `${Math.round(value)}M`
      : `${value.toFixed(1)}M`;
  }
  if (tokens >= 1_000) {
    const value = tokens / 1_000;
    return Math.abs(value - Math.round(value)) < 0.05
      ? `${Math.round(value)}K`
      : `${value.toFixed(1)}K`;
  }
  return String(tokens);
}

function formatLimits(limits: ModelLimits): string | null {
  const parts: string[] = [];
  if (limits.context) parts.push(`${formatTokens(limits.context)} 上下文`);
  if (limits.max_output) parts.push(`${formatTokens(limits.max_output)} 输出`);
  return parts.length ? parts.join(" · ") : null;
}

function gatewayBaseUrl(routeId: string, port = demoGatewayConfig.port): string {
  return `http://127.0.0.1:${port}/gw/${routeId}`;
}

/** 统一入口地址：所有 agent 共用，靠令牌区分池。 */
function unifiedBaseUrl(port = demoGatewayConfig.port): string {
  return `http://127.0.0.1:${port}/v1`;
}

function buildProvidersView(): ProvidersView {
  return {
    providers: demoProviders.map((item) => ({ ...item, models: [...item.models] })),
    bindings: PROVIDER_AGENTS.map((agent) => {
      const bound = demoBindings[agent.agent];
      const mode = bound?.mode ?? "direct";
      const viaGateway = mode !== "direct";
      return {
        agent: agent.agent,
        display_name: agent.display_name,
        supported: agent.supported,
        unsupported_reason: agent.supported ? null : agent.reason,
        provider_id: bound?.provider_id ?? null,
        model: bound?.model ?? null,
        make_default: bound?.make_default ?? false,
        via_gateway: viaGateway,
        // 真实后端给的是网关路由 id（`api/provider.rs` 里按它去 routes 里找），
        // 以前这里是 agent slug，池卡片因此永远查不到成员、拿不到权重/优先级
        route_id: viaGateway ? (bound?.route_id ?? null) : null,
        mode,
        gateway_base_url: viaGateway ? unifiedBaseUrl() : null,
        gateway_token: viaGateway ? agent.agent : null,
        model_slots: { ...(bound?.model_slots ?? {}) },
        pool: [...(bound?.pool ?? [])],
      };
    }),
  };
}

function buildGatewayView(): GatewayView {
  const notes: string[] = [];
  for (const route of demoRoutes) {
    for (const member of route.members) {
      if (!demoProviders.some((provider) => provider.id === member.provider_id)) {
        notes.push(`路由「${route.name}」引用了不存在的 provider ${member.provider_id}`);
      }
    }
  }
  return {
    config: { ...demoGatewayConfig },
    routes: demoRoutes.map((route) => ({
      ...route,
      members: route.members.map((member) => ({ ...member, models: [...member.models] })),
      rules: route.rules.map((rule) => ({ ...rule, provider_ids: [...rule.provider_ids] })),
    })),
    notes,
    base_url_template: `http://127.0.0.1:${demoGatewayConfig.port}/gw/{route}`,
  };
}

/** provider 写入的目标文件（与后端一样，先看设置页有没有目录覆盖）。 */
function providerTarget(agent: string): string {
  const override = demoAgentPaths[agent]?.config_dir;
  if (override) {
    return `${override}/${agent === "claude" ? "settings.json" : "config.toml"}`;
  }
  return agent === "claude" ? p(`${HOME}/.claude/settings.json`) : p(`${HOME}/.codex/config.toml`);
}

interface DemoAgentSeed {
  id: string;
  name: string;
  config: string;
  skills: string;
  mcp: string;
  instructions: string[];
  /** 配置目录 / 技能目录 / MCP 文件 / 指令文件 是否存在 */
  present: [boolean, boolean, boolean, boolean];
  /** 主技能目录之外的兼容目录（OpenCode 用） */
  extra?: string[];
}

const SETTINGS_AGENTS: DemoAgentSeed[] = [
  {
    id: "claude",
    name: "Claude Code",
    config: `${HOME}/.claude`,
    skills: `${HOME}/.claude/skills`,
    mcp: `${HOME}/.claude.json`,
    instructions: [`${HOME}/.claude/CLAUDE.md`],
    present: [true, true, true, true],
  },
  {
    id: "codex",
    name: "Codex",
    config: `${HOME}/.codex`,
    skills: `${HOME}/.codex/skills`,
    mcp: `${HOME}/.codex/config.toml`,
    instructions: [`${HOME}/.codex/AGENTS.md`],
    present: [true, true, true, true],
  },
  {
    id: "codebuddy",
    name: "CodeBuddy",
    config: `${HOME}/.codebuddy`,
    skills: `${HOME}/.codebuddy/skills`,
    mcp: `${HOME}/.codebuddy/mcp.json`,
    instructions: [`${HOME}/.codebuddy/AGENTS.md`],
    present: [true, true, true, false],
  },
  {
    // Windows 安装器把数据放 `%LOCALAPPDATA%\hermes`，macOS 就在 `~/.hermes`（均已实测）
    id: "hermes",
    name: "Hermes",
    config: HERMES_ROOT,
    skills: `${HERMES_ROOT}/skills`,
    mcp: `${HERMES_ROOT}/config.yaml`,
    instructions: [`${HERMES_ROOT}/SOUL.md`, `${HERMES_ROOT}/USER.md`],
    present: [true, true, true, true],
  },
  {
    id: "gemini",
    name: "Gemini CLI",
    config: `${HOME}/.gemini`,
    skills: `${HOME}/.gemini/skills`,
    mcp: `${HOME}/.gemini/settings.json`,
    instructions: [`${HOME}/.gemini/GEMINI.md`],
    present: [false, false, false, false],
  },
  {
    // 演示重点：自己的 skills 目录不存在，技能来自两个兼容目录
    id: "opencode",
    name: "OpenCode",
    config: `${HOME}/.config/opencode`,
    skills: `${HOME}/.config/opencode/skills`,
    mcp: "",
    instructions: [`${HOME}/.config/opencode/AGENTS.md`],
    present: [true, false, false, true],
    extra: [`${HOME}/.agents/skills`, `${HOME}/.claude/skills`],
  },
];

function buildSettings(): SettingsView {
  const agents: AgentPathsView[] = SETTINGS_AGENTS.map((seed) => {
    const override = demoAgentPaths[seed.id] ?? emptyOverride();
    return {
      id: seed.id,
      display_name: seed.name,
      builtin: true,
      overrides: override,
      effective: {
        config_dir: override.config_dir ?? p(seed.config),
        skills_dir: override.skills_dir ?? p(seed.skills),
        extra_skills_dirs: (seed.extra ?? []).map(p),
        mcp_file: override.mcp_file ?? (seed.mcp ? p(seed.mcp) : null),
        instructions: override.instructions.length
          ? override.instructions
          : seed.instructions.map(p),
      },
      presence: {
        config_dir: override.config_dir ? true : seed.present[0],
        skills_dir: override.skills_dir ? true : seed.present[1],
        extra_skills_dirs: (seed.extra ?? []).map(() => true),
        mcp_file: override.mcp_file ? true : seed.present[2],
        instructions: (override.instructions.length ? override.instructions : seed.instructions).map(
          () => seed.present[3]
        ),
      },
    };
  });

  return {
    data_root: p(`${HOME}/.agenthub`),
    store_root: demoStoreRoot,
    store_usage: { skills: 11, bytes: 6_900_000 },
    default_strategy: demoDefaultStrategy,
    platform_strategy: LINK_STRATEGY,
    backups_keep: demoBackupsKeep,
    excluded_paths: [...demoExcluded],
    use_trash_for_dedupe: demoTrash,
    backup_config_files: demoBackupConfig,
    agents,
    custom_agents: demoCustomAgents.map((def) => ({ ...def })),
    providers: buildProvidersView(),
    market_source: demoMarketSource,
    market_source_label: MARKET_SOURCE_LABELS[demoMarketSource],
    market_registry: demoMarketRegistry || null,
    market_default_registry: MARKET_SOURCE_BASES[demoMarketSource],
    features: { ...demoFeatures },
    feature_catalog: buildFeatureCatalog(),
    locale: demoLocaleMode,
    locale_effective: "zh-Hans",
  };
}

function buildFeatureCatalog(): FeatureMeta[] {
  return [
    {
      key: "provider",
      display_name: "供应商",
      description: "上游登记、本地网关路由与按 agent 的绑定写入",
      enabled: demoFeatures.provider,
    },
    {
      key: "usage",
      display_name: "Token 统计",
      description: "只读解析本机会话日志，统计 token 消耗与估算花费",
      enabled: demoFeatures.usage,
    },
  ];
}

/** 记录若是一条链接，返回它指向的路径。 */
function linkTargetOf(record: SkillRecord): string | null {
  const { link } = record;
  if (link.kind === "symlink" || link.kind === "junction" || link.kind === "hardlink") {
    return link.target;
  }
  return null;
}

/** 演示用的市场目录：内容照着真实接口的字段来，界面能验到排序 / 安全标记 / 详情。 */
const MARKET_SEED: MarketSkill[] = [
  {
    slug: "cloudflare-workers",
    display_name: "cloudflare-workers",
    summary:
      "Cloudflare Workers / Pages / D1 / R2 的全流程参考：路由与绑定、限流、队列、可观测性与本地调试。",
    owner: "cloudflare",
    version: "1.4.2",
    downloads: 12_480,
    installs: 326,
    stars: 41,
    updated_at: now() - HOUR * 40,
    topics: ["Cloudflare", "Workers", "Serverless"],
    suspicious: false,
    malware_blocked: false,
    verdict: "clean",
    official: true,
  },
  {
    slug: "agents-sdk",
    display_name: "agents-sdk",
    summary:
      "在 Workers 上构建有状态 Agent：Agent 类、持久化状态、RPC、WebSocket 与工作流编排。",
    owner: "cloudflare",
    version: "0.9.1",
    downloads: 8_204,
    installs: 211,
    stars: 33,
    updated_at: now() - HOUR * 62,
    topics: ["Agents", "Durable Objects"],
    suspicious: false,
    malware_blocked: false,
    verdict: "clean",
    official: true,
  },
  {
    slug: "sql-review",
    display_name: "sql-review",
    summary:
      "审查 SQL 变更：索引覆盖、隐式类型转换、锁等待与回滚代价，输出可执行的改写建议。",
    owner: "cargo-ai",
    version: "1.5.0",
    downloads: 2_193,
    installs: 19,
    stars: 4,
    updated_at: now() - HOUR * 26,
    topics: ["SQL", "Review"],
    suspicious: true,
    malware_blocked: false,
    verdict: "suspicious",
    official: false,
  },
  {
    slug: "playwright-check",
    display_name: "playwright-check",
    summary:
      "用 Playwright 跑端到端回归：定位器策略、等待条件、截图基线与失败重试。",
    owner: "tester",
    version: "2.0.3",
    downloads: 5_772,
    installs: 98,
    stars: 12,
    updated_at: now() - HOUR * 8,
    topics: ["Testing", "Browser"],
    suspicious: false,
    malware_blocked: false,
    verdict: "clean",
    official: false,
  },
];

const MARKET_DETAIL_TEXT: Record<string, string> = {
  "cloudflare-workers": `---
name: cloudflare-workers
description: Cloudflare Workers 开发参考
---

# Cloudflare Workers

## 何时使用
需要写或改 Workers 脚本、Pages Functions、D1 查询、R2 绑定、队列消费者时。

## 快速开始
1. 确认 wrangler.toml 里的 bindings 与代码里读到的名字一致；
2. 本地用 wrangler dev 跑一遍，注意 --remote 才会连真实 D1；
3. 部署前跑 wrangler deploy --dry-run，检查体积与兼容日期。

## 关键约束
- 单请求 CPU 时间有上限，重活放 Queues 或 Durable Object；
- 不要在请求路径里做同步的 KV 全量列举；
- 环境变量用 wrangler secret put，不要写进代码。`,
  "agents-sdk": `---
name: agents-sdk
description: 在 Workers 上构建有状态 Agent
---

# Agents SDK

## 何时使用
需要长连接、可恢复的会话状态、定时任务或 Agent 间 RPC 时。

## 结构
- 一个 Agent = 一个 Durable Object 实例；
- 状态放 this.state，不要放模块级变量；
- WebSocket 升级走 Agent 的 onConnect。`,
  "sql-review": `---
name: sql-review
description: SQL 变更审查清单
---

# SQL 变更审查

逐条检查：索引覆盖、隐式类型转换、锁范围、是否可在线回滚。

> 注意：本技能在示例里把令牌拼进命令，评审建议只读不执行。`,
  "playwright-check": `---
name: playwright-check
description: Playwright 端到端回归
---

# Playwright 回归

1. 先跑冒烟用例，再跑全量；
2. 失败重试 1 次，仍失败则截图存档；
3. 断言基于用户可见文本，不用 CSS 路径。`,
};

let demoApplied = false;

export const demo = {
  isApplied: () => demoApplied,
  locale: demoLocale,
  setLocale: demoSetLocale,
  overview(): Overview {
    return buildOverview(demoApplied);
  },
  instructions(): InstructionOverview {
    return { specs: instructionSpecs(), chain: instructionChain() };
  },
  projects(): ProjectSummary[] {
    return projects();
  },
  /** 演示模式不真的搬文件，指定基线时只在 note 里体现，便于界面自测。 */
  planDedupe(baseline?: string | null): ChangeSet {
    const cs = dedupePlan(demoApplied);
    if (baseline) {
      cs.notes = [...cs.notes, `演示模式：已按指定基线 ${tailPath(baseline, 2)} 生成计划`];
    }
    return cs;
  },
  planRepair(): ChangeSet {
    return {
      id: `cs-repair-${Math.floor(Date.now() / 1000)}`,
      kind: "repair",
      title: "修复链接",
      ops: [
        {
          id: 0,
          label: "删除",
          action: { op: "remove_path", at: p(`${HOME}/.codebuddy/skills/cloudflare`), use_trash: false },
        },
        {
          id: 1,
          label: `建立${LINK_LABEL}`,
          action: {
            op: "create_link",
            at: p(`${HOME}/.codebuddy/skills/cloudflare`),
            target: `${STORE}/cloudflare`,
            strategy: LINK_STRATEGY,
          },
        },
      ],
      notes: ["损坏的链接会被重新指向中央仓库"],
      risks: [],
    };
  },
  apply(): ApplyResult {
    demoApplied = true;
    return {
      batch_id: "cs-demo-applied",
      applied: 5,
      skipped: 0,
      backup_dir: p(`${HOME}/.agenthub/backups/20261027-153024-4821-118-001`),
      log: ["移入仓库", "建立链接 ×4"],
    };
  },
  rollback(batch: string): string[] {
    demoApplied = false;
    return [`已回滚批次 ${batch}（演示模式仅切换本地演示状态）`];
  },
  /** 项目作用域下把目标文件换到项目根内；Hermes 明确标注「不支持项目级」。 */
  mcpView(project: string | null): McpServerView[] {
    const views = mcpViews();
    if (!project) return views;
    return views.map((view) => ({
      ...view,
      targets: Object.entries(PROJECT_MCP).map(([agent, rel]) => ({
        agent,
        file: rel ? p(`${project}/${rel}`) : "",
        format: rel?.endsWith(".toml") ? "TOML" : "JSON",
        present: false,
        writable: Boolean(rel),
        reason: rel ? null : "Hermes 没有项目级 MCP 文件",
      })),
    }));
  },
  planMcpWrite(
    project: string | null,
    servers: McpServerView[],
    agents: string[]
  ): ChangeSet {
    const scopeLabel = project ? `项目 ${basename(project)}` : "全局";
    const ops: ChangeSet["ops"] = [];
    for (const server of servers) {
      for (const target of server.targets) {
        if (!agents.includes(target.agent) || !target.writable) continue;
        ops.push({
          id: ops.length,
          label: `写入 ${basename(target.file)}`,
          action: {
            op: "write_file",
            at: target.file,
            content: `{\n  "//": "${server.name} · ${scopeLabel} · 演示内容"\n}\n`,
            backup: true,
          },
        });
      }
    }
    return {
      id: `cs-mcp-${Math.floor(Date.now() / 1000)}`,
      kind: "mcp",
      title: `写入 ${servers.length} 个 MCP 定义（${scopeLabel}）`,
      ops,
      notes: [
        `作用域：${scopeLabel}`,
        "写入前会先把原文件复制进 ~/.agenthub/backups，可一键回滚",
      ],
      risks: [],
    };
  },
  setSkillStrategy(
    name: string,
    strategy: LinkStrategyName | null
  ): StrategyConfig {
    if (strategy) {
      demoOverrides = { ...demoOverrides, [name]: strategy };
    } else {
      const next = { ...demoOverrides };
      delete next[name];
      demoOverrides = next;
    }
    return strategyConfig();
  },
  /** 演示模式的名单：只存内存，返回归一化（小写、去重、排序）后的完整名单。 */
  setAgentLocalSkill(name: string, on: boolean): string[] {
    const key = name.trim().toLowerCase();
    if (key) {
      if (on) demoAgentLocal.add(key);
      else demoAgentLocal.delete(key);
    }
    return [...demoAgentLocal].sort();
  },
  agentCatalog(probe: boolean): CatalogEntry[] {
    return buildCatalog(probe);
  },
  settings(): SettingsView {
    return buildSettings();
  },
  saveSettings(input: SettingsInput): SettingsView {
    // 与后端一致的最小校验，保证演示模式也能验出错误分支
    if (!input.store_root.trim()) throw new Error("仓库目录不能为空");
    if (input.backups_keep < 1 || input.backups_keep > 100) {
      throw new Error("备份保留份数必须在 1..=100 之间");
    }
    demoStoreRoot = input.store_root;
    demoDefaultStrategy = input.default_strategy;
    demoBackupsKeep = input.backups_keep;
    demoExcluded = [...input.excluded_paths];
    demoTrash = input.use_trash_for_dedupe;
    demoBackupConfig = input.backup_config_files;
    demoAgentPaths = { ...input.agent_paths };
    demoCustomAgents = input.custom_agents.map((def) => ({ ...def }));
    if (input.market_source !== "skillhub" && input.market_source !== "clawhub") {
      throw new Error(`未知的技能市场数据源：${String(input.market_source)}`);
    }
    demoMarketSource = input.market_source;
    const registry = input.market_registry.trim();
    if (registry && !/^https:\/\//.test(registry)) {
      throw new Error(`市场源地址必须是 https:// 开头：${registry}`);
    }
    demoMarketRegistry = registry;
    return buildSettings();
  },
  addManagedAgent(key: string): CatalogEntry[] {
    const seed = CATALOG_SEED.find((item) => item.key === key);
    if (!seed) throw new Error(`目录里没有 ${key}`);
    if (seed.builtin) throw new Error(`${seed.name} 是内置平台，已受管`);
    if (seed.shared) throw new Error(`${seed.name} 的配置与内置 ${seed.shared} 共用，已由它接管`);
    if (seed.blocked) throw new Error(seed.blocked);
    demoManaged.add(key);
    return buildCatalog(false);
  },
  removeManagedAgent(key: string): CatalogEntry[] {
    const seed = CATALOG_SEED.find((item) => item.key === key);
    if (!seed) throw new Error(`目录里没有 ${key}`);
    if (seed.builtin) throw new Error(`${seed.name} 是内置平台，不能移出受管`);
    demoManaged.delete(key);
    return buildCatalog(false);
  },
  providers(): ProvidersView {
    return buildProvidersView();
  },
  saveProvider(input: ProviderInput): ProvidersView {
    // 与后端 build_provider 的校验保持一致，演示模式也能试出错误分支
    const id = input.id.trim().toLowerCase();
    if (!id) throw new Error("Provider id 不能为空");
    if (!/^[a-z0-9_-]+$/.test(id)) {
      throw new Error(`Provider id ${id} 只能用小写字母、数字、-、_`);
    }
    const name = input.name.trim();
    if (!name) throw new Error("Provider 名称不能为空");
    const baseUrl = input.base_url.trim();
    if (!baseUrl) throw new Error("base_url 不能为空");
    if (!/^https?:\/\//.test(baseUrl)) throw new Error("base_url 必须以 http:// 或 https:// 开头");
    const rawKey = input.api_key.trim();
    if (!rawKey) throw new Error("密钥不能为空：请填 env:VAR、keychain:service/account 或明文");
    const isReference = rawKey.startsWith("env:") || rawKey.startsWith("keychain:");
    if (!isReference && !input.acknowledged) {
      throw new Error("明文密钥需要先勾选确认；更推荐 env:VAR 或 keychain: 引用");
    }
    const masked = isReference ? rawKey : "literal:********";
    const models = [...new Set(input.models.map((m) => m.trim()).filter(Boolean))];
    const note = input.note?.trim() ? input.note.trim() : null;
    // 只保留仍存在的模型的长度，和后端 build_provider 一致
    const limits: Record<string, ModelLimits> = {};
    for (const [model, value] of Object.entries(input.model_limits ?? {})) {
      if (models.includes(model)) limits[model] = value;
    }
    const summary = Object.values(limits)
      .map((value) => formatLimits(value))
      .find((text): text is string => Boolean(text));

    const next: ProviderView = {
      id,
      name,
      base_url: baseUrl,
      api_key: masked,
      models,
      note,
      protocol: input.protocol ?? null,
      model_limits: limits,
      catalog_id: input.catalog_id ?? null,
      limits_summary: summary ?? null,
    };
    demoProviders = demoProviders.some((item) => item.id === id)
      ? demoProviders.map((item) => (item.id === id ? next : item)).sort((a, b) => a.id.localeCompare(b.id))
      : [...demoProviders, next].sort((a, b) => a.id.localeCompare(b.id));

    // 模型列表变了，已绑定的模型要收敛，和后端一致
    if (models.length) {
      for (const binding of Object.values(demoBindings)) {
        if (binding.provider_id === id && !models.includes(binding.model)) {
          binding.model = models[0];
        }
      }
    }
    return buildProvidersView();
  },
  deleteProvider(id: string): ProvidersView {
    demoProviders = demoProviders.filter((item) => item.id !== id);
    for (const [agent, binding] of Object.entries(demoBindings)) {
      if (binding.provider_id === id) delete demoBindings[agent];
    }
    return buildProvidersView();
  },
  bindProvider(input: BindProviderInput): ProvidersView {
    const meta = PROVIDER_AGENTS.find((item) => item.agent === input.agent);
    if (!meta) throw new Error(`未知 agent：${input.agent}`);
    if (!meta.supported) throw new Error(`${meta.display_name} 的 API 配置格式尚未接入`);
    const provider = demoProviders.find((item) => item.id === input.provider_id);
    if (!provider) throw new Error(`provider ${input.provider_id} 不存在`);
    const model = input.model?.trim() || provider.models[0];
    if (!model) throw new Error(`provider「${provider.name}」没有登记模型，绑定时必须指定一个`);
    if (provider.models.length && !provider.models.includes(model)) {
      throw new Error(`模型 ${model} 不在 provider「${provider.name}」的列表里`);
    }
    const previous = demoBindings[input.agent];
    const mode = input.mode ?? (input.via_gateway ? "route" : "direct");
    demoBindings[input.agent] = {
      provider_id: provider.id,
      model,
      make_default: input.make_default,
      mode,
      pool: previous?.pool.includes(provider.id)
        ? previous.pool
        : [...(previous?.pool ?? []), provider.id],
      model_slots: { ...(input.model_slots ?? previous?.model_slots ?? {}) },
    };
    return buildProvidersView();
  },
  unbindProvider(agent: string): ProvidersView {
    delete demoBindings[agent];
    return buildProvidersView();
  },
  planProviderApply(agent: string): ChangeSet {
    const meta = PROVIDER_AGENTS.find((item) => item.agent === agent);
    if (!meta) throw new Error(`未知 agent：${agent}`);
    const binding = demoBindings[agent];
    if (!binding) throw new Error(`${meta.display_name} 还没有绑定 provider`);
    const provider = demoProviders.find((item) => item.id === binding.provider_id);
    if (!provider) throw new Error(`provider ${binding.provider_id} 不存在`);
    const target = providerTarget(agent);
    const viaGateway = binding.mode !== "direct";
    const baseUrl = viaGateway ? unifiedBaseUrl() : provider.base_url;
    const key = viaGateway
      ? (demoGatewayConfig.auth_token ?? "agenthub-local")
      : provider.api_key;
    const content =
      agent === "claude"
        ? `{\n  "env": {\n    "ANTHROPIC_BASE_URL": "${baseUrl}",\n    "ANTHROPIC_MODEL": "${binding.model}",\n    "ANTHROPIC_AUTH_TOKEN": "${key}"\n  }\n}\n`
        : `model = "${binding.model}"\nmodel_provider = "${provider.id}"\n\n[model_providers.${provider.id}]\nname = "${provider.name}"\nbase_url = "${baseUrl}"\n`;
    const risks: Risk[] = [];
    if (!viaGateway && provider.api_key.startsWith("literal:")) {
      risks.push({ level: "warn", message: "该 provider 用的是明文密钥：确认后会把密钥写进配置文件" });
    }
    if (binding.make_default) {
      risks.push({ level: "warn", message: `会把「${binding.model}」设为 ${meta.display_name} 的默认模型` });
    }
    return {
      id: `cs-provider-${Math.floor(Date.now() / 1000)}`,
      kind: "provider",
      title: `写入 Provider「${provider.name}」到 ${meta.display_name}`,
      ops: [
        {
          id: 0,
          label: "写入配置文件",
          action: { op: "write_file", at: target, content, backup: true },
        },
      ],
      notes: [
        `${target}（演示内容）`,
        agent === "claude"
          ? "只改 env.ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN / ANTHROPIC_MODEL，其余键保留"
          : `只改 model / model_provider 与 [model_providers.${provider.id}]，其它 provider 与注释保留`,
      ],
      risks,
    };
  },

  // ---- 功能管理（设置 › 功能管理） ----
  features(): FeatureFlags {
    return { ...demoFeatures };
  },
  setFeature(key: FeatureKey, enabled: boolean): FeatureFlags {
    demoFeatures[key] = enabled;
    if (key === "provider" && !enabled) demoGatewayRunning = false;
    return { ...demoFeatures };
  },

  /** 一键应用：演示模式不写盘，只是按预设给出成功 / 401 / 连不上三种结果。 */
  applyProviderBinding(agent: string): ApplyReport {
    const binding = demoBindings[agent];
    if (!binding) {
      throw new Error(`${PROVIDER_AGENTS.find((a) => a.agent === agent)?.display_name ?? agent} 还没有选择供应商`);
    }
    const provider = demoProviders.find((item) => item.id === binding.provider_id);
    if (!provider) throw new Error(`供应商 ${binding.provider_id} 不存在`);

    const plan = demo.planProviderApply(agent);
    const applied = plan.ops.length > 0;

    const endpoint =
      binding.mode !== "direct"
        ? `${unifiedBaseUrl()}/models`
        : `${provider.base_url.replace(/\/+$/, "")}/models`;

    // 用 base_url 里刻意埋的标记复现两种失败，方便在没有真实上游时看界面
    let probe: ProbeReport;
    if (provider.base_url.includes("invalid")) {
      probe = {
        endpoint,
        ok: false,
        detail: "网络请求失败：不知道这样的主机。 (os error 11001)",
        models: 0,
      };
    } else if (provider.base_url.includes("badkey")) {
      probe = { endpoint, ok: false, detail: "密钥无效或没有权限（HTTP 401/403）", models: 0 };
    } else {
      probe = {
        endpoint,
        ok: true,
        detail: `连接正常 · ${provider.models.length} 个模型`,
        models: provider.models.length,
      };
    }

    return {
      applied,
      skipped: applied ? null : "配置已是最新，无需改动",
      files: plan.ops.map((op) => (op.action.op === "write_file" ? op.action.at : "")).filter(Boolean),
      batch_id: applied ? `batch-demo-${Date.now()}` : null,
      probe,
      notes: plan.notes,
    };
  },

  // ---- 供应商池：加入 / 移除 / 设为默认 / 切换模式 ----
  addToPool(agent: string, providerId: string): ApplyReport {
    const provider = demoProviders.find((item) => item.id === providerId);
    if (!provider) throw new Error(`供应商 ${providerId} 不存在`);
    const current = demoBindings[agent];
    const pool = current?.pool ?? [];
    demoBindings[agent] = {
      provider_id: current?.provider_id || provider.id,
      model: current?.model || provider.models[0] || "",
      make_default: true,
      mode: current?.mode ?? "direct",
      pool: pool.includes(provider.id) ? pool : [...pool, provider.id],
      model_slots: current?.model_slots ?? {},
    };
    return demo.applyProviderBinding(agent);
  },

  removeFromPool(agent: string, providerId: string): ApplyReport {
    const current = demoBindings[agent];
    if (!current) throw new Error(`${agent} 还没有供应商池`);
    const pool = current.pool.filter((id) => id !== providerId);
    if (pool.length === 0) {
      delete demoBindings[agent];
      return {
        applied: true,
        skipped: null,
        files: [],
        batch_id: null,
        probe: null,
        notes: ["池已清空，该 agent 的供应商绑定已移除"],
      };
    }
    const nextDefault = current.provider_id === providerId ? pool[0] : current.provider_id;
    const nextProvider = demoProviders.find((item) => item.id === nextDefault);
    demoBindings[agent] = {
      ...current,
      provider_id: nextDefault,
      model: nextProvider?.models[0] ?? current.model,
      pool,
    };
    return demo.applyProviderBinding(agent);
  },

  setDefaultProvider(agent: string, providerId: string): ApplyReport {
    const current = demoBindings[agent];
    const provider = demoProviders.find((item) => item.id === providerId);
    if (!provider) throw new Error(`供应商 ${providerId} 不存在`);
    demoBindings[agent] = {
      provider_id: provider.id,
      model: provider.models[0] ?? current?.model ?? "",
      make_default: true,
      mode: current?.mode ?? "direct",
      pool: current?.pool.includes(provider.id)
        ? current.pool
        : [...(current?.pool ?? []), provider.id],
      model_slots: current?.model_slots ?? {},
    };
    return demo.applyProviderBinding(agent);
  },

  setPoolMode(agent: string, mode: PoolMode): ApplyReport {
    const current = demoBindings[agent];
    if (!current) throw new Error(`${agent} 还没有选择供应商`);
    demoBindings[agent] = { ...current, mode };
    return demo.applyProviderBinding(agent);
  },

  setModelSlots(agent: string, slots: ModelSlots): ApplyReport {
    const current = demoBindings[agent];
    if (!current) throw new Error(`${agent} 还没有选择供应商`);
    demoBindings[agent] = {
      ...current,
      model_slots: Object.fromEntries(
        Object.entries(slots).filter(([, value]) => value.trim().length > 0)
      ),
    };
    return demo.applyProviderBinding(agent);
  },

  // ---- 内置供应商目录 ----
  suppliers(keyword = ""): SupplierDef[] {
    const needle = keyword.trim().toLowerCase();
    if (!needle) return DEMO_SUPPLIERS.map((item) => ({ ...item, models: [...item.models] }));
    return DEMO_SUPPLIERS.filter((item) =>
      `${item.id} ${item.name} ${item.vendor} ${item.base_url} ${item.website}`
        .toLowerCase()
        .includes(needle)
    ).map((item) => ({ ...item, models: [...item.models] }));
  },
  /** 假探测：有的厂商返回长度，有的不返回 —— 正好覆盖「拿不到就留空」那条路径。 */
  probeProviderModels(input: ProbeInput): ProbeResult {
    const endpoint = `${input.base_url.replace(/\/+$/, "")}/models`;
    if (!input.api_key.trim()) {
      return { models: [], endpoint, error: "API Key 不能为空" };
    }
    if (input.base_url.includes("invalid")) {
      return {
        models: [],
        endpoint,
        error: "地址不对（HTTP 404）：API 请求地址通常要带 /v1 之类的版本段",
      };
    }
    if (input.protocol === "anthropic") {
      return {
        models: [
          { id: "claude-sonnet-4-5", context: 200_000, max_output: 64_000 },
          { id: "claude-opus-4-1", context: 200_000, max_output: 32_000 },
        ],
        endpoint,
        error: null,
      };
    }
    return {
      models: [
        { id: "deepseek-chat", context: 128_000, max_output: 8_192 },
        // 上游没给长度：界面要显示「未填」而不是 0
        { id: "deepseek-reasoner", context: null, max_output: null },
      ],
      endpoint,
      error: null,
    };
  },

  // ---- Provider 网关 ----
  gateway(): GatewayView {
    return buildGatewayView();
  },
  saveGatewayConfig(input: GatewayConfigInput): GatewayView {
    if (input.port < 1024) throw new Error("端口必须大于 1023：1024 以下是系统保留区");
    demoGatewayConfig = {
      ...demoGatewayConfig,
      port: input.port,
      auto_start: input.auto_start,
      log_requests: input.log_requests,
      max_log_entries: Math.min(Math.max(input.max_log_entries, 50), 5000),
      auth_token: input.auth_token.trim() || null,
    };
    return buildGatewayView();
  },
  saveGatewayRoute(route: GatewayRouteView): GatewayView {
    if (!/^[a-z0-9-]+$/.test(route.id)) {
      throw new Error("路由 id 只能用小写字母、数字与连字符");
    }
    if (route.members.length === 0) throw new Error("路由至少要有一个成员");
    const index = demoRoutes.findIndex((item) => item.id === route.id);
    if (index >= 0) demoRoutes[index] = route;
    else demoRoutes.push(route);
    demoRoutes.sort((a, b) => a.id.localeCompare(b.id));
    return buildGatewayView();
  },
  deleteGatewayRoute(id: string): RouteDeleteResult {
    const before = demoRoutes.length;
    demoRoutes = demoRoutes.filter((route) => route.id !== id);
    if (demoRoutes.length === before) throw new Error(`网关路由 \`${id}\` 不存在`);
    const reverted: string[] = [];
    for (const [agent, binding] of Object.entries(demoBindings)) {
      // 池路由的 id 就是 agent slug，所以这里直接比 agent
      if (binding.mode !== "direct" && id === agent) {
        binding.mode = "direct";
        reverted.push(agent);
      }
    }
    return { view: buildGatewayView(), reverted };
  },
  gatewayStatus(): GatewayStatusView {
    return {
      running: demoGatewayRunning,
      host: "127.0.0.1",
      port: demoGatewayConfig.port,
      routes: demoRoutes.filter((route) => route.enabled).length,
      auth: Boolean(demoGatewayConfig.auth_token),
      notes: [],
      last_error: null,
    };
  },
  gatewayStart(): GatewayStatusView {
    if (!demoFeatures.provider) throw new Error("功能未启用：供应商（设置 › 功能管理）");
    demoGatewayRunning = true;
    return demo.gatewayStatus();
  },
  gatewayStop(): GatewayStatusView {
    demoGatewayRunning = false;
    return demo.gatewayStatus();
  },
  gatewayRestart(): GatewayStatusView {
    demoGatewayRunning = true;
    return demo.gatewayStatus();
  },
  gatewayLogs(limit = 200): GatewayRequestLog[] {
    return demoGatewayLogs.slice(Math.max(0, demoGatewayLogs.length - limit)).map((log) => ({ ...log }));
  },
  clearGatewayLogs(): void {
    demoGatewayLogs = [];
  },
  planRevertToDirect(agents: string[]): ChangeSet {
    const ops: ChangeSet["ops"] = [];
    for (const slug of agents) {
      const binding = demoBindings[slug];
      if (!binding || binding.mode === "direct") continue;
      const provider = demoProviders.find((item) => item.id === binding.provider_id);
      if (!provider) continue;
      const target = providerTarget(slug);
      const content =
        slug === "claude"
          ? `{\n  "env": {\n    "ANTHROPIC_BASE_URL": "${provider.base_url}",\n    "ANTHROPIC_MODEL": "${binding.model}",\n    "ANTHROPIC_AUTH_TOKEN": "${provider.api_key}"\n  }\n}\n`
          : `model = "${binding.model}"\nmodel_provider = "${provider.id}"\n\n[model_providers.${provider.id}]\nname = "${provider.name}"\nbase_url = "${provider.base_url}"\n`;
      ops.push({
        id: ops.length,
        label: `把 ${PROVIDER_AGENTS.find((item) => item.agent === slug)?.display_name ?? slug} 退回直连`,
        action: { op: "write_file", at: target, content, backup: true },
      });
    }
    return {
      id: `cs-revert-${Math.floor(Date.now() / 1000)}`,
      kind: "provider",
      title: `回退 ${ops.length} 个 agent 到直连`,
      ops,
      notes: ops.length
        ? ["只改 base_url 与密钥，其余配置键保留", "应用后请到绑定区把「经网关」切回「直连」"]
        : ["没有指向网关的绑定，无需回退"],
      risks: [],
    };
  },

  // ---- 删除 ----
  /** 演示模式按真实语义生成计划：只读跳过、未知路径跳过、删真身连带链接。 */
  planDelete(paths: string[], opts: DeleteOptions): ChangeSet {
    const records = buildRecords(demoApplied);
    const byPath = new Map(records.map((record) => [record.path, record]));
    const notes: string[] = [];
    const links: SkillRecord[] = [];
    const copies: SkillRecord[] = [];
    const bodies: SkillRecord[] = [];
    const seen = new Set<string>();

    for (const path of paths) {
      const record = byPath.get(path);
      if (!record) {
        notes.push(`跳过 ${tailPath(path, 3)}：不在当前扫描结果里（可能已被外部改动）`);
        continue;
      }
      if (seen.has(record.path)) continue;
      seen.add(record.path);
      if (record.readonly) {
        notes.push(`跳过 ${record.name}：只读内置技能，AgentHub 不会删除`);
        continue;
      }
      if (record.source === "store" || record.path.startsWith(`${STORE}/`)) {
        bodies.push(record);
      } else if (linkTargetOf(record)) {
        links.push(record);
      } else {
        copies.push(record);
      }
    }

    let dangling = 0;
    if (opts.with_links) {
      const bodyPaths = new Set(bodies.map((record) => record.path));
      for (const record of records) {
        const target = linkTargetOf(record);
        if (!target || !bodyPaths.has(target) || seen.has(record.path)) continue;
        seen.add(record.path);
        links.push(record);
        dangling += 1;
      }
    }

    const ordered = [...links, ...copies, ...bodies];
    const ops = ordered.map((record, index) => ({
      id: index,
      label: opts.use_trash ? "移入回收站" : "删除",
      action: {
        op: "remove_path" as const,
        at: record.path,
        use_trash: opts.use_trash,
      },
    }));

    if (ops.length) {
      notes.unshift(
        `将删除 ${ops.length} 项：仓库真身 ${bodies.length} / agent 副本 ${copies.length} / 链接 ${links.length}${
          opts.use_trash ? "（进回收站，可用批次回滚）" : "（永久删除，仅能靠批次备份恢复）"
        }`
      );
      if (dangling) notes.push(`其中 ${dangling} 条链接指向被删的仓库真身，已一并清理`);
    } else {
      notes.push("没有可删除的项");
    }

    const risks: Risk[] = [];
    if (!opts.use_trash) {
      risks.push({
        level: "warn",
        message: "永久删除：不进系统回收站，只能通过批次回滚恢复",
      });
    }
    if (!opts.with_links && bodies.length) {
      risks.push({
        level: "warn",
        message: `删除了 ${bodies.length} 个仓库真身但保留了指向它们的链接，这些链接会变成失效链接`,
      });
    }

    return {
      id: `cs-delete-${Math.floor(Date.now() / 1000)}`,
      kind: "delete",
      title: `删除 ${ordered.length} 项技能`,
      ops,
      notes,
      risks,
    };
  },

  // ---- 技能市场 ----
  marketAgents(): MarketAgent[] {
    const rows: [string, string, boolean][] = [
      ["claude", "Claude Code", true],
      ["codex", "Codex", true],
      ["codebuddy", "CodeBuddy", true],
      ["gemini", "Gemini CLI", true],
      ["hermes", "Hermes", true],
      ["opencode", "OpenCode", false],
    ];
    const base =
      typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent)
        ? "C:/Users/you"
        : "/Users/you";
    const dirOf: Record<string, string> = {
      claude: `${base}/.claude/skills`,
      codex: `${base}/.codex/skills`,
      codebuddy: `${base}/.codebuddy/skills`,
      gemini: `${base}/.gemini/skills`,
      hermes: `${base}/.hermes/skills`,
      opencode: `${base}/.opencode/skills`,
    };
    return rows
      .map(([id, display_name, available]) => ({
        id,
        display_name,
        skills_dir: dirOf[id],
        detected: available,
        available,
        reason: available ? null : "本机未检测到该 agent",
      }))
      .sort((a, b) => Number(b.available) - Number(a.available));
  },

  marketSearch(
    query: string,
    sort: MarketSort,
    cursor: string | null = null,
    limit = 50
  ): MarketPage {
    const keyword = query.trim().toLowerCase();
    const hits = MARKET_SEED.filter(
      (skill) =>
        !keyword ||
        skill.slug.toLowerCase().includes(keyword) ||
        skill.display_name.toLowerCase().includes(keyword) ||
        skill.summary.toLowerCase().includes(keyword) ||
        skill.topics.some((topic) => topic.toLowerCase().includes(keyword))
    );
    const sorted = [...hits];
    if (sort === "downloads") sorted.sort((a, b) => b.downloads - a.downloads);
    else if (sort === "updated") sorted.sort((a, b) => (b.updated_at ?? 0) - (a.updated_at ?? 0));
    else sorted.sort((a, b) => b.stars - a.stars || b.downloads - a.downloads);

    // 演示数据只有十几条：用 offset 模拟游标，演示也能验到「加载更多」按钮
    const offset = Number(cursor?.replace(/^offset:/, "") ?? 0) || 0;
    const items = sorted.slice(offset, offset + limit);
    const next = offset + items.length;
    const hasMore = next < sorted.length;
    return { items, cursor: hasMore ? `offset:${next}` : null, has_more: hasMore };
  },

  marketDetail(slug: string): MarketSkillDetail {
    const skill = MARKET_SEED.find((item) => item.slug === slug);
    if (!skill) throw new Error(`技能市场里没有 ${slug} 这个技能`);
    return {
      skill,
      description: MARKET_DETAIL_TEXT[slug] ?? `# ${skill.display_name}\n\n${skill.summary}`,
      license: "MIT",
      changelog: "演示数据：最近一次更新补充了示例与排障说明。",
      moderation_summary: skill.suspicious
        ? "评审提示：文档示例把令牌拼进命令，建议只读不执行"
        : "评审通过：未发现联网外传或危险命令",
      reason_codes: skill.suspicious ? ["review.llm_review"] : [],
      homepage: `https://clawhub.ai/${skill.owner}/skills/${skill.slug}`,
      files: [
        { path: "SKILL.md", size: 4_208, content_type: "text/markdown" },
        { path: "references/examples.md", size: 1_920, content_type: "text/markdown" },
        { path: "skill-metadata.json", size: 612, content_type: "application/json" },
      ],
    };
  },

  /** 演示模式不联网：直接把「缓存目录 → 仓库 → 各 agent 链接」的计划摆出来。 */
  planMarketInstall(input: MarketInstallInput): ChangeSet {
    if (!input.agents.length) throw new Error("请至少选择一个目标 agent");
    const detail = demo.marketDetail(input.slug);
    if (detail.skill.malware_blocked) {
      throw new Error(`${detail.skill.display_name} 已被技能市场标记为恶意，AgentHub 拒绝安装`);
    }
    const name = detail.skill.slug;
    const storeTarget = `${STORE}/${name}`;
    const cacheDir = p(`${HOME}/.agenthub/market/cache/${name}-demo`);
    const agents = demo.marketAgents();
    const ops: ChangeSet["ops"] = [
      {
        id: 0,
        label: "移入中央仓库",
        action: { op: "move_into_store", from: cacheDir, to: storeTarget },
      },
    ];
    for (const id of input.agents) {
      const agent = agents.find((item) => item.id === id);
      if (!agent?.available || !agent.skills_dir) continue;
      ops.push({
        id: ops.length,
        label: `建立${LINK_LABEL}`,
        action: {
          op: "create_link",
          at: `${agent.skills_dir}/${name}`,
          target: storeTarget,
          strategy: LINK_STRATEGY,
        },
      });
    }
    const notes = [
      `技能包：${detail.files.length} 个文件 · ZIP`,
      `版本 ${detail.skill.version ?? "latest"}`,
      "下载内容只落在 ~/.agenthub/market/cache，确认后才搬进中央仓库",
    ];
    const risks: Risk[] = [];
    if (detail.skill.suspicious) {
      risks.push({
        level: "warn",
        message: `技能市场把 ${detail.skill.display_name} 标为可疑：${detail.moderation_summary ?? "未提供说明"}`,
      });
    }
    return {
      id: `cs-market-${Math.floor(Date.now() / 1000)}`,
      kind: "market_install",
      title: `从市场安装 ${detail.skill.display_name}`,
      ops,
      notes,
      risks,
    };
  },

  /* ------------------------------------------------------------ Phase 4 用量 */

  /**
   * 演示模式的用量报告：确定性样本（同一范围内每次生成同样的数），
   * 覆盖 5 个可解析来源 + 若干「无用量」来源，并留一个未定价模型验证「未定价」文案。
   */
  usage(range: UsageRange = "days7"): UsageReport {
    const cells = buildDemoUsageCells(range);
    const prices = demoPrices();
    const hourly = range === "today" || range === "last24h";
    return {
      generated_at: now(),
      range,
      range_label: tr(
        RANGE_OPTIONS.find((item) => item.key === range)?.label ?? "usage.range.days7"
      ),
      granularity: hourly ? "hour" : "day",
      start: demoRangeStart(range),
      totals: totalsOf(cells, prices),
      cells,
      by_agent: groupCells(
        cells,
        (cell) => cell.agent,
        (key) => USAGE_AGENT_LABEL[key] ?? key,
        prices
      ),
      by_provider: groupCells(cells, (cell) => providerKey(cell.provider), (key) => key, prices),
      by_model: groupCells(cells, (cell) => cell.model, (key) => key, prices),
      sources: demoUsageSources(),
      stats: {
        files_total: 320,
        files_parsed: 0,
        files_cached: 320,
        bad_lines: 0,
        elapsed_ms: 88,
        pruned: 0,
      },
      prices,
      note: USAGE_NOTE,
    };
  },

  usagePricing(): PricingView {
    return demoPricingView();
  },

  saveUsagePricing(rows: ModelPriceInput[]): PricingView {
    DEMO_PRICES = rows.map((row) => ({
      model: row.model,
      provider: row.provider,
      input: row.input,
      output: row.output,
      cache_read: row.cache_read,
      cache_write: row.cache_write,
      currency: row.currency,
      source: "manual",
      updated_at: now(),
    }));
    return demoPricingView();
  },

  previewUsagePricing(source: string): ModelPrice[] {
    return PUBLIC_PRICE_SAMPLES.slice(0, 6).map((row) => ({
      ...row,
      source,
      updated_at: now(),
    }));
  },

  applyUsagePricing(rows: ModelPrice[]): PricingView {
    for (const row of rows) {
      const hit = DEMO_PRICES.findIndex(
        (price) => price.model === row.model && price.provider === row.provider
      );
      if (hit >= 0) DEMO_PRICES[hit] = { ...row };
      else DEMO_PRICES.push({ ...row });
    }
    return demoPricingView();
  },
};

/* -------------------------------------------------------------- Phase 4 辅助 */

const USAGE_NOTE =
  "合计 = 非缓存输入 + 命中缓存 + 输出（新建缓存本机恒为 0，不单列）；按本地时区切分时间；CodeBuddy / WorkBuddy 日志中的积分（credit）字段不计入";

const USAGE_AGENT_LABEL: Record<string, string> = {
  claude: "Claude Code",
  codex: "Codex",
  codebuddy: "CodeBuddy CLI",
  "codebuddy-ide": "CodeBuddy IDE",
  workbuddy: "WorkBuddy",
  "workbuddy-ai": "WorkBuddy AI",
};

/** 演示价格表。`agnes-3.0-flash` 故意没有价格，用来验证「未定价」而不是 0。 */
let DEMO_PRICES: ModelPrice[] = [
  {
    model: "claude-sonnet-5-5",
    provider: "anthropic",
    input: 3,
    output: 15,
    cache_read: 0.3,
    cache_write: 3.75,
    currency: "USD",
    source: "manual",
    updated_at: 0,
  },
  {
    model: "gpt-6.1-sol",
    provider: "openai",
    input: 2.5,
    output: 10,
    cache_read: 0.25,
    cache_write: 0,
    currency: "USD",
    source: "manual",
    updated_at: 0,
  },
  {
    model: "deepseek-v4.1-flash",
    provider: "deepseek",
    input: 0.28,
    output: 0.42,
    cache_read: 0.028,
    cache_write: 0,
    currency: "USD",
    source: "models.dev",
    updated_at: 0,
  },
  {
    model: "glm-5-3-flash",
    provider: null,
    input: 0.6,
    output: 2.2,
    cache_read: 0.06,
    cache_write: 0,
    currency: "USD",
    source: "manual",
    updated_at: 0,
  },
];

/** 「从公开源导入」的演示候选：只用于展示预览交互，不代表真实报价。 */
const PUBLIC_PRICE_SAMPLES: ModelPrice[] = [
  {
    model: "claude-opus-4-5",
    provider: "anthropic",
    input: 15,
    output: 75,
    cache_read: 1.5,
    cache_write: 18.75,
    currency: "USD",
    source: "models.dev",
    updated_at: 0,
  },
  {
    model: "gpt-6.1",
    provider: "openai",
    input: 5,
    output: 20,
    cache_read: 0.5,
    cache_write: 0,
    currency: "USD",
    source: "models.dev",
    updated_at: 0,
  },
  {
    model: "deepseek-v4-pro",
    provider: "deepseek",
    input: 0.55,
    output: 2.2,
    cache_read: 0.14,
    cache_write: 0,
    currency: "USD",
    source: "models.dev",
    updated_at: 0,
  },
  {
    model: "gemini-3-pro",
    provider: "google",
    input: 1.25,
    output: 10,
    cache_read: 0.31,
    cache_write: 0,
    currency: "USD",
    source: "models.dev",
    updated_at: 0,
  },
  {
    model: "qwen-4-max",
    provider: "alibaba",
    input: 1.2,
    output: 6,
    cache_read: 0.24,
    cache_write: 0,
    currency: "USD",
    source: "models.dev",
    updated_at: 0,
  },
  {
    model: "glm-5-3-flash",
    provider: "zhipu",
    input: 0.6,
    output: 2.2,
    cache_read: 0.06,
    cache_write: 0,
    currency: "USD",
    source: "models.dev",
    updated_at: 0,
  },
];

function demoPrices(): ModelPrice[] {
  return DEMO_PRICES.map((price) => ({ ...price }));
}

function demoPricingView(): PricingView {
  return {
    prices: demoPrices().sort((a, b) => a.model.localeCompare(b.model)),
    sources: [
      { key: "models_dev", label: "models.dev", url: "https://models.dev/api.json" },
      { key: "openrouter", label: "OpenRouter", url: "https://openrouter.ai/api/v1/models" },
    ],
    note: "单价为每 100 万 token；未定价的模型显示「未定价」而不是 0；从公开源导入只在你点击时联网",
  };
}

/** 与 Rust 的 `UsageRange::start_secs` 同义（本地时区）。 */
function demoRangeStart(range: UsageRange): number | null {
  const midnight = new Date();
  midnight.setHours(0, 0, 0, 0);
  const dayStart = Math.floor(midnight.getTime() / 1000);
  switch (range) {
    case "today":
      return dayStart;
    case "last24h":
      return now() - 86_400;
    case "days7":
      return now() - 7 * 86_400;
    case "days15":
      return now() - 15 * 86_400;
    case "days30":
      return now() - 30 * 86_400;
    default:
      return null;
  }
}

function alignHour(ts: number): number {
  return ts - (ts % 3600);
}

function alignDay(ts: number): number {
  const date = new Date(ts * 1000);
  date.setHours(0, 0, 0, 0);
  return Math.floor(date.getTime() / 1000);
}

/** 确定性样本：同一时间窗内每次渲染得到同样的数，截图与断言才可复现。 */
function buildDemoUsageCells(range: UsageRange): UsageCell[] {
  const end = now();
  const start = demoRangeStart(range) ?? end - 60 * 86_400;
  const hourly = range === "today" || range === "last24h";
  const buckets: number[] = [];
  if (hourly) {
    for (let t = alignHour(start); t <= end; t += HOUR) buckets.push(t);
  } else {
    for (let t = alignDay(start); t <= end; t += 86_400) buckets.push(t);
  }

  const plan: { agent: string; model: string; provider: string | null }[] = [
    { agent: "claude", model: "claude-sonnet-5-5", provider: "anthropic" },
    { agent: "claude", model: "glm-5-3-flash", provider: null },
    { agent: "codex", model: "gpt-6.1-sol", provider: "openai" },
    { agent: "codebuddy", model: "deepseek-v4.1-flash", provider: "deepseek" },
    { agent: "codebuddy-ide", model: "agnes-3.0-flash", provider: "agnes" },
    { agent: "workbuddy", model: "glm-5-3-flash", provider: null },
  ];

  const cells: UsageCell[] = [];
  buckets.forEach((bucket, index) => {
    plan.forEach((item, slot) => {
      const seed = (index + 1) * 37 + slot * 91;
      const wave = 0.55 + 0.45 * Math.sin((index + slot) / 2.3);
      const input = Math.round((40_000 + (seed % 90_000)) * wave);
      const cache_read = Math.round((320_000 + (seed % 640_000)) * wave);
      const cache_write = Math.round((seed % 4_200) * wave);
      const output = Math.round((1_800 + (seed % 5_200)) * wave);
      if (input + cache_read + cache_write + output === 0) return;
      cells.push({
        bucket,
        agent: item.agent,
        model: item.model,
        provider: item.provider,
        input,
        cache_read,
        cache_write,
        output,
        calls: 1 + (seed % 7),
      });
    });
  });
  return cells;
}

function demoUsageSources(): UsageSourceStats[] {
  const root = (rel: string) => (IS_WINDOWS ? `${HOME}\\${rel.replace(/\//g, "\\")}` : `${HOME}/${rel}`);
  return [
    {
      source: {
        key: "claude",
        label: "Claude Code",
        format: "claude_jsonl",
        roots: [root(".claude/projects")],
        exists: true,
        unsupported: null,
      },
      files: 69,
      records: 1_391,
      bad_lines: 0,
    },
    {
      source: {
        key: "codex",
        label: "Codex",
        format: "codex_jsonl",
        roots: [root(".codex/sessions")],
        exists: true,
        unsupported: null,
      },
      files: 16,
      records: 448,
      bad_lines: 0,
    },
    {
      source: {
        key: "codebuddy",
        label: "CodeBuddy CLI",
        format: "open_ai_jsonl",
        roots: [root(".codebuddy/projects")],
        exists: true,
        unsupported: null,
      },
      files: 40,
      records: 2_338,
      bad_lines: 0,
    },
    {
      source: {
        key: "codebuddy-ide",
        label: "CodeBuddy IDE",
        format: "codebuddy_ide_index",
        roots: [root(".local/CodeBuddyExtension/Data")],
        exists: true,
        unsupported: null,
      },
      files: 79,
      records: 215,
      bad_lines: 0,
    },
    {
      source: {
        key: "workbuddy",
        label: "WorkBuddy",
        format: "open_ai_jsonl",
        roots: [root(".workbuddy/projects")],
        exists: true,
        unsupported: null,
      },
      files: 116,
      records: 336,
      bad_lines: 0,
    },
    {
      source: {
        key: "hermes",
        label: "Hermes",
        format: "no_usage",
        roots: [],
        exists: false,
        unsupported: "请求日志（sessions/request_dump_*.json）只存请求与响应，不含 usage 字段",
      },
      files: 0,
      records: 0,
      bad_lines: 0,
    },
    {
      source: {
        key: "opencode",
        label: "OpenCode",
        format: "no_usage",
        roots: [],
        exists: false,
        unsupported: "opencode.db 里没有用量表（token 命中的都是 access_token）",
      },
      files: 0,
      records: 0,
      bad_lines: 0,
    },
  ];
}
