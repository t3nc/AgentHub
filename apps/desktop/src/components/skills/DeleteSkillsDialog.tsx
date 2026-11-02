import { useMemo, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { clsx } from "clsx";
import { AlertTriangle, ChevronDown, Trash2, X } from "lucide-react";
import { AgentIcon } from "../agents/AgentIcon";
import { Badge, Button, Toggle } from "../ui";
import { agentInfo, bytes, shortPath } from "../../lib/format";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import type { DeleteOptions, SkillRecord } from "../../lib/types";

/** 记录在删除语义下属于哪一类 —— 决定它会不会连带别的条目。 */
type Role = "body" | "copy" | "link";

function roleOf(record: SkillRecord, storeRoot: string): Role {
  if (record.source === "store" || record.path.startsWith(storeRoot)) return "body";
  if (record.link.kind === "symlink" || record.link.kind === "junction" || record.link.kind === "hardlink") {
    return "link";
  }
  return "copy";
}

const ROLE_LABEL: Record<Role, MessageKey> = {
  body: "del.source.store",
  copy: "del.source.copy",
  link: "del.source.link",
};

function targetOf(record: SkillRecord): string | null {
  if (record.link.kind === "symlink" || record.link.kind === "junction" || record.link.kind === "hardlink") {
    return record.link.target;
  }
  return null;
}

/**
 * 删除确认弹窗。
 *
 * 只做「摆清楚事实 + 收两个开关」：真正的文件操作在计划抽屉里逐条审阅，
 * 这样「人工确认」有两道 —— 这里确认删什么，那里确认改哪些路径。
 */
export function DeleteSkillsDialog({
  records,
  allRecords,
  storeRoot,
  onCancel,
  onConfirm,
}: {
  records: SkillRecord[];
  allRecords: SkillRecord[];
  storeRoot: string;
  onCancel: () => void;
  onConfirm: (opts: DeleteOptions) => void;
}) {
  const t = useT();
  const [withLinks, setWithLinks] = useState(true);
  const [useTrash, setUseTrash] = useState(true);
  const [expanded, setExpanded] = useState(false);

  /** 勾选的记录 + （可能）被连带清理的链接，按路径去重。 */
  const plan = useMemo(() => {
    const seen = new Set(records.map((record) => record.path));
    const bodies = records.filter((record) => roleOf(record, storeRoot) === "body");
    const bodyPaths = new Set(bodies.map((record) => record.path));
    const cascade: SkillRecord[] = [];
    for (const record of allRecords) {
      const target = targetOf(record);
      if (!target || !bodyPaths.has(target) || seen.has(record.path)) continue;
      seen.add(record.path);
      cascade.push(record);
    }
    const links = records.filter((record) => roleOf(record, storeRoot) === "link");
    const copies = records.filter((record) => roleOf(record, storeRoot) === "copy");
    const freed = [...records, ...cascade].reduce((sum, record) => sum + record.size_bytes, 0);
    return {
      bodies,
      copies,
      links,
      cascade,
      freed,
      ops: records.length + (withLinks ? cascade.length : 0),
    };
  }, [records, allRecords, storeRoot, withLinks]);

  const visible = expanded ? records : records.slice(0, 6);
  const hidden = records.length - visible.length;

  return (
    <AnimatePresence>
      <motion.div
        key="delete-skills"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.12 }}
        className="fixed inset-0 z-[66] flex items-start justify-center bg-[rgb(var(--scrim)/0.32)] px-6 pt-[10vh] dark:bg-[rgb(var(--scrim)/0.62)]"
        onMouseDown={(event) => {
          if (event.target === event.currentTarget) onCancel();
        }}
      >
        <motion.div
          role="dialog"
          aria-modal="true"
          aria-label={t("del.aria")}
          data-delete-dialog
          initial={{ opacity: 0, y: -6 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -4 }}
          transition={{ duration: 0.16, ease: "easeOut" }}
          className="w-[540px] overflow-hidden rounded-lg border border-line bg-ink-900 shadow-[var(--shadow-drawer)]"
        >
        <header className="flex items-start gap-2.5 border-b border-line px-4 py-3">
          <span className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-md border border-state-danger/30 bg-state-danger/10 text-state-danger">
            <Trash2 className="h-3.5 w-3.5" />
          </span>
          <div className="min-w-0 flex-1">
            <h2 className="text-[13.5px] font-medium text-text-strong">
              {t("del.title", { count: records.length })}
            </h2>
            <p className="mt-0.5 text-[11px] leading-relaxed text-text-dim">
              {useTrash ? t("del.trashNote") : t("del.permanentNote")}
            </p>
          </div>
          <button
            type="button"
            onClick={onCancel}
            aria-label={t("del.close")}
            className="cursor-pointer rounded p-1 text-text-dim transition-colors hover:bg-surface-hover hover:text-text-strong"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </header>

        <div className="px-4 py-3">
          <div className="flex items-center justify-between rounded-md border border-line bg-surface-2 px-2.5 py-2">
            <span className="text-[11.5px] text-text-muted">
              {t("del.opsCount", { count: plan.ops })}
            </span>
            <span className="mono text-[11px] text-text-dim">
              {t("del.willFree", { bytes: bytes(plan.freed) })}
            </span>
          </div>

          <ul className="mt-2 divide-y divide-line overflow-hidden rounded-md border border-line">
            {visible.map((record) => {
              const role = roleOf(record, storeRoot);
              const info = agentInfo(record.agent);
              return (
                <li
                  key={record.path}
                  data-delete-row={record.name}
                  className="flex items-center gap-2 px-2.5 py-1.5"
                >
                  <span
                    className={clsx(
                      "flex shrink-0 items-center rounded border px-1 py-[1px]",
                      info.accent
                    )}
                    title={info.name}
                  >
                    <AgentIcon id={record.agent ?? "store"} className="h-3.5 w-3.5" />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[12px] text-text-strong">
                      {record.name}
                    </span>
                    <span className="mono block truncate text-[11px] text-text-dim">
                      {shortPath(record.path, 4)}
                    </span>
                  </span>
                  <Badge tone={role === "body" ? "brand" : role === "link" ? "dim" : "warn"}>
                    {t(ROLE_LABEL[role])}
                  </Badge>
                  <span className="mono w-16 shrink-0 text-right text-[11.5px] text-text-dim">
                    {bytes(record.size_bytes)}
                  </span>
                </li>
              );
            })}
          </ul>

          {hidden > 0 && (
            <button
              type="button"
              onClick={() => setExpanded(true)}
              className="mt-1.5 flex cursor-pointer items-center gap-1 text-[11.5px] text-brand-300 transition-colors hover:text-brand-200"
            >
              <ChevronDown className="h-3 w-3" />
              {t("del.moreItems", { count: hidden })}
            </button>
          )}

          <div className="mt-3 space-y-2">
            {plan.cascade.length > 0 && (
              <div className="flex items-start justify-between gap-3 rounded-md border border-line px-2.5 py-2">
                <div className="min-w-0">
                  <p className="text-[11.5px] text-text-strong">
                    {t("del.cascade", { count: plan.cascade.length })}
                  </p>
                  <p className="mt-0.5 text-[11px] leading-relaxed text-text-dim">
                    {t("del.cascadeNote")}
                  </p>
                </div>
                <Toggle checked={withLinks} onChange={setWithLinks} />
              </div>
            )}

            <div className="flex items-start justify-between gap-3 rounded-md border border-line px-2.5 py-2">
              <div className="min-w-0">
                <p className="text-[11.5px] text-text-strong">{t("del.trashToggle")}</p>
                <p className="mt-0.5 text-[11px] leading-relaxed text-text-dim">
                  {t("del.trashToggleNote")}
                </p>
              </div>
              <Toggle checked={useTrash} onChange={setUseTrash} />
            </div>
          </div>

          {!useTrash && (
            <p className="mt-2 flex items-start gap-1.5 rounded-md border border-state-danger/30 bg-state-danger/10 px-2.5 py-1.5 text-[11.5px] leading-relaxed text-state-danger">
              <AlertTriangle className="mt-[1px] h-3 w-3 shrink-0" />
              {t("del.permanentWarn")}
            </p>
          )}
        </div>

        <footer className="flex items-center justify-end gap-1.5 border-t border-line px-4 py-2.5">
          <Button size="sm" variant="ghost" onClick={onCancel}>
            {t("common.cancel")}
          </Button>
          <Button
            size="sm"
            variant="danger"
            onClick={() => onConfirm({ use_trash: useTrash, with_links: withLinks })}
          >
            {t("del.next")}
          </Button>
        </footer>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
