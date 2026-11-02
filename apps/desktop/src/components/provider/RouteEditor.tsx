import { useMemo, useState } from "react";
import { clsx } from "clsx";
import { Plus, X } from "lucide-react";
import { Badge, Button, Toggle } from "../ui";
import { useHub } from "../../lib/useHub";
import { useT } from "../../lib/i18n";
import type { GatewayProtocol, GatewayRouteView, ProviderView, RouteMode } from "../../lib/types";

const input =
  "h-8 w-full rounded-md border border-line bg-ink-950/70 px-2 text-[12px] text-text-strong placeholder:text-text-dim/70 outline-none transition-colors focus:border-brand-500/60 focus:ring-2 focus:ring-brand-500/15";
const mono = "font-mono text-[11px]";

function emptyRoute(): GatewayRouteView {
  return {
    id: "",
    name: "",
    protocol: "anthropic",
    mode: "single",
    members: [],
    rules: [],
    retry: {
      max_attempts: 3,
      first_byte_ms: 120_000,
      idle_ms: 180_000,
      retry_status: [429, 500, 502, 503, 504],
    },
    enabled: true,
  };
}

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-[11.5px] text-text-dim">{label}</span>
      {children}
      {hint && <span className="text-[11px] text-text-dim">{hint}</span>}
    </label>
  );
}

/**
 * 路由编辑抽屉。四段：基本 / 成员 / 规则 / 重试。
 *
 * 校验分两层：这里给即时反馈（id 格式、至少一个成员），后端 `validate_route`
 * 仍是最终门禁 —— 前端只负责让用户少撞一次墙。
 */
