import { useEffect, useMemo, useState, type ReactNode } from "react";
import { clsx } from "clsx";
import { Plus, X } from "lucide-react";
import { AgentPathTable, hasOverride } from "../components/settings/AgentPathTable";
import { AppearancePanel } from "../components/settings/AppearancePanel";
import { CustomAgentEditor } from "../components/settings/CustomAgentEditor";
import { FeatureManager } from "../components/settings/FeatureManager";
import { Badge, Button, Toggle } from "../components/ui";
import { useHub } from "../lib/useHub";
import { useLocale, useT, LOCALE_MODES, LOCALE_SELF_NAME } from "../lib/i18n";
import type { LocaleMode } from "../lib/i18n";
import type { MessageKey } from "../lib/i18n/messages";
import { useContextMenu, useMenuEnv } from "../lib/context-menu";
import { composeMenu } from "../lib/menu-builders/common";
import { excludedPathMenu, settingsPageMenu } from "../lib/menu-builders/settings";
import { bytes } from "../lib/format";
import {
  junctionReason,
  LINK_STRATEGIES,
  isStrategyDisabled,
} from "../lib/link-strategies";
import {
  announceRecordChange,
  readRecordEnabled,
  writeRecordEnabled,
} from "../lib/terminal-prefs";
import type {
  AgentPathOverride,
  LinkStrategyName,
  MarketSourceKey,
  SettingsInput,
  SettingsView,
} from "../lib/types";

/** 两家市场接口协议不通用，切源会同时清空自定义地址，避免「地址 + 协议」错配 */
const MARKET_SOURCES: { value: MarketSourceKey; label: string; hint: MessageKey }[] = [
  { value: "skillhub", label: "SkillHub", hint: "settings.marketSource.skillhubHint" },
  { value: "clawhub", label: "ClawHub", hint: "settings.marketSource.clawhubHint" },
];

const MARKET_SOURCE_BASES: Record<MarketSourceKey, string> = {
  skillhub: "https://api.skillhub.cn",
  clawhub: "https://clawhub.ai",
};

const STRATEGIES: { value: LinkStrategyName | "auto"; label: MessageKey }[] = [
  { value: "auto", label: "common.strategy.followPlatform" },
  ...LINK_STRATEGIES,
];

const inputClass =
  "h-8 w-full rounded-md border border-line bg-ink-950/70 px-2 text-[12px] text-text-strong placeholder:text-text-dim/70 outline-none transition-colors focus:border-brand-500/60 focus:ring-2 focus:ring-brand-500/15";

/** 视图 → 可提交结构；空覆盖会被丢掉，和后端归一化保持一致。 */
function toInput(view: SettingsView): SettingsInput {
  const agentPaths: Record<string, AgentPathOverride> = {};
  for (const agent of view.agents) {
    if (agent.builtin && hasOverride(agent.overrides)) {
      agentPaths[agent.id] = agent.overrides;
    }
  }
  return {
    store_root: view.store_root,
    default_strategy: view.default_strategy,
    backups_keep: view.backups_keep,
    excluded_paths: [...view.excluded_paths],
    use_trash_for_dedupe: view.use_trash_for_dedupe,
    backup_config_files: view.backup_config_files,
    agent_paths: agentPaths,
    custom_agents: view.custom_agents.map((def) => ({ ...def })),
    market_source: view.market_source ?? "skillhub",
    market_registry: view.market_registry ?? "",
  };
}

function normalize(input: SettingsInput): SettingsInput {
  const agentPaths: Record<string, AgentPathOverride> = {};
  for (const [id, override] of Object.entries(input.agent_paths)) {
    if (hasOverride(override)) agentPaths[id] = override;
  }
  return { ...input, agent_paths: agentPaths };
}

function Section({
  title,
  hint,
  children,
  actions,
}: {
  title: string;
  hint: string;
  children: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <section className="panel mb-3 overflow-hidden">
      <header className="flex items-start justify-between gap-3 border-b border-line px-3 py-2">
        <div className="min-w-0">
          <h2 className="text-[12px] font-medium text-text-strong">{title}</h2>
          <p className="mt-0.5 text-[11.5px] text-text-dim">{hint}</p>
        </div>
        {actions}
      </header>
      {children}
    </section>
  );
}

