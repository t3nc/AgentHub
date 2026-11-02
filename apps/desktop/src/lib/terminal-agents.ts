import { tr } from "./i18n/runtime";

/**
 * 终端里的 agent 识别与「交给 agent」的提示词构造。
 *
 * 识别是**纯启发式**：只看用户敲的那一行首词，不去探测进程树
 * （那要引 sysinfo，还得处理 ConPTY 里的中间进程，收益不抵成本）。
 * 从「启动 CLI」按钮起的标签本来就知道自己是哪个 agent，
 * 这里补的是「用户自己在 shell 里敲 claude」的情况。
 */

export interface AgentChoice {
  /** catalog 里的图标 id（builtin ?? key） */
  iconId: string;
  name: string;
  /** 可执行命令名，如 `claude` */
  command: string;
}

/** 取一行命令的首词，去掉提示符前缀、引号与 `sudo`/`&` 之类 */
export function firstWord(line: string): string {
  const trimmed = line.trim().replace(/^[>$#%]\s*/, "");
  const match = trimmed.match(/^("[^"]+"|'[^']+'|\S+)/);
  return match ? match[1] : "";
}

/** `C:\Users\x\AppData\...\claude.cmd` → `claude` */
export function normalizeCommand(word: string): string {
  const unquoted = word.replace(/^["']|["']$/g, "");
  const base = unquoted.split(/[\\/]/).pop() ?? unquoted;
  return base.replace(/\.(exe|cmd|bat|ps1|sh)$/i, "").toLowerCase();
}

/** 首词命中哪个 agent；没命中返回 null */
export function matchAgent(line: string, agents: AgentChoice[]): AgentChoice | null {
  const command = normalizeCommand(firstWord(line));
  if (!command) return null;
  return agents.find((agent) => normalizeCommand(agent.command) === command) ?? null;
}

/**
 * 「把这段输出交给 agent 看」的提示词。
 *
 * 选中文本不进命令行：长文本会被 shell 截断，还牵扯转义；
 * 先落成文件，让 agent 自己去读。
 */
export function handoffPrompt(filePath: string): string {
  return tr("terminal.handoffPrompt", { path: filePath });
}
