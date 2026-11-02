import { invoke, isTauri } from "./api";
import { tr } from "./i18n/runtime";

/** 与 src-tauri/src/terminal.rs 的事件名保持一致。 */
export const TERMINAL_OUTPUT_EVENT = "terminal://output";
export const TERMINAL_EXIT_EVENT = "terminal://exit";

export interface TerminalInfo {
  /** 默认 shell 的可执行文件 */
  shell: string;
  /** 展示用名字：PowerShell 7 / zsh … */
  shell_label: string;
  /** 新终端的默认工作目录 */
  home: string;
  platform: string;
  /** 会话日志目录（页面底部要如实告诉用户输出落在哪） */
  logs_dir?: string;
}

/** 与 crates/agenthub-core/src/terminal/history.rs 的 SessionMeta 对齐 */
export interface TerminalSession {
  id: string;
  title: string;
  command: string | null;
  cwd: string;
  agent: string | null;
  /** unix 秒 */
  started_at: number;
  status: string;
  bytes: number;
  truncated: boolean;
}

export interface TerminalOutput {
  id: string;
  data: string;
}

export interface TerminalExit {
  id: string;
}

/**
 * 演示模式（浏览器里直接打开）没有 PTY：给一套假信息，
 * 由 `DemoShell` 顶一个只读的假终端，交互不会中断。
 */
const DEMO_INFO: TerminalInfo = {
  shell: "demo-shell",
  shell_label: tr("terminal.demoShell"),
  home: "~",
  platform: "demo",
};

export interface OpenOptions {
  id: string;
  cwd: string | null;
  shell: string | null;
  cols: number;
  rows: number;
  /** 起 shell 之后自动敲的一行命令，用于直接拉起 agent CLI */
  command: string | null;
  /** 会话索引用的标题与 agent 图标 id */
  title: string;
  agent: string | null;
  /** 是否把输出落盘（用户的「记录终端输出」开关） */
  record: boolean;
}

/** 终端后端：壳层能力，不进 `api` 门面（那里全是 agenthub-core 的业务命令）。 */
export const terminalApi = {
  info(): Promise<TerminalInfo> {
    return isTauri ? invoke<TerminalInfo>("terminal_info") : Promise.resolve(DEMO_INFO);
  },
  /**
   * 壳层探测到的 Nerd Font family 名（终端最合适的在前）。
   * 浏览器里没有字体枚举能力，演示模式返回空数组，交给内置字体兜底。
   */
  fonts(): Promise<string[]> {
    return isTauri ? invoke<string[]>("terminal_fonts") : Promise.resolve([]);
  },
  open(options: OpenOptions): Promise<void> {
    return isTauri ? invoke<void>("terminal_open", { request: options }) : Promise.resolve();
  },
  write(id: string, data: string): Promise<void> {
    return isTauri ? invoke<void>("terminal_write", { id, data }) : Promise.resolve();
  },
  resize(id: string, cols: number, rows: number): Promise<void> {
    return isTauri ? invoke<void>("terminal_resize", { id, cols, rows }) : Promise.resolve();
  },
  close(id: string): Promise<void> {
    return isTauri ? invoke<void>("terminal_close", { id }) : Promise.resolve();
  },
  /** 导出当前标签的纯文本输出，返回落盘路径 */
  exportText(text: string, title: string): Promise<string> {
    if (!isTauri) return Promise.reject(new Error("演示模式不能导出"));
    return invoke<string>("terminal_export", { text, title });
  },
  /** 上次会话（新的在前）。演示模式给空列表，页面自然不显示历史区。 */
  history(): Promise<TerminalSession[]> {
    return isTauri ? invoke<TerminalSession[]>("terminal_history") : Promise.resolve([]);
  },
  /** 选区落成 md 文件，返回路径；「交给 agent 分析」用它当输入 */
  saveSelection(text: string): Promise<string> {
    if (!isTauri) return Promise.reject(new Error("演示模式不能保存选区"));
    return invoke<string>("terminal_save_selection", { text });
  },
};

/*
 * 字体不在这里写死：Nerd Font 是否可用要问壳层（见 lib/terminal-font.ts），
 * 那份逻辑同时负责内置字体的加载时序。
 *
 * 终端配色也不在这里：10 套主题各带自己的 xterm 色板（见 lib/themes/*），
 * 这里只留 IPC 与输出清洗。
 */
