import { useMemo, useState } from "react";
import { clsx } from "clsx";
import { Copy, Play, RotateCcw, Settings2, Square } from "lucide-react";
import { Badge, Button, Toggle } from "../ui";
import { useHub } from "../../lib/useHub";
import { useT } from "../../lib/i18n";
import { copyText } from "../../lib/clipboard";

/** 该路由对外的接入地址。端口取运行时状态（可能被用户在配置里改过）。 */
export function routeUrl(host: string, port: number, routeId: string): string {
  return `http://${host}:${port}/gw/${routeId}`;
}

/**
 * 网关状态条：是否在跑、监听在哪、启停与全局配置。
 *
 * 这一屏的信息是 Provider 页所有操作的前提（地址没跑起来，绑定再对也连不上），
 * 所以固定在最上面，不塞进任何 tab。
 */
export function GatewayStatusBar() {
  const {
    gateway,
    gatewayStatus,
    gatewayLogs,
    startGateway,
    stopGateway,
    restartGateway,
    saveGatewayConfig,
    notify,
  } = useHub();
  const t = useT();
  const [editing, setEditing] = useState(false);
  const [port, setPort] = useState("");
  const [token, setToken] = useState("");
  const [autoStart, setAutoStart] = useState(false);
  const [logRequests, setLogRequests] = useState(false);

  const running = gatewayStatus?.running ?? false;
  const host = gatewayStatus?.host ?? gateway?.config.host ?? "127.0.0.1";
  const listenPort = gatewayStatus?.port ?? gateway?.config.port ?? 0;
  const authOn = gatewayStatus?.auth ?? Boolean(gateway?.config.auth_token);

  const metrics = useMemo(() => {
    const dayStart = new Date();
    dayStart.setHours(0, 0, 0, 0);
    const today = gatewayLogs.filter((log) => log.at * 1000 >= dayStart.getTime());
    const failed = today.filter((log) => log.status >= 400).length;
    const firstBytes = today.filter((log) => log.first_byte_ms > 0);
    const avgFirst = firstBytes.length
      ? Math.round(firstBytes.reduce((sum, log) => sum + log.first_byte_ms, 0) / firstBytes.length)
      : 0;
    return {
      today: today.length,
      failed,
      rate: today.length ? Math.round((failed / today.length) * 100) : 0,
      avgFirst,
    };
  }, [gatewayLogs]);

  const openEditor = () => {
    setPort(String(gateway?.config.port ?? 43117));
    setToken(gateway?.config.auth_token ?? "");
    setAutoStart(gateway?.config.auto_start ?? false);
    setLogRequests(gateway?.config.log_requests ?? false);
    setEditing(true);
  };

  const commit = () => {
    const next = Number(port);
    if (!Number.isFinite(next) || next < 1024 || next > 65535) {
      notify(t("gateway.portInvalid"));
      return;
    }
    void saveGatewayConfig({
      port: next,
      auto_start: autoStart,
      log_requests: logRequests,
      max_log_entries: gateway?.config.max_log_entries ?? 500,
      auth_token: token,
    }).then((ok) => {
      if (ok) setEditing(false);
    });
  };

  return (
    <section className="panel overflow-hidden">
      <div className="flex flex-wrap items-center gap-3 px-3.5 py-3">
        <span
          className={clsx(
            "h-2 w-2 shrink-0 rounded-full",
            running ? "animate-pulse bg-state-ok" : "bg-text-dim/60"
          )}
          title={running ? t("gateway.running") : t("gateway.stopped")}
        />
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="text-[12.5px] font-medium text-text-strong">
              {running ? t("gateway.runningLabel") : t("gateway.stoppedLabel")}
            </span>
            <span className="mono rounded border border-line bg-surface-2 px-1.5 py-[1px] text-[11.5px] text-text-muted">
              {host}:{listenPort}
            </span>
            {authOn && (
              <Badge tone="warn" title={t("gateway.tokenRequiredHint")}>
                {t("gateway.tokenRequired")}
              </Badge>
            )}
            {gatewayStatus?.last_error && (
              <Badge tone="danger" title={gatewayStatus.last_error}>
                {t("gateway.lastStartFailed")}
              </Badge>
            )}
          </div>
          <p className="mt-0.5 text-[11px] text-text-dim">{t("gateway.safety")}</p>
        </div>

        <div className="ml-auto flex items-center gap-1.5">
          <Button
            size="sm"
            variant="ghost"
            onClick={() =>
              void copyText(`${host}:${listenPort}`).then((ok) =>
                notify(ok ? t("gateway.copied") : t("menu.copyFailed"))
              )
            }
          >
            <Copy className="h-3 w-3" />
            {t("gateway.copyAddress")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={openEditor}
            title={t("gateway.settingsTitle")}
          >
            <Settings2 className="h-3 w-3" />
          </Button>
          {running ? (
            <>
              <Button size="sm" variant="outline" onClick={() => void restartGateway()}>
                <RotateCcw className="h-3 w-3" />
                {t("gateway.restart")}
              </Button>
              <Button size="sm" variant="danger" onClick={() => void stopGateway()}>
                <Square className="h-3 w-3" />
                {t("gateway.stop")}
              </Button>
            </>
          ) : (
            <Button size="sm" variant="primary" onClick={() => void startGateway()}>
              <Play className="h-3 w-3" />
              {t("gateway.start")}
            </Button>
          )}
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-x-5 gap-y-1 border-t border-line px-3.5 py-2 text-[11px] text-text-dim">
        <span>
          {t("gateway.routeCount")}{" "}
          <b className="text-text-strong">{gatewayStatus?.routes ?? 0}</b>
        </span>
        <span>
          {t("gateway.todayRequests")} <b className="text-text-strong">{metrics.today}</b>
        </span>
        <span>
          {t("gateway.failureRate")}{" "}
          <b className={metrics.rate > 0 ? "text-state-warn" : "text-text-strong"}>
            {metrics.rate}%
          </b>
        </span>
        <span>
          {t("gateway.firstByte")}{" "}
          <b className="text-text-strong">{metrics.avgFirst ? `${metrics.avgFirst}ms` : "—"}</b>
        </span>
        {gateway?.notes.length ? (
          <span className="text-state-warn" title={gateway.notes.join("\n")}>
            {t("gateway.notes", { count: gateway.notes.length })}
          </span>
        ) : null}
      </div>

      {editing && (
        <div className="border-t border-line bg-ink-950/40 px-3.5 py-3">
          <div className="grid grid-cols-2 gap-2.5">
            <label className="flex flex-col gap-1">
              <span className="text-[11.5px] text-text-dim">{t("gateway.portLabel")}</span>
              <input
                value={port}
                onChange={(event) => setPort(event.target.value)}
                inputMode="numeric"
                className="mono h-8 w-full rounded-md border border-line bg-ink-950/70 px-2 text-[12px] text-text-strong outline-none focus:border-brand-500/60 focus:ring-2 focus:ring-brand-500/15"
              />
              <span className="text-[11px] text-text-dim">{t("gateway.portNote")}</span>
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-[11.5px] text-text-dim">{t("gateway.tokenLabel")}</span>
              <input
                value={token}
                onChange={(event) => setToken(event.target.value)}
                placeholder={t("gateway.tokenPlaceholder")}
                className="mono h-8 w-full rounded-md border border-line bg-ink-950/70 px-2 text-[12px] text-text-strong placeholder:text-text-dim/70 outline-none focus:border-brand-500/60 focus:ring-2 focus:ring-brand-500/15"
              />
              <span className="text-[11px] text-text-dim">{t("gateway.tokenNote")}</span>
            </label>
          </div>

          <div className="mt-2.5 flex items-center justify-between gap-3 border-t border-line pt-2.5">
            <div>
              <p className="text-[11.5px] text-text-strong">{t("gateway.autoStart")}</p>
              <p className="text-[11px] text-text-dim">{t("gateway.autoStartNote")}</p>
            </div>
            <Toggle checked={autoStart} onChange={setAutoStart} />
          </div>
          <div className="flex items-center justify-between gap-3 border-t border-line pt-2.5">
            <div>
              <p className="text-[11.5px] text-text-strong">{t("gateway.recordMeta")}</p>
              <p className="text-[11px] text-text-dim">{t("gateway.recordMetaNote")}</p>
            </div>
            <Toggle checked={logRequests} onChange={setLogRequests} />
          </div>

          <div className="mt-2.5 flex items-center gap-1.5">
            <Button size="sm" variant="primary" onClick={commit}>
              {t("gateway.save")}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setEditing(false)}>
              {t("common.cancel")}
            </Button>
          </div>
        </div>
      )}
    </section>
  );
}
