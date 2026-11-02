import { useEffect, useState } from "react";
import { Check, Copy, ExternalLink } from "lucide-react";
import { copyText } from "../../lib/clipboard";
import { useT } from "../../lib/i18n";
import type { CatalogEntry } from "../../lib/types";

/**
 * 行内展开的安装指引：全部是「可复制的文本」，没有任何执行入口。
 * 桌面类产品的 command 位是下载页地址，文案用「打开」而不是「运行」。
 */
export function InstallSteps({ entry }: { entry: CatalogEntry }) {
  const t = useT();
  const [copied, setCopied] = useState<string | null>(null);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(null), 1200);
    return () => window.clearTimeout(timer);
  }, [copied]);

  const copy = (value: string) => {
    void copyText(value).then((ok) => {
      if (ok) setCopied(value);
      // 控制台日志面向开发者，不参与界面本地化
      else console.error("copy failed, select the command manually");
    });
  };

  if (entry.steps.length === 0) {
    return (
      <div className="border-t border-line bg-surface-2/40 px-3 py-2.5">
        {entry.evidence.length > 0 && (
          <div className="mb-1 flex flex-wrap items-baseline gap-x-3 gap-y-1">
            <span className="text-[11px] uppercase tracking-wider text-text-dim">
              {t("install.basis")}
            </span>
            {entry.evidence.map((item) => (
              <span key={item} className="mono text-[11.5px] text-text-muted">
                {item}
              </span>
            ))}
          </div>
        )}
        <p className="text-[11px] text-text-dim">{t("install.noGuide")}</p>
      </div>
    );
  }

  return (
    <div className="border-t border-line bg-surface-2/40 px-3 py-2.5">
      {entry.evidence.length > 0 && (
        <div className="mb-2 flex flex-wrap items-baseline gap-x-3 gap-y-1">
          <span className="text-[11px] uppercase tracking-wider text-text-dim">
            {t("install.basis")}
          </span>
          {entry.evidence.map((item) => (
            <span key={item} className="mono text-[11.5px] text-text-muted">
              {item}
            </span>
          ))}
        </div>
      )}
      <div className="mb-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11.5px] text-text-dim">
        <span>
          {t("install.platform")}{" "}
          <span className="mono text-text-muted">{entry.platform || "—"}</span>
        </span>
        {entry.official && (
          <span className="flex min-w-0 items-center gap-1">
            <span>{t("install.source")}</span>
            <span className="mono truncate text-text-muted" title={entry.official}>
              {entry.official}
            </span>
            <button
              type="button"
              onClick={() => copy(entry.official)}
              className="cursor-pointer rounded px-1 text-brand-300 transition-colors hover:bg-surface-hover"
            >
              {t("install.copyUrl")}
            </button>
          </span>
        )}
      </div>

      <ol className="flex flex-col gap-2">
        {entry.steps.map((step, index) => (
          <li key={`${entry.key}-${step.title}`}>
            <div className="flex items-baseline gap-2">
              <span className="mono text-[11px] text-text-dim">
                {String(index + 1).padStart(2, "0")}
              </span>
              <span className="text-[12px] font-medium text-text-strong">{step.title}</span>
            </div>
            <div className="mt-1 flex items-center gap-2 rounded-md border border-line bg-ink-950 px-2 py-1">
              <code
                className="mono min-w-0 flex-1 truncate text-[11.5px] text-text-muted"
                title={step.command}
              >
                {step.command}
              </code>
              {step.url && (
                <span
                  className="shrink-0 text-text-dim"
                  title={step.url}
                  aria-label={t("install.official")}
                >
                  <ExternalLink className="h-3 w-3" />
                </span>
              )}
              <button
                type="button"
                onClick={() => copy(step.command)}
                className="flex shrink-0 cursor-pointer items-center gap-1 rounded border border-line px-1.5 py-[2px] text-[11px] text-text-muted transition-colors hover:bg-surface-hover hover:text-text-strong"
              >
                {copied === step.command ? (
                  <>
                    <Check className="h-2.5 w-2.5 text-state-ok" />
                    {t("install.copied")}
                  </>
                ) : (
                  <>
                    <Copy className="h-2.5 w-2.5" />
                    {t("install.copy")}
                  </>
                )}
              </button>
            </div>
            {step.risk && (
              <p className="mt-1 text-[11.5px] leading-relaxed text-brass">{step.risk}</p>
            )}
          </li>
        ))}
      </ol>

      <p className="mt-2.5 text-[11.5px] leading-relaxed text-text-dim">
        {t("install.footnote")}
      </p>
    </div>
  );
}