export function Settings() {
  const { settings, refreshSettings, saveSettings, notify, features } = useHub();
  const t = useT();
  const { mode, resolved, setMode } = useLocale();
  const menuEnv = useMenuEnv();
  const contextMenu = useContextMenu();
  const [draft, setDraft] = useState<SettingsInput | null>(null);
  const [saving, setSaving] = useState(false);
  const [newExcluded, setNewExcluded] = useState("");
  /** 本机偏好：与 config.json 无关，改完立刻生效 */
  const [recordTerminal, setRecordTerminal] = useState(() => readRecordEnabled());

  const toggleRecordTerminal = (value: boolean) => {
    setRecordTerminal(value);
    writeRecordEnabled(value);
    announceRecordChange(value);
  };

  useEffect(() => {
    if (!settings) void refreshSettings();
  }, [settings, refreshSettings]);

  useEffect(() => {
    if (settings && !draft) setDraft(toInput(settings));
  }, [settings, draft]);

  const baseline = useMemo(() => (settings ? toInput(settings) : null), [settings]);
  const dirty = Boolean(
    draft && baseline && JSON.stringify(normalize(draft)) !== JSON.stringify(baseline)
  );

  if (!settings || !draft) {
    return <p className="text-[11.5px] text-text-dim">{t("settings.loading")}</p>;
  }

  const patch = (next: Partial<SettingsInput>) =>
    setDraft((prev) => (prev ? { ...prev, ...next } : prev));

  const save = () => {
    setSaving(true);
    void saveSettings(draft).then((ok) => {
      setSaving(false);
      if (ok) setDraft(null);
    });
  };

  const addExcluded = () => {
    const value = newExcluded.trim();
    if (!value || draft.excluded_paths.includes(value)) return;
    patch({ excluded_paths: [...draft.excluded_paths, value] });
    setNewExcluded("");
  };

  const openPageMenu = (event: React.MouseEvent) => {
    if (!draft) return;
    contextMenu.open(
      event,
      composeMenu(
        settingsPageMenu({
          dirty: true,
          saving,
          save,
          discard: () => setDraft(null),
          followDefaultRegistry: !draft.market_registry.trim(),
          resetRegistry: () => patch({ market_registry: "" }),
        }),
        menuEnv,
        "settings"
      )
    );
  };

  const openExcludedMenu = (event: React.MouseEvent, path: string) => {
    if (!draft) return;
    contextMenu.open(
      event,
      composeMenu(
        excludedPathMenu(path, {
          remove: () =>
            patch({ excluded_paths: draft.excluded_paths.filter((item) => item !== path) }),
          notify,
        }),
        menuEnv,
        "settings"
      )
    );
  };

  return (
    <div className="pb-2" onContextMenu={openPageMenu}>
      <header className="mb-3 flex flex-wrap items-start justify-between gap-2">
        <div>
          <h1 className="text-[13.5px] font-semibold text-text-strong">{t("settings.title")}</h1>
          <p className="mt-0.5 text-[11.5px] text-text-dim">{t("settings.subtitle")}</p>
        </div>
        <span
          className="mono max-w-[46%] truncate rounded border border-line bg-surface-2 px-2 py-1 text-[11.5px] text-text-muted"
          title={settings.data_root}
        >
          {t("settings.dataRoot")} {settings.data_root}
        </span>
      </header>

      <Section title={t("settings.locale.title")} hint={t("settings.locale.hint")}>
        <div className="flex flex-wrap items-center gap-3 px-3 py-2.5">
          <label className="flex items-center gap-2 text-[11.5px] text-text-dim">
            {t("settings.locale.label")}
            <select
              value={mode}
              onChange={(event) => setMode(event.target.value as LocaleMode)}
              className={clsx(inputClass, "w-[220px]")}
              aria-label={t("settings.locale.label")}
            >
              {LOCALE_MODES.map((item) => (
                <option key={item} value={item}>
                  {item === "system" ? t("settings.locale.system") : LOCALE_SELF_NAME[item]}
                </option>
              ))}
            </select>
          </label>
          <span className="text-[11.5px] text-text-muted">
            {t("settings.locale.effective", { name: LOCALE_SELF_NAME[resolved] })}
            {mode === "system" ? t("settings.locale.following") : ""}
          </span>
        </div>
      </Section>

      <Section
        title={t("settings.features.section")}
        hint={t("settings.features.hint")}
        actions={
          <Badge tone={features.provider && features.usage ? "ok" : "warn"}>
            {t("settings.features.enabledCount", {
              count: [features.provider, features.usage].filter(Boolean).length,
            })}
          </Badge>
        }
      >
        <FeatureManager />
      </Section>

      <Section title={t("settings.appearance.section")} hint={t("settings.appearance.hint")}>
        <AppearancePanel />
      </Section>

      <Section title={t("settings.store.section")} hint={t("settings.store.hint")}>
        <div className="flex flex-col gap-2.5 px-3 py-2.5">
          <label className="flex flex-col gap-1">
            <span className="flex items-center gap-2 text-[11px] text-text-dim">
              {t("settings.store.label")}
              <Badge tone="dim">
                {t("settings.store.usage", {
                  count: settings.store_usage.skills,
                  bytes: bytes(settings.store_usage.bytes),
                })}
              </Badge>
            </span>
            <input
              value={draft.store_root}
              onChange={(event) => patch({ store_root: event.target.value })}
              className={clsx(inputClass, "mono text-[11px]")}
            />
            <span className="text-[11px] text-text-dim">{t("settings.store.note")}</span>
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-[11px] text-text-dim">{t("settings.backups.label")}</span>
            <input
              type="number"
              min={1}
              max={100}
              value={draft.backups_keep}
              onChange={(event) =>
                patch({ backups_keep: Number(event.target.value) || 0 })
              }
              className={clsx(inputClass, "mono w-24")}
            />
            <span className="text-[11px] text-text-dim">{t("settings.backups.note")}</span>
          </label>

          <div className="flex flex-col gap-1">
            <span className="text-[11px] text-text-dim">{t("settings.excluded.label")}</span>
            <div className="flex flex-wrap gap-1.5">
              {draft.excluded_paths.map((path) => (
                <span
                  key={path}
                  data-ctx="excluded-chip"
                  onContextMenu={(event) => openExcludedMenu(event, path)}
                  className="flex items-center gap-1 rounded border border-line bg-surface-2 px-1.5 py-[3px] font-mono text-[11.5px] text-text-muted"
                >
                  {path}
                  <button
                    type="button"
                    onClick={() =>
                      patch({
                        excluded_paths: draft.excluded_paths.filter((item) => item !== path),
                      })
                    }
                    aria-label={t("settings.excluded.remove", { path })}
                    className="cursor-pointer text-text-dim transition-colors hover:text-state-danger"
                  >
                    <X className="h-2.5 w-2.5" />
                  </button>
                </span>
              ))}
              {draft.excluded_paths.length === 0 && (
                <span className="text-[11.5px] text-text-dim">{t("settings.excluded.empty")}</span>
              )}
            </div>
            <div className="flex items-center gap-1.5">
              <input
                value={newExcluded}
                placeholder="~/dev/sandbox/skills"
                onChange={(event) => setNewExcluded(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    addExcluded();
                  }
                }}
                className={clsx(inputClass, "mono text-[11px]")}
              />
              <Button size="sm" variant="outline" onClick={addExcluded} disabled={!newExcluded.trim()}>
                <Plus className="h-3 w-3" />
                {t("settings.excluded.add")}
              </Button>
            </div>
          </div>
        </div>
      </Section>

      <Section
        title={t("settings.market.section")}
        hint={t("settings.market.hint")}
        actions={
          <Badge tone={draft.market_registry.trim() ? "warn" : "dim"}>
            {draft.market_registry.trim()
              ? t("settings.market.customSource")
              : t("settings.market.defaultSource")}
          </Badge>
        }
      >
        <div className="flex flex-col gap-2 px-3 py-2.5">
          <div className="flex flex-col gap-1">
            <span className="text-[11px] text-text-dim">{t("settings.market.sourceLabel")}</span>
            <div className="flex flex-wrap items-center gap-1.5">
              {MARKET_SOURCES.map((item) => (
                <button
                  key={item.value}
                  type="button"
                  title={t(item.hint)}
                  aria-pressed={draft.market_source === item.value}
                  onClick={() =>
                    patch({ market_source: item.value, market_registry: "" })
                  }
                  className={clsx(
                    "cursor-pointer rounded-md border px-2 py-[3px] text-[11px] transition-colors",
                    draft.market_source === item.value
                      ? "border-brand-500/60 bg-surface-active text-text-strong"
                      : "border-line text-text-dim hover:text-text-strong"
                  )}
                >
                  {item.label}
                </button>
              ))}
              <span className="text-[11px] text-text-dim">
                {(() => {
                  const hint = MARKET_SOURCES.find(
                    (item) => item.value === draft.market_source
                  )?.hint;
                  return hint ? t(hint) : null;
                })()}
              </span>
            </div>
          </div>
          <label className="flex flex-col gap-1">
            <span className="flex items-center gap-2 text-[11px] text-text-dim">
              {t("settings.market.urlLabel")}
              <button
                type="button"
                onClick={() => patch({ market_registry: "" })}
                disabled={!draft.market_registry}
                className="cursor-pointer text-[11px] text-brand-300 transition-colors hover:text-brand-200 disabled:cursor-not-allowed disabled:text-text-dim"
              >
                {t("settings.market.followDefault")}
              </button>
            </span>
            <input
              value={draft.market_registry}
              placeholder={MARKET_SOURCE_BASES[draft.market_source]}
              onChange={(event) => patch({ market_registry: event.target.value })}
              className={clsx(inputClass, "mono text-[11px]")}
            />
            <span className="text-[11px] leading-relaxed text-text-dim">
              {t("settings.market.note", { base: MARKET_SOURCE_BASES[draft.market_source] })}
              {t("settings.market.note2")}
              {t("settings.market.note3")}
            </span>
          </label>
        </div>
      </Section>

      <Section title={t("settings.link.section")} hint={t("settings.link.hint")}>
        <div className="flex flex-col gap-2.5 px-3 py-2.5">
          <div
            className="flex flex-wrap items-center gap-1.5"
            data-platform-strategy={settings.platform_strategy}
          >
            <span className="mr-1 text-[11px] text-text-dim">
              {t("settings.link.defaultLabel")}
            </span>
            {STRATEGIES.map((item) => {
              const active =
                item.value === "auto"
                  ? draft.default_strategy === null
                  : draft.default_strategy === item.value;
              const disabled = isStrategyDisabled(item.value, settings.platform_strategy);
              return (
                <button
                  key={item.value}
                  type="button"
                  data-default-strategy={item.value}
                  disabled={disabled}
                  title={disabled ? junctionReason() : undefined}
                  onClick={() =>
                    patch({
                      default_strategy: item.value === "auto" ? null : item.value,
                    })
                  }
                  aria-pressed={active}
                  className={clsx(
                    "rounded-md border px-2 py-1 text-[11px] transition-colors",
                    disabled
                      ? "cursor-not-allowed border-line/60 text-text-dim/50"
                      : "cursor-pointer border-line text-text-muted hover:bg-surface-hover hover:text-text-strong",
                    active &&
                      !disabled &&
                      "border-brand-500/50 bg-brand-500/15 text-brand-300"
                  )}
                >
                  {t(item.label)}
                  {disabled ? t("common.strategy.onlyWindows") : ""}
                </button>
              );
            })}
          </div>
          <p className="text-[11px] text-text-dim">
            {t("settings.link.platformDefault")}
            <span className="mono mx-1 text-text-muted">{settings.platform_strategy}</span>
            {t("settings.link.overrideNote")}
          </p>

          <div className="flex items-center justify-between gap-3 border-t border-line pt-2">
            <div>
              <p className="text-[11.5px] text-text-strong">{t("settings.trash.title")}</p>
              <p className="text-[11px] text-text-dim">{t("settings.trash.note")}</p>
            </div>
            <Toggle
              checked={draft.use_trash_for_dedupe}
              onChange={(value) => patch({ use_trash_for_dedupe: value })}
            />
          </div>

          <div className="flex items-center justify-between gap-3 border-t border-line pt-2">
            <div>
              <p className="text-[11.5px] text-text-strong">
                {t("settings.backupConfig.title")}
              </p>
              <p className="text-[11px] text-text-dim">{t("settings.backupConfig.note")}</p>
            </div>
            <Toggle
              checked={draft.backup_config_files}
              onChange={(value) => patch({ backup_config_files: value })}
            />
          </div>
        </div>
      </Section>

      <Section
        title={t("settings.agentPaths.section")}
        hint={t("settings.agentPaths.hint")}
        actions={
          <Badge tone="warn">
            {t("settings.agentPaths.count", {
              count: Object.keys(normalize(draft).agent_paths).length,
            })}
          </Badge>
        }
      >
        <AgentPathTable
          agents={settings.agents}
          value={draft.agent_paths}
          onChange={(id, next) =>
            patch({ agent_paths: { ...draft.agent_paths, [id]: next } })
          }
        />
      </Section>

      <Section title={t("settings.custom.section")} hint={t("settings.custom.hint")}>
        <CustomAgentEditor
          defs={draft.custom_agents}
          onChange={(next) => patch({ custom_agents: next })}
        />
      </Section>

      <Section title={t("settings.terminal.section")} hint={t("settings.terminal.hint")}>
        <div className="flex flex-col gap-2.5 px-3 py-2.5">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <p className="text-[11.5px] text-text-strong">{t("settings.terminal.recordTitle")}</p>
              <p className="mt-0.5 text-[11.5px] leading-relaxed text-text-dim">
                {t("settings.terminal.recordNote")}
                {t("settings.terminal.recordNote2")}
              </p>
            </div>
            <Toggle checked={recordTerminal} onChange={toggleRecordTerminal} />
          </div>
        </div>
      </Section>

      <div className="sticky bottom-0 z-10 flex items-center justify-between gap-3 rounded-lg border border-line bg-ink-900 px-3 py-2.5 shadow-panel">
        <span className="text-[11px] text-text-dim">
          {dirty ? t("settings.dirty") : t("settings.clean")}
        </span>
        <div className="flex items-center gap-1.5">
          <Button
            size="sm"
            variant="ghost"
            onClick={() => setDraft(baseline)}
            disabled={!dirty || saving}
          >
            {t("settings.discard")}
          </Button>
          <Button size="sm" variant="primary" onClick={save} disabled={!dirty || saving}>
            {saving ? t("settings.saving") : t("settings.save")}
          </Button>
        </div>
      </div>
    </div>
  );
}
