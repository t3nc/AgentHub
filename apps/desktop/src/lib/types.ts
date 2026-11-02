// 与 agenthub-core 的 serde DTO 一一对应。改动 Rust 侧字段时同步这里。

export interface Capabilities {
  skills: boolean;
  mcp: boolean;
  instructions: boolean;
  project_scope: boolean;
  links: boolean;
}

export interface AgentInstall {
  id: string;
  display_name: string;
  config_dir: string | null;
  cli_path: string | null;
  capabilities: Capabilities;
  source: "builtin" | "custom";
  detected: boolean;
}

export type LinkState =
  | { kind: "symlink"; target: string }
  | { kind: "junction"; target: string }
  | { kind: "hardlink"; target: string }
  | { kind: "broken"; target: string }
  | { kind: "foreign"; target: string }
  | { kind: "real_dir" };

export interface SkillRecord {
  name: string;
  path: string;
  agent: string | null;
  scope_label: string;
  source: "store" | "agent_copy" | "readonly";
  readonly: boolean;
  description: string | null;
  tree_hash: string | null;
  size_bytes: number;
  file_count: number;
  modified_at: number;
  link: LinkState;
  canonical: string;
  never_upload: boolean;
  /** 在「agent 独有」白名单里：不参与去重接管 / 链接修复 / 重复统计 */
  agent_local: boolean;
}

export interface DuplicateGroup {
  name: string;
  members: SkillRecord[];
  identical: boolean;
  wasted_bytes: number;
  suggested_source: string;
  reason: string;
}

export interface ScanStats {
  total: number;
  store_skills: number;
  agent_skills: number;
  linked: number;
  copies: number;
  broken: number;
  readonly: number;
  store_bytes: number;
  wasted_bytes: number;
  shared_skills: number;
}

export interface SkillTarget {
  agent: string;
  dir: string;
  readonly: boolean;
  scope: { user?: null } | { project?: { root: string } };
  exists: boolean;
}

export interface ScanReport {
  generated_at: number;
  scope_label: string;
  agents: AgentInstall[];
  targets: SkillTarget[];
  records: SkillRecord[];
  duplicates: DuplicateGroup[];
  stats: ScanStats;
  store_root: string;
  duration_ms: number;
}

export type SecretRef =
  | { kind: "env"; var: string }
  | { kind: "keychain"; service: string; account: string }
  | { kind: "literal"; value: string; acknowledged: boolean };

export type Transport =
  | {
      type: "stdio";
      command: string;
      args: string[];
      env: Record<string, SecretRef>;
      cwd: string | null;
    }
  | { type: "http"; url: string; headers: Record<string, SecretRef> };

export interface McpTargetState {
  agent: string;
  file: string;
  format: string;
  present: boolean;
  writable: boolean;
  reason: string | null;
}

export interface McpServerView {
  name: string;
  transport: Transport;
  enabled: boolean;
  targets: McpTargetState[];
  timeout_secs: number | null;
  note: string | null;
}

export interface InstructionSpec {
  agent: string;
  path: string;
  kind: "rule" | "persona" | "bridge";
  exists: boolean;
  managed: boolean;
  size_bytes: number;
  display: string;
}

export interface ChainEntry {
  agent: string;
  order: number;
  path: string;
  exists: boolean;
  managed: boolean;
  shadowed: boolean;
  note: string | null;
}

export interface ProjectSummary {
  root: string;
  name: string;
  branch: string | null;
  is_git: boolean;
  has_state: boolean;
  skill_count: number;
  mcp_count: number;
  agent_dirs: string[];
}

export interface BatchRecord {
  batch_id: string;
  kind: string;
  title: string;
  created_at: number;
  undone: boolean;
  op_count: number;
}

export interface DoctorIssue {
  level: "info" | "warn" | "danger" | string;
  path: string;
  message: string;
  /**
   * 问题分类，决定清单里给哪个快捷操作：
   * `broken_link` / `foreign_link` / `duplicate_copy` / `duplicate_group` / `store_missing`
   * 老壳没有这个字段时为 undefined，前端退回按 level 展示。
   */
  kind?: string;
  /** `duplicate_copy` / `duplicate_group` 的技能名，用于直接生成收敛计划 */
  subject?: string | null;
}

