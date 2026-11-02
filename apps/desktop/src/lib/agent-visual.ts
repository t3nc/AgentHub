import agenthubMark from "../assets/brand/agenthub-mark-connected.svg";
import claudeColor from "../assets/agents/claude-color.svg";
import codebuddyColor from "../assets/agents/codebuddy-color.svg";
import codexColor from "../assets/agents/codex-color.svg";
import geminiCliColor from "../assets/agents/geminicli-color.svg";
import hermesMono from "../assets/agents/hermes-agent.svg";
import manusMono from "../assets/agents/manus.svg";
import opencodeMono from "../assets/agents/opencode.svg";
import openaiMono from "../assets/agents/openai.svg";
import openworkColor from "../assets/agents/openwork-color.svg";
import openworkLight from "../assets/agents/openwork-light.svg";
import piMono from "../assets/agents/pi.svg";
import traeColor from "../assets/agents/trae-color.svg";
import workbuddyColor from "../assets/agents/workbuddy-color.svg";
import { CATALOG_SHORT } from "./agent-catalog";
import { agentInfo } from "./format";

/**
 * 一个 agent 的图标方案，四选一：
 * - `img`：官方彩色 mark，直接 `<img>`；
 * - `mask`：官方单色 mark（根节点是 `fill="currentColor"` 或纯黑 path），
 *   走 CSS mask + 品牌色填充；
 * - 两者都没有（自定义平台）：`short` 字母块兜底；
 * - 中央仓库：`mask` 指向 AgentHub 连通版标志，靠 `tint` 缺省值跟随 currentColor 自动适配亮暗。
 */
export interface AgentVisual {
  img?: string;
  /**
   * 暗色主题的替代位图：与 `img` 成对出现时，`AgentIcon` 同时渲染两张、用
   * `dark:hidden` / `hidden dark:block` 切换。给「深色外形 + 浅色内形」这类
   * 在暗底上会糊成一片的官方 mark 用（OpenWork）。
   */
  imgDark?: string;
  mask?: string;
  /** mask 的填充色；缺省跟随文字颜色（AgentHub mark 依赖此行为） */
  tint?: string;
  /** 兜底字母，沿用原来的 CC / CX / CB 语义 */
  short: string;
  /** 兜底字母的文字色；底色与边框由外层容器提供，避免出现多余的色块 */
  tone: string;
}

interface Brand {
  img?: string;
  imgDark?: string;
  mask?: string;
  tint?: string;
}

/**
 * 品牌图标表。除 AgentHub 自有标志与 Hermes / OpenWork 外，
 * 文件来自 LobeHub Icons（MIT，详见 `src/assets/agents/LICENSE-lobehub-icons.txt`），
 * 版本锁定 `@lobehub/icons-static-svg@1.101.0`：公司没有单独彩色版的走单色 mask。
 */
const BRANDS: Record<string, Brand> = {
  // AgentHub 中央仓库：连通版 mark（六段相连矩形），不给 tint → 跟随容器文字色
  store: { mask: agenthubMark },
  claude: { img: claudeColor },
  codex: { img: codexColor },
  // OpenAI 官方 mark 在 LobeHub 只有单色版
  openai: { mask: openaiMono, tint: "#10A37F" },
  gemini: { img: geminiCliColor },
  codebuddy: { img: codebuddyColor },
  workbuddy: { img: workbuddyColor },
  trae: { img: traeColor },
  // Hermes 没有公开官网、也没有官方彩色版：仓库内 hermes-agent.svg 是纯黑 path，走 mask 上色
  hermes: { mask: hermesMono, tint: "#22C55E" },
  opencode: { mask: opencodeMono, tint: "#F472B6" },
  // OpenWork mark 是「深色外形 + 白色内形」双色版，暗底上会糊掉，因此单独备一版反色图
  openwork: { img: openworkColor, imgDark: openworkLight },
  manus: { mask: manusMono, tint: "#F59E0B" },
  // pi.svg 对应 pi.dev 的 Pi Agent（编码 agent），不是 Inflection Pi
  pi: { mask: piMono, tint: "#EAB308" },
};

