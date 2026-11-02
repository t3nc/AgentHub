import { useEffect } from "react";
import { IS_MAC } from "./commands";

export interface Hotkey {
  /** `mod+k` / `mod+shift+r` / `esc` / `?` */
  combo: string;
  run: () => void;
  /** 输入框内是否也响应（默认 false，Esc 始终响应） */
  allowInInput?: boolean;
}

/** 焦点是否在可编辑控件里：快捷键与右键菜单共用同一套判定。 */
export function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    target.isContentEditable
  );
}

function matches(combo: string, event: KeyboardEvent): boolean {
  if (event.isComposing) return false;
  if (combo === "esc") return event.key === "Escape";
  if (combo === "?") {
    return event.key === "?" && !event.metaKey && !event.ctrlKey && !event.altKey;
  }

  const parts = combo.toLowerCase().split("+");
  const key = parts[parts.length - 1];
  const wantMod = parts.includes("mod");
  const wantShift = parts.includes("shift");
  const wantAlt = parts.includes("alt");

  // mod = ⌘（Mac）/ Ctrl（其它），不会两个都认，避免 Ctrl+⌘K 也触发
  const modPressed = IS_MAC ? event.metaKey : event.ctrlKey;
  if (wantMod !== modPressed) return false;
  if (wantShift !== event.shiftKey) return false;
  if (wantAlt !== event.altKey) return false;
  if (IS_MAC && !wantMod && event.ctrlKey) return false;

  return event.key.toLowerCase() === key;
}

/**
 * 全局快捷键。刻意只挂一个 window 监听：多条独立监听容易出现
 * 「谁先 preventDefault」的竞态，也让调试变难。
 */
export function useHotkeys(hotkeys: Hotkey[]) {
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      const editable = isEditable(event.target);

      for (const hotkey of hotkeys) {
        if (!matches(hotkey.combo, event)) continue;
        // 打字时只放行 Esc，避免「在搜索框里敲 k 就跳页」
        if (editable && !hotkey.allowInInput && hotkey.combo !== "esc") continue;
        event.preventDefault();
        hotkey.run();
        return;
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [hotkeys]);
}
