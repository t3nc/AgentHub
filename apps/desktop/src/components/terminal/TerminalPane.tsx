import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "../../lib/api";
import { copyText } from "../../lib/clipboard";
import { tr } from "../../lib/i18n/runtime";
import { useTheme } from "../../lib/theme";
import {
  TERMINAL_EXIT_EVENT,
  TERMINAL_OUTPUT_EVENT,
  terminalApi,
  type TerminalExit,
  type TerminalOutput,
} from "../../lib/terminal";
import { terminalTheme } from "../../lib/themes";
import {
  createTerminalBundle,
  searchDecorations,
  stripAnsi,
  type TerminalBundle,
} from "../../lib/terminal-session";
import { DemoShell } from "../../lib/terminal-demo";
import type { LinkHit, LinkTarget } from "../../lib/terminal-links";

export type PaneStatus = "starting" | "running" | "exited" | "error";

/** 页面通过 ref 调用的能力：搜索条、导出、选区。 */
export interface TerminalControls {
  /** 返回是否命中 */
  search(query: string, backwards?: boolean): boolean;
  clearSearch(): void;
  /** 订阅匹配计数变化，返回取消订阅 */
  onSearchResults(listener: (index: number, count: number) => void): () => void;
  /** 导出用纯文本（已剥 ANSI） */
  dumpText(): string;
  selection(): string;
  focus(): void;
}

export interface TerminalPaneProps {
  cwd: string | null;
  shell: string | null;
  command: string | null;
  /** 会话索引用的标题与 agent 图标 id */
  title: string;
  agentId: string | null;
  /** 是否把输出落盘（用户的「记录终端输出」开关） */
  record: boolean;
  active: boolean;
  /** 由页面在字体就绪后传进来；本组件不做探测，见 lib/terminal-font.ts */
  fontFamily: string;
  onStatus: (status: PaneStatus) => void;
  /** Ctrl/Cmd+F：页面据此打开搜索条 */
  onRequestSearch?: () => void;
  /** 把一行文本解析成可点击片段（归属判断在 lib/terminal-links.ts） */
  resolveLinks?: (line: string) => LinkHit[];
  onLinkActivate?: (target: LinkTarget) => void;
  /** 用户敲下的整行（回车时回调），用于识别 shell 里直接起的 agent CLI */
  onCommandLine?: (line: string) => void;
  /** 选区变化：给了文本与视口坐标，null 表示选区被清掉 */
  onSelectionChange?: (selection: { text: string; x: number; y: number } | null) => void;
}

function newSessionId(): string {
  const uuid =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  return `term-${uuid}`;
}

function errorText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/**
 * 一个终端标签 = 一个 xterm 实例 + 一个 PTY 会话。
 *
 * 只在挂载时建会话（`cwd` / `command` 变了不重开，否则改个输入框就会把
 * 正在跑的 agent 顶掉），卸载即关闭；标签切换只做 fit + focus。
 */
