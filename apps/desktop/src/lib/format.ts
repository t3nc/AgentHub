import { currentLocale, tr } from "./i18n/runtime";
import { bcp47 } from "./i18n/types";
import type { LinkState, LinkStrategyName } from "./types";

export function bytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = value;
  let idx = 0;
  while (size >= 1024 && idx < units.length - 1) {
    size /= 1024;
    idx += 1;
  }
  return idx === 0 ? `${value} B` : `${size.toFixed(1)} ${units[idx]}`;
}

/**
 * token 数量的紧凑写法：1.2M / 34.5K。
 *
 * 用 1000 进制而不是 1024：token 是计数不是字节，1M tokens 就是 10^6。
 */
export function compactTokens(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0";
  const units: { scale: number; suffix: string }[] = [
    { scale: 1e12, suffix: "T" },
    { scale: 1e9, suffix: "B" },
    { scale: 1e6, suffix: "M" },
    { scale: 1e3, suffix: "K" },
  ];
  for (const unit of units) {
    if (value >= unit.scale) {
      const size = value / unit.scale;
      return `${size >= 100 ? size.toFixed(0) : size.toFixed(1)}${unit.suffix}`;
    }
  }
  return String(Math.round(value));
}

/** 精确到个位的分组显示，用于明细表（1,234,567）。 */
export function thousands(value: number): string {
  if (!Number.isFinite(value)) return "0";
  return Math.round(value).toLocaleString("en-US");
}

export function percent(part: number, total: number, digits = 1): string {
  if (!Number.isFinite(part) || !Number.isFinite(total) || total <= 0) return "—";
  return `${((part / total) * 100).toFixed(digits)}%`;
}

/** 金额：小额保留 4 位，大额 2 位。 */
export function money(value: number, currency = "USD"): string {
  if (!Number.isFinite(value)) return "—";
  const digits = Math.abs(value) >= 1 ? 2 : 4;
  return `${value.toFixed(digits)} ${currency}`;
}

export function timeAgo(ts: number): string {
  if (!ts) return "—";
  const seconds = Math.max(1, Math.floor(Date.now() / 1000 - ts));
  if (seconds < 60) return tr("common.time.seconds", { count: seconds });
  if (seconds < 3600) return tr("common.time.minutes", { count: Math.floor(seconds / 60) });
  if (seconds < 86400) return tr("common.time.hours", { count: Math.floor(seconds / 3600) });
  if (seconds < 86400 * 30) return tr("common.time.days", { count: Math.floor(seconds / 86400) });
  // 超过一个月直接给日期，按当前界面语言的日期格式
  return new Date(ts * 1000).toLocaleDateString(bcp47(currentLocale()));
}

export function shortHash(hash: string | null): string {
  return hash ? hash.slice(0, 8) : "--------";
}

const WINDOWS_ABS = /^[a-zA-Z]:[\\/]/;

/** 分隔符统一成 `/`：Windows 路径在展示与切分时不再出错。 */
export function toSlash(path: string): string {
  return path.replace(/\\/g, "/");
}

/** 取路径最后一段（文件名 / 目录名），兼容 `\` 与 `/`。 */
export function basename(path: string): string {
  if (!path) return "";
  const normalized = toSlash(path).replace(/\/+$/, "");
  const idx = normalized.lastIndexOf("/");
  return idx >= 0 ? normalized.slice(idx + 1) : normalized;
}

/** 取末尾 n 段并用 `/` 连接，用于「a/b/c」这类紧凑展示。 */
export function tailPath(path: string, n = 2): string {
  if (!path) return "";
  const parts = toSlash(path).split("/").filter(Boolean);
  return parts.slice(-n).join("/");
}

