/**
 * 终端本地偏好。
 *
 * 只放「本机才有意义」的开关，所以不进 `config.json`（那里是会随仓库/备份一起走的
 * 项目配置，改它要同步 SettingsInput/SettingsView/build_config 白名单与回归测试）。
 */

const RECORD_KEY = "agenthub-terminal-record";

/** 默认记录：长任务输出不丢是终端的主要价值，隐私边界靠 UI 明示 + 可关闭。 */
export function readRecordEnabled(): boolean {
  try {
    return window.localStorage.getItem(RECORD_KEY) !== "off";
  } catch {
    return true;
  }
}

export function writeRecordEnabled(enabled: boolean): void {
  try {
    window.localStorage.setItem(RECORD_KEY, enabled ? "on" : "off");
  } catch {
    // 隐私模式存不进也没关系，本次会话内仍然按 state 生效
  }
}

/** 终端页与设置页之间的变更通知（同窗口内跨组件状态同步） */
export const RECORD_CHANGE_EVENT = "agenthub:terminal-record";

export function announceRecordChange(enabled: boolean): void {
  window.dispatchEvent(new CustomEvent(RECORD_CHANGE_EVENT, { detail: enabled }));
}
