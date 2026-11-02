import { readStoredMode, resolveMode } from "./detect";
import { translate } from "./format";
import type { MessageKey } from "./messages";
import type { Args, Locale } from "./types";

/**
 * 模块级「当前语言」。
 *
 * 给非组件代码（纯函数、模块级常量）用：它们拿不到 React context，
 * 但会在某个消费了 `useT()` 的组件里被调用，组件重渲染时自然取到新语言。
 *
 * 组件内部仍然走 `useT()` —— 那样才订阅得到切换事件。
 */
let active: Locale = resolveMode(readStoredMode());

export function currentLocale(): Locale {
  return active;
}

export function setActiveLocale(locale: Locale): void {
  active = locale;
}

/** 纯函数里的取词入口。 */
export function tr(key: MessageKey, args?: Args): string {
  return translate(active, key, args);
}
