import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { clsx } from "clsx";
import { Download, FolderOpen, History, Plus, RotateCcw, Search, SquareTerminal, X } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { AgentIcon } from "../components/agents/AgentIcon";
import { TerminalPane, type PaneStatus, type TerminalControls } from "../components/terminal/TerminalPane";
import { TerminalSearchBar } from "../components/terminal/TerminalSearchBar";
import { RecentSessions } from "../components/terminal/RecentSessions";
import { SelectionToolbar, type SelectionInfo } from "../components/terminal/SelectionToolbar";
import { copyText } from "../lib/clipboard";
import { handoffPrompt, matchAgent, type AgentChoice } from "../lib/terminal-agents";
import { useHub } from "../lib/useHub";
import { useContextMenu, useMenuEnv } from "../lib/context-menu";
import { composeMenu } from "../lib/menu-builders/common";
import { termTabMenu, terminalPageMenu } from "../lib/menu-builders/terminal";
import { isTauri } from "../lib/api";
import { shortPath } from "../lib/format";
import { useT } from "../lib/i18n";
import type { MessageKey } from "../lib/i18n/messages";
import { terminalApi, type TerminalInfo, type TerminalSession } from "../lib/terminal";
import { resolveTerminalFontFamily } from "../lib/terminal-font";
import { RECORD_CHANGE_EVENT, readRecordEnabled } from "../lib/terminal-prefs";
import { scanLinks, type LinkContext, type LinkTarget } from "../lib/terminal-links";

const CWD_KEY = "agenthub-terminal-cwd";

interface TermTab {
  id: string;
  title: string;
  /** 起 shell 后自动敲的命令（agent CLI）；null = 纯 shell */
  command: string | null;
  /** 标签对应的 agent 图标 id；null 用通用终端图标 */
  agentId: string | null;
  cwd: string | null;
  status: PaneStatus;
}

