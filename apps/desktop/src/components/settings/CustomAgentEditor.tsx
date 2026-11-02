import { useState } from "react";
import { ChevronRight, Plus, Trash2 } from "lucide-react";
import { clsx } from "clsx";
import { Badge, Button } from "../ui";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import type { CustomAgentDef, McpFormatSpec } from "../../lib/types";

type FormatKind = "none" | "json" | "toml" | "yaml";

/** 只有 `none` 需要翻译，三种格式名本身就是 JSON / TOML / YAML。 */
const FORMATS: { value: FormatKind; label?: MessageKey }[] = [
  { value: "none", label: "custom.mcpNone" },
  { value: "json" },
  { value: "toml" },
  { value: "yaml" },
];

const inputClass =
  "h-8 w-full rounded-md border border-line bg-ink-950/70 px-2 text-[12px] text-text-strong placeholder:text-text-dim/70 outline-none transition-colors focus:border-brand-500/60 focus:ring-2 focus:ring-brand-500/15";

function newDef(): CustomAgentDef {
  return {
    id: "",
    display_name: "",
    skills_dir: null,
    project_skills_dir: null,
    mcp_file: null,
    mcp_format: null,
    project_mcp_file: null,
    instructions: [],
  };
}

function formatKind(def: CustomAgentDef): FormatKind {
  const format = def.mcp_format;
  if (!format) return "none";
  return format.format;
}

function formatDetail(def: CustomAgentDef): string {
  const format = def.mcp_format;
  if (!format) return "";
  if (format.format === "json") return format.pointer.join(".");
  if (format.format === "toml") return format.table;
  return format.top_key;
}

function buildFormat(kind: FormatKind, detail: string): McpFormatSpec | null {
  const parts = detail
    .split(/[.,]/)
    .map((part) => part.trim())
    .filter(Boolean);
  if (kind === "json") {
    return { format: "json", pointer: parts.length ? parts : ["mcpServers"] };
  }
  if (kind === "toml") {
    return { format: "toml", table: detail.trim() || "mcp_servers" };
  }
  if (kind === "yaml") {
    return { format: "yaml", top_key: detail.trim() || "mcp_servers" };
  }
  return null;
}

function countTargets(def: CustomAgentDef): number {
  return [
    def.skills_dir,
    def.project_skills_dir,
    def.mcp_file,
    def.project_mcp_file,
    def.instructions.length > 0 ? "instructions" : null,
  ].filter(Boolean).length;
}

