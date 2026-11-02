import type { ITheme } from "@xterm/xterm";
import { Terminal as Xterm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { SerializeAddon } from "@xterm/addon-serialize";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";

/**
 * 一个 xterm 实例和它的全套 addon。
 *
 * 抽出来单独放，是因为 TerminalPane 要管 PTY 会话、事件、焦点、主题，
 * 再塞四个 addon 的加载顺序与回退逻辑就会超出一屏。
 */

export interface TerminalBundle {
  term: Xterm;
  fit: FitAddon;
  search: SearchAddon;
  serialize: SerializeAddon;
  /** 彻底销毁（含 WebGL 的纹理与事件） */
  dispose: () => void;
}

/**
 * WebGL 一次失败就全局不再尝试。
 *
 * 远程桌面、旧显卡、被并行占满的 GPU 进程都会让上下文创建失败；
 * 每个标签都试一遍只会让开关标签变卡，还会刷一屏控制台错误。
 */
let webglBlocked = false;

export interface BundleOptions {
  fontFamily: string;
  theme: ITheme;
}

function loadWebgl(term: Xterm): WebglAddon | null {
  if (webglBlocked) return null;
  try {
    const webgl = new WebglAddon();
    // 标签长时间隐藏在 display:none 里，上下文丢失是真实会发生的
    webgl.onContextLoss(() => {
      webglBlocked = true;
      try {
        webgl.dispose();
      } catch {
        // 已经 dispose 过就算了
      }
    });
    term.loadAddon(webgl);
    return webgl;
  } catch (err) {
    webglBlocked = true;
    console.warn("WebGL 渲染不可用，回退默认渲染器", err);
    return null;
  }
}

export function createTerminalBundle(host: HTMLElement, options: BundleOptions): TerminalBundle {
  const term = new Xterm({
    fontFamily: options.fontFamily,
    fontSize: 12.5,
    lineHeight: 1.3,
    letterSpacing: 0,
    cursorBlink: true,
    cursorStyle: "bar",
    scrollback: 8000,
    convertEol: false,
    macOptionIsMeta: true,
    theme: options.theme,
    allowTransparency: false,
    // @xterm/addon-unicode11 走的是 proposed API（字符宽度表），不开这个开关
    // 设置 activeVersion 会直接抛错——整个终端都建不起来
    allowProposedApi: true,
  });

  // Unicode 11 的宽度表必须在首次渲染前生效，否则中文/emoji 首屏就按旧规则排
  const unicode = new Unicode11Addon();
  term.loadAddon(unicode);
  term.unicode.activeVersion = "11";

  const fit = new FitAddon();
  term.loadAddon(fit);
  term.loadAddon(new WebLinksAddon());

  term.open(host);

  const webgl = loadWebgl(term);
  const search = new SearchAddon();
  term.loadAddon(search);
  const serialize = new SerializeAddon();
  term.loadAddon(serialize);

  return {
    term,
    fit,
    search,
    serialize,
    dispose: () => {
      try {
        webgl?.dispose();
      } catch {
        // 上下文丢失后可能已经释放
      }
      term.dispose();
    },
  };
}

/** #RRGGBB + alpha → #RRGGBBAA（xterm 的 decorations 只吃 8 位 hex） */
function withAlpha(hex: string, alpha: number): string {
  const step = Math.round(Math.min(1, Math.max(0, alpha)) * 255)
    .toString(16)
    .padStart(2, "0");
  return `${hex}${step}`;
}

/**
 * 搜索高亮的配色：跟随当前主题的强调色，半透明底不糊字。
 * 当前命中用更高不透明度区分，避免和各主题的次级强调色撞车。
 */
export function searchDecorations(accent: string) {
  return {
    matchBackground: withAlpha(accent, 0.26),
    matchOverviewRuler: accent,
    activeMatchBackground: withAlpha(accent, 0.48),
    activeMatchOverviewRuler: accent,
    activeMatchColorOverviewRuler: accent,
  };
}

/**
 * 去掉 ANSI 转义，得到能直接贴进 issue 的纯文本。
 *
 * `serialize()` 的产物是「可重放」的带色文本，导出成 .txt 后全是乱码，
 * 所以导出前必须洗一遍；CSI / OSC / 两字节转义三类够覆盖终端输出。
 */
const ANSI_PATTERN =
  // eslint-disable-next-line no-control-regex
  /\u001b\[[0-?]*[ -/]*[@-~]|\u001b\][^\u0007\u001b]*(?:\u0007|\u001b\\)|\u001b[@-_]/g;

export function stripAnsi(text: string): string {
  return text.replace(ANSI_PATTERN, "");
}
