import type { Terminal } from "@xterm/xterm";

/**
 * 演示模式下的假 shell。
 *
 * 浏览器里没有 PTY，但 README 承诺「演示模式能走通所有交互」，所以这里保留
 * 一个最小的行编辑器：能回显、能退格、能清屏，认得几个无害命令，其余一律
 * 说明「演示模式不执行真实进程」。它不是终端模拟器，只是让页面不变成死面板。
 */
const PROMPT = "\x1b[38;2;45;212;191magenthub\x1b[0m \x1b[38;2;126;120;111m演示模式\x1b[0m ❯ ";

const HELP = [
  "可用命令（演示模式）：",
  "  help          显示这段说明",
  "  clear         清屏",
  "  echo <text>   原样输出",
  "  agents        列出可启动的 agent CLI",
  "  ls            列出示例目录",
  "",
  "真实终端请在桌面端（Tauri 窗口）里打开本页。",
].join("\r\n");

const AGENTS = [
  "claude    Claude Code",
  "codex     Codex CLI",
  "codebuddy CodeBuddy",
  "gemini    Gemini CLI",
  "opencode  OpenCode",
].join("\r\n");

export class DemoShell {
  private buffer = "";

  constructor(private readonly term: Terminal) {
    term.write(
      "\x1b[38;2;126;120;111mAgentHub 演示终端 · 没有 PTY，只做输入回显\x1b[0m\r\n" +
        // 这一行是字体自检：能看见图标说明 Nerd Font（系统或内置）生效了
        "\x1b[38;2;126;120;111m图标自检：\x1b[0m" +
        "\x1b[38;2;45;212;191m\uE0B0 \uE0B2 \uE718 \uF07B \uF015 \uF121\x1b[0m" +
        "\x1b[38;2;126;120;111m  （方块 = 字体没加载上）\x1b[0m\r\n" +
        "\x1b[38;2;126;120;111m输入 help 看可用命令。\x1b[0m\r\n\r\n"
    );
    this.prompt();
  }

  private prompt() {
    this.term.write(PROMPT);
  }

  private line(text: string) {
    this.term.write(`${text}\r\n`);
  }

  handle(data: string) {
    for (const ch of data) {
      if (ch === "\r" || ch === "\n") {
        this.term.write("\r\n");
        this.run(this.buffer.trim());
        this.buffer = "";
        this.prompt();
        continue;
      }
      if (ch === "\u0003") {
        // Ctrl+C
        this.term.write("^C\r\n");
        this.buffer = "";
        this.prompt();
        continue;
      }
      if (ch === "\u000c") {
        this.term.clear();
        this.prompt();
        continue;
      }
      if (ch === "\u007f" || ch === "\b") {
        if (this.buffer.length > 0) {
          this.buffer = this.buffer.slice(0, -1);
          this.term.write("\b \b");
        }
        continue;
      }
      if (ch < " ") continue;
      this.buffer += ch;
      this.term.write(ch);
    }
  }

  private run(line: string) {
    if (!line) return;
    const [name, ...rest] = line.split(/\s+/);
    const arg = rest.join(" ");
    switch (name.toLowerCase()) {
      case "help":
        this.line(HELP);
        return;
      case "clear":
      case "cls":
        this.term.clear();
        return;
      case "echo":
        this.line(arg);
        return;
      case "agents":
        this.line(AGENTS);
        return;
      case "ls":
        this.line("skills/   mcp/   AGENTS.md   README.md");
        return;
      case "whoami":
        this.line("demo-user");
        return;
      case "exit":
        this.line("演示模式不会真的退出");
        return;
      default:
        this.line(
          `\x1b[38;2;232;141;86m${name}\x1b[0m：演示模式不执行真实命令（桌面端可用）`
        );
    }
  }
}
