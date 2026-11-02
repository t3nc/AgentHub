import { dictionary, type MessageKey } from "./messages";
import { bcp47, type Args, type Locale } from "./types";

const PLACEHOLDER = /\{(\w+)\}/g;

/** 缺失 key 只告警一次，避免切换语言时把一个 key 刷满控制台。 */
const warned = new Set<string>();

/**
 * `{name}` 占位符替换。
 *
 * 参数里没有的占位符原样保留（模板里真的写了 `{` 也不会被吃掉），
 * 这样拼错参数名时界面上能直接看见，而不是静默变成空字符串。
 */
export function interpolate(template: string, args?: Args): string {
  if (!args) return template;
  return template.replace(PLACEHOLDER, (raw, name: string) =>
    Object.prototype.hasOwnProperty.call(args, name) ? String(args[name]) : raw
  );
}

/**
 * 取文案。英文单复数只在显式提供了 `key#one` 变体、且 `count === 1` 时切换，
 * 不做完整 ICU —— 需要复数的条目本来就只有个位数。
 */
export function translate(locale: Locale, key: MessageKey, args?: Args): string {
  const dict = dictionary(locale) as Record<string, string | undefined>;
  const one = args && args.count === 1 ? dict[`${key}#one`] : undefined;
  const template = one ?? dict[key];
  if (template === undefined) {
    if (!warned.has(key)) {
      warned.add(key);
      console.warn(`[i18n] 缺少文案：${key}（${locale}）`);
    }
    return key;
  }
  return interpolate(template, args);
}

/** 该语言的 BCP-47 标签，给 `Intl` 系列 API 与 `<html lang>` 用。 */
export function localeTag(locale: Locale): string {
  return bcp47(locale);
}
