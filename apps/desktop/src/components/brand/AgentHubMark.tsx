import { clsx } from "clsx";
import markUrl from "../../assets/brand/agenthub-mark-connected.svg";
import { maskStyle } from "../../lib/mask-style";

export interface AgentHubMarkProps {
  /** 只给尺寸与文字色（`h-4 w-4 text-brand-300`）；mark 本身跟随 currentColor */
  className?: string;
  /** 悬停提示；相邻已有文字说明时可省略 */
  title?: string;
}

/**
 * AgentHub 品牌标志（连通版：六段相连矩形）。
 *
 * 源文件（`ui/icon/agenthub-mark-connected.svg`）是纯色几何图形，亮暗两版仅 `fill` 不同：
 * 用 CSS mask 只取 alpha、再用 `backgroundColor: currentColor` 上色，一个文件就能同时适配
 * 亮色/暗色与任意容器文字色 —— 不需要两套位图，也不依赖 `useTheme()`（组件可以在 Provider
 * 之外渲染而不报错）。
 */
export function AgentHubMark({ className, title }: AgentHubMarkProps) {
  return (
    <span
      role={title ? "img" : undefined}
      aria-label={title}
      aria-hidden={title ? undefined : true}
      title={title}
      className={clsx("block shrink-0", className ?? "h-4 w-4")}
      style={maskStyle(markUrl)}
    />
  );
}
