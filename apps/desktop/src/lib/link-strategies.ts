import type { MessageKey } from "./i18n/messages";
import { tr } from "./i18n/runtime";
import type { LinkStrategyName } from "./types";

/**
 * 链接策略的唯一事实来源：技能详情、工作台检查器、设置页、右键菜单共用同一份顺序与文案。
 *
 * 后端的 `LinkStrategy` 实现在 `link/strategy.rs`：`junction` 只有 Windows 分支
 * （`cfg(windows)` 调 junction crate），macOS / Linux 上 apply 阶段直接返回
 * `Unsupported`。所以 UI 必须在**选择之前**就置灰，而不是等用户点了才报错。
 */
export const LINK_STRATEGIES: { value: LinkStrategyName; label: MessageKey }[] = [
  { value: "symlink", label: "common.strategy.symlink" },
  { value: "junction", label: "common.strategy.junction" },
  { value: "hardlink", label: "common.strategy.hardlink" },
  { value: "copy", label: "common.strategy.copy" },
];

/**
 * 置灰时的悬停说明（右键菜单的 reason、select 的 title 共用）。
 *
 * 做成函数而不是常量：文案要跟着界面语言变。
 */
export function junctionReason(): string {
  return tr("common.strategy.junctionReason");
}

/**
 * 当前平台能否用「目录联接」。
 *
 * `platform` 来自后端的平台默认策略（`OverviewDto.strategies.platform` /
 * `SettingsView.platform_strategy`）：Windows = `junction`，macOS / Linux = `symlink`。
 * 拿不到（概览尚未加载 / 老后端）时返回 `true`，宁可不置灰也不误伤。
 */
export function junctionSupported(
  platform: LinkStrategyName | null | undefined
): boolean {
  if (!platform) return true;
  return platform === "junction";
}

/** `junction` 且当前平台不支持 → 该选项要置灰。 */
export function isStrategyDisabled(
  value: LinkStrategyName | "auto",
  platform: LinkStrategyName | null | undefined
): boolean {
  return value === "junction" && !junctionSupported(platform);
}
