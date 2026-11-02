import { useCallback, useEffect, useState } from "react";
import { clsx } from "clsx";
import { X } from "lucide-react";
import { api } from "../../lib/api";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import { Badge, Button } from "../ui";
import type { ModelPrice, ModelPriceInput, PricingView } from "../../lib/types";

const EMPTY_ROW: ModelPriceInput = {
  model: "",
  provider: null,
  input: 0,
  output: 0,
  cache_read: 0,
  cache_write: 0,
  currency: "USD",
};

const NUMERIC_FIELDS: {
  key: "input" | "output" | "cache_read" | "cache_write";
  label: MessageKey;
}[] = [
  { key: "input", label: "pricing.col.input" },
  { key: "output", label: "pricing.col.output" },
  { key: "cache_read", label: "pricing.col.cacheRead" },
  { key: "cache_write", label: "pricing.col.cacheWrite" },
];

function toInput(price: ModelPrice): ModelPriceInput {
  return {
    model: price.model,
    provider: price.provider ?? null,
    input: price.input,
    output: price.output,
    cache_read: price.cache_read,
    cache_write: price.cache_write,
    currency: price.currency,
  };
}

/**
 * 价格设置浮层。
 *
 * 两条写入路径都只在用户点击时发生：手填后「保存」，或从公开源「导入」并二次确认。
 * 这张表属于配置登记（不是对 agent 目录的改写），所以没有 Plan/Apply 抽屉。
 */
