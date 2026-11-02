import { clsx } from "clsx";
import { Check } from "lucide-react";
import { Badge } from "../ui";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import { useTheme, type ThemeMode } from "../../lib/theme";
import { themesOf, type AppTheme, type ThemeKind } from "../../lib/themes";

/**
 * 设置页「外观」模块。
 *
 * 读写的是 localStorage 里的本机偏好（lib/theme.tsx），与 config.json 的
 * draft/baseline/dirty 完全隔离：这里点一下立刻生效，不会让底部出现「有未保存的改动」。
 */

const MODES: { value: ThemeMode; label: MessageKey; hint: MessageKey }[] = [
  { value: "light", label: "appear.light", hint: "appear.lightHint" },
  { value: "dark", label: "appear.dark", hint: "appear.darkHint" },
  { value: "system", label: "appear.system", hint: "appear.systemHint" },
];

function SwatchRow({ theme }: { theme: AppTheme }) {
  const t = useT();
  const dots: { color: string; label: MessageKey }[] = [
    { color: theme.swatch.bg, label: "appear.role.bg" },
    { color: theme.swatch.panel, label: "appear.role.panel" },
    { color: theme.swatch.accent, label: "appear.role.accent" },
    { color: theme.swatch.text, label: "appear.role.text" },
  ];
  return (
    <span className="flex items-center gap-1">
      {dots.map((dot) => (
        <span
          key={dot.label}
          title={t(dot.label)}
          style={{ background: dot.color }}
          className="h-2.5 w-2.5 rounded-full ring-1 ring-inset ring-black/15"
        />
      ))}
    </span>
  );
}

function ThemeCard({
  theme,
  selected,
  live,
  onSelect,
}: {
  theme: AppTheme;
  selected: boolean;
  live: boolean;
  onSelect: () => void;
}) {
  const t = useT();
  return (
    <button
      type="button"
      aria-pressed={selected}
      onClick={onSelect}
      title={`${theme.name} · ${theme.family}`}
      className={clsx(
        "group flex cursor-pointer flex-col gap-1 rounded-md border px-2.5 py-2 text-left transition-colors",
        selected
          ? "border-brand-500/60 bg-surface-active"
          : "border-line bg-surface-2 hover:border-line-strong hover:bg-surface-hover"
      )}
    >
      <span className="flex items-center justify-between gap-2">
        <SwatchRow theme={theme} />
        {selected ? (
          <Check className="h-3 w-3 shrink-0 text-brand-300" />
        ) : (
          <span className="h-3 w-3 shrink-0" />
        )}
      </span>
      <span className="truncate text-[11.5px] font-medium text-text-strong">{theme.name}</span>
      <span className="flex items-center gap-1.5">
        <span className="truncate text-[11px] text-text-dim">{theme.family}</span>
        {live && (
          <Badge tone="brand" className="shrink-0 px-1 py-[1px] text-[11px]">
            {t("appear.active")}
          </Badge>
        )}
      </span>
    </button>
  );
}

function ThemeGroup({
  kind,
  title,
  hint,
  selectedId,
  liveId,
  onSelect,
}: {
  kind: ThemeKind;
  title: string;
  hint: string;
  selectedId: string;
  liveId: string;
  onSelect: (id: string) => void;
}) {
  const themes = themesOf(kind);
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-[11px] text-text-dim">{title}</span>
        <span className="truncate text-[11px] text-text-dim">{hint}</span>
      </div>
      <div className="grid grid-cols-2 gap-1.5 md:grid-cols-3">
        {themes.map((theme) => (
          <ThemeCard
            key={theme.id}
            theme={theme}
            selected={theme.id === selectedId}
            live={theme.id === liveId}
            onSelect={() => onSelect(theme.id)}
          />
        ))}
      </div>
    </div>
  );
}

export function AppearancePanel() {
  const {
    mode,
    resolved,
    active,
    darkThemeId,
    lightThemeId,
    setMode,
    setDarkTheme,
    setLightTheme,
    resetThemes,
  } = useTheme();
  const t = useT();

  const isDefault = darkThemeId === "github-dark" && lightThemeId === "github-light";

  return (
    <div className="flex flex-col gap-2.5 px-3 py-2.5">
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="mr-1 text-[11px] text-text-dim">{t("appear.modeLabel")}</span>
        {MODES.map((item) => {
          const selected = mode === item.value;
          return (
            <button
              key={item.value}
              type="button"
              title={t(item.hint)}
              aria-pressed={selected}
              onClick={() => setMode(item.value)}
              className={clsx(
                "cursor-pointer rounded-md border px-2 py-1 text-[11px] transition-colors",
                selected
                  ? "border-brand-500/50 bg-brand-500/15 text-brand-300"
                  : "border-line text-text-muted hover:bg-surface-hover hover:text-text-strong"
              )}
            >
              {t(item.label)}
            </button>
          );
        })}
        <span className="text-[11px] text-text-dim">
          {t("appear.effective")}
          <span className="text-text-muted">{active.name}</span>
          <span className="ml-1">
            {resolved === "dark" ? t("appear.effectiveDark") : t("appear.effectiveLight")}
          </span>
        </span>
      </div>

      <ThemeGroup
        kind="dark"
        title={t("appear.darkTheme")}
        hint={t("appear.darkThemeHint")}
        selectedId={darkThemeId}
        liveId={active.id}
        onSelect={setDarkTheme}
      />

      <ThemeGroup
        kind="light"
        title={t("appear.lightTheme")}
        hint={t("appear.lightThemeHint")}
        selectedId={lightThemeId}
        liveId={active.id}
        onSelect={setLightTheme}
      />

      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-line pt-2">
        <span className="text-[11px] leading-relaxed text-text-dim">{t("appear.note")}</span>
        <button
          type="button"
          onClick={resetThemes}
          disabled={isDefault}
          className="cursor-pointer rounded border border-line px-1.5 py-[2px] text-[11.5px] text-text-muted transition-colors hover:border-line-strong hover:text-text-strong disabled:cursor-not-allowed disabled:text-text-dim disabled:hover:border-line"
        >
          {t("appear.reset")}
        </button>
      </div>
    </div>
  );
}
