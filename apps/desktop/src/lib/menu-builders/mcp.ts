import { Copy, Eye, Plug, Upload } from "lucide-react";
import type { MenuEnv } from "../context-menu";
import { copyItem } from "./common";
import { sep, type MenuEntry } from "../menu";
import { tr } from "../i18n/runtime";
import type { McpServerView } from "../types";

type Notify = (message: string) => void;

export interface McpRowActions {
  open: () => void;
  planWrite: () => void;
  /** 有可写目标的 agent 数，为 0 时「生成写入计划」置灰 */
  writableAgents: number;
  notify: Notify;
}

export function mcpRowMenu(server: McpServerView, actions: McpRowActions): MenuEntry[] {
  return [
    {
      id: "mcp:open",
      label: tr("menu.mcp.open"),
      icon: Eye,
      run: actions.open,
    },
    {
      id: "mcp:plan",
      label: tr("mcp.planWrite"),
      icon: Upload,
      pending: true,
      disabled: actions.writableAgents === 0,
      reason: actions.writableAgents === 0 ? tr("menu.mcp.noTarget") : undefined,
      run: actions.planWrite,
    },
    sep(),
    {
      id: "mcp:transport",
      // `transport` 是对象，不能直接插值；带上类型更好读（stdio / http / sse）
      label: tr("menu.mcp.transport", {
        transport:
          server.transport.type === "stdio" ? tr("mcp.noteLocal") : tr("mcp.noteRemote"),
        state: server.enabled ? tr("mcp.enabled") : tr("mcp.disabled"),
      }),
      icon: Plug,
      disabled: true,
      reason: server.note ?? tr("menu.mcp.editInPanel"),
    },
    copyItem(tr("menu.mcp.copyName"), server.name, actions.notify, {
      id: "mcp:copy-name",
      icon: Copy,
    }),
    copyItem(
      tr("menu.mcp.copyTargets"),
      server.targets.map((target) => target.file).join("\n"),
      actions.notify,
      { id: "mcp:copy-targets" }
    ),
  ];
}

export interface McpPageActions {
  scope: "user" | "project";
  setScope: (scope: "user" | "project") => void;
  projectMissing: boolean;
}

export function mcpPageMenu(actions: McpPageActions, _env: MenuEnv): MenuEntry[] {
  return [
    {
      id: "mcp:scope-user",
      label: tr("menu.mcp.scopeUser"),
      disabled: actions.scope === "user",
      reason: actions.scope === "user" ? tr("menu.mcp.alreadyUser") : undefined,
      run: () => actions.setScope("user"),
    },
    {
      id: "mcp:scope-project",
      label: tr("menu.mcp.scopeProject"),
      disabled: actions.scope === "project" || actions.projectMissing,
      reason: actions.projectMissing
        ? tr("menu.mcp.noProject")
        : actions.scope === "project"
          ? tr("menu.mcp.alreadyProject")
          : undefined,
      run: () => actions.setScope("project"),
    },
    sep(),
  ];
}
