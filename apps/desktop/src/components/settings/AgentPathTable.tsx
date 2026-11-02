import { useState } from "react";
import { ChevronRight, RotateCcw } from "lucide-react";
import { clsx } from "clsx";
import { Badge } from "../ui";
import { AgentIcon } from "../agents/AgentIcon";
import { shortPath } from "../../lib/format";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import type { AgentPathOverride, AgentPathsView } from "../../lib/types";

type FieldKey = "config_dir" | "skills_dir" | "mcp_file";

const FIELDS: { key: FieldKey; label: MessageKey; hint: MessageKey }[] = [
  { key: "config_dir", label: "path.configDir", hint: "path.configDirHint" },
  { key: "skills_dir", label: "path.skillsDir", hint: "path.skillsDirHint" },
  { key: "mcp_file", label: "path.mcpFile", hint: "path.mcpFileHint" },
];

export const emptyOverride = (): AgentPathOverride => ({
  config_dir: null,
  skills_dir: null,
  mcp_file: null,
  instructions: [],
});

export function hasOverride(override: AgentPathOverride | undefined): boolean {
  if (!override) return false;
  return Boolean(
    override.config_dir ||
      override.skills_dir ||
      override.mcp_file ||
      override.instructions.length
  );
}

/** 只改一个字段：写成显式分支，避免计算属性把类型放宽。 */
function withField(
  override: AgentPathOverride,
  key: FieldKey,
  value: string
): AgentPathOverride {
  const clean = value.trim() ? value : null;
  if (key === "config_dir") return { ...override, config_dir: clean };
  if (key === "skills_dir") return { ...override, skills_dir: clean };
  return { ...override, mcp_file: clean };
}

