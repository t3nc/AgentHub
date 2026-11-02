/**
 * 界面语言相关的基础类型。
 *
 * 三种语言首发：英文 / 简体中文 / 繁体中文（台湾正体）。
 * `system` 不是一种语言，而是「跟随操作系统」这个选择本身，所以单独区分
 * `LocaleMode`（用户选了什么）与 `Locale`（实际生效哪一种）。
 */
export type LocaleMode = "system" | "en" | "zh-Hans" | "zh-Hant";

/** 实际生效的界面语言。 */
export type Locale = Exclude<LocaleMode, "system">;

export const LOCALES: Locale[] = ["zh-Hans", "zh-Hant", "en"];

export const LOCALE_MODES: LocaleMode[] = ["system", "zh-Hans", "zh-Hant", "en"];

/** 插值参数：只接受字符串与数字，避免把对象塞进文案。 */
export type Args = Record<string, string | number>;

/**
 * 文案语言标签（下拉里显示的名字）。
 *
 * 语言名一律用「它自己那种语言」写：英文用户看到 English，繁体用户看到
 * 繁體中文。只有「跟随系统」需要翻译。
 */
export const LOCALE_SELF_NAME: Record<Locale, string> = {
  "zh-Hans": "简体中文",
  "zh-Hant": "繁體中文（台灣）",
  en: "English",
};

/** 语言标签的 BCP-47 写法，用于 `Intl` / `localeCompare` / `toLocaleDateString`。 */
export function bcp47(locale: Locale): string {
  switch (locale) {
    case "zh-Hans":
      return "zh-CN";
    case "zh-Hant":
      return "zh-TW";
    default:
      return "en-US";
  }
}

export function isLocale(value: unknown): value is Locale {
  return value === "en" || value === "zh-Hans" || value === "zh-Hant";
}

export function isLocaleMode(value: unknown): value is LocaleMode {
  return value === "system" || isLocale(value);
}