export type RiskLevel = "info" | "warn" | "danger";

export interface Risk {
  level: RiskLevel;
  message: string;
}

export type OpAction =
  | { op: "create_link"; at: string; target: string; strategy: string }
  | { op: "replace_with_link"; at: string; target: string; strategy: string }
  | { op: "move_into_store"; from: string; to: string }
  | { op: "remove_path"; at: string; use_trash: boolean }
  | { op: "write_file"; at: string; content: string; backup: boolean };

export interface FileOp {
  id: number;
  label: string;
  action: OpAction;
}

export interface ChangeSet {
  id: string;
  kind: string;
  title: string;
  ops: FileOp[];
  notes: string[];
  risks: Risk[];
}

export interface ApplyResult {
  batch_id: string;
  applied: number;
  skipped: number;
  backup_dir: string | null;
  log: string[];
}

export type LinkStrategyName = "symlink" | "junction" | "hardlink" | "copy";

/** 链接策略的当前解析结果：单条覆盖 > default（全局/平台默认）。 */
export interface StrategyConfig {
  /** 全局默认解析后的值（配置默认，或平台默认） */
  default: LinkStrategyName;
  /** 平台默认：mac/Linux 软链，Windows 目录联接 */
  platform: LinkStrategyName;
  /** 单条技能覆盖，key = 技能名 */
  overrides: Record<string, LinkStrategyName>;
}

/** 壳层文件监听推来的变化摘要（与 agenthub_core::watch::WatchDelta 对应）。 */
export interface WatchDelta {
  added: string[];
  removed: string[];
  changed: string[];
}

export interface Overview {
  report: ScanReport;
  mcp: McpServerView[];
  batches: BatchRecord[];
  doctor: DoctorIssue[];
  data_root: string;
  platform: string;
  strategies: StrategyConfig;
  demo: boolean;
}

export interface InstructionOverview {
  specs: InstructionSpec[];
  chain: ChainEntry[];
}

// ---- Agent 环境与安装指引（ext/catalog.rs 的 DTO） ----

export type AgentKind = "cli" | "desktop" | "ide" | "cloud";

/** ready = 主入口在；config_only = 只有配置残留；missing = 都没检测到 */
export type EnvState = "ready" | "config_only" | "missing";

/**
 * builtin = 内置已受管；managed = 已登记；addable = 可登记；
 * shared = 配置与某个内置适配器共用（如 OpenAI 桌面端与 Codex CLI）；
 * blocked = 不支持
 */
export type ManagedState = "builtin" | "managed" | "addable" | "shared" | "blocked";

export interface InstallStep {
  title: string;
  /** 桌面产品的命令位是下载页地址，不是 shell 命令 */
  command: string;
  risk: string;
  url: string | null;
}

/** 「纳入受管」将写入配置的路径，确认前先展示 */
export interface ManagedTarget {
  label: string;
  path: string;
  exists: boolean;
  scope: "user" | "project";
}

// ---- 设置页（api/settings.rs 的 DTO） ----

/** MCP 配置格式（与 Rust `McpFormat` 的 serde 形状一致） */
export type McpFormatSpec =
  | { format: "json"; pointer: string[] }
  | { format: "toml"; table: string }
  | { format: "yaml"; top_key: string };

export interface CustomAgentDef {
  id: string;
  display_name: string;
  skills_dir: string | null;
  project_skills_dir: string | null;
  mcp_file: string | null;
  mcp_format: McpFormatSpec | null;
  project_mcp_file: string | null;
  instructions: string[];
}

/** 单个内置 agent 的路径覆盖；空字段 = 跟随自动检测 */
export interface AgentPathOverride {
  config_dir: string | null;
  skills_dir: string | null;
  mcp_file: string | null;
  instructions: string[];
}

export interface EffectivePaths {
  config_dir: string | null;
  skills_dir: string | null;
  /** 主技能目录之外还会读的位置（OpenCode 的 .agents/skills、.claude/skills） */
  extra_skills_dirs: string[];
  mcp_file: string | null;
  instructions: string[];
}

