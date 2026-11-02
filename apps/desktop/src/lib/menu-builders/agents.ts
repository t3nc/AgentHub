import { Bot, Copy, Download, Eye, ListChecks, Trash2 } from "lucide-react";
import type { MenuEnv } from "../context-menu";
import { copyItem, dangerSection, openFileItem, revealItem } from "./common";
import { sep, type MenuEntry } from "../menu";
import { tr } from "../i18n/runtime";
import type { CatalogEntry } from "../types";

type Notify = (message: string) => void;

export interface AgentRowActions {
  entry: CatalogEntry;
  openSteps: () => void;
  requestAdd: () => void;
  confirmAdd: () => void;
  requestRemove: () => void;
  pendingAdd: boolean;
  env: MenuEnv;
  notify: Notify;
}

export function agentRowMenu(actions: AgentRowActions): MenuEntry[] {
  const { entry, env, notify } = actions;
  const manageable = entry.state === "ready" || entry.state === "config_only";
  const managed = entry.managed === "builtin" || entry.managed === "managed";
  const installCommand = entry.steps.find((step) => step.command)?.command ?? entry.docs;

  return [
    {
      id: "agent:skills",
      label: tr("menu.agent.onlyIts"),
      icon: Eye,
      disabled: !entry.builtin && !entry.shared_agent && !entry.managed_id,
      reason: tr("menu.agent.noSkillDir"),
      run: () => env.openAgent(entry.shared_agent ?? entry.builtin ?? entry.key),
    },
    {
      id: "agent:steps",
      label: tr("menu.agent.expandSteps"),
      icon: ListChecks,
      disabled: entry.steps.length === 0,
      reason: entry.steps.length === 0 ? tr("menu.agent.noSteps") : undefined,
      run: actions.openSteps,
    },
    sep(),
    {
      id: "agent:add",
      label: actions.pendingAdd
        ? tr("agents.row.confirmRegister")
        : tr("agents.managed.add"),
      icon: Download,
      disabled: actions.pendingAdd ? false : managed || !manageable,
      reason: managed
        ? tr("menu.agent.alreadyManaged")
        : manageable
          ? undefined
          : tr("menu.agent.notDetected"),
      run: actions.pendingAdd ? actions.confirmAdd : actions.requestAdd,
    },
    ...(entry.config_path ? [revealItem(entry.config_path, notify, "agent:reveal")] : []),
    ...(entry.config_path
      ? [openFileItem(entry.config_path, notify, tr("menu.agent.openConfig"))]
      : []),
    ...(entry.config_path
      ? [
          copyItem(tr("menu.agent.copyConfigPath"), entry.config_path, notify, {
            id: "agent:copy-path",
            icon: Copy,
          }),
        ]
      : []),
    ...(installCommand
      ? [
          copyItem(tr("menu.agent.copyInstall"), installCommand, notify, {
            id: "agent:copy-install",
            icon: Bot,
          }),
        ]
      : []),
    {
      id: "agent:family",
      label: `${entry.family} · ${entry.vendor}`,
      disabled: true,
      reason: entry.version
        ? tr("menu.agent.version", { version: entry.version })
        : tr("menu.agent.noVersion"),
    },
    ...dangerSection(
      managed && entry.managed !== "builtin"
        ? [
            {
              id: "agent:remove",
              label: tr("agents.managed.remove"),
              icon: Trash2,
              danger: true,
              run: actions.requestRemove,
            },
          ]
        : []
    ),
  ];
}

export interface AgentsPageActions {
  probe: () => void;
  probing: boolean;
}

export function agentsPageMenu(actions: AgentsPageActions): MenuEntry[] {
  return [
    {
      id: "agents:probe",
      label: tr("menu.agent.probe"),
      icon: Bot,
      pending: true,
      disabled: actions.probing,
      reason: actions.probing ? tr("menu.agent.probing") : undefined,
      run: actions.probe,
    },
  ];
}