/** 目录 key（Agent 环境）→ 品牌：同族两版共用同一个 mark。 */
const KEY_TO_BRAND: Record<string, string> = {
  "claude-code": "claude",
  "claude-desktop": "claude",
  codex: "codex",
  "chatgpt-desktop": "openai",
  "gemini-cli": "gemini",
  codebuddy: "codebuddy",
  "codebuddy-ide": "codebuddy",
  workbuddy: "workbuddy",
  "workbuddy-ai": "workbuddy",
  trae: "trae",
  "trae-cn": "trae",
  opencode: "opencode",
  openwork: "openwork",
  hermes: "hermes",
  pi: "pi",
  manus: "manus",
};

/** 内置适配器 slug → 品牌（技能库 / MCP / 指令页拿到的是这一套 id）。 */
const SLUG_TO_BRAND: Record<string, string> = {
  claude: "claude",
  codex: "codex",
  codebuddy: "codebuddy",
  hermes: "hermes",
  gemini: "gemini",
  opencode: "opencode",
};

/** 没有 agent 归属的条目（技能真身 / 中央仓库）统一走 AgentHub 品牌。 */
const STORE_IDS = new Set(["store", "agenthub", "central"]);

/**
 * 把任意形态的 agent 标识折成可比较的 key：`custom:WorkBuddy`、`Work Buddy`、`workbuddy`
 * 都折成 `workbuddy`。
 *
 * 为什么需要：自定义平台的 AgentId 用的是**用户填的 display_name** 而不是配置里的 id
 * （见 `crates/agenthub-core/src/adapters/custom.rs` 的 `AgentId::Custom(def.display_name)`），
 * 于是记录里的 agent 值长成 `custom:WorkBuddy` 这种大小写不受控的形态，
 * 直接拿它去查小写连字符形式的 `KEY_TO_BRAND` 会全部落空、退化成字母块（WO / TR）。
 */
const normalizeKey = (raw: string) => raw.toLowerCase().replace(/[^a-z0-9]+/g, "");

/** `custom:` 只是 AgentId 的命名空间前缀，不参与品牌匹配。 */
const stripCustom = (raw: string) => raw.trim().replace(/^custom:/i, "");

/** 归一化索引：slug 先放、目录 key 后放，同名冲突时以更具体的目录 key 为准。 */
const NORMALIZED_BRAND = new Map<string, string>(
  [...Object.entries(SLUG_TO_BRAND), ...Object.entries(KEY_TO_BRAND)].map(
    ([key, brand]) => [normalizeKey(key), brand] as const
  )
);

const NORMALIZED_CATALOG_SHORT = new Map<string, string>(
  Object.entries(CATALOG_SHORT).map(([key, short]) => [normalizeKey(key), short] as const)
);

function brandOf(id: string | null): string | undefined {
  if (!id) return "store";
  // 原值与去前缀后的值各试一遍：`store` 这类精确 id 不受大小写影响，
  // `custom:WorkBuddy` 则要走归一化那一层。
  for (const candidate of [id.trim(), stripCustom(id)]) {
    if (!candidate) continue;
    if (STORE_IDS.has(candidate.toLowerCase())) return "store";
    const exact = KEY_TO_BRAND[candidate] ?? SLUG_TO_BRAND[candidate];
    if (exact) return exact;
    const normalized = NORMALIZED_BRAND.get(normalizeKey(candidate));
    if (normalized) return normalized;
  }
  return undefined;
}

/// 从 `agentInfo().accent` 里取文字色那一项：它是 `text-x border-x/30 bg-x/10`，
/// 兜底字母只该染色，不该带底色（外层容器已经有底色）。
function toneOf(accent: string): string {
  return accent.split(" ").find((token) => token.startsWith("text-")) ?? "text-text-muted";
}

export function agentVisual(id: string | null): AgentVisual {
  const fallback = agentInfo(id);
  const brand = brandOf(id);
  const visual = brand ? BRANDS[brand] : undefined;
  return {
    img: visual?.img,
    imgDark: visual?.imgDark,
    mask: visual?.mask,
    tint: visual?.tint,
    // 目录 key 的兜底字母取 CATALOG_SHORT（WB / TR / TN…），agent slug 走 agentInfo；
    // 自定义平台的 `custom:WorkBuddy` 也要能拿到 WB 而不是 agentInfo 简写的 WO
    short:
      (id
        ? (CATALOG_SHORT[id] ??
          NORMALIZED_CATALOG_SHORT.get(normalizeKey(stripCustom(id))))
        : undefined) || fallback.short,
    tone: toneOf(fallback.accent),
  };
}