export interface PathPresence {
  config_dir: boolean;
  skills_dir: boolean;
  extra_skills_dirs: boolean[];
  mcp_file: boolean;
  instructions: boolean[];
}

export interface AgentPathsView {
  id: string;
  display_name: string;
  builtin: boolean;
  overrides: AgentPathOverride;
  effective: EffectivePaths;
  presence: PathPresence;
}

export interface StoreUsage {
  skills: number;
  bytes: number;
}

/** 技能市场数据源：两家接口协议不通用，切源等于换适配器 */
export type MarketSourceKey = "skillhub" | "clawhub";

export interface SettingsView {
  data_root: string;
  store_root: string;
  store_usage: StoreUsage;
  default_strategy: LinkStrategyName | null;
  platform_strategy: LinkStrategyName;
  backups_keep: number;
  excluded_paths: string[];
  use_trash_for_dedupe: boolean;
  backup_config_files: boolean;
  agents: AgentPathsView[];
  custom_agents: CustomAgentDef[];
  /** Phase 3：provider 注册表与各 agent 的绑定状态（只读，改它走 provider 命令） */
  providers: ProvidersView;
  /** 技能市场生效数据源 */
  market_source: MarketSourceKey;
  /** 数据源展示名，市场浮层徽标直接用 */
  market_source_label: string;
  /** 技能市场自定义源；null = 跟随默认 */
  market_registry: string | null;
  /** 当前数据源的官方地址，用作输入框占位 */
  market_default_registry: string;
  /** 功能开关当前状态 */
  features: FeatureFlags;
  /** 功能管理一行的元数据（名称 / 说明 / 是否启用） */
  feature_catalog: FeatureMeta[];
  /** 界面语言的选择：`system` / `zh-Hans` / `zh-Hant` / `en` */
  locale: LocaleModeKey;
  /** 实际生效的语言代码（「跟随系统」时用它回显） */
  locale_effective: Exclude<LocaleModeKey, "system">;
}

/** 与 `lib/i18n/types.ts` 的 `LocaleMode` 对齐；写在这里避免 types 反向依赖 i18n。 */
export type LocaleModeKey = "system" | "zh-Hans" | "zh-Hant" | "en";

export interface LocaleView {
  mode: LocaleModeKey;
  effective: Exclude<LocaleModeKey, "system">;
  tag: string;
}

// ---- 功能管理（设置 › 功能管理） ----

/** 可开关的功能模块。只有这两个：其余模块是主链路，不做开关。 */
export type FeatureKey = "provider" | "usage";

export interface FeatureFlags {
  provider: boolean;
  usage: boolean;
}

export interface FeatureMeta {
  key: FeatureKey;
  display_name: string;
  description: string;
  enabled: boolean;
}

/** 默认全开：后端老版本没有这个命令时，前端不应把模块藏起来。 */
export const DEFAULT_FEATURES: FeatureFlags = { provider: true, usage: true };

export interface SettingsInput {
  store_root: string;
  default_strategy: LinkStrategyName | null;
  backups_keep: number;
  excluded_paths: string[];
  use_trash_for_dedupe: boolean;
  backup_config_files: boolean;
  agent_paths: Record<string, AgentPathOverride>;
  custom_agents: CustomAgentDef[];
  /** 技能市场数据源 */
  market_source: MarketSourceKey;
  /** 空串 = 跟随默认市场源地址 */
  market_registry: string;
}

// ---- 技能删除 ----

export interface DeleteOptions {
  /** 移入回收站（false = 永久删除，仅靠批次备份恢复） */
  use_trash: boolean;
  /** 删仓库真身时连带删除指向它的链接 */
  with_links: boolean;
}

// ---- 技能市场（model/market.rs 的 DTO） ----

export type MarketSort = "recommended" | "downloads" | "updated";

export interface MarketSkill {
  slug: string;
  display_name: string;
  summary: string;
  owner: string;
  version: string | null;
  downloads: number;
  installs: number;
  stars: number;
  updated_at: number | null;
  topics: string[];
  suspicious: boolean;
  malware_blocked: boolean;
  verdict: string | null;
  official: boolean;
}

