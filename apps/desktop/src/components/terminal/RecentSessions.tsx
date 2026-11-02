import { clsx } from "clsx";
import { History, RotateCcw } from "lucide-react";
import { AgentIcon } from "../agents/AgentIcon";
import { bytes, shortPath, timeAgo } from "../../lib/format";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import type { TerminalSession } from "../../lib/terminal";

const STATUS_TEXT: Record<string, MessageKey> = {
  running: "sess.running",
  exited: "sess.exited",
  error: "sess.error",
  starting: "sess.starting",
};

/**
 * 「上次会话」浮层：给的是「上次在这干过什么」，点一下按原命令与目录重开。
 *
 * 不恢复进程——那需要把 PTY 常驻在壳层，收益不抵复杂度；
 * 恢复「意图」已经够用：cd 到同一个目录、重新敲同一条命令。
 */
export function RecentSessions({
  sessions,
  logsDir,
  onReopen,
  onClose,
}: {
  sessions: TerminalSession[];
  logsDir?: string;
  onReopen: (session: TerminalSession) => void;
  onClose: () => void;
}) {
  const t = useT();
  return (
    <div className="fixed inset-0 z-30" role="presentation" onClick={onClose}>
      <div
        role="dialog"
        aria-label={t("terminal.recent")}
        onClick={(event) => event.stopPropagation()}
        className="absolute right-4 top-14 flex max-h-[70vh] w-[360px] flex-col overflow-hidden rounded-lg border border-line bg-bg-elev shadow-2xl"
      >
        <header className="flex items-start justify-between gap-2 border-b border-line px-3 py-2">
          <div className="min-w-0">
            <p className="flex items-center gap-1.5 text-[12px] font-semibold text-text-strong">
              <History className="h-3.5 w-3.5 text-brand-500" />
              {t("sess.recent")}
            </p>
            <p className="mt-0.5 truncate text-[11.5px] text-text-dim" title={logsDir}>
              {t("sess.retention", {
                dir: logsDir ? shortPath(logsDir, 3) : t("sess.notRecorded"),
              })}
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={t("sess.closeList")}
            className="shrink-0 cursor-pointer rounded px-1 py-0.5 text-[11px] text-text-dim transition-colors hover:text-text-strong"
          >
            {t("common.close")}
          </button>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto">
          {sessions.length === 0 ? (
            <p className="px-3 py-6 text-center text-[11px] text-text-dim">
              {t("sess.empty")}
            </p>
          ) : (
            <ul className="divide-y divide-line/70">
              {sessions.map((session) => (
                <li key={session.id}>
                  <button
                    type="button"
                    onClick={() => onReopen(session)}
                    title={t("sess.reopen", {
                      cwd: session.cwd,
                      command: session.command ?? t("sess.shellOnly"),
                    })}
                    className="group flex w-full cursor-pointer items-start gap-2 px-3 py-2 text-left transition-colors hover:bg-surface-hover"
                  >
                    <span className="mt-0.5 shrink-0 text-text-dim">
                      {session.agent ? (
                        <AgentIcon id={session.agent} className="h-4 w-4" />
                      ) : (
                        <History className="h-3.5 w-3.5" />
                      )}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="flex items-center gap-1.5">
                        <span className="truncate text-[11.5px] text-text-strong">
                          {session.title}
                        </span>
                        <span
                          className={clsx(
                            "shrink-0 text-[11px]",
                            session.status === "error" ? "text-state-danger" : "text-text-dim"
                          )}
                        >
                          {STATUS_TEXT[session.status]
                            ? t(STATUS_TEXT[session.status])
                            : session.status}
                        </span>
                      </span>
                      <span className="mt-0.5 flex items-center gap-1.5 text-[11px] text-text-dim">
                        <span className="truncate font-mono" title={session.cwd}>
                          {shortPath(session.cwd, 2)}
                        </span>
                        <span className="shrink-0">·</span>
                        <span className="shrink-0">{timeAgo(session.started_at)}</span>
                        <span className="shrink-0">·</span>
                        <span className="shrink-0">
                          {session.bytes > 0 ? bytes(session.bytes) : "—"}
                        </span>
                        {session.truncated && (
                          <span className="shrink-0 text-state-warn">
                            {t("sess.truncated")}
                          </span>
                        )}
                      </span>
                    </span>
                    <RotateCcw className="mt-0.5 h-3 w-3 shrink-0 text-text-dim opacity-0 transition-opacity group-hover:opacity-100" />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
