import { useState } from "react";
import { clsx } from "clsx";
import { Pencil, Plus, Trash2 } from "lucide-react";
import { Badge, Button, Empty } from "../ui";
import { SupplierWizard } from "./SupplierWizard";
import { useHub } from "../../lib/useHub";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import type { GatewayProtocol, ProviderView, ProvidersView } from "../../lib/types";

const PROTOCOL_LABEL: Record<GatewayProtocol, string> = {
  anthropic: "Anthropic",
  chat_completions: "Chat",
  responses: "Responses",
};

/** `env:` / `keychain:` 是引用（安全），其余是明文（要提醒）。 */
function keyTone(apiKey: string): "ok" | "warn" {
  return apiKey.startsWith("env:") || apiKey.startsWith("keychain:") ? "ok" : "warn";
}

/** `env:` / `keychain:` 的标签本身就是技术名词，保持原样，只有明文需要翻译。 */
function keyLabel(apiKey: string): MessageKey | "env" | "keychain" {
  if (apiKey.startsWith("env:")) return "env";
  if (apiKey.startsWith("keychain:")) return "keychain";
  return "panel.keyClass.plain";
}

/**
 * 供应商列表。
 *
 * 只负责「看」和「进向导」：所有字段编辑都在 `SupplierWizard` 里，
 * 避免列表行内塞一整套表单（字段已经多到行内放不下）。
 */
export function ProviderPanel({
  view,
  showBindings = true,
}: {
  view: ProvidersView;
  /** Provider 页把绑定拆到独立视图，这里只留供应商登记 */
  showBindings?: boolean;
}) {
  const { deleteProvider } = useHub();
  const t = useT();
  const [wizard, setWizard] = useState<{ open: boolean; editing: ProviderView | null }>({
    open: false,
    editing: null,
  });
  const [pendingDelete, setPendingDelete] = useState<string | null>(null);

  return (
    <div className="flex flex-col">
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-line px-3 py-2">
        <div>
          <p className="text-[12px] font-medium text-text-strong">{t("panel.title")}</p>
          <p className="mt-0.5 text-[11px] text-text-dim">{t("panel.hint")}</p>
        </div>
        <Button
          size="sm"
          variant="primary"
          onClick={() => setWizard({ open: true, editing: null })}
        >
          <Plus className="h-3 w-3" />
          {t("panel.add")}
        </Button>
      </div>

      {view.providers.length === 0 ? (
        <Empty title={t("panel.emptyTitle")} hint={t("panel.emptyHint")} />
      ) : (
        <div className="divide-y divide-line">
          {view.providers.map((provider) => (
            <div
              key={provider.id}
              className="flex flex-wrap items-center gap-x-3 gap-y-1 px-3 py-2 transition-colors duration-120 hover:bg-surface-hover/40"
            >
              <span className="flex w-[132px] shrink-0 items-center gap-1.5">
                <span className="truncate text-[12px] text-text-strong">{provider.name}</span>
                {provider.catalog_id && (
                  <Badge
                    tone="dim"
                    title={t("panel.builtinTitle", { id: provider.catalog_id })}
                  >
                    {t("panel.builtin")}
                  </Badge>
                )}
              </span>

              <Badge tone={provider.protocol ? "brand" : "dim"}>
                {provider.protocol
                  ? PROTOCOL_LABEL[provider.protocol]
                  : t("panel.undeclared")}
              </Badge>

              <span className="mono min-w-0 flex-1 truncate text-[11px] text-text-muted">
                {provider.base_url}
              </span>

              <span className="shrink-0 text-[11px] text-text-dim">
                {t("panel.models", { count: provider.models.length })}
              </span>

              {provider.limits_summary && (
                <span className="shrink-0 text-[11px] text-text-dim">
                  {provider.limits_summary}
                </span>
              )}

              <Badge tone={keyTone(provider.api_key)} title={provider.api_key}>
                {(() => {
                  const label = keyLabel(provider.api_key);
                  return label === "env" || label === "keychain" ? label : t(label);
                })()}
              </Badge>

              <div className="ml-auto flex shrink-0 items-center gap-0.5">
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setWizard({ open: true, editing: provider })}
                  title={t("panel.edit")}
                >
                  <Pencil className="h-3 w-3" />
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setPendingDelete(provider.id)}
                  title={t("panel.delete")}
                >
                  <Trash2 className="h-3 w-3" />
                </Button>
              </div>

              {pendingDelete === provider.id && (
                <div className="flex w-full items-center justify-end gap-1.5 pt-1">
                  <span className="text-[11px] text-state-warn">{t("panel.deleteNote")}</span>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => {
                      setPendingDelete(null);
                      void deleteProvider(provider.id);
                    }}
                  >
                    {t("panel.confirmDelete")}
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setPendingDelete(null)}>
                    {t("common.cancel")}
                  </Button>
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {showBindings && (
        <div className="border-t border-line px-3 py-2">
          <p className="text-[11px] text-text-dim">{t("panel.footnote")}</p>
        </div>
      )}

      {wizard.open && (
        <SupplierWizard
          editing={wizard.editing}
          providers={view.providers}
          onClose={() => setWizard({ open: false, editing: null })}
        />
      )}
    </div>
  );
}
