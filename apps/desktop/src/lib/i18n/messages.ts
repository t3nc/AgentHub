import { en } from "./en";
import { zhHans } from "./zh-Hans";
import { zhHant } from "./zh-Hant";
import type { Args, Locale } from "./types";

/**
 * 文案 key 的联合类型。
 *
 * 由简体中文词典推导：新增一条文案必须先写简体，另两种语言不补就编译不过。
 */
export type MessageKey = keyof typeof zhHans;

export type TFn = (key: MessageKey, args?: Args) => string;

const DICTS: Record<Locale, Record<MessageKey, string>> = {
  "zh-Hans": zhHans,
  "zh-Hant": zhHant,
  en,
};

export function dictionary(locale: Locale): Record<MessageKey, string> {
  return DICTS[locale];
}
