import { useEffect, useMemo, useState } from "react";
import { clsx } from "clsx";
import { ArrowLeft, ChevronDown, Info, X } from "lucide-react";
import { Badge, Button, Toggle } from "../ui";
import { ModelTable } from "./ModelTable";
import { SupplierCatalog } from "./SupplierCatalog";
import { useHub } from "../../lib/useHub";
import { useT } from "../../lib/i18n";
import type {
  GatewayProtocol,
  ModelLimits,
  ProbeResult,
  ProviderView,
  SupplierDef,
} from "../../lib/types";

const input =
  "h-8 w-full rounded-md border border-line bg-ink-950/70 px-2 text-[12px] text-text-strong placeholder:text-text-dim/70 outline-none transition-colors focus:border-brand-500/60 focus:ring-2 focus:ring-brand-500/15";

interface Form {
  id: string;
  name: string;
  note: string;
  website: string;
  protocol: GatewayProtocol;
  base_url: string;
  api_key: string;
  models: string[];
  limits: Record<string, ModelLimits>;
  default_model: string;
  acknowledged: boolean;
  catalog_id: string | null;
}

function blank(): Form {
  return {
    id: "",
    name: "",
    note: "",
    website: "",
    protocol: "chat_completions",
    base_url: "",
    api_key: "",
    models: [],
    limits: {},
    default_model: "",
    acknowledged: false,
    catalog_id: null,
  };
}

/** 中文名 slug 不出可用 id，退回渠道 id 或时间戳后缀。 */
function slugify(name: string, fallback: string): string {
  const ascii = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  if (ascii.length >= 2) return ascii.slice(0, 32);
  return fallback;
}

function fromSupplier(supplier: SupplierDef, existing: ProviderView[]): Form {
  const id = existing.some((item) => item.id === supplier.id)
    ? `${supplier.id}-${existing.length + 1}`
    : supplier.id;
  return {
    ...blank(),
    id,
    name: supplier.name,
    website: supplier.website,
    protocol: supplier.protocol,
    base_url: supplier.base_url,
    models: [...supplier.models],
    default_model: supplier.default_model,
    catalog_id: supplier.id,
  };
}

function fromProvider(view: ProviderView): Form {
  return {
    id: view.id,
    name: view.name,
    note: view.note ?? "",
    website: "",
    protocol: view.protocol ?? "anthropic",
    base_url: view.base_url,
    // 编辑时保留脱敏后的原值：用户不动它就等于「不改密钥」
    api_key: view.api_key,
    models: [...view.models],
    limits: { ...view.model_limits },
    default_model: view.models[0] ?? "",
    acknowledged: false,
    catalog_id: view.catalog_id,
  };
}

function Field({
  label,
  hint,
  required,
  children,
}: {
  label: string;
  hint?: string;
  required?: boolean;
  children: React.ReactNode;
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="flex items-center gap-1 text-[11.5px] text-text-dim">
        {label}
        {required && <span className="text-state-danger">*</span>}
        {hint && (
          <span title={hint} className="inline-flex">
            <Info className="h-3 w-3 text-text-dim/70" />
          </span>
        )}
      </span>
      {children}
    </label>
  );
}

/**
 * 添加 / 编辑供应商。
 *
 * 两步：先选渠道（地址、协议、默认模型都预置好），再填表。
 * 编辑时直接进第二步，不去强制用户重选渠道。
 */
