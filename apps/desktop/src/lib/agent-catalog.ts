import { agentInfo } from "./format";
import type { MessageKey } from "./i18n/messages";
import { tr } from "./i18n/runtime";
import type { AgentInstall, CatalogEntry, EnvState, ManagedState } from "./types";

/** 行首短码：内置沿用 CC/CX/CB/HM/GM，其余给产品缩写。 */
export const CATALOG_SHORT: Record<string, string> = {
  "claude-code": "CC",
  "claude-desktop": "CD",
  codex: "CX",
  "chatgpt-desktop": "GP",
  codebuddy: "CB",
  "codebuddy-ide": "CI",
  workbuddy: "WB",
  "workbuddy-ai": "WA",
  trae: "TR",
  "trae-cn": "TN",
  opencode: "OC",
  pi: "PI",
  openwork: "OW",
  hermes: "HM",
  "gemini-cli": "GM",
  manus: "MN",
};

export function catalogShort(entry: CatalogEntry): string {
  return CATALOG_SHORT[entry.key] ?? entry.name.slice(0, 2).toUpperCase();
}

export const KIND_LABEL: Record<CatalogEntry["kind"], MessageKey> = {
  cli: "agents.kind.cli",
  desktop: "agents.kind.desktop",
  ide: "agents.kind.ide",
  cloud: "agents.kind.cloud",
};

export const STATE_LABEL: Record<EnvState, MessageKey> = {
  ready: "agents.state.ready",
  config_only: "agents.state.configOnly",
  missing: "agents.state.missing",
};

export interface StateBadgeInfo {
  label: string;
  tone: "ok" | "warn" | "danger" | "dim";
}

/** 文案按形态分支：桌面产品说「已安装」比「CLI 就绪」贴切。 */
export function stateBadge(entry: CatalogEntry): StateBadgeInfo {
  if (entry.state === "ready") {
    return {
      label: tr(entry.kind === "cli" ? "agents.env.cliReady" : "agents.env.installed"),
      tone: "ok",
    };
  }
  if (entry.state === "config_only") {
    return {
      label: tr(entry.kind === "cli" ? "agents.env.configLeftover" : "agents.env.tracesOnly"),
      tone: "warn",
    };
  }
  return { label: tr("agents.env.notDetected"), tone: "danger" };
}

/** 主入口优先，其次才是配置路径（展示用，不是真实可执行路径）。 */
export function entryPath(entry: CatalogEntry): string {
  return entry.cli_path ?? entry.app_path ?? entry.config_path ?? "";
}

export function isManaged(entry: CatalogEntry): boolean {
  return entry.managed === "builtin" || entry.managed === "managed";
}

export interface CatalogGroups {
  managed: CatalogEntry[];
  catalog: CatalogEntry[];
}

/** 两组：已接入（内置 + 已登记）与「目录里还没接的」。 */
export function groupCatalog(entries: CatalogEntry[]): CatalogGroups {
  return {
    managed: entries.filter(isManaged),
    catalog: entries.filter((entry) => !isManaged(entry)),
  };
}

export interface StateCounts {
  ready: number;
  configOnly: number;
  missing: number;
  total: number;
}

export function countStates(entries: CatalogEntry[]): StateCounts {
  return {
    ready: entries.filter((entry) => entry.state === "ready").length,
    configOnly: entries.filter((entry) => entry.state === "config_only").length,
    missing: entries.filter((entry) => entry.state === "missing").length,
    total: entries.length,
  };
}

export type ManagedActionKind = "add" | "remove" | "jump" | "none";

export interface ManagedAction {
  kind: ManagedActionKind;
  label: string;
  reason?: string;
  /** kind = jump 时要跳去的 agent slug（shared 场景） */
  agent?: string;
}

/** 行尾按钮：能不能登记 / 能不能移出 / 该去哪个已受管页面，以及不能时的原因。 */
export function managedAction(entry: CatalogEntry): ManagedAction {
  const state: ManagedState = entry.managed;
  if (state === "builtin") {
    return {
      kind: "none",
      label: tr("agents.managed.builtin"),
      reason: tr("agents.managed.builtinReason"),
    };
  }
  if (state === "shared") {
    const agent = entry.shared_agent ?? "";
    return {
      kind: agent ? "jump" : "none",
      label: agent
        ? tr("agents.managed.openSkills", { name: agentInfo(agent).name })
        : tr("agents.managed.takenOver"),
      reason: entry.managed_note ?? tr("agents.managed.sharedReason"),
      agent: agent || undefined,
    };
  }
  if (state === "managed") {
    return { kind: "remove", label: tr("agents.managed.remove") };
  }
  if (state === "addable") {
    return { kind: "add", label: tr("agents.managed.add") };
  }
  return {
    kind: "none",
    label: tr("agents.managed.blocked"),
    reason: entry.managed_note ?? tr("agents.managed.blockedReason"),
  };
}

/**
 * 老版本桌面壳没有 `hub_agent_catalog`，页面退回扫描结果里的 5 条内置记录：
 * 路径与状态还在，安装步骤为空（提示升级壳）。
 */
export function fromAgentInstalls(installs: AgentInstall[]): CatalogEntry[] {
  return installs.map((install) => ({
    key: install.id,
    family: "builtin",
    name: install.display_name,
    vendor: tr("agents.vendor.builtin"),
    kind: "cli",
    official: "",
    needs_confirm: false,
    builtin: install.id,
    state: install.detected ? "ready" : "missing",
    cli_name: null,
    cli_path: install.cli_path,
    version: null,
    app_path: null,
    config_path: install.config_dir,
    config_exists: Boolean(install.config_dir),
    evidence: install.cli_path
      ? [tr("agents.evidence.pathHit", { path: install.cli_path })]
      : [],
    managed: "builtin",
    shared_agent: null,
    managed_id: null,
    managed_note: null,
    managed_targets: [],
    steps: [],
    docs: "",
    platform: "",
  }));
}
