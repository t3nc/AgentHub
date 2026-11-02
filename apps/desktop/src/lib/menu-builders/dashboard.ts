import { Copy, Eye, Layers, MousePointerClick, Wrench } from "lucide-react";
import type { MenuEnv } from "../context-menu";
import { copyItem } from "./common";
import { sep, type MenuEntry } from "../menu";
import { tr } from "../i18n/runtime";
import type { DuplicateGroup } from "../types";

type Notify = (message: string) => void;

export interface DupRowActions {
  group: DuplicateGroup;
  selected: boolean;
  select: () => void;
  toggle: () => void;
  planOne: () => void;
  env: MenuEnv;
  notify: Notify;
}

/** 总览的重复技能行 */
export function dupRowMenu(actions: DupRowActions): MenuEntry[] {
  const { group, env } = actions;
  const representative = group.members[0]?.path ?? "";
  return [
    {
      id: "dup:select",
      label: tr("menu.dash.reviewGroup"),
      icon: Eye,
      run: actions.select,
    },
    {
      id: "dup:plan",
      label: tr("menu.dash.planGroup"),
      icon: Layers,
      hint: tr("menu.dash.planGroupHint", {
        copies: group.members.length,
        bytes: group.wasted_bytes,
      }),
      run: actions.planOne,
    },
    {
      id: "dup:filter",
      label: tr("menu.dash.filterSkill"),
      icon: MousePointerClick,
      run: () => env.openSkill(group.name),
    },
    sep(),
    copyItem(tr("menu.dash.copyName"), group.name, actions.notify, {
      id: "dup:copy-name",
      icon: Copy,
    }),
    ...(representative
      ? [copyItem(tr("menu.dash.copyPath"), representative, actions.notify, { id: "dup:copy-path" })]
      : []),
    {
      id: "dup:pick",
      label: actions.selected ? tr("menu.dash.unpick") : tr("menu.dash.pick"),
      run: actions.toggle,
    },
  ];
}

export interface DashPageActions {
  pickedCount: number;
  planPicked: () => void;
  repair: () => void;
  /** 不传 agent 就是「打开技能库」；图节点右键时传 agent id */
  openSkills: (agentId?: string) => void;
}

/** 总览空白处 / 图谱背景 / 中央仓库节点 */
export function dashboardPageMenu(actions: DashPageActions): MenuEntry[] {
  return [
    {
      id: "dash:plan-picked",
      label:
        actions.pickedCount > 0
          ? tr("menu.dash.planPickedWith", { count: actions.pickedCount })
          : tr("menu.dash.planPicked"),
      icon: Layers,
      pending: true,
      disabled: actions.pickedCount === 0,
      reason: actions.pickedCount === 0 ? tr("menu.dash.pickFirst") : undefined,
      run: actions.planPicked,
    },
    {
      id: "dash:repair",
      label: tr("menu.dash.repair"),
      icon: Wrench,
      pending: true,
      run: actions.repair,
    },
    {
      id: "dash:skills",
      label: tr("menu.dash.openLibrary"),
      icon: MousePointerClick,
      run: () => actions.openSkills(),
    },
  ];
}

/** 图谱里的 agent 节点 */
export function agentNodeMenu(agentId: string, env: MenuEnv): MenuEntry[] {
  return [
    {
      id: `node:skills:${agentId}`,
      label: tr("menu.dash.onlyAgent"),
      icon: MousePointerClick,
      run: () => env.openAgent(agentId),
    },
    {
      id: `node:env:${agentId}`,
      label: tr("menu.dash.inAgentEnv"),
      run: () => env.openAgentEnv(agentId),
    },
  ];
}