export const TerminalPane = forwardRef<TerminalControls, TerminalPaneProps>(function TerminalPane(
  {
    cwd,
    shell,
    command,
    title,
    agentId,
    record,
    active,
    fontFamily,
    onStatus,
    onRequestSearch,
    resolveLinks,
    onLinkActivate,
    onCommandLine,
    onSelectionChange,
  },
  ref
) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const bundleRef = useRef<TerminalBundle | null>(null);
  // 主题里的 `active` 和标签页的 `active` 重名，这里只取配色相关的部分
  const { active: activeTheme } = useTheme();
  // 搜索高亮的回调在 useImperativeHandle 里定死，靠 ref 拿到最新强调色
  const accentRef = useRef(activeTheme.swatch.accent);

  // 回调走 ref：会话只在挂载时建一次，回调却每次渲染都可能变
  const resolveLinksRef = useRef(resolveLinks);
  const activateLinkRef = useRef(onLinkActivate);
  const commandLineRef = useRef(onCommandLine);
  const selectionRef = useRef(onSelectionChange);
  useEffect(() => {
    resolveLinksRef.current = resolveLinks;
    activateLinkRef.current = onLinkActivate;
    commandLineRef.current = onCommandLine;
    selectionRef.current = onSelectionChange;
  }, [resolveLinks, onLinkActivate, onCommandLine, onSelectionChange]);

  useImperativeHandle(
    ref,
    () => ({
      search(query, backwards = false) {
        const bundle = bundleRef.current;
        if (!bundle || !query) return false;
        const options = { decorations: searchDecorations(accentRef.current), incremental: true };
        return backwards
          ? bundle.search.findPrevious(query, options)
          : bundle.search.findNext(query, options);
      },
      clearSearch() {
        bundleRef.current?.search.clearDecorations();
      },
      onSearchResults(listener) {
        const bundle = bundleRef.current;
        if (!bundle) return () => {};
        const disposable = bundle.search.onDidChangeResults(({ resultIndex, resultCount }) => {
          listener(resultIndex, resultCount);
        });
        return () => disposable.dispose();
      },
      dumpText() {
        const bundle = bundleRef.current;
        if (!bundle) return "";
        return stripAnsi(bundle.serialize.serialize({ excludeModes: true }));
      },
      selection() {
        return bundleRef.current?.term.getSelection() ?? "";
      },
      focus() {
        bundleRef.current?.term.focus();
      },
    }),
    []
  );

  // 建会话：listener 必须先于 open 注册，否则 shell 的 banner 会丢
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    // xterm / addon 初始化失败（坏字体、WebGL 驱动、插件版本不符）不该让整页白屏：
    // 就地显示原因，用户还能切别的标签继续用
    let bundle: TerminalBundle;
    try {
      bundle = createTerminalBundle(host, {
        fontFamily,
        theme: terminalTheme(activeTheme),
      });
    } catch (err) {
      console.error("终端初始化失败", err);
      host.className = "flex h-full w-full items-center justify-center p-4";
      const message = document.createElement("p");
      message.className = "max-w-[520px] text-center text-[11.5px] leading-relaxed text-state-danger";
      message.textContent = tr("term.initFailed", { error: errorText(err) });
      host.appendChild(message);
      onStatus("error");
      return;
    }
    bundleRef.current = bundle;
    const { term } = bundle;
    // 开发期调试钩子：浏览器里能直接调 term.selectAll() 验证选区浮条等行为。
    // 生产构建里 import.meta.env.DEV 是字面量 false，整块会被摇掉。
    if (import.meta.env.DEV) {
      (host as HTMLElement & { __term?: unknown }).__term = term;
    }

    term.attachCustomKeyEventHandler((event) => {
      if (event.type !== "keydown") return true;
      const mod = event.ctrlKey || event.metaKey;
      if (!mod) return true;
      const key = event.key.toLowerCase();
      // Ctrl+F 交给页面开搜索条，别让它当 forward-char 打进 shell
      if (key === "f" && !event.shiftKey && !event.altKey) {
        onRequestSearch?.();
        return false;
      }
      // Ctrl+Shift+C 复制选区；没有选区就放行（Ctrl+C 仍然是中断信号）
      if (key === "c" && event.shiftKey) {
        const selected = term.getSelection();
        if (!selected) return true;
        void copyText(selected);
        return false;
      }
      return true;
    });

    // 输出里的路径 / 批次号变成可点链接。归属判断由页面注入：
    // 只认受管范围（技能目录、项目根、MCP 配置），别的路径不给链接。
    term.registerLinkProvider({
      provideLinks(bufferLineNumber, callback) {
        const resolve = resolveLinksRef.current;
        if (!resolve) return callback(undefined);
        const line = term.buffer.active.getLine(bufferLineNumber - 1);
        if (!line) return callback(undefined);
        const text = line.translateToString(true);
        const hits = resolve(text);
        if (hits.length === 0) return callback(undefined);
        callback(
          hits.map((hit) => ({
            range: {
              start: { x: hit.start + 1, y: bufferLineNumber },
              end: { x: hit.start + hit.length, y: bufferLineNumber },
            },
            text: text.slice(hit.start, hit.start + hit.length),
            decorations: { underline: true, pointerCursor: true },
            activate: () => {
              activateLinkRef.current?.(hit.target);
            },
          }))
        );
      },
    });

    const sessionId = newSessionId();
    let disposed = false;
    let demo: DemoShell | null = null;
    const unlisteners: Array<() => void> = [];

    const fitNow = () => {
      const el = hostRef.current;
      if (!el || el.clientWidth < 8 || el.clientHeight < 8) return;
      try {
        bundle.fit.fit();
      } catch {
        // 容器还没完成布局时 fit 会抛，下一帧 ResizeObserver 会再来一次
      }
    };

    const frame = window.requestAnimationFrame(fitNow);
    const observer = new ResizeObserver(fitNow);
    observer.observe(host);

    // 行缓冲：只为认出「用户敲的是不是某个 agent CLI」，不参与 PTY 传输。
    // 退格、Ctrl+C / Ctrl+U 要同步，否则 `claX<BS>ude` 会认不出来。
    let typed = "";
    const trackTyped = (data: string) => {
      // onData 里混着 xterm 自己的应答（DSR 的 `ESC[1;1R`、焦点事件 `ESC[I`），
      // 先剥掉转义序列，否则识别 CLI 时会得到 `[1;1R[Iclaude` 这种垃圾
      for (const ch of stripAnsi(data)) {
        if (ch === "\r" || ch === "\n") {
          const line = typed.trim();
          typed = "";
          if (line) commandLineRef.current?.(line);
          continue;
        }
        if (ch === "\u007f" || ch === "\b") {
          typed = typed.slice(0, -1);
          continue;
        }
        if (ch === "\u0003" || ch === "\u0015") {
          typed = "";
          continue;
        }
        if (ch < " ") continue;
        if (typed.length < 200) typed += ch;
      }
    };

    term.onData((data) => {
      trackTyped(data);
      if (demo) {
        demo.handle(data);
        return;
      }
      void terminalApi
        .write(sessionId, data)
        .catch((err) => console.warn("写入终端失败", err));
    });

    // 选区浮条的定位：用字符网格换算成视口坐标，比追鼠标位置稳
    term.onSelectionChange(() => {
      const callback = selectionRef.current;
      if (!callback) return;
      const text = term.getSelection();
      if (!text.trim()) {
        callback(null);
        return;
      }
      const position = term.getSelectionPosition();
      const screen = host.querySelector(".xterm-screen");
      if (!position || !screen) {
        callback(null);
        return;
      }
      const rect = screen.getBoundingClientRect();
      const cellW = rect.width / Math.max(term.cols, 1);
      const cellH = rect.height / Math.max(term.rows, 1);
      // getSelectionPosition 给的是 1-based 的 buffer 坐标，先换算到视口行
      const viewportY = term.buffer.active.viewportY;
      const endRow = position.end.y - 1 - viewportY;
      const row = Math.min(Math.max(endRow + 1, 1), term.rows);
      const col = Math.min(Math.max(position.start.x - 1, 0), term.cols - 1);
      callback({
        text,
        x: rect.left + col * cellW + cellW / 2,
        y: rect.top + row * cellH,
      });
    });

    term.onResize(({ cols, rows }) => {
      if (demo || !isTauri) return;
      void terminalApi.resize(sessionId, cols, rows).catch(() => {
        // 会话已关（标签正在卸载）时 resize 必然失败，不用打扰用户
      });
    });

    const bootstrap = async () => {
      if (!isTauri) {
        demo = new DemoShell(term);
        onStatus("exited");
        return;
      }
      // 「open 还没返回进程就退出了」是真的会发生（命令不存在、shell 立刻崩），
      // 用这个标记压掉随后到达的 running，别把死会话标成绿灯
      let finished = false;
      try {
        const [offOutput, offExit] = await Promise.all([
          listen<TerminalOutput>(TERMINAL_OUTPUT_EVENT, (event) => {
            if (event.payload?.id !== sessionId) return;
            term.write(event.payload.data);
          }),
          listen<TerminalExit>(TERMINAL_EXIT_EVENT, (event) => {
            if (event.payload?.id !== sessionId) return;
            finished = true;
            term.write(`\r\n\x1b[38;2;126;120;111m${tr("term.exited")}\x1b[0m\r\n`);
            onStatus("exited");
          }),
        ]);
        if (disposed) {
          offOutput();
          offExit();
          return;
        }
        unlisteners.push(offOutput, offExit);

        const info = await terminalApi.info();
        if (disposed) return;
        await terminalApi.open({
          id: sessionId,
          cwd,
          shell: shell ?? info.shell,
          cols: term.cols,
          rows: term.rows,
          command,
          title,
          agent: agentId,
          record,
        });
        if (!finished) onStatus("running");
      } catch (err) {
        if (disposed) return;
        term.write(
          `\r\n\x1b[31m${tr("term.startFailed", { error: errorText(err) })}\x1b[0m\r\n`
        );
        onStatus("error");
      }
    };

    void bootstrap();

    return () => {
      disposed = true;
      window.cancelAnimationFrame(frame);
      observer.disconnect();
      for (const off of unlisteners) off();
      void terminalApi.close(sessionId).catch(() => {});
      bundle.dispose();
      bundleRef.current = null;
    };
    // 只在挂载时建一次：cwd / command 是启动参数，变化不应该重建会话
  }, []);

  // 主题切换：xterm 的配色不读 CSS 变量，得手动换（只换配色，不重建会话）
  useEffect(() => {
    accentRef.current = activeTheme.swatch.accent;
    if (bundleRef.current) {
      bundleRef.current.term.options.theme = terminalTheme(activeTheme);
    }
  }, [activeTheme]);

  // 切到这个标签：容器刚从 display:none 恢复，必须重新 fit 拿真实尺寸
  useEffect(() => {
    if (!active) return;
    const frame = window.requestAnimationFrame(() => {
      const el = hostRef.current;
      if (el && el.clientWidth > 8 && el.clientHeight > 8) {
        try {
          bundleRef.current?.fit.fit();
        } catch {
          /* 忽略：下一帧还有 ResizeObserver */
        }
      }
      bundleRef.current?.term.focus();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [active]);

  return <div ref={hostRef} className="h-full w-full overflow-hidden" />;
});