export function RouteEditor({
  route,
  providers,
  onClose,
}: {
  /** null = 新建 */
  route: GatewayRouteView | null;
  providers: ProviderView[];
  onClose: () => void;
}) {
  const { saveGatewayRoute, notify } = useHub();
  const t = useT();
  const [draft, setDraft] = useState<GatewayRouteView>(() => route ?? emptyRoute());
  const [modelsText, setModelsText] = useState<Record<string, string>>(() =>
    Object.fromEntries((route?.members ?? []).map((m) => [m.provider_id, m.models.join(", ")]))
  );
  const [newPattern, setNewPattern] = useState("");
  const [newTarget, setNewTarget] = useState("");
  const [busy, setBusy] = useState(false);

  const isNew = route === null;
  const patch = (next: Partial<GatewayRouteView>) => setDraft((prev) => ({ ...prev, ...next }));

  /**
   * 校验结果分成「阻断」与「建议」两类。
   *
   * 原来靠文案里有没有「建议」两个字来区分阻断项，文案一旦本地化就失效，
   * 所以这里直接带上 `blocking` 标记，提交时只看这个字段。
   */
  const problems = useMemo<{ text: string; blocking: boolean }[]>(() => {
    const list: { text: string; blocking: boolean }[] = [];
    if (!/^[a-z0-9-]+$/.test(draft.id))
      list.push({ text: t("route.err.id"), blocking: true });
    if (!draft.name.trim()) list.push({ text: t("route.err.name"), blocking: true });
    if (draft.members.length === 0)
      list.push({ text: t("route.err.members"), blocking: true });
    if (draft.mode === "route" && draft.rules.length === 0)
      list.push({ text: t("route.err.rules"), blocking: false });
    return list;
    // t 随语言变化，但只要 draft 不变，文案也不必重算
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draft]);

  const toggleMember = (providerId: string) => {
    const exists = draft.members.find((m) => m.provider_id === providerId);
    patch({
      members: exists
        ? draft.members.filter((m) => m.provider_id !== providerId)
        : [
            ...draft.members,
            {
              provider_id: providerId,
              weight: 1,
              priority: draft.members.length + 1,
              models: [],
              model_map: {},
              enabled: true,
            },
          ],
    });
  };

  const commit = () => {
    const blocking = problems.filter((item) => item.blocking);
    if (blocking.length) {
      notify(blocking[0].text);
      return;
    }
    const members = draft.members.map((member) => ({
      ...member,
      models: (modelsText[member.provider_id] ?? "")
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean),
    }));
    setBusy(true);
    void saveGatewayRoute({ ...draft, name: draft.name.trim(), members }).then((ok) => {
      setBusy(false);
      if (ok) onClose();
    });
  };

  return (
    <div className="fixed inset-0 z-40 flex justify-end bg-black/30" onClick={onClose}>
      <aside
        className="flex h-full w-[440px] max-w-[92vw] flex-col border-l border-line bg-ink-900 shadow-panel"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="flex items-center justify-between gap-2 border-b border-line px-4 py-3">
          <div>
            <h2 className="text-[13.5px] font-medium text-text-strong">
              {isNew ? t("route.title.new") : t("route.title.edit", { name: route.name })}
            </h2>
            <p className="mt-0.5 text-[11px] text-text-dim">
              {t("route.endpoint", { id: draft.id || "<id>" })}
            </p>
          </div>
          <Button size="sm" variant="ghost" onClick={onClose} title={t("route.close")}>
            <X className="h-3.5 w-3.5" />
          </Button>
        </header>

        <div className="flex-1 space-y-4 overflow-y-auto px-4 py-3">
          <section className="space-y-2">
            <p className="text-[11px] font-semibold uppercase tracking-wider text-text-dim">
              {t("route.section.basic")}
            </p>
            <div className="grid grid-cols-2 gap-2">
              <Field label={t("route.field.id")} hint={t("route.field.idHint")}>
                <input
                  value={draft.id}
                  disabled={!isNew}
                  onChange={(event) => patch({ id: event.target.value.toLowerCase() })}
                  placeholder="claude-main"
                  className={clsx(input, mono, !isNew && "opacity-60")}
                />
              </Field>
              <Field label={t("route.field.name")}>
                <input
                  value={draft.name}
                  onChange={(event) => patch({ name: event.target.value })}
                  placeholder={t("route.field.namePlaceholder")}
                  className={input}
                />
              </Field>
              <Field label={t("route.field.protocol")}>
                <select
                  value={draft.protocol}
                  onChange={(event) => patch({ protocol: event.target.value as GatewayProtocol })}
                  className={input}
                >
                  <option value="anthropic">Anthropic Messages</option>
                  <option value="chat_completions">Chat Completions</option>
                  <option value="responses">{t("route.protocol.responses")}</option>
                </select>
              </Field>
              <Field label={t("route.field.mode")}>
                <select
                  value={draft.mode}
                  onChange={(event) => patch({ mode: event.target.value as RouteMode })}
                  className={input}
                >
                  <option value="single">{t("route.mode.single")}</option>
                  <option value="route">{t("route.mode.route")}</option>
                  <option value="aggregate">{t("route.mode.aggregate")}</option>
                </select>
              </Field>
            </div>
            <div className="flex items-center justify-between gap-3 rounded-md border border-line px-2.5 py-2">
              <span className="text-[11.5px] text-text-muted">{t("route.enabled")}</span>
              <Toggle checked={draft.enabled} onChange={(next) => patch({ enabled: next })} />
            </div>
          </section>

          <section className="space-y-2">
            <p className="text-[11px] font-semibold uppercase tracking-wider text-text-dim">
              {t("route.members")}
            </p>
            {providers.length === 0 && (
              <p className="text-[11.5px] text-text-dim">{t("route.membersEmpty")}</p>
            )}
            {providers.map((provider) => {
              const member = draft.members.find((m) => m.provider_id === provider.id);
              // 供应商未声明协议时跟随本路由，属于「直通」
              const upstream = provider.protocol ?? draft.protocol;
              const mode =
                upstream === draft.protocol
                  ? "pass"
                  : (draft.protocol === "anthropic" && upstream === "chat_completions") ||
                      (draft.protocol === "chat_completions" && upstream === "anthropic")
                    ? "convert"
                    : "unsupported";
              return (
                <div
                  key={provider.id}
                  className={clsx(
                    "rounded-md border border-line px-2.5 py-2",
                    mode === "unsupported" && "opacity-60"
                  )}
                >
                  <label className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={Boolean(member)}
                      disabled={mode === "unsupported"}
                      onChange={() => toggleMember(provider.id)}
                    />
                    <span className="text-[12px] text-text-strong">{provider.name}</span>
                    {mode === "convert" && (
                      <Badge tone="warn">{t("route.needConvert")}</Badge>
                    )}
                    <Badge tone="dim" className="ml-auto">
                      {provider.id}
                    </Badge>
                  </label>
                  {mode === "unsupported" && (
                    <p className="mt-1 text-[11px] leading-relaxed text-state-danger">
                      {t("route.incompatible")}
                    </p>
                  )}
                  {member && (
                    <div className="mt-2 grid grid-cols-2 gap-2">
                      <Field label={t("route.field.weight")}>
                        <input
                          type="number"
                          min={1}
                          value={member.weight}
                          onChange={(event) =>
                            patch({
                              members: draft.members.map((m) =>
                                m.provider_id === provider.id
                                  ? { ...m, weight: Math.max(1, Number(event.target.value) || 1) }
                                  : m
                              ),
                            })
                          }
                          className={clsx(input, mono)}
                        />
                      </Field>
                      <Field label={t("route.field.priority")}>
                        <input
                          type="number"
                          min={1}
                          value={member.priority}
                          onChange={(event) =>
                            patch({
                              members: draft.members.map((m) =>
                                m.provider_id === provider.id
                                  ? { ...m, priority: Math.max(1, Number(event.target.value) || 1) }
                                  : m
                              ),
                            })
                          }
                          className={clsx(input, mono)}
                        />
                      </Field>
                      <div className="col-span-2">
                        <Field label={t("route.field.allowlist")}>
                          <input
                            value={modelsText[provider.id] ?? ""}
                            onChange={(event) =>
                              setModelsText((prev) => ({ ...prev, [provider.id]: event.target.value }))
                            }
                            placeholder={provider.models.join(", ")}
                            className={clsx(input, mono)}
                          />
                        </Field>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </section>

          <section className="space-y-2">
            <p className="text-[11px] font-semibold uppercase tracking-wider text-text-dim">
              {t("route.rules")}
            </p>
            {draft.rules.map((rule) => (
              <div
                key={`${rule.pattern}-${rule.provider_ids.join()}`}
                className="flex items-center gap-2 rounded-md border border-line px-2 py-1.5"
              >
                <span className={clsx(mono, "text-text-strong")}>{rule.pattern}</span>
                <span className="text-[11px] text-text-dim">→</span>
                <span className="truncate text-[11.5px] text-text-muted">
                  {rule.provider_ids.join(" / ")}
                </span>
                <button
                  type="button"
                  onClick={() => patch({ rules: draft.rules.filter((r) => r !== rule) })}
                  className="ml-auto cursor-pointer text-text-dim hover:text-state-danger"
                  title={t("route.rule.remove")}
                >
                  <X className="h-3 w-3" />
                </button>
              </div>
            ))}
            <div className="flex items-end gap-1.5">
              <Field label={t("route.field.pattern")}>
                <input
                  value={newPattern}
                  onChange={(event) => setNewPattern(event.target.value)}
                  placeholder="claude-opus*"
                  className={clsx(input, mono)}
                />
              </Field>
              <Field label={t("route.field.target")}>
                <select
                  value={newTarget}
                  onChange={(event) => setNewTarget(event.target.value)}
                  className={input}
                >
                  <option value="">{t("route.target.pick")}</option>
                  {draft.members.map((member) => (
                    <option key={member.provider_id} value={member.provider_id}>
                      {providers.find((p) => p.id === member.provider_id)?.name ?? member.provider_id}
                    </option>
                  ))}
                </select>
              </Field>
              <Button
                size="sm"
                variant="outline"
                disabled={!newPattern.trim() || !newTarget}
                onClick={() => {
                  patch({
                    rules: [
                      ...draft.rules,
                      { pattern: newPattern.trim(), provider_ids: [newTarget] },
                    ],
                  });
                  setNewPattern("");
                  setNewTarget("");
                }}
              >
                <Plus className="h-3 w-3" />
              </Button>
            </div>
          </section>

          <section className="space-y-2">
            <p className="text-[11px] font-semibold uppercase tracking-wider text-text-dim">
              {t("route.retry")}
            </p>
            <div className="grid grid-cols-2 gap-2">
              <Field label={t("route.field.attempts")}>
                <input
                  type="number"
                  min={1}
                  max={6}
                  value={draft.retry.max_attempts}
                  onChange={(event) =>
                    patch({
                      retry: {
                        ...draft.retry,
                        max_attempts: Math.min(6, Math.max(1, Number(event.target.value) || 1)),
                      },
                    })
                  }
                  className={clsx(input, mono)}
                />
              </Field>
              <Field label={t("route.field.firstByteTimeout")}>
                <input
                  type="number"
                  min={1000}
                  step={1000}
                  value={draft.retry.first_byte_ms}
                  onChange={(event) =>
                    patch({
                      retry: { ...draft.retry, first_byte_ms: Number(event.target.value) || 120000 },
                    })
                  }
                  className={clsx(input, mono)}
                />
              </Field>
            </div>
            <Field label={t("route.field.retryStatus")}>
              <input
                value={draft.retry.retry_status.join(", ")}
                onChange={(event) =>
                  patch({
                    retry: {
                      ...draft.retry,
                      retry_status: event.target.value
                        .split(",")
                        .map((item) => Number(item.trim()))
                        .filter((code) => Number.isFinite(code) && code >= 100 && code < 600),
                    },
                  })
                }
                className={clsx(input, mono)}
              />
            </Field>
          </section>

          {problems.length > 0 && (
            <ul className="space-y-1 rounded-md border border-state-warn/30 bg-state-warn/10 px-2.5 py-2">
              {problems.map((item) => (
                <li key={item.text} className="text-[11px] leading-relaxed text-state-warn">
                  {item.text}
                </li>
              ))}
            </ul>
          )}
        </div>

        <footer className="flex items-center justify-between gap-2 border-t border-line px-4 py-3">
          <span className="text-[11px] text-text-dim">
            {t("route.summary", { members: draft.members.length, rules: draft.rules.length })}
          </span>
          <div className="flex items-center gap-1.5">
            <Button size="sm" variant="ghost" onClick={onClose} disabled={busy}>
              {t("common.cancel")}
            </Button>
            <Button size="sm" variant="primary" onClick={commit} disabled={busy}>
              {busy ? t("route.saving") : t("route.save")}
            </Button>
          </div>
        </footer>
      </aside>
    </div>
  );
}