export function AgentPathTable({
  agents,
  value,
  onChange,
}: {
  agents: AgentPathsView[];
  value: Record<string, AgentPathOverride>;
  onChange: (id: string, next: AgentPathOverride) => void;
}) {
  const t = useT();
  const [open, setOpen] = useState<string | null>(null);
  const builtin = agents.filter((agent) => agent.builtin);

  return (
    <div>
      {builtin.map((agent) => {
        const override = value[agent.id] ?? emptyOverride();
        const expanded = open === agent.id;
        const overridden = hasOverride(override);

        return (
          <div key={agent.id} className="border-t border-line first:border-t-0">
            <button
              type="button"
              onClick={() => setOpen(expanded ? null : agent.id)}
              aria-expanded={expanded}
              className="flex w-full cursor-pointer items-center gap-2 px-3 py-2.5 text-left transition-colors hover:bg-surface-hover"
            >
              <span className="flex h-6 w-7 shrink-0 items-center justify-center rounded border border-line bg-surface-2 text-text-muted">
                <AgentIcon id={agent.id} className="h-4 w-4" />
              </span>
              <span className="text-[12.5px] font-medium text-text-strong">
                {agent.display_name}
              </span>
              {overridden && <Badge tone="warn">{t("path.overridden")}</Badge>}
              <span className="mono ml-auto hidden min-w-0 truncate text-[11.5px] text-text-dim md:block">
                {agent.effective.skills_dir ? shortPath(agent.effective.skills_dir, 2) : "—"}
              </span>
              <ChevronRight
                className={clsx(
                  "h-3 w-3 shrink-0 text-text-dim transition-transform",
                  expanded && "rotate-90"
                )}
              />
            </button>

            {expanded && (
              <div className="border-t border-line bg-surface-2/40 px-3 py-2">
                {FIELDS.map((field) => {
                  const effective = agent.effective[field.key];
                  const present =
                    field.key === "config_dir"
                      ? agent.presence.config_dir
                      : field.key === "skills_dir"
                        ? agent.presence.skills_dir
                        : agent.presence.mcp_file;
                  const current = override[field.key] ?? "";
                  return (
                    <div key={field.key} className="py-1.5">
                      <div className="flex items-center gap-2">
                        <span className="w-16 shrink-0 text-[11px] text-text-dim">
                          {t(field.label)}
                        </span>
                        <input
                          value={current}
                          placeholder={effective ?? t("path.notDetected")}
                          onChange={(event) =>
                            onChange(agent.id, withField(override, field.key, event.target.value))
                          }
                          className="h-8 min-w-0 flex-1 rounded-md border border-line bg-ink-950/70 px-2 font-mono text-[11px] text-text-strong placeholder:text-text-dim/70 outline-none transition-colors focus:border-brand-500/60 focus:ring-2 focus:ring-brand-500/15"
                        />
                        {current ? (
                          <button
                            type="button"
                            onClick={() => onChange(agent.id, withField(override, field.key, ""))}
                            title={t("path.clearOverride")}
                            className="flex shrink-0 cursor-pointer items-center gap-1 rounded px-1.5 py-1 text-[11.5px] text-text-muted transition-colors hover:bg-surface-hover hover:text-text-strong"
                          >
                            <RotateCcw className="h-2.5 w-2.5" />
                            {t("path.reset")}
                          </button>
                        ) : (
                          <Badge tone={present ? "ok" : "danger"}>
                            {present ? t("path.exists") : t("path.missing")}
                          </Badge>
                        )}
                      </div>
                      <div className="mt-1 flex items-baseline gap-1.5 pl-[4.5rem] text-[11px] text-text-dim">
                        <span className="mono truncate" title={effective ?? ""}>
                          {t("path.autoDetected", { path: effective ?? "—" })}
                        </span>
                        <span className="shrink-0 opacity-70">{t(field.hint)}</span>
                      </div>
                    </div>
                  );
                })}

                {agent.effective.extra_skills_dirs.length > 0 && (
                  <div className="py-1.5">
                    <div className="flex items-start gap-2">
                      <span className="w-16 shrink-0 pt-1 text-[11px] text-text-dim">
                        {t("path.compatDir")}
                      </span>
                      <div className="flex min-w-0 flex-1 flex-col gap-1">
                        {agent.effective.extra_skills_dirs.map((dir, index) => {
                          const present = agent.presence.extra_skills_dirs[index] ?? false;
                          return (
                            <span key={dir} className="flex items-center gap-2">
                              <span
                                className="mono min-w-0 flex-1 truncate text-[11.5px] text-text-muted"
                                title={dir}
                              >
                                {dir}
                              </span>
                              <Badge tone={present ? "ok" : "danger"}>
                                {present ? t("path.exists") : t("path.missing")}
                              </Badge>
                            </span>
                          );
                        })}
                        <span className="text-[11px] text-text-dim">
                          {t("path.fixedValue")}
                        </span>
                      </div>
                    </div>
                  </div>
                )}

                <div className="py-1.5">
                  <div className="flex items-start gap-2">
                    <span className="w-16 shrink-0 pt-1.5 text-[11px] text-text-dim">
                      {t("path.instructions")}
                    </span>
                    <textarea
                      value={override.instructions.join("\n")}
                      placeholder={
                        agent.effective.instructions.join("\n") || t("path.notDetected")
                      }
                      onChange={(event) =>
                        onChange(agent.id, {
                          ...override,
                          instructions: event.target.value
                            .split("\n")
                            .map((line) => line.trim())
                            .filter(Boolean),
                        })
                      }
                      rows={Math.max(2, Math.min(5, override.instructions.length || 2))}
                      className="min-h-[52px] min-w-0 flex-1 resize-y rounded-md border border-line bg-ink-950/70 px-2 py-1.5 font-mono text-[11px] leading-relaxed text-text-strong placeholder:text-text-dim/70 outline-none transition-colors focus:border-brand-500/60 focus:ring-2 focus:ring-brand-500/15"
                    />
                  </div>
                  <p className="mt-1 pl-[4.5rem] text-[11px] text-text-dim">
                    {t("path.instructionsNote")}
                  </p>
                </div>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
