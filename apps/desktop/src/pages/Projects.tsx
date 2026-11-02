import { useEffect, useState } from "react";
import { clsx } from "clsx";
import { FolderGit2, GitBranch, Terminal } from "lucide-react";
import { useHub } from "../lib/useHub";
import { useContextMenu, useMenuEnv } from "../lib/context-menu";
import { composeMenu } from "../lib/menu-builders/common";
import { projectPageMenu, projectRowMenu } from "../lib/menu-builders/projects";
import { useT } from "../lib/i18n";
import { Badge, Button, Empty, Panel } from "../components/ui";

export function Projects({
  onOpenTerminal,
  focusRoot,
}: {
  /** 从项目详情直接开一个工作目录在本项目根的终端 */
  onOpenTerminal?: (root: string, name?: string) => void;
  /** 从别处（终端里的项目链接、命令面板）跳过来时要聚焦的仓库 */
  focusRoot?: string | null;
}) {
  const { projects, overview, notify } = useHub();
  const t = useT();
  const menuEnv = useMenuEnv();
  const contextMenu = useContextMenu();
  const [activeRoot, setActiveRoot] = useState<string | null>(
    focusRoot ?? projects[0]?.root ?? null
  );
  const current = projects.find((p) => p.root === activeRoot) ?? projects[0] ?? null;

  useEffect(() => {
    if (focusRoot) setActiveRoot(focusRoot);
  }, [focusRoot]);
  const skills = overview?.report.records.filter((r) => r.source === "store") ?? [];

  const openRowMenu = (event: React.MouseEvent, project: (typeof projects)[number]) => {
    contextMenu.open(
      event,
      composeMenu(
        projectRowMenu(project, {
          select: () => setActiveRoot(project.root),
          initScope: () => notify(t("projects.initCmd", { root: project.root })),
          env: menuEnv,
          notify,
        }),
        menuEnv,
        "projects"
      )
    );
  };

  const openPageMenu = (event: React.MouseEvent) => {
    contextMenu.open(
      event,
      composeMenu(projectPageMenu({ count: projects.length }), menuEnv, "projects")
    );
  };

  if (projects.length === 0) {
    return (
      <Panel>
        <Empty title={t("projects.emptyTitle")} hint={t("projects.emptyHint")} />
      </Panel>
    );
  }

  return (
    <div
      className="grid grid-cols-[300px_1fr] gap-3.5 animate-fade-up"
      onContextMenu={openPageMenu}
    >
      <Panel
        title={t("projects.panelTitle")}
        subtitle={t("projects.repoCount", { count: projects.length })}
        bodyClassName="p-2"
      >
        <ul className="space-y-1">
          {projects.map((project) => (
            <li key={project.root}>
              <button
                type="button"
                data-ctx="project-item"
                onClick={() => setActiveRoot(project.root)}
                onContextMenu={(event) => openRowMenu(event, project)}
                className={clsx(
                  "flex w-full cursor-pointer items-start gap-2 rounded-lg px-2 py-2 text-left transition-colors",
                  current?.root === project.root
                    ? "bg-surface-hover"
                    : "hover:bg-surface-2"
                )}
              >
                <FolderGit2 className="mt-[2px] h-3.5 w-3.5 shrink-0 text-text-dim" />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-1.5">
                    <span className="truncate text-[12px] text-text-strong">{project.name}</span>
                    {project.has_state ? (
                      <Badge tone="brand">{t("projects.enabled")}</Badge>
                    ) : (
                      <Badge tone="dim">{t("projects.notInit")}</Badge>
                    )}
                  </div>
                  <p className="mono truncate text-[11.5px] text-text-dim">{project.root}</p>
                  <p className="mt-0.5 flex items-center gap-1 text-[11.5px] text-text-dim">
                    <GitBranch className="h-2.5 w-2.5" />
                    {project.branch ?? t("projects.notGit")}
                    <span className="ml-1">
                      {t("projects.skillsCount", { count: project.skill_count })}
                    </span>
                  </p>
                </div>
              </button>
            </li>
          ))}
        </ul>
      </Panel>

      {current && (
        <div className="space-y-3">
          <Panel
            title={current.name}
            subtitle={current.root}
            actions={
              <>
                <Badge tone={current.is_git ? "ok" : "dim"}>
                  {current.is_git
                    ? t("projects.gitBranch", { branch: current.branch ?? "-" })
                    : t("projects.plainDir")}
                </Badge>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => onOpenTerminal?.(current.root, current.name)}
                >
                  <Terminal className="h-3 w-3" />
                  {t("projects.openTerminal")}
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => notify(t("projects.initCmd", { root: current.root }))}
                >
                  {t("projects.initScope")}
                </Button>
              </>
            }
          >
            <div className="grid grid-cols-3 gap-2.5">
              <div className="panel-soft px-3 py-2.5">
                <p className="text-[11px] text-text-dim">{t("projects.projectSkills")}</p>
                <p className="mono mt-1 text-[18px] text-text-strong">{current.skill_count}</p>
                <p className="mt-0.5 text-[11.5px] text-text-dim">
                  {t("projects.projectSkillsHint")}
                </p>
              </div>
              <div className="panel-soft px-3 py-2.5">
                <p className="text-[11px] text-text-dim">{t("projects.projectMcp")}</p>
                <p className="mono mt-1 text-[18px] text-text-strong">{current.mcp_count}</p>
                <p className="mt-0.5 text-[11.5px] text-text-dim">
                  {t("projects.projectMcpHint")}
                </p>
              </div>
              <div className="panel-soft px-3 py-2.5">
                <p className="text-[11px] text-text-dim">{t("projects.stateFile")}</p>
                <p className="mono mt-1 truncate text-[12px] text-text-muted">.agenthub/project.json</p>
                <p className="mt-0.5 text-[11.5px] text-text-dim">
                  {t("projects.stateFileHint")}
                </p>
              </div>
            </div>

            <div className="mt-3">
              <p className="mb-1.5 text-[11px] text-text-dim">{t("projects.agentDirs")}</p>
              {current.agent_dirs.length === 0 ? (
                <p className="rounded-lg border border-dashed border-line px-2.5 py-2 text-[11px] text-text-dim">
                  {t("projects.noAgentDirs")}
                </p>
              ) : (
                <ul className="space-y-1">
                  {current.agent_dirs.map((dir) => (
                    <li
                      key={dir}
                      className="mono flex items-center gap-2 rounded border border-line bg-surface-2 px-2 py-1 text-[11px] text-text-muted"
                    >
                      <FolderGit2 className="h-3 w-3 text-text-dim" />
                      {dir}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </Panel>

          <Panel
            title={t("projects.enableTitle")}
            subtitle={t("projects.enableSubtitle")}
            actions={
              <Badge tone="dim">{t("projects.available", { count: skills.length })}</Badge>
            }
          >
            <div className="grid grid-cols-2 gap-2">
              {skills.map((skill, idx) => {
                const enabled = idx < current.skill_count;
                return (
                  <div
                    key={skill.name}
                    className="flex items-center gap-2 rounded-lg border border-line bg-surface-2 px-2.5 py-2"
                  >
                    <span className="truncate text-[12px] text-text-strong">{skill.name}</span>
                    <Badge tone={enabled ? "ok" : "dim"} className="ml-auto shrink-0">
                      {enabled ? t("projects.enabled") : t("projects.notEnabled")}
                    </Badge>
                  </div>
                );
              })}
              {skills.length === 0 && (
                <p className="text-[11.5px] text-text-dim">{t("projects.storeEmpty")}</p>
              )}
            </div>
            <div className="mt-2.5 flex items-start gap-2 rounded-lg border border-line bg-surface-2 px-2.5 py-2 text-[11.5px] leading-relaxed text-text-dim">
              <Terminal className="mt-[1px] h-3 w-3 shrink-0" />
              <span>
                {t("projects.enableCmd")}
                <span className="mono text-text-muted">
                  agenthub project enable &quot;{current.root}&quot; --skills skill-a,skill-b
                </span>
                {t("projects.enableNote")}
              </span>
            </div>
          </Panel>
        </div>
      )}
    </div>
  );
}
