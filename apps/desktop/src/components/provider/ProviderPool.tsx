import { useMemo, useState } from "react";
import { clsx } from "clsx";
import { AlertTriangle, Check, Copy, Info, Loader2, Plus, SlidersHorizontal } from "lucide-react";
import { Badge, Button, Empty } from "../ui";
import { PoolCard } from "./PoolCard";
import { useT } from "../../lib/i18n";
import { tr } from "../../lib/i18n/runtime";
import type { MessageKey } from "../../lib/i18n/messages";
import type {
  ApplyReport,
  GatewayRouteView,
  PoolMode,
  ProviderBindingView,
  ProviderView,
} from "../../lib/types";

const MODES: { key: PoolMode; label: MessageKey; hint: MessageKey }[] = [
  { key: "direct", label: "pool.mode.direct", hint: "pool.mode.directHint" },
  { key: "route", label: "pool.mode.route", hint: "pool.mode.routeHint" },
  { key: "aggregate", label: "pool.mode.aggregate", hint: "pool.mode.aggregateHint" },
];

function reportOf(report?: ApplyReport): { tone: "ok" | "warn" | "danger" | "dim"; text: string } {
  if (!report) return { tone: "dim", text: "" };
  if (!report.probe) {
    return report.applied
      ? { tone: "warn", text: tr("pool.status.written") }
      : { tone: "dim", text: report.skipped ?? tr("pool.status.noChange") };
  }
  if (!report.probe.ok) {
    return {
      tone: "danger",
      text: report.applied
        ? tr("pool.status.appliedNoConn", { detail: report.probe.detail })
        : report.probe.detail,
    };
  }
  return {
    tone: "ok",
    text: report.applied
      ? tr("pool.status.applied", { detail: report.probe.detail })
      : tr("pool.status.upToDate", { detail: report.probe.detail }),
  };
}

/**
 * 右栏：当前 agent 的供应商池。
 *
 * 五段自上而下：操作条 → 统一网关 → 三模式 → 默认供应商 → 已添加 / 可添加。
 * 每个动作都会立刻写盘并测一次连通，用户不需要再点「应用」。
 */