export function PricingDialog({
  onClose,
  notify,
}: {
  onClose: () => void;
  notify: (message: string) => void;
}) {
  const t = useT();
  const [view, setView] = useState<PricingView | null>(null);
  const [rows, setRows] = useState<ModelPriceInput[]>([]);
  const [busy, setBusy] = useState(false);
  const [preview, setPreview] = useState<{ source: string; rows: ModelPrice[] } | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());

  const load = useCallback(async () => {
    const next = await api.usagePricing();
    setView(next);
    setRows(next.prices.map(toInput));
  }, []);

  useEffect(() => {
    // 控制台日志是给开发者看的，不参与界面本地化
    void load().catch((err) => console.error("failed to load pricing", err));
  }, [load]);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      if (preview) setPreview(null);
      else onClose();
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [onClose, preview]);

  const patch = (index: number, next: Partial<ModelPriceInput>) => {
    setRows((prev) => prev.map((row, i) => (i === index ? { ...row, ...next } : row)));
  };

  const save = async () => {
    setBusy(true);
    try {
      const next = await api.saveUsagePricing(rows.filter((row) => row.model.trim()));
      setView(next);
      setRows(next.prices.map(toInput));
      notify(t("pricing.saved", { count: next.prices.length }));
    } catch (err) {
      notify(
        t("pricing.saveFailed", {
          error: err instanceof Error ? err.message : String(err),
        })
      );
    } finally {
      setBusy(false);
    }
  };

  const runImport = async (source: string) => {
    setBusy(true);
    try {
      const candidates = await api.previewUsagePricing(source);
      setPreview({ source, rows: candidates });
      setPicked(new Set(candidates.map((row) => `${row.model}|${row.provider ?? ""}`)));
    } catch (err) {
      notify(
        t("pricing.importFailed", {
          error: err instanceof Error ? err.message : String(err),
        })
      );
    } finally {
      setBusy(false);
    }
  };

  const applyImport = async () => {
    if (!preview) return;
    setBusy(true);
    try {
      const chosen = preview.rows.filter((row) => picked.has(`${row.model}|${row.provider ?? ""}`));
      const next = await api.applyUsagePricing(chosen);
      setView(next);
      setRows(next.prices.map(toInput));
      setPreview(null);
      notify(t("pricing.imported", { count: chosen.length }));
    } catch (err) {
      notify(
        t("pricing.applyFailed", {
          error: err instanceof Error ? err.message : String(err),
        })
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-scrim/60 p-6" data-pricing-dialog>
      <div className="panel flex max-h-full w-[860px] flex-col overflow-hidden">
        <header className="flex items-start justify-between gap-4 border-b border-line px-4 py-3">
          <div>
            <h2 className="text-[15px] font-medium text-text-strong">{t("pricing.title")}</h2>
            <p className="mt-0.5 text-[11.5px] text-text-dim">{t("pricing.subtitle")}</p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="cursor-pointer rounded-md p-1 text-text-dim transition-colors hover:bg-surface-hover hover:text-text-strong"
            aria-label={t("pricing.close")}
          >
            <X className="h-4 w-4" />
          </button>
        </header>

        <div className="flex items-center gap-2 border-b border-line px-4 py-2">
          <span className="text-[11.5px] text-text-dim">{t("pricing.importFrom")}</span>
          {(view?.sources ?? []).map((source) => (
            <Button
              key={source.key}
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => void runImport(source.key)}
              title={source.url}
            >
              {source.label}
            </Button>
          ))}
          <span className="ml-auto text-[11px] text-text-dim">{t("pricing.network")}</span>
        </div>

        <div className="min-h-0 flex-1 overflow-auto px-4 py-3">
          {preview ? (
            <div>
              <div className="mb-2 flex items-center gap-2 text-[12px] text-text-strong">
                <Badge tone="info">{preview.source}</Badge>
                {t("pricing.preview", { count: preview.rows.length })}
              </div>
              <div className="max-h-[380px] overflow-auto rounded-md border border-line">
                <table className="w-full border-collapse text-[11.5px]">
                  <thead className="sticky top-0 bg-surface-2">
                    <tr className="text-left text-text-dim">
                      <th className="w-8 px-2 py-1.5" />
                      <th className="px-2 py-1.5">{t("pricing.col.model")}</th>
                      <th className="px-2 py-1.5">provider</th>
                      <th className="px-2 py-1.5 text-right">{t("pricing.col.input")}</th>
                      <th className="px-2 py-1.5 text-right">{t("pricing.col.output")}</th>
                      <th className="px-2 py-1.5 text-right">{t("pricing.col.cacheRead")}</th>
                      <th className="px-2 py-1.5 text-right">{t("pricing.col.cacheWrite")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {preview.rows.map((row) => {
                      const key = `${row.model}|${row.provider ?? ""}`;
                      return (
                        <tr key={key} className="border-t border-line/60">
                          <td className="px-2 py-1.5">
                            <input
                              type="checkbox"
                              checked={picked.has(key)}
                              onChange={() =>
                                setPicked((prev) => {
                                  const next = new Set(prev);
                                  if (next.has(key)) next.delete(key);
                                  else next.add(key);
                                  return next;
                                })
                              }
                              aria-label={t("pricing.selectModel", { model: row.model })}
                            />
                          </td>
                          <td className="max-w-[240px] truncate px-2 py-1.5">{row.model}</td>
                          <td className="px-2 py-1.5 text-text-dim">{row.provider ?? "-"}</td>
                          <td className="px-2 py-1.5 text-right font-mono">{row.input.toFixed(4)}</td>
                          <td className="px-2 py-1.5 text-right font-mono">{row.output.toFixed(4)}</td>
                          <td className="px-2 py-1.5 text-right font-mono">{row.cache_read.toFixed(4)}</td>
                          <td className="px-2 py-1.5 text-right font-mono">{row.cache_write.toFixed(4)}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </div>
          ) : (
            <table className="w-full border-collapse text-[11.5px]" data-pricing-table>
              <thead>
                <tr className="border-b border-line text-left text-text-dim">
                  <th className="px-1.5 py-1.5">{t("pricing.col.model")}</th>
                  <th className="px-1.5 py-1.5">provider</th>
                  {NUMERIC_FIELDS.map((field) => (
                    <th key={field.key} className="px-1.5 py-1.5 text-right">
                      {t(field.label)}
                    </th>
                  ))}
                  <th className="px-1.5 py-1.5 text-right">{t("pricing.col.currency")}</th>
                  <th className="w-8 px-1 py-1.5" />
                </tr>
              </thead>
              <tbody>
                {rows.map((row, index) => (
                  <tr key={`${row.model}-${row.provider ?? ""}-${index}`} className="border-b border-line/50">
                    <td className="px-1.5 py-1">
                      <input
                        value={row.model}
                        onChange={(event) => patch(index, { model: event.target.value })}
                        placeholder="claude-sonnet-5-5"
                        className="h-7 w-full rounded border border-line bg-transparent px-1.5 font-mono text-[11.5px] text-text-strong outline-none focus:border-brand-500/50"
                      />
                    </td>
                    <td className="px-1.5 py-1">
                      <input
                        value={row.provider ?? ""}
                        onChange={(event) =>
                          patch(index, { provider: event.target.value.trim() || null })
                        }
                        placeholder={t("pricing.defaultPrice")}
                        className="h-7 w-full rounded border border-line bg-transparent px-1.5 text-[11.5px] text-text-strong outline-none focus:border-brand-500/50"
                      />
                    </td>
                    {NUMERIC_FIELDS.map((field) => (
                      <td key={field.key} className="px-1.5 py-1">
                        <input
                          type="number"
                          min={0}
                          step="0.0001"
                          value={row[field.key]}
                          onChange={(event) =>
                            patch(index, { [field.key]: Number(event.target.value) || 0 })
                          }
                          className="h-7 w-20 rounded border border-line bg-transparent px-1.5 text-right font-mono text-[11.5px] text-text-strong outline-none focus:border-brand-500/50"
                        />
                      </td>
                    ))}
                    <td className="px-1.5 py-1">
                      <input
                        value={row.currency}
                        onChange={(event) => patch(index, { currency: event.target.value })}
                        className="h-7 w-14 rounded border border-line bg-transparent px-1.5 text-[11.5px] text-text-strong outline-none focus:border-brand-500/50"
                      />
                    </td>
                    <td className="px-1 py-1 text-right">
                      <button
                        type="button"
                        onClick={() => setRows((prev) => prev.filter((_, i) => i !== index))}
                        className="cursor-pointer rounded p-1 text-text-dim transition-colors hover:bg-state-danger/10 hover:text-state-danger"
                        aria-label={t("pricing.deleteRow", {
                          model: row.model || t("pricing.thisRow"),
                        })}
                      >
                        <X className="h-3.5 w-3.5" />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        <footer className="flex items-center gap-2 border-t border-line px-4 py-3">
          {preview ? (
            <>
              <span className="text-[11.5px] text-text-dim">
                {t("pricing.picked", { count: picked.size })}
              </span>
              <div className="ml-auto flex items-center gap-2">
                <Button variant="ghost" onClick={() => setPreview(null)}>
                  {t("pricing.back")}
                </Button>
                <Button
                  variant="primary"
                  disabled={busy || picked.size === 0}
                  onClick={() => void applyImport()}
                >
                  {t("pricing.importPicked")}
                </Button>
              </div>
            </>
          ) : (
            <>
              <Button
                variant="outline"
                onClick={() => setRows((prev) => [...prev, { ...EMPTY_ROW }])}
              >
                {t("pricing.addRow")}
              </Button>
              <span className="text-[11px] text-text-dim">{t("pricing.unpricedNote")}</span>
              <div className="ml-auto flex items-center gap-2">
                <Button variant="ghost" onClick={onClose}>
                  {t("pricing.close")}
                </Button>
                <Button variant="primary" disabled={busy} onClick={() => void save()}>
                  {busy ? t("pricing.saving") : t("pricing.save")}
                </Button>
              </div>
            </>
          )}
        </footer>
      </div>
    </div>
  );
}

