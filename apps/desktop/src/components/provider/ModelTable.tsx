import { clsx } from "clsx";
import { Download, Plus, X } from "lucide-react";
import { Badge, Button } from "../ui";
import { useT } from "../../lib/i18n";
import { tr } from "../../lib/i18n/runtime";
import type { ModelLimits } from "../../lib/types";

/** 与后端 `ext::provider::format_tokens` 同一套规则。 */
function formatTokens(tokens: number | null): string {
  if (!tokens) return tr("model.emptyValue");
  if (tokens >= 1_000_000) {
    const value = tokens / 1_000_000;
    return Math.abs(value - Math.round(value)) < 0.05
      ? `${Math.round(value)}M`
      : `${value.toFixed(1)}M`;
  }
  if (tokens >= 1_000) {
    const value = tokens / 1_000;
    return Math.abs(value - Math.round(value)) < 0.05
      ? `${Math.round(value)}K`
      : `${value.toFixed(1)}K`;
  }
  return String(tokens);
}

const cell =
  "h-7 w-full rounded border border-transparent bg-transparent px-1 text-[11.5px] text-text-strong outline-none transition-colors hover:border-line focus:border-brand-500/60 focus:bg-ink-950/60";

/**
 * 模型列表编辑器：id、上下文、最大输出、来源。
 *
 * 长度是可点击编辑的数字单元格，失焦即保存到父表单状态；
 * 上游没给就显示「未填」而不是 0 —— 0 会让人以为模型没有输出能力。
 */
export function ModelTable({
  models,
  limits,
  onChange,
  onProbe,
  probing,
  probeError,
  probeEndpoint,
}: {
  models: string[];
  limits: Record<string, ModelLimits>;
  onChange: (models: string[], limits: Record<string, ModelLimits>) => void;
  onProbe: () => void;
  probing: boolean;
  probeError: string | null;
  probeEndpoint: string | null;
}) {
  const t = useT();
  const patchLimit = (model: string, patch: Partial<ModelLimits>) => {
    const current: ModelLimits = limits[model] ?? {
      context: null,
      max_output: null,
      source: "manual",
    };
    onChange(models, { ...limits, [model]: { ...current, ...patch, source: "manual" } });
  };

  const remove = (model: string) => {
    const nextLimits = { ...limits };
    delete nextLimits[model];
    onChange(
      models.filter((item) => item !== model),
      nextLimits
    );
  };

  const add = () => {
    const id = `new-model-${models.length + 1}`;
    if (models.includes(id)) return;
    onChange([...models, id], limits);
  };

  return (
    <div className="rounded-md border border-line">
      <div className="flex flex-wrap items-center gap-1.5 border-b border-line px-2 py-1.5">
        <span className="text-[11.5px] text-text-muted">{t("model.title")}</span>
        <Badge tone="dim">{models.length}</Badge>
        <div className="ml-auto flex items-center gap-1.5">
          <Button size="sm" variant="outline" onClick={onProbe} disabled={probing}>
            <Download className="h-3 w-3" />
            {probing ? t("model.fetching") : t("model.fetch")}
          </Button>
          <Button size="sm" variant="ghost" onClick={add}>
            <Plus className="h-3 w-3" />
            {t("model.addManual")}
          </Button>
        </div>
      </div>

      {probeError && (
        <p className="border-b border-line bg-state-warn/10 px-2 py-1.5 text-[11px] leading-relaxed text-state-warn">
          {probeError}
          {probeEndpoint && (
            <span className="mono ml-1 text-text-dim">（{probeEndpoint}）</span>
          )}
        </p>
      )}

      {models.length === 0 ? (
        <p className="px-2.5 py-3 text-[11.5px] text-text-dim">{t("model.empty")}</p>
      ) : (
        <div className="max-h-[240px] overflow-y-auto">
          <table className="w-full border-collapse text-[11.5px]">
            <thead className="sticky top-0 bg-ink-900 text-left text-[10.5px] text-text-dim">
              <tr>
                <th className="px-2 py-1 font-normal">{t("model.col.id")}</th>
                <th className="w-[86px] px-1 py-1 font-normal">{t("model.col.context")}</th>
                <th className="w-[86px] px-1 py-1 font-normal">{t("model.col.maxOutput")}</th>
                <th className="w-[52px] px-1 py-1 font-normal">{t("model.col.source")}</th>
                <th className="w-[28px] px-1 py-1" />
              </tr>
            </thead>
            <tbody>
              {models.map((model) => {
                const limit = limits[model];
                return (
                  <tr key={model} className="border-t border-line/60">
                    <td className="px-2 py-1">
                      <input
                        value={model}
                        onChange={(event) => {
                          const next = event.target.value;
                          if (!next || models.includes(next)) return;
                          onChange(
                            models.map((item) => (item === model ? next : item)),
                            Object.fromEntries(
                              Object.entries(limits).map(([key, value]) => [
                                key === model ? next : key,
                                value,
                              ])
                            )
                          );
                        }}
                        className={clsx(cell, "font-mono")}
                      />
                    </td>
                    <td className="px-1 py-1">
                      <input
                        type="number"
                        min={0}
                        step={1000}
                        value={limit?.context ?? ""}
                        placeholder={t("model.emptyValue")}
                        title={limit?.context ? formatTokens(limit.context) : t("model.emptyValue")}
                        onChange={(event) =>
                          patchLimit(model, {
                            context: event.target.value ? Number(event.target.value) : null,
                          })
                        }
                        className={clsx(cell, "font-mono text-right")}
                      />
                    </td>
                    <td className="px-1 py-1">
                      <input
                        type="number"
                        min={0}
                        step={1000}
                        value={limit?.max_output ?? ""}
                        placeholder={t("model.emptyValue")}
                        title={
                          limit?.max_output
                            ? formatTokens(limit.max_output)
                            : t("model.emptyValue")
                        }
                        onChange={(event) =>
                          patchLimit(model, {
                            max_output: event.target.value ? Number(event.target.value) : null,
                          })
                        }
                        className={clsx(cell, "font-mono text-right")}
                      />
                    </td>
                    <td className="px-1 py-1">
                      {limit && (limit.context || limit.max_output) ? (
                        <Badge tone={limit.source === "auto" ? "ok" : "warn"}>
                          {limit.source === "auto"
                            ? t("model.source.auto")
                            : t("model.source.manual")}
                        </Badge>
                      ) : (
                        <span className="text-[10.5px] text-text-dim">—</span>
                      )}
                    </td>
                    <td className="px-1 py-1">
                      <button
                        type="button"
                        onClick={() => remove(model)}
                        className="cursor-pointer text-text-dim transition-colors hover:text-state-danger"
                        title={t("model.remove")}
                      >
                        <X className="h-3 w-3" />
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
