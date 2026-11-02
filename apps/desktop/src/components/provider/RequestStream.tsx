import { useMemo, useState } from "react";
import { clsx } from "clsx";
import { Eraser, Pause, Play } from "lucide-react";
import { Badge, Button, Empty } from "../ui";
import { useHub } from "../../lib/useHub";
import { useT } from "../../lib/i18n";
import type { GatewayRequestLog } from "../../lib/types";

function clock(at: number): string {
  const date = new Date(at * 1000);
  return `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}:${String(
    date.getSeconds()
  ).padStart(2, "0")}`;
}

function statusTone(status: number): "ok" | "warn" | "danger" | "dim" {
  if (status >= 500) return "danger";
  if (status >= 400) return "warn";
  if (status >= 200 && status < 300) return "ok";
  return "dim";
}

/**
 * 实时请求流。
 *
 * 只渲染元数据（网关侧也不记正文）：prompt 里可能有代码与密钥，
 * 一份「方便排障」的请求体日志会把它们原样抄到磁盘上。
 */
export function RequestStream() {
  const { gatewayLogs, clearGatewayLogs, gatewayStatus } = useHub();
  const t = useT();
  const [paused, setPaused] = useState(false);
  const [onlyFailed, setOnlyFailed] = useState(false);
  const [frozen, setFrozen] = useState<GatewayRequestLog[]>([]);

  // 暂停时冻结当前内容，别让新行继续顶掉正在看的那几条
  const source = paused ? frozen : gatewayLogs;
  const rows = useMemo(
    () => (onlyFailed ? source.filter((log) => log.status >= 400) : source).slice(-200).reverse(),
    [source, onlyFailed]
  );

  const togglePause = () => {
    if (!paused) setFrozen(gatewayLogs);
    setPaused(!paused);
  };

  return (
    <section className="panel overflow-hidden">
      <header className="flex flex-wrap items-center gap-2 border-b border-line px-3 py-2">
        <span className="text-[12px] font-medium text-text-strong">{t("stream.title")}</span>
        <Badge tone="dim">{t("stream.count", { count: source.length })}</Badge>
        {!gatewayStatus?.running && (
          <Badge tone="warn">{t("stream.gatewayDown")}</Badge>
        )}
        <div className="ml-auto flex items-center gap-1.5">
          <Button size="sm" variant="ghost" onClick={togglePause}>
            {paused ? <Play className="h-3 w-3" /> : <Pause className="h-3 w-3" />}
            {paused ? t("stream.resume") : t("stream.pause")}
          </Button>
          <Button
            size="sm"
            variant={onlyFailed ? "outline" : "ghost"}
            onClick={() => setOnlyFailed(!onlyFailed)}
          >
            {t("stream.failedOnly")}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => void clearGatewayLogs()}>
            <Eraser className="h-3 w-3" />
            {t("common.clear")}
          </Button>
        </div>
      </header>

      {rows.length === 0 ? (
        <Empty title={t("stream.emptyTitle")} hint={t("stream.emptyHint")} />
      ) : (
        <div className="max-h-[420px] overflow-y-auto">
          <table className="w-full border-collapse text-[11.5px]">
            <thead className="sticky top-0 z-10 bg-ink-900">
              <tr className="text-left text-[11px] text-text-dim">
                <th className="px-3 py-1.5 font-normal">{t("stream.col.time")}</th>
                <th className="px-2 py-1.5 font-normal">{t("stream.col.source")}</th>
                <th className="px-2 py-1.5 font-normal">{t("stream.col.route")}</th>
                <th className="px-2 py-1.5 font-normal">{t("stream.col.upstream")}</th>
                <th className="px-2 py-1.5 font-normal">{t("stream.col.model")}</th>
                <th className="px-2 py-1.5 font-normal">{t("stream.col.status")}</th>
                <th className="px-2 py-1.5 text-right font-normal">{t("stream.col.total")}</th>
                <th className="px-2 py-1.5 text-right font-normal">{t("stream.col.firstByte")}</th>
                <th className="px-3 py-1.5 text-right font-normal">{t("stream.col.retries")}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((log, index) => (
                <tr
                  key={`${log.at}-${index}-${log.route_id}`}
                  className={clsx(
                    "border-t border-line/60 transition-colors duration-120 hover:bg-surface-hover/40",
                    log.status >= 400 && "border-l-2 border-l-state-danger bg-state-danger/5"
                  )}
                  title={log.error ?? undefined}
                >
                  <td className="mono px-3 py-1.5 text-text-dim">{clock(log.at)}</td>
                  <td className="px-2 py-1.5 text-text-muted">{log.agent ?? "—"}</td>
                  <td className="mono px-2 py-1.5 text-text-muted">{log.route_id}</td>
                  <td className="px-2 py-1.5 text-text-muted">
                    {log.provider_name ?? (
                      <span className="text-state-danger">{t("stream.allFailed")}</span>
                    )}
                  </td>
                  <td className="mono max-w-[180px] truncate px-2 py-1.5 text-text-dim">
                    {log.model ?? "—"}
                  </td>
                  <td className="px-2 py-1.5">
                    <Badge tone={statusTone(log.status)}>{log.status}</Badge>
                  </td>
                  <td className="mono px-2 py-1.5 text-right text-text-muted">{log.total_ms}ms</td>
                  <td className="mono px-2 py-1.5 text-right text-text-dim">
                    {log.first_byte_ms ? `${log.first_byte_ms}ms` : "—"}
                  </td>
                  <td className="mono px-3 py-1.5 text-right text-text-dim">
                    {log.attempts > 1 ? (
                      <span className="text-state-warn">{log.attempts}</span>
                    ) : (
                      log.attempts
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
