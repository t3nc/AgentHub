import { useMemo, useState } from "react";
import { clsx } from "clsx";
import { ArrowDown, FileText, Link2, Sparkles } from "lucide-react";
import { useHub } from "../lib/useHub";
import { useContextMenu, useMenuEnv } from "../lib/context-menu";
import { composeMenu } from "../lib/menu-builders/common";
import { instructionPageMenu, instructionRowMenu } from "../lib/menu-builders/instructions";
import { copyText } from "../lib/clipboard";
import { agentInfo, basename } from "../lib/format";
import { useT } from "../lib/i18n";
import { AgentIcon } from "../components/agents/AgentIcon";
import { Badge, Button, Empty, Panel } from "../components/ui";

const PLACEHOLDER = `## 团队共享约定

- 提交前先跑 \`cargo test\` 与 \`npm run typecheck\`
- 生产环境的密钥只允许通过 env: 引用，不入仓
- 回答里给出具体文件路径与行号`;

export function Instructions() {
  const { instructionData, notify } = useHub();
  const t = useT();
  const menuEnv = useMenuEnv();
  const contextMenu = useContextMenu();
  const { specs, chain } = instructionData;
  const [activePath, setActivePath] = useState<string | null>(specs[0]?.path ?? null);
  const [snippet, setSnippet] = useState(PLACEHOLDER);

  const grouped = useMemo(() => {
    const map = new Map<string, typeof specs>();
    for (const spec of specs) {
      const list = map.get(spec.agent) ?? [];
      list.push(spec);
      map.set(spec.agent, list);
    }
    return Array.from(map.entries());
  }, [specs]);

  const active = specs.find((s) => s.path === activePath) ?? specs[0] ?? null;

  const openRowMenu = (event: React.MouseEvent, spec: (typeof specs)[number]) => {
    contextMenu.open(
      event,
      composeMenu(
        instructionRowMenu(spec, {
          open: () => setActivePath(spec.path),
          env: menuEnv,
          notify,
        }),
        menuEnv,
        "instructions"
      )
    );
  };

  const openPageMenu = (event: React.MouseEvent) => {
    const inject = () =>
      notify(t("instr.runInject", { agent: active?.agent ?? "claude" }));
    contextMenu.open(
      event,
      composeMenu(
        instructionPageMenu({
          planInject: inject,
          copySnippet: () => {
            void copyText(snippet).then((ok) =>
              notify(ok ? t("instr.copiedSnippet") : t("menu.copyFailed"))
            );
          },
          bridge: () => notify(t("instr.bridgeCmd")),
          notify,
        }),
        menuEnv,
        "instructions"
      )
    );
  };

  return (
    <div
      className="grid grid-cols-[264px_1fr_300px] gap-3.5 animate-fade-up"
      onContextMenu={openPageMenu}
    >
      <Panel title={t("instr.title")} subtitle={t("instr.globalScope")} bodyClassName="p-2">
        <div className="space-y-2.5">
          {grouped.map(([agent, list]) => (
            <div key={agent}>
              <div className="flex items-center gap-1.5 px-1.5 py-1">
                <span
                  className={clsx(
                    "flex items-center rounded border px-1 py-[1px]",
                    agentInfo(agent).accent
                  )}
                >
                  <AgentIcon id={agent} className="h-3.5 w-3.5" />
                </span>
                <span className="text-[11px] text-text-muted">{agentInfo(agent).name}</span>
              </div>
              <ul className="space-y-0.5">
                {list.map((spec) => (
                  <li key={spec.path}>
                    <button
                      type="button"
                      data-ctx="instr-item"
                      onClick={() => setActivePath(spec.path)}
                      onContextMenu={(event) => openRowMenu(event, spec)}
                      className={clsx(
                        "flex w-full cursor-pointer items-center gap-1.5 rounded-md px-1.5 py-1.5 text-left transition-colors",
                        active?.path === spec.path
                          ? "bg-surface-hover text-text-strong"
                          : "text-text-muted hover:bg-surface-hover"
                      )}
                    >
                      <FileText className="h-3 w-3 shrink-0 text-text-dim" />
                      <span className="truncate text-[11.5px]">{basename(spec.path)}</span>
                      {spec.managed && (
                        <Badge tone="brand" className="ml-auto">
                          {t("instr.badge.managed")}
                        </Badge>
                      )}
                      {!spec.exists && (
                        <Badge tone="dim" className="ml-auto">
                          {t("instr.badge.missing")}
                        </Badge>
                      )}
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          ))}
          {specs.length === 0 && (
            <p className="px-2 py-4 text-[11.5px] text-text-dim">{t("instr.none")}</p>
          )}
        </div>
      </Panel>

      <div className="space-y-3">
        <Panel
          title={active ? basename(active.path) : t("instr.snippet")}
          subtitle={active?.display ?? ""}
          actions={
            <>
              <Badge tone={active?.managed ? "brand" : "dim"}>
                {active?.managed ? t("instr.controlled") : t("instr.unmanaged")}
              </Badge>
              <Button
                size="sm"
                variant="outline"
                onClick={() =>
                  notify(t("instr.runInject", { agent: active?.agent ?? "claude" }))
                }
              >
                {t("instr.planInject")}
              </Button>
            </>
          }
        >
          <div className="mb-2 flex items-center gap-2 rounded-lg border border-line bg-surface-2 px-2.5 py-1.5 text-[11px] text-text-dim">
            <Sparkles className="h-3.5 w-3.5 text-brand-300" />
            {t("instr.mergeNote")}
          </div>
          <textarea
            value={snippet}
            onChange={(event) => setSnippet(event.target.value)}
            spellCheck={false}
            className="mono h-[260px] w-full resize-none rounded-lg border border-line bg-ink-950/70 p-3 text-[11.5px] leading-relaxed text-text-strong/90 outline-none transition-colors focus:border-brand-500/40"
          />
          <div className="mt-2 flex items-center justify-between text-[11.5px] text-text-dim">
            <span>{t("instr.snippetSize", { count: snippet.length })}</span>
            <button
              type="button"
              onClick={() => {
                void navigator.clipboard?.writeText(snippet);
                notify(t("instr.copied"));
              }}
              className="cursor-pointer rounded px-1.5 py-0.5 transition-colors hover:bg-surface-hover hover:text-text-strong"
            >
              {t("instr.copySnippet")}
            </button>
          </div>
        </Panel>

        <Panel title={t("instr.writeTargets")} subtitle={t("instr.writeTargetsHint")}>
          <div className="grid grid-cols-2 gap-2">
            {specs
              .filter((s) => s.kind === "rule")
              .map((spec) => (
                <div
                  key={spec.path}
                  className="flex items-center gap-2 rounded-lg border border-line bg-surface-2 px-2.5 py-2"
                >
                  <span
                    className={clsx(
                      "flex items-center rounded border px-1 py-[1px]",
                      agentInfo(spec.agent).accent
                    )}
                  >
                    <AgentIcon id={spec.agent} className="h-3.5 w-3.5" />
                  </span>
                  <span className="mono truncate text-[11px] text-text-muted">{spec.display}</span>
                  <Badge tone={spec.exists ? "ok" : "dim"} className="ml-auto">
                    {spec.exists ? t("instr.exists") : t("instr.creatable")}
                  </Badge>
                </div>
              ))}
          </div>
        </Panel>
      </div>

      <Panel
        title={t("instr.chain")}
        subtitle={t("instr.chainHint")}
        actions={<Link2 className="h-3.5 w-3.5 text-text-dim" />}
      >
        {chain.length === 0 ? (
          <Empty title={t("instr.chainEmpty")} hint={t("instr.chainEmptyHint")} />
        ) : (
          <ol className="space-y-1.5">
            {chain.map((entry) => (
              <li key={`${entry.agent}-${entry.path}`} className="relative">
                <div
                  className={clsx(
                    "rounded-lg border px-2.5 py-2 text-[11px]",
                    entry.exists
                      ? "border-line bg-surface-2"
                      : "border-dashed border-line bg-transparent"
                  )}
                >
                  <div className="flex items-center gap-1.5">
                    <span
                      className={clsx(
                        "flex items-center rounded border px-1 py-[1px]",
                        agentInfo(entry.agent).accent
                      )}
                    >
                      <AgentIcon id={entry.agent} className="h-3.5 w-3.5" />
                    </span>
                    <span className="mono truncate text-text-muted">
                      {basename(entry.path)}
                    </span>
                    {entry.managed && (
                      <Badge tone="brand">{t("instr.badge.managed")}</Badge>
                    )}
                  </div>
                  <p className="mt-1 text-[11.5px] leading-relaxed text-text-dim">{entry.note}</p>
                </div>
                {entry !== chain[chain.length - 1] && (
                  <ArrowDown className="mx-auto my-0.5 h-3 w-3 text-line-strong" />
                )}
              </li>
            ))}
          </ol>
        )}
        <Button
          className="mt-2 w-full"
          variant="outline"
          size="sm"
          onClick={() => notify(t("instr.bridgeCmd"))}
        >
          {t("instr.bridge")}
        </Button>
      </Panel>
    </div>
  );
}
