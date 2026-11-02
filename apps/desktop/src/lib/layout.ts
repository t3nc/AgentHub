import { useCallback, useState } from "react";

/** 与主题一致的前缀，便于一次性清理本机偏好。 */
const PREFIX = "agenthub-";

function readFlag(key: string, fallback: boolean): boolean {
  try {
    const raw = window.localStorage.getItem(PREFIX + key);
    if (raw === "1") return true;
    if (raw === "0") return false;
  } catch {
    // 隐私模式 / 存储被禁用时按默认值走，不影响渲染
  }
  return fallback;
}

function writeFlag(key: string, value: boolean) {
  try {
    window.localStorage.setItem(PREFIX + key, value ? "1" : "0");
  } catch {
    // 忽略：偏好没存下来只影响下次启动
  }
}

/** 布尔型布局偏好（侧栏折叠、检查器开关），落 localStorage。 */
export function useLayoutFlag(
  key: string,
  fallback: boolean
): [boolean, (next: boolean) => void, () => void] {
  const [value, setValue] = useState<boolean>(() => readFlag(key, fallback));

  const set = useCallback(
    (next: boolean) => {
      setValue(next);
      writeFlag(key, next);
    },
    [key]
  );

  const toggle = useCallback(() => {
    setValue((prev) => {
      writeFlag(key, !prev);
      return !prev;
    });
  }, [key]);

  return [value, set, toggle];
}

export const NAV_COLLAPSED_KEY = "nav-collapsed";
export const INSPECTOR_OPEN_KEY = "inspector-open";

/** 通高侧栏的两种宽度：展开 224，折叠成图标栏 60。 */
export const NAV_W = 224;
export const NAV_W_COLLAPSED = 60;
