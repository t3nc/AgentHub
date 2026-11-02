import { Copy, Sparkles } from "lucide-react";
import { AgentIcon } from "../agents/AgentIcon";
import { useT } from "../../lib/i18n";
import type { AgentChoice } from "../../lib/terminal-agents";

export interface SelectionInfo {
  text: string;
  /** 视口坐标（fixed 定位用），由 TerminalPane 按字符网格算好 */
  x: number;
  y: number;
}

/**
 * 选中即问：终端里选中一段报错，浮条提供「复制」和「交给某个 agent 分析」。
 *
 * 位置由 pane 给出（选区末行的下一行、选区首列），这里只负责画。
 * 最多列三个 agent——浮条不是启动器，多了反而挡住终端。
 */
export function SelectionToolbar({
  selection,
  agents,
  onCopy,
  onHandoff,
}: {
  selection: SelectionInfo;
  agents: AgentChoice[];
  onCopy: () => void;
  onHandoff: (agent: AgentChoice) => void;
}) {
  const t = useT();
  const visible = agents.slice(0, 3);
  // 贴边时别让浮条被窗口裁掉：左/右留出一个胶囊的宽度，顶部避开工具栏
  const left = Math.min(Math.max(selection.x, 150), Math.max(window.innerWidth - 150, 150));
  const top = Math.min(Math.max(selection.y - 8, 56), window.innerHeight - 10);

  return (
    <div
      className="fixed z-40 flex -translate-x-1/2 -translate-y-full items-center gap-0.5 rounded-lg border border-line bg-ink-900 p-1 shadow-2xl"
      style={{ left, top }}
      // 点浮条不该清掉终端里的选区
      onMouseDown={(event) => event.preventDefault()}
    >
      <button
        type="button"
        onClick={onCopy}
        title={t("sel.copyChars", { count: selection.text.length })}
        className="flex cursor-pointer items-center gap-1 rounded px-1.5 py-0.5 text-[11.5px] text-text-muted transition-colors hover:bg-surface-hover hover:text-text-strong"
      >
        <Copy className="h-3 w-3" />
        {t("install.copy")}
      </button>

      {visible.length > 0 && <span className="mx-0.5 h-3 w-px bg-line" />}

      {visible.map((agent) => (
        <button
          key={agent.command}
          type="button"
          onClick={() => onHandoff(agent)}
          title={t("sel.sendChars", { count: selection.text.length, agent: agent.name })}
          className="flex cursor-pointer items-center gap-1 rounded px-1.5 py-0.5 text-[11.5px] text-text-muted transition-colors hover:bg-brand-500/15 hover:text-text-strong"
        >
          <Sparkles className="h-3 w-3 text-brand-400" />
          <AgentIcon id={agent.iconId} className="h-3.5 w-3.5" />
          {agent.name}
        </button>
      ))}
    </div>
  );
}