export interface MarketFile {
  path: string;
  size: number;
  content_type: string | null;
}

/** 一页市场结果：游标对前端不透明，拿到什么就原样传回去取下一批 */
export interface MarketPage {
  items: MarketSkill[];
  cursor: string | null;
  has_more: boolean;
}

export interface MarketSkillDetail {
  skill: MarketSkill;
  description: string;
  license: string | null;
  changelog: string | null;
  moderation_summary: string | null;
  reason_codes: string[];
  homepage: string | null;
  files: MarketFile[];
}

export interface MarketAgent {
  id: string;
  display_name: string;
  skills_dir: string | null;
  detected: boolean;
  available: boolean;
  reason: string | null;
}

export interface MarketInstallInput {
  slug: string;
  version: string | null;
  agents: string[];
}

// ---- Provider（Phase 3，api/provider.rs 的 DTO） ----

/** 登记过的 provider；`api_key` 永远只是引用形式 */
export interface ProviderView {
  id: string;
  name: string;
  base_url: string;
  /** `env:VAR` / `keychain:svc/acct` / `literal:********` */
  api_key: string;
  models: string[];
  note: string | null;
  /** 上游协议；null = 未声明（按路由下游协议透传） */
  protocol: GatewayProtocol | null;
  model_limits: Record<string, ModelLimits>;
  /** 内置目录来源 id；自定义供应商为 null */
  catalog_id: string | null;
  /** 形如「200K 上下文 · 64K 输出」；全没填时为 null */
  limits_summary: string | null;
}

/** 某个 agent 的绑定状态；未绑定时 provider_id / model 为 null */
export interface ProviderBindingView {
  agent: string;
  display_name: string;
  /** 本轮是否接入了该 agent 的 provider 写入 */
  supported: boolean;
  /** 不支持时的具体原因，直接展示 */
  unsupported_reason: string | null;
  provider_id: string | null;
  model: string | null;
  make_default: boolean;
  /** 经本地网关（true）还是直连上游 */
  via_gateway: boolean;
  route_id: string | null;
  /** 三模式：直连 / 路由 / 聚合 */
  mode: PoolMode;
  /** 经网关时 agent 配置里会写的本地地址（统一入口 /v1） */
  gateway_base_url: string | null;
  /** 经网关时的令牌 —— 就是 agent slug */
  gateway_token: string | null;
  /** Claude 的四个模型槽位：sonnet/opus/haiku/fable → 上游模型名 */
  model_slots: Record<string, string>;
  /** 池里已订阅的供应商 id */
  pool: string[];
}

/** 供应商池的接入方式（界面三模式） */
export type PoolMode = "direct" | "route" | "aggregate";

/** 槽位名 → 上游模型名 */
export type ModelSlots = Record<string, string>;

export interface ProvidersView {
  providers: ProviderView[];
  bindings: ProviderBindingView[];
}

export interface ProviderInput {
  id: string;
  name: string;
  base_url: string;
  api_key: string;
  models: string[];
  note: string | null;
  /** 明文密钥的风险确认；不清空后端会拒绝 */
  acknowledged: boolean;
  protocol?: GatewayProtocol | null;
  model_limits?: Record<string, ModelLimits>;
  catalog_id?: string | null;
}

/** 写入后的一次连通性探测 */
export interface ProbeReport {
  endpoint: string;
  ok: boolean;
  /** 一句话结论：「连接正常 · 12 个模型」/「密钥无效或没有权限（HTTP 401/403）」 */
  detail: string;
  models: number;
}

/** 「点一下就写入」的完整结果 */
export interface ApplyReport {
  /** 是否真的写了盘（配置已是最新时为 false） */
  applied: boolean;
  /** 没写盘时的原因 */
  skipped: string | null;
  /** 涉及的文件（绝对路径） */
  files: string[];
  /** 可用于回滚的批次号 */
  batch_id: string | null;
  probe: ProbeReport | null;
  notes: string[];
}

