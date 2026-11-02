import { useMemo } from "react";
import { clsx } from "clsx";
import { PencilLine } from "lucide-react";
import { Badge, SearchInput } from "../ui";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import type { SupplierCategory, SupplierDef } from "../../lib/types";

const CATEGORY_LABEL: Record<SupplierCategory, MessageKey> = {
  official: "market.badge.official",
  model_vendor: "find.modelVendor",
  aggregator: "find.aggregator",
};

const ORDER: SupplierCategory[] = ["official", "model_vendor", "aggregator"];

function domainOf(baseUrl: string): string {
  const rest = baseUrl.trim().replace(/^https?:\/\//, "");
  return rest.split("/")[0] ?? rest;
}

function initial(name: string): string {
  const trimmed = name.trim();
  return trimmed ? trimmed.slice(0, 1).toUpperCase() : "?";
}

/**
 * 步骤一：选渠道。
 *
 * 第一张永远是「自定义配置」——它不是默认选项，而是一条明确的退路；
 * 用中性描边而不是品牌色，避免抢走内置渠道的注意力。
 */
export function SupplierCatalog({
  keyword,
  onKeyword,
  suppliers,
  onPick,
}: {
  keyword: string;
  onKeyword: (value: string) => void;
  suppliers: SupplierDef[];
  /** null = 自定义配置 */
  onPick: (supplier: SupplierDef | null) => void;
}) {
  const t = useT();
  const groups = useMemo(() => {
    return ORDER.map((category) => ({
      category,
      items: suppliers.filter((item) => item.category === category),
    })).filter((group) => group.items.length > 0);
  }, [suppliers]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="border-b border-line px-4 py-2.5">
        <SearchInput
          value={keyword}
          onChange={onKeyword}
          placeholder={t("find.placeholder")}
        />
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        <button
          type="button"
          onClick={() => onPick(null)}
          className="mb-3 flex w-full cursor-pointer items-center gap-2.5 rounded-lg border border-dashed border-line bg-surface-2/40 px-3 py-2.5 text-left transition-colors hover:border-brand-500/50 hover:bg-surface-hover/60"
        >
          <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md border border-line bg-ink-950 text-text-dim">
            <PencilLine className="h-3.5 w-3.5" />
          </span>
          <span className="min-w-0">
            <span className="block text-[12px] font-medium text-text-strong">
              {t("find.custom")}
            </span>
            <span className="block text-[11px] text-text-dim">{t("find.customHint")}</span>
          </span>
        </button>

        {groups.length === 0 && (
          <p className="px-1 py-6 text-center text-[11.5px] text-text-dim">
            {t("find.empty", { keyword })}
          </p>
        )}

        {groups.map((group) => (
          <section key={group.category} className="mb-3">
            <div className="mb-1.5 flex items-center gap-2">
              <span className="text-[10.5px] font-semibold uppercase tracking-wider text-text-dim">
                {t(CATEGORY_LABEL[group.category])}
              </span>
              <Badge tone="dim">{group.items.length}</Badge>
            </div>
            <div className="grid grid-cols-2 gap-2">
              {group.items.map((supplier) => (
                <button
                  key={supplier.id}
                  type="button"
                  onClick={() => onPick(supplier)}
                  title={`${supplier.name} · ${supplier.source_url}`}
                  className={clsx(
                    "flex cursor-pointer items-center gap-2 rounded-lg border border-line bg-surface-2/40 px-2 py-1.5 text-left",
                    "transition-colors duration-120 hover:border-brand-500/50 hover:bg-surface-hover/60"
                  )}
                >
                  <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md bg-brand-500/15 text-[11px] font-semibold text-brand-300">
                    {initial(supplier.name)}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="flex items-center gap-1">
                      <span className="truncate text-[11.5px] font-medium text-text-strong">
                        {supplier.name}
                      </span>
                      {!supplier.verified && (
                        <Badge tone="warn" title={t("find.unverified")}>
                          {t("find.unverifiedBadge")}
                        </Badge>
                      )}
                    </span>
                    <span className="mono block truncate text-[10.5px] text-text-dim">
                      {domainOf(supplier.base_url)}
                    </span>
                  </span>
                  {supplier.models.length > 0 && (
                    <span className="mono shrink-0 text-[10px] text-text-dim">
                      {supplier.models.length}
                    </span>
                  )}
                </button>
              ))}
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}