export function CustomAgentEditor({
  defs,
  onChange,
}: {
  defs: CustomAgentDef[];
  onChange: (next: CustomAgentDef[]) => void;
}) {
  const t = useT();
  const [openIndex, setOpenIndex] = useState<number | null>(null);

  const update = (index: number, patch: Partial<CustomAgentDef>) => {
    onChange(defs.map((def, idx) => (idx === index ? { ...def, ...patch } : def)));
  };

  const add = () => {
    onChange([...defs, newDef()]);
    setOpenIndex(defs.length);
  };

  const remove = (index: number) => {
    onChange(defs.filter((_, idx) => idx !== index));
    setOpenIndex(null);
  };

  return (
    <div>
      <div className="flex items-center justify-between border-b border-line px-3 py-2">
        <span className="text-[11px] text-text-dim">{t("custom.hint")}</span>
        <Button size="sm" variant="outline" onClick={add}>
          <Plus className="h-3 w-3" />
          {t("custom.add")}
        </Button>
      </div>

      {defs.length === 0 && (
        <p className="px-3 py-4 text-[11.5px] text-text-dim">{t("custom.empty")}</p>
      )}

      {defs.map((def, index) => {
        const expanded = openIndex === index;
        return (
          <div key={`${def.id}-${index}`} className="border-t border-line first:border-t-0">
            <button
              type="button"
              onClick={() => setOpenIndex(expanded ? null : index)}
              aria-expanded={expanded}
              className="flex w-full cursor-pointer items-center gap-2 px-3 py-2.5 text-left transition-colors hover:bg-surface-hover"
            >
              <span className="mono rounded border border-line bg-surface-2 px-1.5 py-[2px] text-[11px] text-text-muted">
                {def.id || "new"}
              </span>
              <span className="truncate text-[12.5px] text-text-strong">
                {def.display_name || t("custom.unnamed")}
              </span>
              <Badge tone={countTargets(def) > 0 ? "brand" : "warn"}>
                {t("custom.targets", { count: countTargets(def) })}
              </Badge>
              <ChevronRight
                className={clsx(
                  "ml-auto h-3 w-3 shrink-0 text-text-dim transition-transform",
                  expanded && "rotate-90"
                )}
              />
            </button>

            {expanded && (
              <div className="flex flex-col gap-2 border-t border-line bg-surface-2/40 px-3 py-2.5">
                <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
                  <label className="flex flex-col gap-1">
                    <span className="text-[11.5px] text-text-dim">{t("custom.field.id")}</span>
                    <input
                      value={def.id}
                      placeholder="deepseek-harness"
                      onChange={(event) => update(index, { id: event.target.value })}
                      className={clsx(inputClass, "font-mono")}
                    />
                  </label>
                  <label className="flex flex-col gap-1">
                    <span className="text-[11.5px] text-text-dim">
                      {t("custom.field.displayName")}
                    </span>
                    <input
                      value={def.display_name}
                      placeholder="DeepSeek Harness"
                      onChange={(event) => update(index, { display_name: event.target.value })}
                      className={inputClass}
                    />
                  </label>
                </div>

                {(
                  [
                    ["skills_dir", t("custom.field.skillsDir"), "~/.dsh/skills"],
                    ["project_skills_dir", t("custom.field.projectSkillsDir"), ".dsh/skills"],
                    ["mcp_file", t("custom.field.mcpFile"), "~/.dsh/config.json"],
                    ["project_mcp_file", t("custom.field.projectMcpFile"), ".dsh/mcp.json"],
                  ] as const
                ).map(([key, label, placeholder]) => (
                  <label key={key} className="flex flex-col gap-1">
                    <span className="text-[11.5px] text-text-dim">{label}</span>
                    <input
                      value={def[key] ?? ""}
                      placeholder={placeholder}
                      onChange={(event) =>
                        update(index, { [key]: event.target.value || null })
                      }
                      className={clsx(inputClass, "font-mono text-[11px]")}
                    />
                  </label>
                ))}

                <div className="grid grid-cols-1 gap-2 md:grid-cols-[140px_1fr]">
                  <label className="flex flex-col gap-1">
                    <span className="text-[11.5px] text-text-dim">
                      {t("custom.field.mcpFormat")}
                    </span>
                    <select
                      value={formatKind(def)}
                      onChange={(event) =>
                        update(index, {
                          mcp_format: buildFormat(
                            event.target.value as FormatKind,
                            formatDetail(def)
                          ),
                        })
                      }
                      className="h-8 cursor-pointer rounded-md border border-line bg-ink-950/70 px-2 text-[12px] text-text-strong outline-none"
                    >
                      {FORMATS.map((format) => (
                        <option key={format.value} value={format.value}>
                          {format.label ? t(format.label) : format.value.toUpperCase()}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="flex flex-col gap-1">
                    <span className="text-[11.5px] text-text-dim">
                      {formatKind(def) === "json"
                        ? t("custom.field.keyPath")
                        : t("custom.field.tableName")}
                    </span>
                    <input
                      value={formatDetail(def)}
                      disabled={formatKind(def) === "none"}
                      placeholder={formatKind(def) === "json" ? "mcpServers" : "mcp_servers"}
                      onChange={(event) =>
                        update(index, {
                          mcp_format: buildFormat(formatKind(def), event.target.value),
                        })
                      }
                      className={clsx(inputClass, "font-mono text-[11px] disabled:opacity-40")}
                    />
                  </label>
                </div>

                <label className="flex flex-col gap-1">
                  <span className="text-[11.5px] text-text-dim">
                    {t("custom.field.instructions")}
                  </span>
                  <textarea
                    value={def.instructions.join("\n")}
                    placeholder="~/.dsh/AGENTS.md"
                    onChange={(event) =>
                      update(index, {
                        instructions: event.target.value
                          .split("\n")
                          .map((line) => line.trim())
                          .filter(Boolean),
                      })
                    }
                    rows={2}
                    className="min-h-[48px] resize-y rounded-md border border-line bg-ink-950/70 px-2 py-1.5 font-mono text-[11px] leading-relaxed text-text-strong placeholder:text-text-dim/70 outline-none focus:border-brand-500/60 focus:ring-2 focus:ring-brand-500/15"
                  />
                </label>

                <div className="flex items-center justify-between">
                  <span className="text-[11px] text-text-dim">{t("custom.note")}</span>
                  <Button size="sm" variant="danger" onClick={() => remove(index)}>
                    <Trash2 className="h-3 w-3" />
                    {t("custom.remove")}
                  </Button>
                </div>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
