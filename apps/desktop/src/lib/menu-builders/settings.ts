import { Copy, RotateCcw, Save, Trash2 } from "lucide-react";
import { copyItem, dangerSection } from "./common";
import { sep, type MenuEntry } from "../menu";
import { tr } from "../i18n/runtime";

type Notify = (message: string) => void;

/** 设置页的排除路径 chip */
export function excludedPathMenu(path: string, actions: { remove: () => void; notify: Notify }): MenuEntry[] {
  return [
    copyItem(tr("menu.instr.copyPath"), path, actions.notify, { id: "excluded:copy", icon: Copy }),
    ...dangerSection([
      {
        id: "excluded:remove",
        label: tr("menu.settings.removeExcluded"),
        icon: Trash2,
        danger: true,
        run: actions.remove,
      },
    ]),
  ];
}

export interface SettingsPageActions {
  dirty: boolean;
  saving: boolean;
  save: () => void;
  discard: () => void;
  followDefaultRegistry: boolean;
  resetRegistry: () => void;
}

export function settingsPageMenu(actions: SettingsPageActions): MenuEntry[] {
  return [
    {
      id: "settings:save",
      label: tr("settings.save"),
      icon: Save,
      pending: true,
      disabled: actions.saving,
      reason: actions.saving
        ? tr("menu.settings.saving")
        : actions.dirty
          ? undefined
          : tr("menu.settings.noUnsaved"),
      run: actions.save,
    },
    {
      id: "settings:discard",
      label: tr("menu.settings.discardChanges"),
      icon: RotateCcw,
      disabled: !actions.dirty,
      reason: actions.dirty ? undefined : tr("menu.settings.noChanges"),
      run: actions.discard,
    },
    sep(),
    {
      id: "settings:registry",
      label: tr("menu.settings.followDefaultRegistry"),
      icon: RotateCcw,
      disabled: actions.followDefaultRegistry,
      reason: actions.followDefaultRegistry ? tr("menu.settings.alreadyDefault") : undefined,
      run: actions.resetRegistry,
    },
  ];
}
