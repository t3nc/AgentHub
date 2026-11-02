import { Copy, Eye, FolderGit2, Layers, SquareTerminal } from "lucide-react";
import type { MenuEnv } from "../context-menu";
import { copyItem, revealItem } from "./common";
import { sep, type MenuEntry } from "../menu";
import { tr } from "../i18n/runtime";
import type { ProjectSummary } from "../types";

type Notify = (message: string) => void;

export interface ProjectRowActions {
  select: () => void;
  initScope: () => void;
  env: MenuEnv;
  notify: Notify;
}

export function projectRowMenu(project: ProjectSummary, actions: ProjectRowActions): MenuEntry[] {
  const { env } = actions;
  return [
    { id: "project:select", label: tr("menu.project.open"), icon: Eye, run: actions.select },
    {
      id: "project:terminal",
      label: tr("menu.project.terminal"),
      icon: SquareTerminal,
      run: () => env.openTerminal(project.root, { title: project.name }),
    },
    sep(),
    revealItem(project.root, actions.notify, "project:reveal"),
    copyItem(tr("menu.project.copyRoot"), project.root, actions.notify, {
      id: "project:copy",
      icon: Copy,
    }),
    sep(),
    {
      id: "project:init",
      label: tr("menu.project.init"),
      icon: Layers,
      hint: project.has_state ? tr("menu.project.initialized") : undefined,
      disabled: project.has_state,
      reason: project.has_state ? tr("menu.project.hasState") : undefined,
      run: actions.initScope,
    },
    {
      id: "project:info",
      label: tr("menu.project.counts", {
        skills: project.skill_count,
        mcp: project.mcp_count,
      }),
      icon: FolderGit2,
      disabled: true,
      reason: project.branch
        ? tr("menu.project.branch", { branch: project.branch })
        : tr("menu.project.notGit"),
    },
  ];
}

export interface ProjectPageActions {
  count: number;
}

export function projectPageMenu(actions: ProjectPageActions): MenuEntry[] {
  return [
    {
      id: "project:list",
      label: tr("menu.project.registered", { count: actions.count }),
      icon: FolderGit2,
      disabled: true,
      reason: tr("menu.project.fromHistory"),
    },
  ];
}