export function ProviderPool({
  binding,
  providers,
  route,
  report,
  busy,
  gatewayRunning,
  onAdd,
  onRemove,
  onSetDefault,
  onMode,
  onOpenSlots,
  onAddProvider,
  onEdit,
  onStartGateway,
}: {
  binding: ProviderBindingView;
  providers: ProviderView[];
  /** 该 agent 当前的网关路由（由页面按 `binding.route_id` 查出）：成员权重/优先级从这里来 */
  route?: GatewayRouteView | null;
  report?: ApplyReport;
  busy: string | null;
  gatewayRunning: boolean;
  onAdd: (providerId: string) => void;
  onRemove: (providerId: string) => void;
  onSetDefault: (providerId: string) => void;
  onMode: (mode: PoolMode) => void;
  onOpenSlots: () => void;
  onAddProvider: () => void;
  onEdit: (provider: ProviderView) => void;
  onStartGateway: () => void;
}) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  /** Codex 的 export 行单独一个复制态，避免与网关地址那枚按钮互相串 */
  const [envCopied, setEnvCopied] = useState(false);

  const members = useMemo(
    () =>
      binding.pool
        .map((id) => providers.find((item) => item.id === id))
        .filter((item): item is ProviderView => Boolean(item)),
    [binding.pool, providers]
  );
  const available = useMemo(
    () => providers.filter((item) => !binding.pool.includes(item.id)),
    [providers, binding.pool]
  );
  const current = providers.find((item) => item.id === binding.provider_id);
  const status = reportOf(report);
  const slotCount = Object.keys(binding.model_slots ?? {}).length;

  const copy = (text: string) => {
    void navigator.clipboard?.writeText(text).then(() => {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    });
  };

  const copyEnv = (text: string) => {
    void navigator.clipboard?.writeText(text).then(() => {
      setEnvCopied(true);
      window.setTimeout(() => setEnvCopied(false), 1500);
    });
  };

  return (
    <section aria-busy={Boolean(busy)} className="flex min-w-0 flex-1 flex-col">
      <header className="flex flex-wrap items-center gap-2 border-b border-line px-4 py-2.5">
        <div className="min-w-0">
          <h2 className="text-[13px] font-semibold text-text-strong">{binding.display_name}</h2>
          <p className="mono mt-0.5 truncate text-[10.5px] text-text-dim">
            {binding.supported
              ? t("pool.token", { agent: binding.agent })
              : binding.unsupported_reason}
          </p>
        </div>

        <div className="ml-auto flex items-center gap-2">
          {status.text && (
            <span
              className={clsx(
                "flex items-center gap-1 text-[11px]",
                status.tone === "ok" && "text-state-ok",
                status.tone === "warn" && "text-state-warn",
                status.tone === "danger" && "text-state-danger",
                status.tone === "dim" && "text-text-dim"
              )}
              title={report?.probe?.endpoint}
            >
              {busy ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : status.tone === "ok" ? (
                <Check className="h-3 w-3" />
              ) : (
                <AlertTriangle className="h-3 w-3" />
              )}
              <span className="max-w-[280px] truncate">
                {busy ? t("pool.busy") : status.text}
              </span>
            </span>
          )}
          <Button size="sm" variant="primary" onClick={onAddProvider}>
            <Plus className="h-3 w-3" />
            {t("pool.addProvider")}
          </Button>
        </div>
      </header>

      {!binding.supported ? (
        <Empty
          title={t("pool.unsupportedTitle")}
          hint={binding.unsupported_reason ?? t("pool.unsupportedHint")}
        />
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
          {/* 统一网关 */}
          <div
            className={clsx(
              "mb-3 flex flex-wrap items-center gap-2 rounded-md border px-2.5 py-1.5",
              binding.mode === "direct"
                ? "border-line bg-surface-2/30"
                : "border-brand-500/30 bg-brand-500/[0.06]"
            )}
          >
            <span className="text-[10.5px] uppercase tracking-wider text-text-dim">
              {t("pool.gateway")}
            </span>
            <span
              className={clsx(
                "mono text-[11.5px]",
                binding.mode === "direct" ? "text-text-dim" : "text-text-strong"
              )}
            >
              {binding.gateway_base_url ?? "http://127.0.0.1:43117/v1"}
            </span>
            {binding.gateway_token && (
              <Badge tone="brand">Bearer {binding.gateway_token}</Badge>
            )}
            {binding.mode === "direct" ? (
              <span className="text-[10.5px] text-text-dim">{t("pool.directNoGateway")}</span>
            ) : (
              <button
                type="button"
                onClick={() =>
                  copy(`${binding.gateway_base_url ?? ""}\nBearer ${binding.gateway_token ?? ""}`)
                }
                className="flex cursor-pointer items-center gap-1 text-[10.5px] text-text-muted hover:text-brand-300"
              >
                {copied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
                {copied ? t("pool.copied") : t("pool.copy")}
              </button>
            )}
            {!gatewayRunning && binding.mode !== "direct" && (
              <button
                type="button"
                onClick={onStartGateway}
                className="ml-auto cursor-pointer text-[10.5px] text-state-warn hover:underline"
              >
                {t("pool.gatewayStart")}
              </button>
            )}
          </div>

          {/* 三模式 */}
          <div className="mb-4 flex items-center gap-1.5">
            <div className="inline-flex rounded-md border border-line p-0.5">
              {MODES.map((item) => (
                <button
                  key={item.key}
                  type="button"
                  disabled={Boolean(busy)}
                  onClick={() => onMode(item.key)}
                  className={clsx(
                    "flex items-center gap-1 rounded px-2.5 py-1 text-[11.5px] transition-colors duration-120",
                    binding.mode === item.key
                      ? "bg-brand-500/15 text-brand-200 shadow-[inset_0_0_0_1px_rgb(var(--brand-500)/0.4)]"
                      : "text-text-muted hover:text-text-strong",
                    busy ? "cursor-not-allowed" : "cursor-pointer"
                  )}
                >
                  {t(item.label)}
                  <span title={t(item.hint)} className="inline-flex">
                    <Info className="h-2.5 w-2.5 opacity-60" />
                  </span>
                </button>
              ))}
            </div>
            <span className="text-[10.5px] text-text-dim">
              {(() => {
                const hint = MODES.find((item) => item.key === binding.mode)?.hint;
                return hint ? t(hint) : null;
              })()}
            </span>
          </div>

          {/* 默认供应商 */}
          <div className="mb-4">
            <div className="mb-1.5 flex items-center gap-2">
              <span className="text-[10.5px] font-semibold uppercase tracking-wider text-text-dim">
                {t("pool.default")}
              </span>
              {binding.agent === "claude" && (
                <button
                  type="button"
                  onClick={onOpenSlots}
                  className="ml-auto flex cursor-pointer items-center gap-1 text-[10.5px] text-text-muted hover:text-brand-300"
                >
                  <SlidersHorizontal className="h-3 w-3" />
                  {t("pool.modelMapping")}
                  {slotCount > 0 && (
                    <Badge tone="brand">{t("pool.slots", { count: slotCount })}</Badge>
                  )}
                </button>
              )}
            </div>
            {current ? (
              <PoolCard
                provider={current}
                state="default"
                busy={busy === `default:${current.id}`}
                model={binding.model}
                member={route?.members.find((item) => item.provider_id === current.id) ?? null}
                onRemove={() => onRemove(current.id)}
                onEdit={() => onEdit(current)}
              />
            ) : (
              <div className="rounded-lg border border-dashed border-line px-3 py-3 text-center text-[11.5px] text-text-dim">
                {t("pool.noDefault")}
              </div>
            )}
            {binding.agent === "claude" && (
              <p className="mt-1.5 text-[10.5px] leading-relaxed text-text-dim">
                {t("pool.slotNote")}
                {slotCount === 0 && t("pool.slotNoteEmpty")}。
              </p>
            )}
            {binding.agent === "codex" && (
              <CodexEnvHint
                binding={binding}
                current={current ?? null}
                members={members}
                onCopy={copyEnv}
                copied={envCopied}
              />
            )}
          </div>

          {/* 已添加 */}
          <div className="mb-4">
            <div className="mb-1.5 flex items-center gap-2">
              <span className="text-[10.5px] font-semibold uppercase tracking-wider text-text-dim">
                {t("pool.added")}
              </span>
              <Badge tone="dim">{members.length}</Badge>
            </div>
            {members.length > 0 && (
              <p
                data-pool-legend
                className="mb-1.5 text-[10.5px] leading-relaxed text-text-dim"
              >
                {t("pool.weightNote")}
                {t("pool.weightNote2")}
              </p>
            )}
            {members.length === 0 ? (
              <p className="text-[11px] text-text-dim">{t("pool.emptyPool")}</p>
            ) : (
              <div className="grid grid-cols-1 gap-1.5 xl:grid-cols-2">
                {members.map((provider, index) => (
                  <PoolCard
                    key={provider.id}
                    provider={provider}
                    state={provider.id === binding.provider_id ? "default" : "member"}
                    busy={busy === `member:${provider.id}`}
                    member={route?.members.find((item) => item.provider_id === provider.id) ?? null}
                    order={index}
                    onRemove={() => onRemove(provider.id)}
                    onSetDefault={() => onSetDefault(provider.id)}
                    onEdit={() => onEdit(provider)}
                  />
                ))}
              </div>
            )}
          </div>

          {/* 可添加 */}
          <div>
            <div className="mb-1.5 flex items-center gap-2">
              <span className="text-[10.5px] font-semibold uppercase tracking-wider text-text-dim">
                {t("pool.available")}
              </span>
              <Badge tone="dim">{available.length}</Badge>
            </div>
            {available.length === 0 ? (
              <p className="text-[11px] text-text-dim">{t("pool.allAdded")}</p>
            ) : (
              <div className="grid grid-cols-1 gap-1.5 xl:grid-cols-2">
                {available.map((provider) => (
                  <PoolCard
                    key={provider.id}
                    provider={provider}
                    state="available"
                    busy={busy === `add:${provider.id}`}
                    onAdd={() => onAdd(provider.id)}
                    onEdit={() => onEdit(provider)}
                  />
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </section>
  );
}

/**
 * Codex 的密钥提示。
 *
 * `ext/provider.rs` 里 Codex 的目标键是 `model_providers.<id>.env_key`——写进去的是**变量名**，
 * 真值只能由启动 Codex 的那个 shell 提供。直连时要填供应商密钥，经网关时值就是 agent 名
 * （网关的令牌 = agent slug，等于本机回环上的无鉴权设计）。这里只显示变量名与用法，
 * 不回显、不复制密钥本身。
 */
function CodexEnvHint({
  binding,
  current,
  members,
  onCopy,
  copied,
}: {
  binding: ProviderBindingView;
  current: ProviderView | null;
  members: ProviderView[];
  onCopy: (text: string) => void;
  copied: boolean;
}) {
  const t = useT();
  const viaGateway = binding.mode !== "direct";
  const envVar =
    current && current.api_key.startsWith("env:") ? current.api_key.slice(4).trim() : null;
  // `env:` 之外的形式（keychain / literal）Codex 认不了，写进去只会 401
  const incompatible = members.filter((item) => !item.api_key.startsWith("env:"));

  if (!current) {
    return (
      <p
        data-codex-env
        data-codex-env-kind="empty"
        className="mt-1.5 text-[10.5px] leading-relaxed text-text-dim"
      >
        {t("pool.codexNote")}
      </p>
    );
  }

  const value = viaGateway ? (binding.gateway_token ?? "codex") : "";
  const line = envVar ? `export ${envVar}=${value}` : null;

  return (
    <div
      data-codex-env
      data-codex-env-kind={envVar ? (viaGateway ? "gateway" : "direct") : "warning"}
      data-codex-env-var={envVar ?? ""}
      className={clsx(
        "mt-1.5 rounded-md border px-2.5 py-2 text-[10.5px] leading-relaxed",
        envVar
          ? "border-line bg-surface-2/30 text-text-dim"
          : "border-state-warn/40 bg-state-warn/[0.06] text-text-muted"
      )}
    >
      <div className="flex items-center gap-1.5">
        <span className="font-semibold text-text-muted">{t("pool.codexKey")}</span>
        <span className="text-text-dim">{t("pool.codexEnvOnly")}</span>
        {line && (
          <button
            type="button"
            onClick={() => onCopy(line)}
            className="ml-auto flex cursor-pointer items-center gap-1 text-text-muted hover:text-brand-300"
          >
            {copied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
            {copied ? t("pool.copied") : t("pool.copy")}
          </button>
        )}
      </div>

      {envVar ? (
        <>
          <code className="mono mt-1 block break-all text-[11px] text-text-strong">{line}</code>
          <span>
            {t("pool.runOnce")}
            {viaGateway ? t("pool.runOnceGateway") : t("pool.runOnceValue")}
            {t("pool.persistHint")}
          </span>
        </>
      ) : (
        <span>
          {t("pool.keyFormNote", { prefix: `${current.api_key.split(":")[0]}:` })}
        </span>
      )}

      {incompatible.length > 0 && (
        <p data-codex-env-warn className="mt-1 text-state-warn/90">
          {t("pool.incompatible", {
            names: incompatible.map((item) => item.name).join(" / "),
          })}
        </p>
      )}
    </div>
  );
}
