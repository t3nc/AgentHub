import { useState } from "react";
import { clsx } from "clsx";
import { ChevronDown, Info, Loader2, X } from "lucide-react";
import { Badge, Button } from "../ui";
import { OFFICIAL_ALIAS, SLOT_COMMAND, SLOT_KEYS, SLOT_LABEL } from "../../lib/supplier";
import { useT } from "../../lib/i18n";
import type { ModelSlots, ProviderView } from "../../lib/types";

const input =
  "h-8 w-full rounded-md border border-line bg-ink-950/70 px-2 font-mono text-[11.5px] text-text-strong outline-none transition-colors focus:border-brand-500/60";

/**
 * 模型槽位映射抽屉。
 *
 * 目的只有一个：让 Claude Code 里的 `/model opus` 真的换到另一个上游模型。
 * 每个槽位留「跟随默认」选项，不配也能用（行为与改造前一致）。
 */
export function ModelSlotDrawer({
  provider,
  slots,
  mode,
  defaultModel,
  onClose,
  onSave,
  saving,
}: {
  provider: ProviderView | null;
  slots: ModelSlots;
  /** 经网关时展示的是官方别名，直连展示上游模型名 */
  mode: "direct" | "route" | "aggregate";
  defaultModel: string | null;
  onClose: () => void;
  onSave: (slots: ModelSlots) => void;
  saving: boolean;
}) {
  const t = useT();
  const [draft, setDraft] = useState<ModelSlots>({ ...slots });
  const [advanced, setAdvanced] = useState(false);

  const options = provider?.models ?? [];
  const fallback = defaultModel || options[0] || t("slots.noModel");
  const unsupported = provider?.protocol === "responses";

  const set = (slot: string, value: string) => {
    setDraft((prev) => {
      const next = { ...prev };
      if (value) next[slot] = value;
      else delete next[slot];
      return next;
    });
  };

  const row = (slot: (typeof SLOT_KEYS)[number]) => (
    <div key={slot} className="flex items-center gap-2">
      <span className="w-[62px] shrink-0 text-[11.5px] text-text-muted">{SLOT_LABEL[slot]}</span>
      <select
        value={draft[slot] ?? ""}
        onChange={(event) => set(slot, event.target.value)}
        className={input}
        aria-label={t("slots.aria", { slot: SLOT_LABEL[slot] })}
      >
        <option value="">{t("slots.followDefault", { model: fallback })}</option>
        {options.map((model) => (
          <option key={model} value={model}>
            {model}
          </option>
        ))}
      </select>
    </div>
  );

  return (
    <div className="fixed inset-0 z-40 flex justify-end bg-black/30" onClick={onClose}>
      <aside
        className="flex h-full w-[440px] max-w-[94vw] flex-col border-l border-line bg-ink-900 shadow-panel"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="flex items-center gap-2 border-b border-line px-4 py-3">
          <div className="min-w-0 flex-1">
            <h2 className="text-[13.5px] font-medium text-text-strong">{t("slots.title")}</h2>
            <p className="mt-0.5 truncate text-[11px] text-text-dim">
              {t("slots.subtitle", {
                provider: provider ? provider.name : t("slots.noProvider"),
              })}
            </p>
          </div>
          <Button size="sm" variant="ghost" onClick={onClose} title={t("slots.close")}>
            <X className="h-3.5 w-3.5" />
          </Button>
        </header>

        <div className="min-h-0 flex-1 space-y-3 overflow-y-auto px-4 py-3">
          {!provider ? (
            <p className="text-[11.5px] text-text-dim">{t("slots.needDefault")}</p>
          ) : unsupported ? (
            <p className="rounded-md border border-state-warn/30 bg-state-warn/10 px-2.5 py-2 text-[11px] leading-relaxed text-state-warn">
              {t("slots.responsesBlocked")}
            </p>
          ) : (
            <>
              <div className="space-y-2 rounded-md border border-line px-2.5 py-2.5">
                {SLOT_KEYS.filter((slot) => slot !== "fable").map(row)}
              </div>

              <div className="rounded-md border border-line">
                <button
                  type="button"
                  onClick={() => setAdvanced(!advanced)}
                  className="flex w-full cursor-pointer items-center gap-1.5 px-2.5 py-2 text-left"
                >
                  <ChevronDown
                    className={clsx(
                      "h-3 w-3 text-text-dim transition-transform duration-120",
                      advanced && "rotate-180"
                    )}
                  />
                  <span className="text-[11.5px] text-text-muted">{t("slots.advanced")}</span>
                  <span className="text-[10.5px] text-text-dim">{t("slots.fableFamily")}</span>
                </button>
                {advanced && (
                  <div className="space-y-2 border-t border-line px-2.5 py-2.5">
                    {row("fable")}
                  </div>
                )}
              </div>

              <div className="rounded-md bg-surface-2/40 px-2.5 py-2">
                <p className="flex items-center gap-1 text-[10.5px] text-text-dim">
                  <Info className="h-3 w-3" />
                  {mode === "direct" ? t("slots.directNote") : t("slots.gatewayNote")}
                </p>
                <ul className="mt-1.5 space-y-0.5">
                  {SLOT_KEYS.map((slot) => {
                    const target = draft[slot] ?? fallback;
                    return (
                      <li key={slot} className="mono text-[10.5px] text-text-dim">
                        {SLOT_COMMAND[slot]} → {target}
                        {mode !== "direct" && (
                          <span className="ml-1 text-text-dim/60">
                            {t("slots.aliasMapped", { alias: OFFICIAL_ALIAS[slot] })}
                          </span>
                        )}
                      </li>
                    );
                  })}
                </ul>
              </div>

              <p className="text-[10.5px] leading-relaxed text-text-dim">
                {t("slots.cleanupNote")}
              </p>
            </>
          )}
        </div>

        <footer className="flex items-center justify-between gap-2 border-t border-line px-4 py-3">
          <span className="flex items-center gap-1.5 text-[11px] text-text-dim">
            {t("slots.configured")}{" "}
            <Badge tone="brand">{Object.keys(draft).length}</Badge> / 4
          </span>
          <div className="flex items-center gap-1.5">
            <Button size="sm" variant="ghost" onClick={onClose} disabled={saving}>
              {t("common.cancel")}
            </Button>
            <Button
              size="sm"
              variant="primary"
              onClick={() => onSave(draft)}
              disabled={saving || !provider || unsupported}
            >
              {saving ? (
                <>
                  <Loader2 className="h-3 w-3 animate-spin" />
                  {t("slots.saving")}
                </>
              ) : (
                t("slots.save")
              )}
            </Button>
          </div>
        </footer>
      </aside>
    </div>
  );
}
