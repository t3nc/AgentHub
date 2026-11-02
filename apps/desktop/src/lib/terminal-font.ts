import { isTauri } from "./api";
import { terminalApi } from "./terminal";

/**
 * 终端字体解析：让 xterm 既能显示 Nerd Font 图标，又能正常显示中文。
 *
 * 为什么需要这一层：xterm 只在 `term.open()` 那一刻测量字符宽度，字体没就绪
 * 会把列宽算错；而 Nerd Font 又分「系统已装」和「应用自带」两条来源，
 * 谁先谁后、什么时候能开始建 pane，都收在这里统一决定。
 *
 * 顺序：系统探测到的 Nerd Font（mono 变体优先）→ 内置 Nerd Font → 拉丁等宽 → 中文。
 * Nerd Font 不含 CJK，中文必须靠列表末尾的系统字体逐字回退。
 */

/** 必须与 index.css 里 `@font-face` 的 family 名一致 */
export const BUNDLED_NERD_FAMILY = "AgentHub Nerd Mono";

/** 内置字体与系统 Nerd Font 都没有时的等宽拉丁字体 */
const LATIN_FALLBACKS = [
  "JetBrains Mono",
  "Cascadia Mono",
  "SF Mono",
  "Menlo",
  "Consolas",
  "Liberation Mono",
];

/** 中文回退：上面那些字体都不含 CJK */
const CJK_FALLBACKS = [
  "Microsoft YaHei",
  "PingFang SC",
  "Noto Sans CJK SC",
  "Source Han Sans SC",
];

function quoted(name: string): string {
  // family 名可能带空格（"JetBrainsMono Nerd Font Mono"），必须加引号
  return `"${name.replace(/["\\]/g, "")}"`;
}

/** 纯函数，便于用眼睛核对拼装结果。 */
export function buildFontFamily(detected: readonly string[]): string {
  const parts = [
    ...detected.filter((name) => name.trim().length > 0).map(quoted),
    quoted(BUNDLED_NERD_FAMILY),
    ...LATIN_FALLBACKS.map(quoted),
    ...CJK_FALLBACKS.map(quoted),
    "monospace",
  ];
  return parts.join(", ");
}

/**
 * 等内置字体真正可用。
 *
 * 只在「系统没有 Nerd Font」时才需要它，但探测结果可能因为字体名差异而
 * 匹配失败（Windows 上同一字体有缩写与全称两套名字），所以一律等它加载完，
 * 免得 xterm 量宽度时用的是别的字体。
 */
async function ensureBundledFont(): Promise<void> {
  const fonts = typeof document !== "undefined" ? document.fonts : undefined;
  if (!fonts?.load) return;
  try {
    await fonts.load(`12px ${quoted(BUNDLED_NERD_FAMILY)}`);
  } catch {
    // 加载失败不阻塞：fontFamily 里还有后续回退，最差只是没有图标
  }
}

async function compute(): Promise<string> {
  let detected: string[] = [];
  if (isTauri) {
    try {
      detected = await terminalApi.fonts();
    } catch (err) {
      // 探测失败按「没装 Nerd Font」处理，内置字体兜底
      console.warn("字体探测失败，改用内置字体", err);
    }
  }
  await ensureBundledFont();
  return buildFontFamily(detected);
}

let pending: Promise<string> | null = null;

/** 全局只解析一次：字体列表在进程生命周期内不会变，重复 IPC 没意义。 */
export function resolveTerminalFontFamily(): Promise<string> {
  pending ??= compute();
  return pending;
}
