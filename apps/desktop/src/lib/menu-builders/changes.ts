import { Copy, History, Play, RotateCcw, X } from "lucide-react";
import { copyItem, dangerSection } from "./common";
import { sep, type MenuEntry } from "../menu";
import { tr } from "../i18n/runtime";
import type { BatchRecord } from "../types";

type Notify = (message: string) => void;

export interface BatchRowActions {
  openBatch: () => void;
  /** 已确认要回滚时才真正执行 */
  rollback: () => void;
  confirmRollback: () => void;
  notify: Notify;
}

export function batchRowMenu(batch: BatchRecord, actions: BatchRowActions): MenuEntry[] {
  return [
    { id: "batch:open", label: tr("menu.changes.locate"), icon: History, run: actions.openBatch },
    sep(),
    copyItem(tr("menu.changes.copyBatch"), batch.batch_id, actions.notify, {
      id: "batch:copy",
      icon: Copy,
    }),
    {
      id: "batch:info",
      label: tr("menu.changes.info", { kind: batch.kind, count: batch.op_count }),
      disabled: true,
      reason: batch.undone
        ? tr("menu.changes.alreadyRolledBack")
        : tr("menu.changes.canStillRollback"),
    },
    ...dangerSection([
      {
        id: "batch:rollback",
        label: tr("menu.changes.rollbackBatch"),
        icon: RotateCcw,
        danger: true,
        disabled: batch.undone,
        reason: batch.undone ? tr("menu.changes.rolledBackBefore") : undefined,
        run: actions.confirmRollback,
      },
    ]),
  ];
}

export interface ChangesPageActions {
  hasPlan: boolean;
  planOps: number;
  applying: boolean;
  apply: () => void;
  discard: () => void;
}

export function changesPageMenu(actions: ChangesPageActions): MenuEntry[] {
  return [
    {
      id: "changes:apply",
      label: actions.hasPlan
        ? tr("menu.changes.applyPlanWith", { count: actions.planOps })
        : tr("menu.changes.applyPlan"),
      icon: Play,
      pending: true,
      disabled: !actions.hasPlan || actions.applying,
      reason: actions.applying
        ? tr("menu.changes.applying")
        : actions.hasPlan
          ? undefined
          : tr("menu.changes.noPlan"),
      run: actions.apply,
    },
    {
      id: "changes:discard",
      label: tr("menu.changes.discard"),
      icon: X,
      disabled: !actions.hasPlan,
      reason: actions.hasPlan ? undefined : tr("menu.changes.noPlan"),
      run: actions.discard,
    },
    sep(),
  ];
}
