import { Badge } from "../ui";
import { withTilde } from "../../lib/format";
import { useT } from "../../lib/i18n";
import { tr } from "../../lib/i18n/runtime";
import type { UsageSourceStats } from "../../lib/types";

function formatLabel(format: string): string {
  switch (format) {
    case "claude_jsonl":
      // 格式名本身是技术名词，直接原样
      return "Claude JSONL";
    case "codex_jsonl":
      return tr("src.codex");
    case "open_ai_jsonl":
      return tr("src.openaiJsonl");
    case "codebuddy_ide_index":
      return tr("src.ideIndex");
    default:
      return tr("src.noUsageField");
  }
}

/**
 * 数据源面板。
 *
 * 「拿不到用量」的产品逐条列出并写明原因，而不是显示 0 —— 这是本功能的诚实边界：
 * 能解析的五家全部计入，其余的看着像「有数据」其实只是没数。
 */
export function SourcePanel({ sources }: { sources: UsageSourceStats[] }) {
  const t = useT();
  const usable = sources.filter((item) => !item.source.unsupported);
  const unusable = sources.filter((item) => item.source.unsupported);

  return (
    <div className="flex flex-col gap-2.5" data-usage-sources>
      {usable.map((item) => (
        <div key={item.source.key} className="rounded-md border border-line/70 px-2.5 py-2">
          <div className="flex items-center justify-between gap-2">
            <span className="truncate text-[12px] text-text-strong">{item.source.label}</span>
            {item.source.exists ? (
              <Badge tone="ok">{t("src.parsed")}</Badge>
            ) : (
              <Badge tone="dim">{t("path.notDetected")}</Badge>
            )}
          </div>
          <div className="mt-1 text-[11px] text-text-dim">
            {formatLabel(item.source.format)} ·{" "}
            {t("src.counts", { files: item.files, records: item.records })}
            {item.bad_lines > 0 ? t("src.badLines", { count: item.bad_lines }) : ""}
          </div>
          {item.source.roots[0] && (
            <div className="mt-0.5 truncate text-[11px] text-text-dim" title={item.source.roots[0]}>
              {withTilde(item.source.roots[0])}
            </div>
          )}
        </div>
      ))}

      {unusable.length > 0 && (
        <div className="mt-1">
          <div className="mb-1.5 flex items-center gap-1.5 text-[11px] text-text-dim">
            <span className="inline-block h-1.5 w-1.5 rounded-full bg-brass" />
            {t("usage.unusableHint")}
          </div>
          <ul className="flex flex-col gap-1">
            {unusable.map((item) => (
              <li
                key={item.source.key}
                className="rounded-md border border-brass/25 bg-brass/5 px-2.5 py-1.5"
                data-unsupported={item.source.key}
              >
                <div className="text-[11.5px] text-text-strong">{item.source.label}</div>
                <div className="text-[11px] leading-relaxed text-text-dim">
                  {item.source.unsupported}
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