/** 家目录前缀缩成 `~`：Unix 的 `~` 与 Windows 的 `C:\Users\x` 都识别。 */
export function withTilde(path: string): string {
  if (!path) return "";
  const p = toSlash(path);
  if (WINDOWS_ABS.test(p)) {
    const win = p.match(/^[a-zA-Z]:\/(?:Users|用户)\/[^/]+/i);
    if (win) return `~${p.slice(win[0].length)}`;
    return p;
  }
  const unix = p.match(/^\/(?:Users|home)\/[^/]+/);
  return unix ? `~${p.slice(unix[0].length)}` : p;
}

export function shortPath(path: string, keep = 3): string {
  if (!path) return "";
  const home = withTilde(path);
  const parts = home.split("/").filter((s) => s.length > 0);
  if (parts.length <= keep + 1) return home;
  const root = home.startsWith("/") ? "/" : "";
  return `${root}${parts.slice(0, 2).join("/")}/…/${parts.slice(-keep).join("/")}`;
}

export function strategyLabel(strategy: LinkStrategyName): string {
  switch (strategy) {
    case "symlink":
      return tr("common.strategy.symlink");
    case "junction":
      return tr("common.strategy.junction");
    case "hardlink":
      return tr("common.strategy.hardlink");
    case "copy":
      return tr("common.strategy.copy");
  }
}

export interface LinkBadgeInfo {
  label: string;
  tone: "ok" | "warn" | "danger" | "dim";
}

export function linkBadge(link: LinkState): LinkBadgeInfo {
  switch (link.kind) {
    case "symlink":
      return { label: tr("common.link.symlink"), tone: "ok" };
    case "junction":
      return { label: tr("common.link.junction"), tone: "ok" };
    case "hardlink":
      return { label: tr("common.link.hardlink"), tone: "ok" };
    case "broken":
      return { label: tr("common.link.broken"), tone: "danger" };
    case "foreign":
      return { label: tr("common.link.foreign"), tone: "warn" };
    default:
      return { label: tr("common.link.copy"), tone: "warn" };
  }
}

export const agentMeta: Record<string, { name: string; short: string; accent: string }> = {
  claude: { name: "Claude Code", short: "CC", accent: "text-[#D97757] border-[#D97757]/30 bg-[#D97757]/10" },
  codex: { name: "Codex", short: "CX", accent: "text-[#8B7BFF] border-[#8B7BFF]/30 bg-[#8B7BFF]/10" },
  codebuddy: { name: "CodeBuddy", short: "CB", accent: "text-[#22D3EE] border-[#22D3EE]/30 bg-[#22D3EE]/10" },
  hermes: { name: "Hermes", short: "HM", accent: "text-[#22C55E] border-[#22C55E]/30 bg-[#22C55E]/10" },
  gemini: { name: "Gemini CLI", short: "GM", accent: "text-[#3B82F6] border-[#3B82F6]/30 bg-[#3B82F6]/10" },
  opencode: { name: "OpenCode", short: "OC", accent: "text-[#F472B6] border-[#F472B6]/30 bg-[#F472B6]/10" },
  // 技能真身 / 中央仓库：图标由 AgentHub 方阵 mark 提供（见 agent-visual 的 store 品牌），
  // 这里的短码只在字母块兜底时才会被用到。
  // 只有 `agentInfo(null)` 会用到它，那里会用 tr("common.store") 覆盖成本地化名字；
  // 这里的字面量仅作结构占位，所以保持英文。
  store: { name: "Hub store", short: "ST", accent: "text-text-muted border-white/10 bg-white/5" },
};

export function agentInfo(id: string | null) {
  // 中央仓库这个名字要跟着界面语言走，其余是产品名（不翻译）
  if (!id) return { ...agentMeta.store, name: tr("common.store") };
  const meta = agentMeta[id];
  if (meta) return meta;
  const cleaned = id.replace(/^custom:/, "");
  return {
    name: cleaned,
    short: cleaned.slice(0, 2).toUpperCase(),
    accent: "text-[#F59E0B] border-[#F59E0B]/30 bg-[#F59E0B]/10",
  };
}
