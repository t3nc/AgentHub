import {
  Check,
  Copy,
  Eye,
  GitCompare,
  Layers,
  Link2,
  Lock,
  LockOpen,
  MousePointerClick,
  PackageOpen,
  ShieldQuestion,
  Trash2,
} from "lucide-react";
import type { MenuEnv } from "../context-menu";
import { dangerSection, copyItem, openFileItem, revealItem, terminalItem } from "./common";
import { sep, type MenuEntry, type MenuItem } from "../menu";
import { tr } from "../i18n/runtime";
import type { LinkStrategyName, SkillRecord } from "../types";

type Notify = (message: string) => void;

import { LINK_STRATEGIES, isStrategyDisabled, junctionReason } from "../link-strategies";

export interface SkillRowActions {
  openDetail: () => void;
  toggleSelect: () => void;
  isSelected: boolean;
  /** 同名记录数 > 1 才能对比 */
  hasSiblings: boolean;
  effectiveStrategy: LinkStrategyName | null;
  /** 平台默认策略（Windows = junction），用来把当前平台用不了的策略置灰 */
  platformStrategy?: LinkStrategyName | null;
  setStrategy: (name: string, strategy: LinkStrategyName | null) => void;
  /** 标记 / 取消「agent 独有」：该名字不参与去重接管与链接修复 */
  toggleAgentLocal: (record: SkillRecord) => void;
  requestDelete: (record: SkillRecord) => void;
  env: MenuEnv;
  notify: Notify;
}

/** 技能行 / 网格卡片：条目级菜单 */
export function skillRowMenu(record: SkillRecord, actions: SkillRowActions): MenuEntry[] {
  const { env, notify } = actions;
  const strategyChildren: MenuItem[] = [
    {
      id: "strategy:default",
      label: tr("skills.menu.followDefault"),
      icon: actions.effectiveStrategy === null ? Check : undefined,
      hint: actions.effectiveStrategy === null ? tr("skills.menu.current") : undefined,
      run: () => actions.setStrategy(record.name, null),
    },
    ...LINK_STRATEGIES.map<MenuItem>((strategy) => {
      // 目录联接只有 Windows 有实现：置灰在前，别让用户点了才在 apply 阶段看到 Unsupported
      const disabled = isStrategyDisabled(strategy.value, actions.platformStrategy);
      return {
        id: `strategy:${strategy.value}`,
        label: disabled ? `${tr(strategy.label)}${tr("common.strategy.onlyWindows")}` : tr(strategy.label),
        icon: actions.effectiveStrategy === strategy.value ? Check : undefined,
        hint: actions.effectiveStrategy === strategy.value ? tr("skills.menu.current") : undefined,
        disabled,
        reason: disabled ? junctionReason() : undefined,
        run: () => actions.setStrategy(record.name, strategy.value),
      };
    }),
  ];

  const entries: MenuEntry[] = [
    { id: "skill:detail", label: tr("skills.menu.detail"), icon: Eye, run: actions.openDetail },
    {
      id: "skill:diff",
      label: tr("skills.menu.diff"),
      icon: GitCompare,
      disabled: !actions.hasSiblings,
      reason: actions.hasSiblings ? undefined : tr("skills.menu.noSiblings"),
      run: () => env.openSkillDiff(record.name),
    },
    {
      id: "skill:filter",
      label: tr("skills.menu.filter"),
      icon: MousePointerClick,
      run: () => env.openSkill(record.name),
    },
    sep(),
    terminalItem(record.path, env),
    revealItem(record.path, notify, "skill:reveal"),
    openFileItem(`${record.path}/SKILL.md`, notify, tr("skills.menu.openSkillMd")),
    sep(),
    copyItem(tr("skills.menu.copyPath"), record.path, notify, { id: "skill:copy-path", icon: Copy }),
    copyItem(tr("skills.menu.copyName"), record.name, notify, { id: "skill:copy-name" }),
    {
      id: "skill:strategy",
      label: tr("skills.menu.strategy"),
      icon: Link2,
      disabled: record.readonly,
      reason: record.readonly ? tr("skills.menu.readonlyReason") : undefined,
      children: strategyChildren,
    },
    {
      // computer-use 这类技能各 agent 的实现互不通用，收敛只会互相覆盖；
      // 标记后不参与去重接管、链接修复与重复统计，但技能本身照常可用。
      id: "skill:agent-local",
      label: record.agent_local ? tr("skills.menu.unmarkLocal") : tr("skills.menu.markLocal"),
      icon: record.agent_local ? LockOpen : Lock,
      hint: record.agent_local ? tr("skills.menu.agentLocalHint") : undefined,
      disabled: record.readonly,
      reason: record.readonly ? tr("skills.menu.readonlyReason") : undefined,
      run: () => actions.toggleAgentLocal(record),
    },
    {
      id: "skill:select",
      label: actions.isSelected
        ? tr("skills.menu.removeFromSelection")
        : tr("skills.menu.addToSelection"),
      icon: MousePointerClick,
      run: actions.toggleSelect,
    },
    ...dangerSection([
      {
        id: "skill:delete",
        label: tr("skills.menu.deleteOne"),
        icon: Trash2,
        danger: true,
        disabled: record.readonly,
        reason: record.readonly ? tr("skills.menu.readonlyDelete") : undefined,
        run: () => actions.requestDelete(record),
      },
    ]),
  ];
  return entries;
}

export interface SkillPageActions {
  selectedCount: number;
  planSelected: () => void;
  clearSelection: () => void;
  deleteSelected: () => void;
  openMarket: () => void;
  toggleLinkedOnly: () => void;
  linkedOnly: boolean;
  toggleView: () => void;
  view: "list" | "grid";
}

/** 技能库空白处：页面级菜单 */
export function skillPageMenu(actions: SkillPageActions): MenuEntry[] {
  return [
    {
      id: "page:market",
      label: tr("skills.menu.installFromMarket"),
      icon: PackageOpen,
      run: actions.openMarket,
    },
    {
      id: "page:plan-selected",
      label:
        actions.selectedCount > 0
          ? tr("skills.menu.planSelectedWith", { count: actions.selectedCount })
          : tr("skills.menu.planSelected"),
      icon: Layers,
      pending: true,
      disabled: actions.selectedCount === 0,
      reason: actions.selectedCount === 0 ? tr("skills.menu.pickFirst") : undefined,
      run: actions.planSelected,
    },
    {
      id: "page:linked-only",
      label: actions.linkedOnly ? tr("skills.menu.showAdopted") : tr("skills.menu.onlyUnadopted"),
      icon: ShieldQuestion,
      run: actions.toggleLinkedOnly,
    },
    {
      id: "page:view",
      label: actions.view === "list" ? tr("skills.menu.toGrid") : tr("skills.menu.toList"),
      run: actions.toggleView,
    },
    {
      id: "page:clear",
      label: tr("skills.menu.clearSelection"),
      disabled: actions.selectedCount === 0,
      reason: actions.selectedCount === 0 ? tr("skills.menu.noSelection") : undefined,
      run: actions.clearSelection,
    },
    ...dangerSection([
      {
        id: "page:delete",
        label:
          actions.selectedCount > 0
            ? tr("skills.menu.deleteSelectedWith", { count: actions.selectedCount })
            : tr("skills.menu.deleteSelected"),
        icon: Trash2,
        danger: true,
        disabled: actions.selectedCount === 0,
        reason: actions.selectedCount === 0 ? tr("skills.menu.pickFirst") : undefined,
        run: actions.deleteSelected,
      },
    ]),
  ];
}
