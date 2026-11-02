import { useEffect, useMemo, useState } from "react";
import { clsx } from "clsx";
import { Check, Minus, Plug } from "lucide-react";
import { useHub } from "../lib/useHub";
import { useContextMenu, useMenuEnv } from "../lib/context-menu";
import { composeMenu } from "../lib/menu-builders/common";
import { mcpPageMenu, mcpRowMenu } from "../lib/menu-builders/mcp";
import { agentInfo, basename, shortPath } from "../lib/format";
import { useT } from "../lib/i18n";
import { AgentIcon } from "../components/agents/AgentIcon";
import { Badge, Button, Empty, Panel, SearchInput } from "../components/ui";
import type { McpServerView, Transport } from "../lib/types";

function transportBadge(t: Transport) {
  return t.type === "stdio" ? "stdio" : "http";
}

function secretLabel(ref: { kind: string; var?: string; service?: string; account?: string; value?: string }) {
  if (ref.kind === "env") return `env:${ref.var}`;
  if (ref.kind === "keychain") return `keychain:${ref.service}/${ref.account}`;
  return ref.value ?? "********";
}

function snippet(server: McpServerView, format: string) {
  const t = server.transport;
  if (format === "TOML") {
    const lines = [`[mcp_servers.${server.name}]`];
    if (t.type === "stdio") {
      lines.push(`command = "${t.command}"`);
      if (t.args.length) lines.push(`args = [${t.args.map((a) => `"${a}"`).join(", ")}]`);
    } else {
      lines.push(`url = "${t.url}"`);
    }
    lines.push(`enabled = ${server.enabled}`);
    if (server.timeout_secs) lines.push(`startup_timeout_sec = ${server.timeout_secs}`);
    return lines.join("\n");
  }
  if (format === "YAML") {
    const lines = ["mcp_servers:", `  ${server.name}:`];
    if (t.type === "stdio") {
      lines.push(`    command: ${t.command}`);
      if (t.args.length) {
        lines.push("    args:");
        t.args.forEach((a) => lines.push(`      - ${a}`));
      }
    } else {
      lines.push(`    url: ${t.url}`);
    }
    lines.push(`    enabled: ${server.enabled}`);
    return lines.join("\n");
  }
  const payload = {
    mcpServers: {
      [server.name]:
        t.type === "stdio"
          ? { command: t.command, args: t.args }
          : { type: "http", url: t.url },
    },
  };
  return JSON.stringify(payload, null, 2);
}

