import { isTauri } from "./api";
import { copyText } from "./clipboard";
import { tr } from "./i18n/runtime";

export interface OpenResult {
  ok: boolean;
  /** 给 toast 用的一句话 */
  message: string;
}

/**
 * 「在文件管理器中显示」/「用默认程序打开」。
 *
 * 走 `tauri-plugin-opener`（官方插件，能力在 `capabilities/default.json` 里授权）。
 * 浏览器演示模式没有系统集成，降级成复制路径——保证 `npm run dev` 里点得通、有反馈。
 */
export function revealPath(target: string): Promise<OpenResult> {
  if (!isTauri) return degrade(tr("open.demoReveal"), target);
  return import("@tauri-apps/plugin-opener")
    .then((module) => module.revealItemInDir(target))
    .then(
      () => ({ ok: true, message: tr("open.revealed") }),
      (err: unknown) => ({
        ok: false,
        message: tr("open.revealFailed", { error: err instanceof Error ? err.message : String(err) }),
      })
    );
}

export function openWithDefault(target: string): Promise<OpenResult> {
  if (!isTauri) return degrade(tr("open.demoOpen"), target);
  return import("@tauri-apps/plugin-opener")
    .then((module) => module.openPath(target))
    .then(
      () => ({ ok: true, message: tr("open.opened") }),
      (err: unknown) => ({
        ok: false,
        message: tr("open.openFailed", { error: err instanceof Error ? err.message : String(err) }),
      })
    );
}

/** 演示模式：复制路径，并在文案里说清楚「为什么没打开」 */
function degrade(reason: string, target: string): Promise<OpenResult> {
  return copyText(target).then(
    (copied) => ({
      ok: false,
      message: copied ? tr("open.copiedPath", { reason }) : reason,
    }),
    () => ({ ok: false, message: reason })
  );
}