export interface BindProviderInput {
  agent: string;
  provider_id: string;
  model: string | null;
  make_default: boolean;
  /** 缺省 = 直连 */
  via_gateway?: boolean;
  route_id?: string | null;
  /** 三模式；缺省按 via_gateway 推导 */
  mode?: PoolMode;
  /** Claude 的模型槽位映射 */
  model_slots?: ModelSlots;
}

// ---- Provider 网关（model/gateway.rs 的 DTO） ----

/** `single` 单上游透传 / `route` 按模型分派 / `aggregate` 多上游池 */
export type RouteMode = "single" | "route" | "aggregate";

/**
 * 通信协议。
 *
 * `anthropic` 与 `chat_completions` 之间可以互相转换；`responses` 只做同协议直通，
 * 跨协议组合在保存配置时就会被后端拒绝。
 */
export type GatewayProtocol = "anthropic" | "chat_completions" | "responses";

/** 一个模型的长度登记（只展示，不做请求拦截） */
export interface ModelLimits {
  context: number | null;
  max_output: number | null;
  source: "manual" | "auto";
}

export type SupplierCategory = "official" | "model_vendor" | "aggregator";

/** 内置渠道定义（后端静态表，前端只读） */
export interface SupplierDef {
  id: string;
  name: string;
  vendor: string;
  category: SupplierCategory;
  website: string;
  base_url: string;
  protocol: GatewayProtocol;
  default_model: string;
  models: string[];
  /** 官方文档地址：地址对不对以它为准 */
  source_url: string;
  /** false = 没能用官方文档确认，界面要提示「地址待核实」 */
  verified: boolean;
}

export interface ProbedModel {
  id: string;
  context: number | null;
  max_output: number | null;
}

export interface ProbeResult {
  models: ProbedModel[];
  /** 实际请求的地址，失败时显示出来便于核对 */
  endpoint: string;
  error: string | null;
}

export interface ProbeInput {
  base_url: string;
  api_key: string;
  protocol?: GatewayProtocol | null;
}

export interface RouteMemberView {
  provider_id: string;
  /** 聚合模式下按权重轮询；0 视为 1 */
  weight: number;
  /** 故障转移顺序，越小越先试 */
  priority: number;
  /** 模型白名单；空 = 全部接受 */
  models: string[];
  /** 对外模型名 → 上游模型名 */
  model_map: Record<string, string>;
  enabled: boolean;
}

export interface RouteRuleView {
  /** 模型名通配：精确、`前缀*`、`*后缀`、`*包含*` */
  pattern: string;
  provider_ids: string[];
}

export interface RetryPolicyView {
  max_attempts: number;
  /** 首字节超时（毫秒） */
  first_byte_ms: number;
  /** 空闲超时（毫秒），0 = 不限 */
  idle_ms: number;
  /** 视为可重试的状态码 */
  retry_status: number[];
}

export interface GatewayRouteView {
  id: string;
  name: string;
  protocol: GatewayProtocol;
  mode: RouteMode;
  members: RouteMemberView[];
  rules: RouteRuleView[];
  retry: RetryPolicyView;
  enabled: boolean;
}

export interface GatewayConfigView {
  auto_start: boolean;
  /** 只可能是回环地址 */
  host: string;
  port: number;
  /** null = 不校验；配置里从不回传明文给前端以外的地方 */
  auth_token: string | null;
  log_requests: boolean;
  max_log_entries: number;
}

export interface GatewayView {
  config: GatewayConfigView;
  routes: GatewayRouteView[];
  /** 配置一致性问题：成员引用了不存在的 provider、绑定指向已删路由… */
  notes: string[];
  /** 含 `{route}` 占位 */
  base_url_template: string;
}

export interface GatewayStatusView {
  running: boolean;
  host: string;
  port: number;
  routes: number;
  auth: boolean;
  notes: string[];
  last_error: string | null;
}

/** 一次经过网关的请求。**不含正文**：只记元数据。 */
export interface GatewayRequestLog {
  at: number;
  route_id: string;
  provider_id: string | null;
  provider_name: string | null;
  model: string | null;
  protocol: string;
  agent: string | null;
  status: number;
  attempts: number;
  first_byte_ms: number;
  total_ms: number;
  bytes: number;
  error: string | null;
}

