import { isLocaleMode, type Locale, type LocaleMode } from "./types";

/** localStorage 里的镜像键：只为首帧渲染服务，权威值在 `config.json`。 */
export const LOCALE_STORAGE_KEY = "agenthub-locale";

/**
 * 把系统语言标签映射到三种界面语言之一。
 *
 * - `zh-TW` / `zh-HK` / `zh-MO` / `zh-Hant-*` → 繁体（台湾正体）；
 * - `zh-CN` / `zh-SG` / `zh-Hans-*` / 裸 `zh` → 简体；
 * - 其余一律英文（含 `zh` 系列里认不出来的变体，英文是更安全的默认）。
 *
 * 注意：`index.html` 的首帧内联脚本里抄了同一套判断，改这里要同步改那边。
 */
export function resolveTag(tag: string | undefined | null): Locale | null {
  if (!tag) return null;
  const normalized = tag.replace(/_/g, "-").toLowerCase();
  if (normalized.startsWith("zh")) {
    if (
      normalized.includes("-tw") ||
      normalized.includes("-hk") ||
      normalized.includes("-mo") ||
      normalized.includes("hant")
    ) {
      return "zh-Hant";
    }
    return "zh-Hans";
  }
  if (normalized.startsWith("en")) return "en";
  return null;
}

/** 从 `navigator.languages` 里挑第一个认得的标签。 */
export function detectSystemLocale(): Locale {
  if (typeof navigator === "undefined") return "en";
  const candidates = [
    ...(Array.isArray(navigator.languages) ? navigator.languages : []),
    navigator.language,
  ];
  for (const candidate of candidates) {
    const hit = resolveTag(candidate);
    if (hit) return hit;
  }
  return "en";
}

/** 读用户选择；非法值当作「跟随系统」。 */
export function readStoredMode(): LocaleMode {
  try {
    const raw = window.localStorage.getItem(LOCALE_STORAGE_KEY);
    // 老版本把三种语言写成小写，与后端 config.json 的取值保持一致
    if (raw === "zh-hans") return "zh-Hans";
    if (raw === "zh-hant") return "zh-Hant";
    if (isLocaleMode(raw)) return raw;
  } catch {
    // localStorage 被禁用（隐私模式）时静默回落系统语言
  }
  return "system";
}

export function storeMode(mode: LocaleMode): void {
  try {
    window.localStorage.setItem(LOCALE_STORAGE_KEY, mode);
  } catch {
    // 存不下不影响本次会话，下次启动重新检测而已
  }
}

/** 用户选择 → 实际生效语言。 */
export function resolveMode(mode: LocaleMode): Locale {
  return mode === "system" ? detectSystemLocale() : mode;
}