export function SupplierWizard({
  editing,
  providers,
  onClose,
}: {
  editing: ProviderView | null;
  providers: ProviderView[];
  onClose: () => void;
}) {
  const { suppliers, loadSuppliers, saveProvider, probeModels } = useHub();
  const t = useT();
  const [step, setStep] = useState<"pick" | "form">(editing ? "form" : "pick");
  const [keyword, setKeyword] = useState("");
  const [form, setForm] = useState<Form>(() =>
    editing ? fromProvider(editing) : blank()
  );
  const [picked, setPicked] = useState<SupplierDef | null>(null);
  const [probing, setProbing] = useState(false);
  const [probe, setProbe] = useState<ProbeResult | null>(null);
  const [advanced, setAdvanced] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void loadSuppliers(keyword);
  }, [keyword, loadSuppliers]);

  const patch = (next: Partial<Form>) => setForm((prev) => ({ ...prev, ...next }));

  const problems = useMemo(() => {
    const list: string[] = [];
    if (!form.name.trim()) list.push(t("wizard.err.name"));
    if (!form.base_url.trim()) list.push(t("wizard.err.baseUrl"));
    else if (!/^https?:\/\//.test(form.base_url.trim()))
      list.push("API 请求地址必须以 http:// 或 https:// 开头");
    if (!form.api_key.trim()) list.push(t("wizard.err.key"));
    else if (
      !form.api_key.startsWith("env:") &&
      !form.api_key.startsWith("keychain:") &&
      !form.api_key.includes("****") &&
      !form.acknowledged
    ) {
      list.push(t("wizard.err.plainKey"));
    }
    return list;
  }, [form]);

  const choose = (supplier: SupplierDef | null) => {
    setPicked(supplier);
    if (supplier) {
      setForm(fromSupplier(supplier, providers));
    } else {
      setForm({ ...blank(), protocol: "chat_completions" });
    }
    setProbe(null);
    setStep("form");
  };

  const runProbe = () => {
    setProbing(true);
    setProbe(null);
    void probeModels({
      base_url: form.base_url,
      api_key: form.api_key,
      protocol: form.protocol,
    }).then((result) => {
      setProbing(false);
      if (!result) return;
      setProbe(result);
      if (result.error || result.models.length === 0) return;
      // 探测成功：合并模型与长度，但不覆盖用户已经手填的值
      const models = result.models.map((item) => item.id);
      const limits: Record<string, ModelLimits> = { ...form.limits };
      for (const item of result.models) {
        if (!item.context && !item.max_output) continue;
        const current = limits[item.id];
        if (current && (current.context || current.max_output)) continue;
        limits[item.id] = {
          context: item.context,
          max_output: item.max_output,
          source: "auto",
        };
      }
      patch({
        models,
        limits,
        default_model: form.default_model || models[0] || "",
      });
    });
  };

  const submit = () => {
    if (problems.length > 0) return;
    setBusy(true);
    const id = form.id || slugify(form.name, form.catalog_id ?? `supplier-${Date.now() % 10000}`);
    void saveProvider({
      id,
      name: form.name.trim(),
      base_url: form.base_url.trim(),
      api_key: form.api_key.trim(),
      models: form.models,
      note: form.note.trim() || null,
      acknowledged: form.acknowledged || form.api_key.includes("****"),
      protocol: form.protocol,
      model_limits: form.limits,
      catalog_id: form.catalog_id,
    }).then((ok) => {
      setBusy(false);
      if (ok) onClose();
    });
  };

  return (
    <div className="fixed inset-0 z-40 flex justify-end bg-black/30" onClick={onClose}>
      <aside
        className="flex h-full w-[460px] max-w-[94vw] flex-col border-l border-line bg-ink-900 shadow-panel"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="flex items-center gap-2 border-b border-line px-4 py-3">
          {step === "form" && !editing && (
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setStep("pick")}
              title={t("wizard.back")}
            >
              <ArrowLeft className="h-3.5 w-3.5" />
            </Button>
          )}
          <div className="min-w-0 flex-1">
            <h2 className="truncate text-[13.5px] font-medium text-text-strong">
              {editing
                ? t("wizard.title.edit", { name: editing.name })
                : step === "pick"
                  ? t("wizard.title.add")
                  : t("wizard.title.fill")}
            </h2>
            <p className="mt-0.5 truncate text-[11px] text-text-dim">
              {step === "pick"
                ? t("wizard.pickHint")
                : picked
                  ? t("wizard.picked", { name: picked.name })
                  : editing
                    ? editing.base_url
                    : t("wizard.custom")}
            </p>
          </div>
          <Button size="sm" variant="ghost" onClick={onClose} title={t("wizard.close")}>
            <X className="h-3.5 w-3.5" />
          </Button>
        </header>

        {step === "pick" ? (
          <SupplierCatalog
            keyword={keyword}
            onKeyword={setKeyword}
            suppliers={suppliers}
            onPick={choose}
          />
        ) : (
          <>
            <div className="min-h-0 flex-1 space-y-3 overflow-y-auto px-4 py-3">
              <div className="grid grid-cols-2 gap-2">
                <Field label={t("wizard.field.name")} required>
                  <input
                    value={form.name}
                    onChange={(event) => patch({ name: event.target.value })}
                    placeholder={t("wizard.field.namePlaceholder")}
                    className={input}
                  />
                </Field>
                <Field label={t("wizard.field.note")}>
                  <input
                    value={form.note}
                    onChange={(event) => patch({ note: event.target.value })}
                    placeholder={t("wizard.field.notePlaceholder")}
                    className={input}
                  />
                </Field>
              </div>

              <Field label={t("wizard.field.homepage")}>
                <input
                  value={form.website}
                  onChange={(event) => patch({ website: event.target.value })}
                  placeholder="https://example.com（可选）"
                  className={input}
                />
              </Field>

              <Field
                label={t("wizard.field.protocol")}
                required
                hint={t("wizard.field.protocolHint")}
              >
                <select
                  value={form.protocol}
                  onChange={(event) =>
                    patch({ protocol: event.target.value as GatewayProtocol })
                  }
                  className={input}
                >
                  <option value="anthropic">Anthropic Messages</option>
                  <option value="chat_completions">Chat Completions</option>
                  <option value="responses">{t("route.protocol.responses")}</option>
                </select>
              </Field>

              <Field
                label={t("wizard.field.key")}
                required
                hint={t("wizard.field.keyHint")}
              >
                <input
                  value={form.api_key}
                  onChange={(event) => patch({ api_key: event.target.value })}
                  placeholder={t("wizard.field.keyPlaceholder")}
                  className={clsx(input, "font-mono")}
                />
              </Field>

              <Field
                label={t("wizard.field.baseUrl")}
                required
                hint={t("wizard.field.baseUrlHint")}
              >
                <input
                  value={form.base_url}
                  onChange={(event) => patch({ base_url: event.target.value })}
                  placeholder="https://your-api-endpoint.com/v1"
                  className={clsx(input, "font-mono")}
                />
              </Field>

              <ModelTable
                models={form.models}
                limits={form.limits}
                onChange={(models, limits) =>
                  patch({
                    models,
                    limits,
                    default_model: models.includes(form.default_model)
                      ? form.default_model
                      : (models[0] ?? ""),
                  })
                }
                onProbe={runProbe}
                probing={probing}
                probeError={probe?.error ?? null}
                probeEndpoint={probe?.endpoint ?? null}
              />

              <Field label={t("wizard.field.defaultModel")}>
                <select
                  value={form.default_model}
                  onChange={(event) => patch({ default_model: event.target.value })}
                  className={clsx(input, "font-mono")}
                >
                  <option value="">{t("wizard.field.defaultModelEmpty")}</option>
                  {form.models.map((model) => (
                    <option key={model} value={model}>
                      {model}
                    </option>
                  ))}
                </select>
              </Field>

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
                  <span className="text-[11.5px] text-text-muted">{t("wizard.advanced")}</span>
                </button>
                {advanced && (
                  <div className="space-y-2 border-t border-line px-2.5 py-2">
                    <Field label={t("wizard.field.id")} hint={t("wizard.field.idHint")}>
                      <input
                        value={form.id}
                        onChange={(event) =>
                          patch({ id: event.target.value.toLowerCase().replace(/[^a-z0-9_-]/g, "") })
                        }
                        placeholder={t("wizard.field.idPlaceholder")}
                        className={clsx(input, "font-mono")}
                      />
                    </Field>
                    <div className="flex items-center justify-between gap-3 pt-1">
                      <div>
                        <p className="text-[11.5px] text-text-strong">
                          {t("wizard.plainKey.title")}
                        </p>
                        <p className="text-[11px] text-text-dim">{t("wizard.plainKey.note")}</p>
                      </div>
                      <Toggle
                        checked={form.acknowledged}
                        onChange={(next) => patch({ acknowledged: next })}
                      />
                    </div>
                  </div>
                )}
              </div>

              {problems.length > 0 && (
                <ul className="space-y-1 rounded-md border border-state-warn/30 bg-state-warn/10 px-2.5 py-2">
                  {problems.map((item) => (
                    <li key={item} className="text-[11px] leading-relaxed text-state-warn">
                      {item}
                    </li>
                  ))}
                </ul>
              )}
            </div>

            <footer className="flex items-center justify-between gap-2 border-t border-line px-4 py-3">
              <span className="flex items-center gap-1.5 text-[11px] text-text-dim">
                {t("wizard.models", { count: form.models.length })}
                {form.protocol !== "anthropic" && (
                  <Badge tone="dim">{t("wizard.chatCompat")}</Badge>
                )}
              </span>
              <div className="flex items-center gap-1.5">
                <Button size="sm" variant="ghost" onClick={onClose} disabled={busy}>
                  {t("common.cancel")}
                </Button>
                <Button
                  size="sm"
                  variant="primary"
                  onClick={submit}
                  disabled={busy || problems.length > 0}
                >
                  {busy ? t("wizard.saving") : editing ? t("wizard.save") : t("wizard.add")}
                </Button>
              </div>
            </footer>
          </>
        )}
      </aside>
    </div>
  );
}