export interface GatewayConfigInput {
  port: number;
  auto_start: boolean;
  log_requests: boolean;
  max_log_entries: number;
  /** 空串 = 关闭鉴权 */
  auth_token: string;
}

export interface RouteDeleteResult {
  view: GatewayView;
  /** 被退回直连的 agent slug */
  reverted: string[];
}

export interface CatalogEntry {
  key: string;
  family: string;
  name: string;
  vendor: string;
  kind: AgentKind;
  official: string;
  needs_confirm: boolean;
  /** 对应内置适配器的 slug（claude / codex / …），自定义产品为 null */
  builtin: string | null;
  state: EnvState;
  cli_name: string | null;
  cli_path: string | null;
  version: string | null;
  app_path: string | null;
  /** 存在的配置路径；不存在时是第一个预期位置（看 config_exists 区分） */
  config_path: string | null;
  config_exists: boolean;
  evidence: string[];
  managed: ManagedState;
  /** managed = shared 时，接管它的内置适配器 slug（如 codex） */
  shared_agent: string | null;
  managed_id: string | null;
  managed_note: string | null;
  managed_targets: ManagedTarget[];
  steps: InstallStep[];
  docs: string;
  platform: string;
}

/* ---------------------------------------------------------------- Phase 4 用量 */

/** 与 `api::usage::UsageRange` 的 serde 名一致。 */
export type UsageRange = "today" | "last24h" | "days7" | "days15" | "days30" | "all";
export type UsageGranularity = "hour" | "day";

export interface UsageTotals {
  /** 非缓存输入（OpenAI 风格来源已减去缓存命中） */
  input: number;
  cache_read: number;
  cache_write: number;
  output: number;
  calls: number;
  total: number;
  /** 已定价部分的成本；全部未定价时为 null/undefined */
  cost?: number | null;
  /** > 0 表示有模型没定价，成本要标「部分未定价」 */
  unpriced_calls: number;
  currency: string;
}

export interface UsageCell {
  /** 桶起点（epoch 秒）：小时档是整点，天档是本地 00:00 */
  bucket: number;
  agent: string;
  model: string;
  provider: string | null;
  input: number;
  cache_read: number;
  cache_write: number;
  output: number;
  calls: number;
}

export interface UsageGroup extends UsageTotals {
  key: string;
  label: string;
}

export type UsageFormat =
  | "claude_jsonl"
  | "codex_jsonl"
  | "open_ai_jsonl"
  | "codebuddy_ide_index"
  | "no_usage";

export interface UsageSourceInfo {
  key: string;
  label: string;
  format: UsageFormat;
  roots: string[];
  exists: boolean;
  /** 有值 = 拿不到用量，文案即原因 */
  unsupported: string | null;
}

export interface UsageSourceStats {
  source: UsageSourceInfo;
  files: number;
  records: number;
  bad_lines: number;
}

export interface UsageScanStats {
  files_total: number;
  files_parsed: number;
  files_cached: number;
  bad_lines: number;
  elapsed_ms: number;
  pruned: number;
}

/** 每 100 万 token 的单价。 */
export interface ModelPrice {
  model: string;
  provider?: string | null;
  input: number;
  output: number;
  cache_read: number;
  cache_write: number;
  currency: string;
  source?: string | null;
  updated_at?: number | null;
}

export interface UsageReport {
  generated_at: number;
  range: UsageRange;
  range_label: string;
  granularity: UsageGranularity;
  start: number | null;
  totals: UsageTotals;
  cells: UsageCell[];
  by_agent: UsageGroup[];
  by_provider: UsageGroup[];
  by_model: UsageGroup[];
  sources: UsageSourceStats[];
  stats: UsageScanStats;
  prices: ModelPrice[];
  note: string;
}

export interface PricingSourceView {
  key: string;
  label: string;
  url: string;
}

export interface PricingView {
  prices: ModelPrice[];
  sources: PricingSourceView[];
  note: string;
}

export interface ModelPriceInput {
  model: string;
  provider: string | null;
  input: number;
  output: number;
  cache_read: number;
  cache_write: number;
  currency: string;
}
