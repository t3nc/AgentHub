import { Copy, Eye, FileText, FolderOpen, Hammer } from "lucide-react";
import type { MenuEnv } from "../context-menu";
import { copyItem, openFileItem, revealItem } from "./common";
import { sep, type MenuEntry } from "../menu";
import { tr } from "../i18n/runtime";
import type { InstructionSpec } from "../types";

type Notify = (message: string) => void;

export interface InstructionRowActions {
  open: () => void;
  env: MenuEnv;
  notify: Notify;
}

export function instructionRowMenu(
  spec: InstructionSpec,
  actions: InstructionRowActions
): MenuEntry[] {
  const dir = spec.path.replace(/[\\/][^\\/]+$/, "");
  return [
    { id: "instr:open", label: tr("menu.instr.open"), icon: Eye, run: actions.open },
    sep(),
    revealItem(dir, actions.notify, "instr:reveal"),
    openFileItem(spec.path, actions.notify),
    copyItem(tr("menu.instr.copyPath"), spec.path, actions.notify, {
      id: "instr:copy",
      icon: Copy,
    }),
    sep(),
    {
      id: "instr:managed",
      label: spec.managed ? tr("menu.instr.managed") : tr("menu.instr.unmanaged"),
      icon: FileText,
      disabled: true,
      reason: spec.exists
        ? spec.managed
          ? tr("menu.instr.managedReason")
          : tr("menu.instr.unmanagedReason")
        : tr("menu.instr.missing"),
    },
    {
      id: "instr:terminal",
      label: tr("menu.instr.terminal"),
      icon: Hammer,
      run: () => actions.env.openTerminal(dir),
    },
  ];
}

export interface InstructionPageActions {
  planInject: () => void;
  copySnippet: () => void;
  bridge: () => void;
  notify: Notify;
}

export function instructionPageMenu(actions: InstructionPageActions): MenuEntry[] {
  return [
    {
      id: "instr:plan",
      label: tr("menu.instr.plan"),
      icon: Hammer,
      pending: true,
      run: actions.planInject,
    },
    {
      id: "instr:copy-snippet",
      label: tr("menu.instr.copySnippet"),
      icon: Copy,
      run: actions.copySnippet,
    },
    {
      id: "instr:bridge",
      label: tr("menu.instr.bridge"),
      icon: FileText,
      pending: true,
      run: actions.bridge,
    },
    sep(),
    {
      id: "instr:dir",
      label: tr("menu.instr.dir"),
      icon: FolderOpen,
      disabled: true,
      reason: tr("menu.instr.dirReason"),
    },
  ];
}