export function Mcp() {
  const { overview, notify, projects, mcpView, planMcpWrite } = useHub();
  const t = useT();
  const menuEnv = useMenuEnv();
  const contextMenu = useContextMenu();
  const [query, setQuery] = useState("");
  // 作用域：null = 全局（各 agent 的用户级配置），否则是某个项目根
  const [scope, setScope] = useState<string | null>(null);
  const [scoped, setScoped] = useState<McpServerView[] | null>(null);
  const [scopeLoading, setScopeLoading] = useState(false);

  useEffect(() => {
    if (!scope) {
      setScoped(null);
      return;
    }
    let cancelled = false;
    setScopeLoading(true);
    void mcpView(scope)
      .then((list) => {
        if (!cancelled) setScoped(list);
      })
      .catch((err) => {
        if (!cancelled) notify(t("mcp.readFailed", { error: String(err) }));
      })
      .finally(() => {
        if (!cancelled) setScopeLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [scope, mcpView, notify]);

  const servers = scope ? scoped ?? [] : overview?.mcp ?? [];
  const [active, setActive] = useState<string | null>(null);

  const filtered = useMemo(
    () =>
      servers.filter((s) =>
        query ? s.name.toLowerCase().includes(query.toLowerCase()) : true
      ),
    [servers, query]
  );
  const current = servers.find((s) => s.name === active) ?? filtered[0] ?? null;

  if (!overview) return null;

  const openServerMenu = (event: React.MouseEvent, server: McpServerView) => {
    const writable = server.targets.filter((target) => target.writable).map((target) => target.agent);
    contextMenu.open(
      event,
      composeMenu(
        mcpRowMenu(server, {
          open: () => setActive(server.name),
          planWrite: () => void planMcpWrite(scope, [server], writable),
          writableAgents: writable.length,
          notify,
        }),
        menuEnv,
        "mcp"
      )
    );
  };

  const openPageMenu = (event: React.MouseEvent) => {
    contextMenu.open(
      event,
      composeMenu(
        mcpPageMenu(
          {
            scope: scope ? "project" : "user",
            setScope: (next) => setScope(next === "project" ? (projects[0]?.root ?? null) : null),
            projectMissing: projects.length === 0,
          },
          menuEnv
        ),
        menuEnv,
        "mcp"
      )
    );
  };

  return (
    <div className="space-y-3.5 animate-fade-up" onContextMenu={openPageMenu}>
      <div className="panel flex flex-wrap items-center gap-2 px-3 py-2.5">
        <div className="w-[240px]">
          <SearchInput
            value={query}
            onChange={setQuery}
            placeholder={t("mcp.searchPlaceholder")}
          />
        </div>
        <select
          data-mcp-scope
          title={projects.length === 0 ? t("mcp.noProjects") : t("mcp.pickScope")}
          className="cursor-pointer rounded border border-line bg-ink-950 px-2 py-1 text-[11.5px] text-text-muted outline-none"
          value={scope ?? ""}
          onChange={(e) => setScope(e.target.value || null)}
        >
          <option value="">{t("mcp.globalScope")}</option>
          {projects.map((proj) => (
            <option key={proj.root} value={proj.root}>
              {t("mcp.projectScope", { name: proj.name })}
            </option>
          ))}
        </select>
        <Badge tone="brand">{t("mcp.definitions", { count: servers.length })}</Badge>
        <Badge tone="dim">
          {t("mcp.writtenCount", {
            count: servers.reduce(
              (n, s) => n + s.targets.filter((target) => target.present).length,
              0
            ),
          })}
        </Badge>
        {scopeLoading && <Badge tone="dim">{t("mcp.loading")}</Badge>}
        {scope && (
          <span className="mono max-w-[220px] truncate text-[11.5px] text-text-dim" title={scope}>
            {shortPath(scope, 2)}
          </span>
        )}
        <div className="ml-auto flex items-center gap-2 text-[11.5px] text-text-dim">
          <Plug className="h-3.5 w-3.5" />
          {t("mcp.kicker")}
        </div>
      </div>

      <div className="grid grid-cols-[300px_1fr] gap-3.5">
        <div className="space-y-2">
          {filtered.length === 0 ? (
            <Panel>
              <Empty title={t("mcp.emptyTitle")} hint={t("mcp.emptyHint")} />
            </Panel>
          ) : (
            filtered.map((server) => {
              const present = server.targets.filter((target) => target.present).length;
              return (
                <button
                  key={server.name}
                  type="button"
                  data-ctx="mcp-card"
                  onClick={() => setActive(server.name)}
                  onContextMenu={(event) => openServerMenu(event, server)}
                  className={clsx(
                    "panel hover-lift w-full cursor-pointer px-3 py-2.5 text-left",
                    current?.name === server.name && "border-brand-500/40 bg-brand-500/[0.07]"
                  )}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate text-[12.5px] text-text-strong">{server.name}</span>
                    <Badge tone={server.enabled ? "ok" : "dim"}>
                      {server.enabled ? t("mcp.enabled") : t("mcp.disabled")}
                    </Badge>
                  </div>
                  <p className="mono mt-1 truncate text-[11.5px] text-text-dim">
                    {transportBadge(server.transport)} · {server.transport.type === "stdio" ? server.transport.command : server.transport.url}
                  </p>
                  <div className="mt-1.5 flex items-center gap-1">
                    {server.targets.map((target) => (
                      <span
                        key={target.agent}
                        title={t("mcp.targetTitle", {
                          agent: agentInfo(target.agent).name,
                          state: target.present ? t("mcp.targetPresent") : t("mcp.targetMissing"),
                        })}
                        className={clsx(
                          "flex items-center rounded border px-1 py-[1px]",
                          target.present
                            ? agentInfo(target.agent).accent
                            : "border-line bg-surface-2 text-text-dim"
                        )}
                      >
                        <AgentIcon id={target.agent} className="h-3.5 w-3.5" />
                      </span>
                    ))}
                    <span className="ml-auto text-[11.5px] text-text-dim">{present}/5</span>
                  </div>
                </button>
              );
            })
          )}
        </div>

        {current ? (
          <div className="space-y-3">
            <Panel
              title={current.name}
              subtitle={
                current.note ??
                t("mcp.noteDefault", {
                  kind:
                    current.transport.type === "stdio"
                      ? t("mcp.noteLocal")
                      : t("mcp.noteRemote"),
                })
              }
              actions={
                <Button
                  size="sm"
                  variant="outline"
                  data-plan-mcp
                  onClick={() => {
                    const writable = current.targets
                      .filter((target) => target.writable)
                      .map((target) => target.agent);
                    if (writable.length === 0) {
                      notify(t("mcp.noTarget"));
                      return;
                    }
                    void planMcpWrite(scope, [current], writable);
                  }}
                >
                  {t("mcp.planWrite")}
                </Button>
              }
            >
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <p className="text-[11px] text-text-dim">{t("mcp.args")}</p>
                  <pre className="mono overflow-x-auto rounded-lg border border-line bg-ink-950/70 p-2.5 text-[11px] leading-relaxed text-text-muted">
                    {current.transport.type === "stdio"
                      ? `${current.transport.command} ${current.transport.args.join(" ")}`
                      : current.transport.url}
                  </pre>
                  <p className="text-[11px] text-text-dim">{t("mcp.env")}</p>
                  <ul className="space-y-1">
                    {Object.entries(
                      current.transport.type === "stdio" ? current.transport.env : current.transport.headers
                    ).map(([key, value]) => (
                      <li
                        key={key}
                        className="flex items-center justify-between rounded border border-line bg-surface-2 px-2 py-1 text-[11px]"
                      >
                        <span className="mono text-text-muted">{key}</span>
                        <span className="mono text-state-info">{secretLabel(value)}</span>
                      </li>
                    ))}
                    {Object.keys(
                      current.transport.type === "stdio" ? current.transport.env : current.transport.headers
                    ).length === 0 && (
                      <li className="text-[11px] text-text-dim">{t("mcp.noEnv")}</li>
                    )}
                  </ul>
                </div>

                <div className="space-y-1.5">
                  <p className="text-[11px] text-text-dim">{t("mcp.distribution")}</p>
                  <ul className="space-y-1">
                    {current.targets.map((target) => (
                      <li
                        key={target.agent}
                        title={target.reason ?? undefined}
                        className="flex items-center gap-2 rounded border border-line bg-surface-2 px-2 py-1 text-[11px]"
                      >
                        {target.present ? (
                          <Check className="h-3 w-3 text-state-ok" />
                        ) : (
                          <Minus className="h-3 w-3 text-text-dim" />
                        )}
                        <span className="text-text-muted">{agentInfo(target.agent).name}</span>
                        <Badge tone="dim">{target.format}</Badge>
                        {!target.writable && (
                          <Badge tone="warn">{t("mcp.unsupported")}</Badge>
                        )}
                        <span className="mono ml-auto truncate text-[11px] text-text-dim">
                          {target.file ? basename(target.file) : "—"}
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              </div>
            </Panel>

            <Panel
              title={t("mcp.previewTitle")}
              subtitle={t("mcp.previewSubtitle")}
            >
              <div className="grid grid-cols-3 gap-2.5">
                {(["JSON", "TOML", "YAML"] as const).map((format) => (
                  <div key={format}>
                    <div className="mb-1 flex items-center justify-between">
                      <span className="text-[11px] text-text-muted">{format}</span>
                      <span className="flex items-center gap-1">
                        {current.targets
                          .filter((t) => t.format === format)
                          .map((t) => (
                            <span
                              key={t.agent}
                              title={agentInfo(t.agent).name}
                              className="flex items-center"
                            >
                              <AgentIcon id={t.agent} className="h-3.5 w-3.5" />
                            </span>
                          ))}
                      </span>
                    </div>
                    <pre className="mono h-[150px] overflow-auto rounded-lg border border-line bg-ink-950/70 p-2 text-[11.5px] leading-relaxed text-text-muted">
                      {snippet(current, format)}
                    </pre>
                  </div>
                ))}
              </div>
              <p className="mt-2 text-[11.5px] leading-relaxed text-text-dim">
                {t("mcp.mergeNote")}
              </p>
            </Panel>
          </div>
        ) : (
          <Panel>
            <Empty title={t("mcp.selectTitle")} hint={t("mcp.selectHint")} />
          </Panel>
        )}
      </div>
    </div>
  );
}