function newId(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function makeTab(partial: Omit<TermTab, "id" | "status">): TermTab {
  return { ...partial, id: newId(), status: "starting" };
}

const STATUS_TONE: Record<PaneStatus, string> = {
  starting: "bg-state-warn",
  running: "bg-state-ok",
  exited: "bg-text-dim",
  error: "bg-state-danger",
};

const STATUS_LABEL: Record<PaneStatus, MessageKey> = {
  starting: "terminal.status.starting",
  running: "terminal.status.running",
  exited: "terminal.status.exited",
  error: "terminal.status.error",
};

/** 其它页面「在终端打开」时带过来的请求；token 变化即消费一次 */
export interface TerminalOpenRequest {
  token: number;
  /** 绝对路径 */
  cwd: string;
  title?: string;
  /** 起 shell 后自动敲的一行 */
  command?: string | null;
  agentId?: string | null;
}

/**
 * 终端页：上排是 agent CLI 的快捷启动，主体是 xterm 面板。
 *
 * 每个标签是一条独立的 PTY 会话；「启动 CLI」等价于在 shell 里敲一行命令，
 * 进程退出后标签仍保留输出（可以点重启再来一次）。
 */
export function Terminal({
  openRequest,
  onConsumed,
  onOpenTarget,
}: {
  openRequest?: TerminalOpenRequest | null;
  onConsumed?: () => void;
  /** 点到输出里的链接：技能 / 项目 / MCP / 批次（跳转由 App 负责） */
  onOpenTarget?: (target: LinkTarget) => void;
}) {
  const { catalog, notify, overview, projects } = useHub();
  const t = useT();
  const menuEnv = useMenuEnv();
  const contextMenu = useContextMenu();
  const [info, setInfo] = useState<TerminalInfo | null>(null);
  const [cwd, setCwd] = useState<string>(() => {
    try {
      return window.localStorage.getItem(CWD_KEY) ?? "";
    } catch {
      return "";
    }
  });
  const [tabs, setTabs] = useState<TermTab[]>(() => [
    makeTab({ title: t("terminal.title"), command: null, agentId: null, cwd: null }),
  ]);
  const [activeId, setActiveId] = useState<string>(() => tabs[0].id);
  /**
   * 字体解析完之前不建任何 pane：xterm 只在 `term.open()` 时量字宽，
   * 用错字体（比如缺图标字形的回退字体）会导致整屏列宽错位。
   */
  const [fontFamily, setFontFamily] = useState<string | null>(null);
  /** 每个标签的 xterm 能力句柄；ref 回调在卸载时置 null，不用手动清理 */
  const controlsRef = useRef<Record<string, TerminalControls | null>>({});
  const [searchOpen, setSearchOpen] = useState(false);
  /** 「记录终端输出」开关：本机偏好，设置页改动通过事件同步过来 */
  const [record, setRecord] = useState<boolean>(() => readRecordEnabled());
  const [historyOpen, setHistoryOpen] = useState(false);
  const [sessions, setSessions] = useState<TerminalSession[]>([]);
  /** 选区浮条的位置与内容；null = 没有选区 */
  const [selection, setSelection] = useState<SelectionInfo | null>(null);

  useEffect(() => {
    const onRecordChange = (event: Event) => {
      setRecord((event as CustomEvent<boolean>).detail);
    };
    window.addEventListener(RECORD_CHANGE_EVENT, onRecordChange);
    return () => window.removeEventListener(RECORD_CHANGE_EVENT, onRecordChange);
  }, []);

  // 标签栏的历史角标要有数：进页面先读一次索引（就一个 json，很快）
  useEffect(() => {
    if (!isTauri) return;
    void terminalApi
      .history()
      .then(setSessions)
      .catch((err) => console.warn("读取会话历史失败", err));
  }, []);

  useEffect(() => {
    let alive = true;
    void resolveTerminalFontFamily().then((resolved) => {
      if (alive) setFontFamily(resolved);
    });
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    void terminalApi
      .info()
      .then((next) => {
        if (disposed) return;
        setInfo(next);
        // 首次进入：工作目录留空时落到用户主目录
        setCwd((prev) => prev || next.home);
      })
      .catch((err) => console.warn("读取终端信息失败", err));
    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    try {
      window.localStorage.setItem(CWD_KEY, cwd);
    } catch {
      // 隐私模式下存不进也没关系
    }
  }, [cwd]);

  /** catalog 里「CLI 真的在 PATH 里」的条目，按命令去重（同族两版可能共用一个命令）。 */
  const launches = useMemo(() => {
    const seen = new Set<string>();
    const out: { iconId: string; name: string; command: string }[] = [];
    for (const entry of catalog) {
      if (entry.state !== "ready" || !entry.cli_name) continue;
      const key = entry.cli_name.toLowerCase();
      if (seen.has(key)) continue;
      seen.add(key);
      out.push({
        iconId: entry.builtin ?? entry.key,
        name: entry.name,
        command: entry.cli_name,
      });
    }
    return out;
  }, [catalog]);

  const workDir = cwd.trim() || info?.home || null;

  /** 输出里能点什么的判断依据：技能目录来自扫描结果，项目来自登记表 */
  const linkContext = useMemo<LinkContext>(
    () => ({
      skills: (overview?.report.records ?? []).map((record) => ({
        name: record.name,
        path: record.path,
      })),
      projects: projects.map((project) => ({ root: project.root, name: project.name })),
      home: info?.home ?? null,
    }),
    [overview, projects, info]
  );

  const resolveLinks = useCallback(
    (line: string) => scanLinks(line, linkContext),
    [linkContext]
  );

  // 开发期调试钩子：直接拿真实数据验证「这一行能不能点」。生产构建会被摇掉。
  useEffect(() => {
    if (!import.meta.env.DEV) return;
    const target = window as unknown as { __scanLinks?: (line: string) => unknown };
    target.__scanLinks = resolveLinks;
    return () => {
      delete target.__scanLinks;
    };
  }, [resolveLinks]);

  const openTab = (tab: TermTab) => {
    setTabs((prev) => [...prev, tab]);
    setActiveId(tab.id);
  };

  const openShell = () =>
    openTab(
      makeTab({
        title: info?.shell_label ?? t("terminal.title"),
        command: null,
        agentId: null,
        cwd: workDir,
      })
    );

  const launch = (target: { name: string; command: string; iconId: string }) =>
    openTab(
      makeTab({
        title: target.name,
        command: target.command,
        agentId: target.iconId,
        cwd: workDir,
      })
    );

  const openHistory = () => {
    setHistoryOpen(true);
    if (!isTauri) return;
    // 每次打开都重读：别的标签可能刚退出，字节数与状态要新
    void terminalApi
      .history()
      .then(setSessions)
      .catch((err) => console.warn("读取会话历史失败", err));
  };

  /** 按原命令与目录重开一个标签；进程不恢复，只恢复意图 */
  const reopen = (session: TerminalSession) => {
    openTab(
      makeTab({
        title: session.title,
        command: session.command,
        agentId: session.agent,
        cwd: session.cwd || workDir,
      })
    );
    setHistoryOpen(false);
  };

  const agentChoices: AgentChoice[] = launches.map((target) => ({
    iconId: target.iconId,
    name: target.name,
    command: target.command,
  }));

  /** 用户在 shell 里直接敲了某个 agent CLI：给标签换上它的图标与标题 */
  const handleCommandLine = (tabId: string, line: string) => {
    const hit = matchAgent(line, agentChoices);
    if (!hit) return;
    setTabs((prev) =>
      prev.map((tab) =>
        tab.id === tabId && !tab.agentId ? { ...tab, agentId: hit.iconId, title: hit.name } : tab
      )
    );
  };

  const copySelection = () => {
    const text = selection?.text ?? "";
    if (!text) return;
    void copyText(text).then((ok) => {
      if (!ok) notify(t("terminal.copyFailed"));
    });
  };

  /** 选中文本落成文件再交给 agent：不把长文本拼进命令行 */
  const handoffSelection = (agent: AgentChoice) => {
    const text = selection?.text ?? "";
    if (!text.trim()) return;
    void terminalApi
      .saveSelection(text)
      .then((path) => {
        openTab(
          makeTab({
            title: t("terminal.selectionTitle", { name: agent.name }),
            command: `${agent.command} "${handoffPrompt(path)}"`,
            agentId: agent.iconId,
            cwd: workDir,
          })
        );
        setSelection(null);
      })
      .catch((err) =>
        notify(
          t("terminal.saveSelectionFailed", {
            error: err instanceof Error ? err.message : String(err),
          })
        )
      );
  };

  /** 别的页面发来的「在终端打开」：消费一次后回调清状态，避免重复开标签 */
  const consumedToken = useRef(0);
  useEffect(() => {
    if (!openRequest || openRequest.token === consumedToken.current) return;
    consumedToken.current = openRequest.token;
    openTab(
      makeTab({
        title: openRequest.title ?? t("terminal.title"),
        command: openRequest.command ?? null,
        agentId: openRequest.agentId ?? null,
        cwd: openRequest.cwd,
      })
    );
    // 顺带把输入框的目录切过去，接下来点「新建」也落在这里
    setCwd(openRequest.cwd);
    onConsumed?.();
  }, [openRequest, onConsumed]);

  const closeTab = (id: string) => {
    const index = tabs.findIndex((tab) => tab.id === id);
    const rest = tabs.filter((tab) => tab.id !== id);
    // 关掉最后一个标签时补一个空 shell，避免出现「没有任何终端」的死页面
    const next =
      rest.length > 0
        ? rest
        : [
            makeTab({
              title: info?.shell_label ?? t("terminal.title"),
              command: null,
              agentId: null,
              cwd: workDir,
            }),
          ];
    setTabs(next);
    if (activeId === id) setActiveId(next[Math.min(index, next.length - 1)].id);
  };

  /** 重启 = 换一个会话 id 让面板重新挂载，命令与工作目录沿用原标签。 */
  const restart = (id: string) => {
    const nextId = newId();
    setTabs((prev) =>
      prev.map((tab) => (tab.id === id ? { ...tab, id: nextId, status: "starting" } : tab))
    );
    if (activeId === id) setActiveId(nextId);
  };

  const setStatus = (id: string, status: PaneStatus) =>
    setTabs((prev) => prev.map((tab) => (tab.id === id ? { ...tab, status } : tab)));

  /**
   * 原生文件夹选择。只改「新标签的默认工作目录」，不往正在跑的 shell 里塞 cd——
   * 用户可能正在 TUI 里，替她敲命令容易把界面搅乱。
   */
  const pickWorkDir = async () => {
    if (!isTauri) return;
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        title: t("terminal.pickCwd"),
        // 从当前值开始找，省得每次从「此电脑」点起
        defaultPath: cwd.trim() || info?.home || undefined,
      });
      if (typeof picked === "string" && picked.trim()) setCwd(picked);
    } catch (err) {
      console.warn("打开目录选择器失败", err);
    }
  };

  const subscribeSearch = useCallback(
    (listener: (index: number, count: number) => void) => {
      const controls = activeId ? controlsRef.current[activeId] : null;
      return controls ? controls.onSearchResults(listener) : () => {};
    },
    [activeId]
  );

  const runSearch = useCallback(
    (query: string, backwards: boolean) => {
      const controls = activeId ? controlsRef.current[activeId] : null;
      return controls ? controls.search(query, backwards) : false;
    },
    [activeId]
  );

  const clearSearch = useCallback(() => {
    if (activeId) controlsRef.current[activeId]?.clearSearch();
  }, [activeId]);

  /** 导出当前标签的输出：纯文本，落到日志目录并回报完整路径 */
  const exportActive = useCallback(() => {
    const controls = activeId ? controlsRef.current[activeId] : null;
    if (!controls) return;
    const text = controls.dumpText();
    if (!text.trim()) {
      notify(t("terminal.noOutput"));
      return;
    }
    const title = tabs.find((tab) => tab.id === activeId)?.title ?? "terminal";
    void terminalApi
      .exportText(text, title)
      .then((path) => notify(t("terminal.exported", { path })))
      .catch((err) =>
        notify(
          t("terminal.exportFailed", {
            error: err instanceof Error ? err.message : String(err),
          })
        )
      );
  }, [activeId, notify, tabs]);

  /** 终端标签右键：切/重启/关/复制 cwd。页面另外挂一份，用于标签条空白与面板空白 */
  const openTabMenu = (event: React.MouseEvent, tab: TermTab) => {
    contextMenu.open(
      event,
      composeMenu(
        termTabMenu({
          id: tab.id,
          title: tab.title,
          cwd: tab.cwd ?? cwd,
          activate: () => setActiveId(tab.id),
          restart: () => restart(tab.id),
          close: () => closeTab(tab.id),
          notify,
        }),
        menuEnv,
        "terminal"
      )
    );
  };

  const openPageMenu = (event: React.MouseEvent) => {
    contextMenu.open(
      event,
      composeMenu(
        terminalPageMenu({
          newShell: openShell,
          openHistory,
          openSearch: () => setSearchOpen(true),
          exportOutput: exportActive,
          hasTabs: tabs.length > 0,
        }),
        menuEnv,
        "terminal"
      )
    );
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-ink-950" onContextMenu={openPageMenu}>
      {/* 标签条 */}
      <div className="flex h-9 shrink-0 items-stretch gap-1 border-b border-line bg-ink-900 px-2">
        <button
          type="button"
          onClick={openHistory}
          title={t("terminal.recent")}
          aria-label={t("terminal.recent")}
          className="flex shrink-0 cursor-pointer items-center gap-1 self-center rounded-md border border-line bg-surface-2 px-1.5 py-1 text-[11px] text-text-muted transition-colors hover:border-line-strong hover:text-text-strong"
        >
          <History className="h-3.5 w-3.5" />
          {sessions.length > 0 && (
            <span className="rounded bg-brand-500/15 px-1 font-mono text-[11px] text-brand-300">
              {sessions.length}
            </span>
          )}
        </button>
        <div className="flex min-w-0 flex-1 items-stretch gap-1 overflow-x-auto">
          {tabs.map((tab) => {
            const isActive = tab.id === activeId;
            return (
              <div
                key={tab.id}
                role="tab"
                aria-selected={isActive}
                title={tab.cwd ?? undefined}
                data-ctx="term-tab"
                onClick={() => setActiveId(tab.id)}
                onContextMenu={(event) => openTabMenu(event, tab)}
                className={clsx(
                  "group flex max-w-[220px] shrink-0 cursor-pointer items-center gap-1.5 self-center rounded-md border px-2 py-1 text-[11.5px] transition-colors",
                  isActive
                    ? "border-line-strong bg-surface-2 text-text-strong"
                    : "border-transparent text-text-muted hover:bg-surface-hover hover:text-text-strong"
                )}
              >
                <span
                  className={clsx("h-1.5 w-1.5 shrink-0 rounded-full", STATUS_TONE[tab.status])}
                  title={t(STATUS_LABEL[tab.status])}
                />
                {tab.agentId ? (
                  <AgentIcon id={tab.agentId} className="h-4 w-4" />
                ) : (
                  <SquareTerminal className="h-3.5 w-3.5 shrink-0 text-text-dim" />
                )}
                <span className="truncate">{tab.title}</span>
                {tab.status === "exited" && (
                  <button
                    type="button"
                    aria-label={t("terminal.restart")}
                    title={t("terminal.restart")}
                    onClick={(event) => {
                      event.stopPropagation();
                      restart(tab.id);
                    }}
                    className="shrink-0 cursor-pointer rounded p-0.5 text-text-dim transition-colors hover:text-brand-300"
                  >
                    <RotateCcw className="h-3 w-3" />
                  </button>
                )}
                <button
                  type="button"
                  aria-label={t("terminal.closeTab", { name: tab.title })}
                  title={t("terminal.close")}
                  onClick={(event) => {
                    event.stopPropagation();
                    closeTab(tab.id);
                  }}
                  className={clsx(
                    "shrink-0 cursor-pointer rounded p-0.5 text-text-dim transition-all hover:text-state-danger",
                    isActive ? "opacity-100" : "opacity-0 group-hover:opacity-100"
                  )}
                >
                  <X className="h-3 w-3" />
                </button>
              </div>
            );
          })}
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {searchOpen ? (
            <TerminalSearchBar
              open
              onClose={() => {
                setSearchOpen(false);
                clearSearch();
              }}
              onSearch={runSearch}
              onClear={clearSearch}
              subscribe={subscribeSearch}
            />
          ) : (
            <button
              type="button"
              onClick={() => setSearchOpen(true)}
              disabled={!fontFamily}
              title={t("terminal.searchTitle")}
              aria-label={t("terminal.searchAria")}
              className="cursor-pointer rounded-md border border-line bg-surface-2 p-1 text-text-muted transition-colors hover:border-line-strong hover:text-text-strong disabled:cursor-default disabled:opacity-40"
            >
              <Search className="h-3.5 w-3.5" />
            </button>
          )}
          <button
            type="button"
            onClick={exportActive}
            disabled={!fontFamily}
            title={t("terminal.exportTitle")}
            aria-label={t("terminal.exportTitle")}
            className="cursor-pointer rounded-md border border-line bg-surface-2 p-1 text-text-muted transition-colors hover:border-line-strong hover:text-text-strong disabled:cursor-default disabled:opacity-40"
          >
            <Download className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={openShell}
            title={t("terminal.newTitle", { shell: info?.shell_label ?? "shell" })}
            className="flex cursor-pointer items-center gap-1 rounded-md border border-line bg-surface-2 px-2 py-1 text-[11px] text-text-muted transition-colors hover:border-line-strong hover:text-text-strong"
          >
            <Plus className="h-3.5 w-3.5" />
            {t("terminal.new")}
          </button>
        </div>
      </div>

      {/* 启动栏：agent CLI + 工作目录 */}
      <div className="flex h-10 shrink-0 items-center gap-1.5 border-b border-line bg-ink-900/50 px-2">
        <span className="shrink-0 text-[11.5px] text-text-dim">{t("terminal.launchCli")}</span>
        <div className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
          {launches.map((target) => (
            <button
              key={target.command}
              type="button"
              onClick={() => launch(target)}
              title={t("terminal.launchHint", {
                cwd: workDir ?? t("terminal.home"),
                command: target.command,
              })}
              className="flex shrink-0 cursor-pointer items-center gap-1.5 rounded-md border border-line bg-surface-2 px-2 py-1 text-[11px] text-text-muted transition-colors hover:border-brand-500/40 hover:bg-brand-500/10 hover:text-text-strong"
            >
              <AgentIcon id={target.iconId} className="h-4 w-4" />
              {target.name}
            </button>
          ))}
          {launches.length === 0 && (
            <span className="truncate text-[11px] text-text-dim">{t("terminal.noCli")}</span>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-1.5">
          <div className="hidden items-center gap-1 text-[11.5px] text-text-dim md:flex">
            <label htmlFor="terminal-cwd">{t("terminal.cwd")}</label>
            <input
              id="terminal-cwd"
              value={cwd}
              onChange={(event) => setCwd(event.target.value)}
              placeholder={info?.home ?? "~"}
              spellCheck={false}
              className="h-6 w-[240px] rounded border border-line bg-ink-950/60 px-1.5 font-mono text-[11px] text-text-strong outline-none transition-colors focus:border-brand-500/50 focus:ring-2 focus:ring-brand-500/15"
            />
            <button
              type="button"
              onClick={() => void pickWorkDir()}
              disabled={!isTauri}
              aria-label={t("terminal.pickFolder")}
              title={isTauri ? t("terminal.pickFolderHint") : t("terminal.demoNoPick")}
              className="flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center rounded border border-line bg-ink-950/60 text-text-dim transition-colors hover:border-brand-500/40 hover:text-brand-300 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:border-line disabled:hover:text-text-dim"
            >
              <FolderOpen className="h-3.5 w-3.5" />
            </button>
          </div>
          {info && (
            <span
              className="hidden max-w-[180px] truncate rounded border border-line px-1.5 py-0.5 text-[11px] text-text-dim xl:inline"
              title={t("terminal.shellDefault", { shell: info.shell })}
            >
              {info.shell_label}
            </span>
          )}
        </div>
      </div>

      {/* 面板区：所有标签都保持挂载，切走只是 display:none */}
      <div className="relative min-h-0 flex-1 bg-ink-700/30">
        {fontFamily ? (
          tabs.map((tab) => (
            <div
              key={tab.id}
              className={clsx("absolute inset-0 p-2", tab.id === activeId ? "block" : "hidden")}
            >
              <TerminalPane
                key={tab.id}
                ref={(instance) => {
                  controlsRef.current[tab.id] = instance;
                }}
                cwd={tab.cwd}
                shell={info?.shell ?? null}
                command={tab.command}
                title={tab.title}
                agentId={tab.agentId}
                record={record}
                active={tab.id === activeId}
                fontFamily={fontFamily}
                onStatus={(status) => setStatus(tab.id, status)}
                onRequestSearch={() => setSearchOpen(true)}
                resolveLinks={resolveLinks}
                onLinkActivate={onOpenTarget}
                onCommandLine={(line) => handleCommandLine(tab.id, line)}
                onSelectionChange={setSelection}
              />
            </div>
          ))
        ) : (
          <div className="flex h-full items-center justify-center text-[11.5px] text-text-dim">
            {t("terminal.preparing")}
          </div>
        )}
      </div>

      <div className="flex h-7 shrink-0 items-center gap-2 border-t border-line bg-ink-900 px-2 text-[11.5px] text-text-dim">
        <SquareTerminal className="h-3 w-3 shrink-0" />
        <span className="truncate">
          {isTauri
            ? record
              ? t("terminal.footRunning", {
                  dir: info?.logs_dir ?? "~/.agenthub/logs/terminal",
                })
              : t("terminal.footRunningNoLog")
            : t("terminal.footDemo")}
        </span>
        <span className="ml-auto hidden shrink-0 font-mono md:inline">
          {t("terminal.sessions", {
            count: tabs.length,
            cwd: shortPath(workDir ?? "~", 2),
          })}
        </span>
      </div>

      {selection && (
        <SelectionToolbar
          selection={selection}
          agents={agentChoices}
          onCopy={copySelection}
          onHandoff={handoffSelection}
        />
      )}

      {historyOpen && (
        <RecentSessions
          sessions={sessions}
          logsDir={info?.logs_dir}
          onReopen={reopen}
          onClose={() => setHistoryOpen(false)}
        />
      )}
    </div>
  );
}
