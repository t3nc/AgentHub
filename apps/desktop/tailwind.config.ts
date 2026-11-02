import type { Config } from "tailwindcss";
import animate from "tailwindcss-animate";

/**
 * 颜色一律映射到 CSS 变量（定义在 src/index.css 的 :root[data-theme="..."]），
 * 同一套 class 在 10 套内置主题下都成立，换肤只换 html[data-theme]，不用维护多份组件样式。
 * `.dark` 只用来触发下面的 dark: 变体，不承载色值。
 */
const v = (name: string) => `rgb(var(${name}) / <alpha-value>)`;

export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        ink: {
          950: v("--bg"),
          900: v("--bg-elev"),
          850: v("--bg-elev-2"),
          800: v("--bg-elev-2"),
          700: v("--bg-sunken"),
          600: v("--line-strong"),
        },
        line: {
          DEFAULT: v("--line"),
          strong: v("--line-strong"),
        },
        surface: {
          2: v("--surface-2"),
          hover: v("--surface-hover"),
          active: v("--surface-active"),
        },
        brand: {
          500: v("--brand-500"),
          400: v("--brand-400"),
          300: v("--brand-300"),
          // 实心 brand 底上的文字色：亮橙/亮蓝主题用深字，深底主题用白字
          on: v("--on-brand"),
        },
        brass: v("--brass"),
        nav: {
          bg: v("--nav-bg"),
          "bg-2": v("--nav-bg-2"),
          fg: v("--nav-fg"),
          dim: v("--nav-dim"),
          active: v("--nav-active"),
          line: v("--nav-line"),
          accent: v("--nav-accent"),
        },
        canvas: {
          grid: v("--grid-line"),
          node: v("--node-bg"),
        },
        cyan: { 400: "#22D3EE" },
        text: {
          strong: v("--fg-strong"),
          muted: v("--fg-muted"),
          dim: v("--fg-dim"),
        },
        state: {
          ok: v("--ok"),
          warn: v("--warn"),
          danger: v("--danger"),
          info: v("--info"),
        },
      },
      fontFamily: {
        sans: [
          "PingFang SC",
          "-apple-system",
          "BlinkMacSystemFont",
          "Segoe UI",
          "Microsoft YaHei",
          "sans-serif",
        ],
        mono: [
          "SF Mono",
          "JetBrains Mono",
          "Menlo",
          "Consolas",
          "Liberation Mono",
          "monospace",
        ],
      },
      boxShadow: {
        glow: "var(--shadow-glow)",
        panel: "var(--shadow-panel)",
      },
      keyframes: {
        "fade-up": {
          "0%": { opacity: "0", transform: "translateY(8px)" },
          // 收尾必须是 `none` 而不是 `translateY(0)`：animation-fill-mode 是 both，
          // 动画结束后 `transform: translateY(0)` 会留在元素上，给内部的 `position: fixed`
          // 子元素造出一个包含块（底部悬浮操作条会跑到文档末尾，成百上千 px 之外）。
          "100%": { opacity: "1", transform: "none" },
        },
        "pulse-soft": {
          "0%, 100%": { opacity: "1" },
          "50%": { opacity: "0.45" },
        },
        shimmer: {
          "0%": { backgroundPosition: "-200% 0" },
          "100%": { backgroundPosition: "200% 0" },
        },
      },
      animation: {
        // 不能用 fill-mode `both`：动画落幕后 transform 仍留在元素上（哪怕值是 none，
        // Chromium 也会把它当成有 transform 的元素），从而给页面内的 `position: fixed`
        // 子元素造出包含块 —— 底部悬浮操作条就会跑到整页末尾、几百上千 px 之外。
        "fade-up": "fade-up 260ms cubic-bezier(0.22,1,0.36,1)",
        "pulse-soft": "pulse-soft 2.4s ease-in-out infinite",
        shimmer: "shimmer 2.2s linear infinite",
      },
    },
  },
  plugins: [animate],
} satisfies Config;
