import { clsx } from "clsx";
import { agentVisual } from "../../lib/agent-visual";
import { maskStyle } from "../../lib/mask-style";

export interface AgentIconProps {
  /** 内置 agent slug（claude / codex / custom:xxx / store）或目录 key（claude-code / trae-cn …） */
  id: string | null;
  /** 只给尺寸（h-4 w-4 之类）；颜色由外层容器决定 */
  className?: string;
  /** 悬停提示；相邻已有文字名称时可省略 */
  title?: string;
}

/**
 * agent 品牌图标：彩色 mark 用 `<img>`（亮暗两版时自动切换），单色 mark 用 CSS mask 上色，
 * 只有自定义平台（`custom:xxx`）退回字母块。
 *
 * 只渲染字形本身，不加边框与底色——容器由调用方决定，避免各处布局漂移。
 */
export function AgentIcon({ id, className, title }: AgentIconProps) {
  const visual = agentVisual(id);
  const size = clsx("block shrink-0", className ?? "h-3.5 w-3.5");

  if (visual.img) {
    // 亮暗两套位图：同一位置叠两张，靠 dark: 变体切换，不引入 useTheme（组件可在 Provider 外渲染）
    if (visual.imgDark) {
      return (
        <>
          <img
            src={visual.img}
            alt=""
            aria-hidden="true"
            draggable={false}
            title={title}
            className={clsx(size, "object-contain dark:hidden")}
          />
          <img
            src={visual.imgDark}
            alt=""
            aria-hidden="true"
            draggable={false}
            title={title}
            className={clsx(size, "hidden object-contain dark:block")}
          />
        </>
      );
    }
    return (
      <img
        src={visual.img}
        alt=""
        aria-hidden="true"
        draggable={false}
        title={title}
        className={clsx(size, "object-contain")}
      />
    );
  }

  if (visual.mask) {
    // 单色 SVG 根节点是 fill="currentColor"，当 <img> 用会渲染成纯黑；
    // 用 mask + backgroundColor 才能真正着色（引号细节见 lib/mask-style.ts）。
    return (
      <span
        aria-hidden="true"
        title={title}
        className={size}
        style={maskStyle(visual.mask, visual.tint)}
      />
    );
  }

  return (
    <span
      aria-hidden="true"
      title={title}
      className={clsx("font-mono leading-none", visual.tone, className ?? "text-[11px]")}
    >
      {visual.short}
    </span>
  );
}
