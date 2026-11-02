import type { CSSProperties } from "react";

/**
 * 单色 SVG 图标的着色样式：用 CSS mask 取形状，再用 `backgroundColor` 上色。
 *
 * 为什么必须给 `url()` 加引号（别再「顺手」去掉）：
 * 生产构建会把小于 `build.assetsInlineLimit`（Vite 默认 4096B）的 SVG 内联成
 * `data:image/svg+xml,...`，Vite 会把 SVG 属性里的双引号转义成**单引号**留在 data URI 里。
 * CSS 规范不允许未加引号的 `url()` 出现引号字符，整条 `mask-image` 声明会被判定非法并丢弃，
 * 元素就只剩 `background-color` —— 表现为一坨实心色块。开发模式下资源是真实路径、没有引号，
 * 所以这个坑**只在生产包/打包后的应用里出现**，只在 dev 上验证是发现不了的。
 *
 * @param src 资源 URL（真实路径或 data URI 都可以）
 * @param tint mask 的填充色；缺省 `currentColor`，即跟随容器文字色（AgentHub 标志依赖此行为）
 */
export function maskStyle(src: string, tint?: string): CSSProperties {
  const image = `url("${src}")`;
  return {
    backgroundColor: tint ?? "currentColor",
    maskImage: image,
    WebkitMaskImage: image,
    maskRepeat: "no-repeat",
    WebkitMaskRepeat: "no-repeat",
    maskPosition: "center",
    WebkitMaskPosition: "center",
    maskSize: "contain",
    WebkitMaskSize: "contain",
  };
}
