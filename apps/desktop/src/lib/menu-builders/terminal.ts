import { Copy, Download, History, RotateCcw, Search, SquareTerminal, X } from "lucide-react";
import { copyItem, dangerSection } from "./common";
import { sep, type MenuEntry } from "../menu";
import { tr } from "../i18n/runtime";

type Notify = (message: string) => void;

export interface TermTabActions {
  id: string;
  title: string;
  cwd: string;
  activate: () => void;
  restart: () => void;
  close: () => void;
  notify: Notify;
}

export function termTabMenu(actions: TermTabActions): MenuEntry[] {
  return [
    {
      id: "tab:activate",
      label: tr("menu.term.activate"),
      icon: SquareTerminal,
      run: actions.activate,
    },
    {
      id: "tab:restart",
      label: tr("terminal.restart"),
      icon: RotateCcw,
      pending: true,
      run: actions.restart,
    },
    sep(),
    copyItem(tr("menu.term.copyCwd"), actions.cwd, actions.notify, {
      id: "tab:copy-cwd",
      icon: Copy,
    }),
    ...dangerSection([
      { id: "tab:close", label: tr("menu.term.closeTab"), icon: X, danger: true, run: actions.close },
    ]),
  ];
}

export interface TerminalPageActions {
  newShell: () => void;
  openHistory: () => void;
  openSearch: () => void;
  exportOutput: () => void;
  hasTabs: boolean;
}

export function terminalPageMenu(actions: TerminalPageActions): MenuEntry[] {
  return [
    {
      id: "term:new",
      label: tr("menu.term.newShell"),
      icon: SquareTerminal,
      run: actions.newShell,
    },
    {
      id: "term:history",
      label: tr("terminal.recent"),
      icon: History,
      run: actions.openHistory,
    },
    {
      id: "term:search",
      label: tr("menu.term.search"),
      icon: Search,
      disabled: !actions.hasTabs,
      reason: actions.hasTabs ? undefined : tr("menu.term.noTabs"),
      run: actions.openSearch,
    },
    {
      id: "term:export",
      label: tr("menu.term.export"),
      icon: Download,
      disabled: !actions.hasTabs,
      reason: actions.hasTabs ? undefined : tr("menu.term.noTabs"),
      run: actions.exportOutput,
    },
  ];
}
