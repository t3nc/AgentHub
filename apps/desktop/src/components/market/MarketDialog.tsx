import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { clsx } from "clsx";
import {
  ChevronDown,
  Download,
  Loader2,
  PackageOpen,
  RefreshCw,
  ShieldAlert,
  ShieldCheck,
  X,
} from "lucide-react";
import { AgentIcon } from "../agents/AgentIcon";
import { Badge, Button, Empty, SearchInput, Toggle } from "../ui";
import { useHub } from "../../lib/useHub";
import { agentInfo, bytes, timeAgo } from "../../lib/format";
import { useT } from "../../lib/i18n";
import type { MessageKey } from "../../lib/i18n/messages";
import type { MarketAgent, MarketSkill, MarketSkillDetail, MarketSort } from "../../lib/types";

const SORTS: { key: MarketSort; label: MessageKey }[] = [
  { key: "recommended", label: "market.sort.recommended" },
  { key: "downloads", label: "market.sort.downloads" },
  { key: "updated", label: "market.sort.updated" },
];

/** 首屏 / 每次「加载更多」的批量；单次上限 100 由后端兜住 */
const FIRST_PAGE = 50;
const PAGE_SIZE = 50;
/** 浏览榜单累计上限：到顶就别再翻了，改用搜索 */
const BROWSE_CAP = 200;

function compact(value: number): string {
  if (value >= 10_000) return `${(value / 1000).toFixed(1)}k`;
  if (value >= 1000) return `${(value / 1000).toFixed(1)}k`;
  return String(value);
}

/** 技能市场浮层：搜索 → 看详情与安全结论 → 选 agent → 生成安装计划。 */
export function MarketDialog({ onClose }: { onClose: () => void }) {
  const { settings, marketSearch, marketDetail, marketAgents, planMarketInstall, notify } =
    useHub();
  const t = useT();

  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<MarketSort>("recommended");
  const [safeOnly, setSafeOnly] = useState(false);
  const [items, setItems] = useState<MarketSkill[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  /** 下一页游标（源自己解释，前端只原样回传）；null = 没有下一页 */
  const [cursor, setCursor] = useState<string | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);

  const [activeSlug, setActiveSlug] = useState<string | null>(null);
  const [detail, setDetail] = useState<MarketSkillDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

  const [agents, setAgents] = useState<MarketAgent[]>([]);
  const [picked, setPicked] = useState<string[]>([]);
  const [installing, setInstalling] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);

  /** 请求序号：搜索框打字很快，只认最后一次结果，避免旧响应覆盖新结果 */
  const seq = useRef(0);

  useEffect(() => {
    marketAgents()
      .then((next) => {
        setAgents(next);
        setPicked(next.filter((agent) => agent.available).map((agent) => agent.id));
      })
      .catch((err: unknown) => console.warn("读取可安装 agent 失败", err));
  }, [marketAgents]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      const id = ++seq.current;
      setLoading(true);
      setLoadingMore(false);
      setError(null);
      // 换关键词 / 切排序 = 回到第一页，旧游标必须立刻作废
      setCursor(null);
      setHasMore(false);
      marketSearch(query.trim(), sort, null, FIRST_PAGE)
        .then((page) => {
          if (id !== seq.current) return;
          setItems(page.items);
          setCursor(page.cursor);
          setHasMore(page.has_more);
          setActiveSlug((prev) =>
            prev && page.items.some((item) => item.slug === prev)
              ? prev
              : page.items[0]?.slug ?? null
          );
        })
        .catch((err: unknown) => {
          if (id !== seq.current) return;
          setItems([]);
          setCursor(null);
          setHasMore(false);
          setError(err instanceof Error ? err.message : String(err));
        })
        .finally(() => {
          if (id === seq.current) setLoading(false);
        });
    }, 360);
    return () => window.clearTimeout(timer);
  }, [query, sort, marketSearch, reloadKey]);

  useEffect(() => {
    if (!activeSlug) {
      setDetail(null);
      return;
    }
    let disposed = false;
    setDetailLoading(true);
    marketDetail(activeSlug)
      .then((next) => {
        if (!disposed) setDetail(next);
      })
      .catch((err: unknown) => {
        if (!disposed) setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!disposed) setDetailLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [activeSlug, marketDetail]);

  const visible = useMemo(
    () => (safeOnly ? items.filter((item) => !item.suspicious && !item.malware_blocked) : items),
    [items, safeOnly]
  );

  /** 有关键词 = 搜索结果，可以一直翻；浏览榜单到 200 条就收手，引导改搜索 */
  const browsing = !query.trim();
  const capped = browsing && items.length >= BROWSE_CAP;
  const canLoadMore = hasMore && Boolean(cursor) && !capped;

  /** 取下一批并追加：按 slug 去重，保持服务端顺序 */
  const loadMore = () => {
    if (loading || loadingMore || !canLoadMore) return;
    const id = seq.current;
    setLoadingMore(true);
    marketSearch(query.trim(), sort, cursor, PAGE_SIZE)
      .then((page) => {
        if (id !== seq.current) return;
        const seen = new Set(items.map((item) => item.slug));
        const merged = [...items];
        for (const item of page.items) {
          if (seen.has(item.slug)) continue;
          seen.add(item.slug);
          merged.push(item);
        }
        setItems(merged);
        setCursor(page.cursor);
        // 到顶后即便源说还有，也不再给按钮：继续翻只会白跑请求
        setHasMore(page.has_more && !(browsing && merged.length >= BROWSE_CAP));
        setError(null);
      })
      .catch((err: unknown) => {
        if (id !== seq.current) return;
        // 保留已加载的列表，只把错误摊在底部，用户可以直接重试
        setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (id === seq.current) setLoadingMore(false);
      });
  };

  const availableAgents = agents.filter((agent) => agent.available);
  const toggleAgent = useCallback((id: string) => {
    setPicked((prev) => (prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]));
  }, []);

  const install = () => {
    if (!detail || picked.length === 0) return;
    setInstalling(true);
    void planMarketInstall({
      slug: detail.skill.slug,
      version: detail.skill.version,
      agents: picked,
    }).then((ok) => {
      setInstalling(false);
      if (ok) {
        notify(t("market.planCreated"));
        onClose();
      }
    });
  };

  return (
    <AnimatePresence>
      <motion.div
        key="market"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.12 }}
        className="fixed inset-0 z-[65] flex items-start justify-center bg-[rgb(var(--scrim)/0.32)] px-6 pt-[4vh] dark:bg-[rgb(var(--scrim)/0.62)]"
        onMouseDown={(event) => {
          if (event.target === event.currentTarget) onClose();
        }}
      >
        <motion.div
          role="dialog"
          aria-modal="true"
          aria-label={t("market.title")}
          data-market-dialog
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -6 }}
          transition={{ duration: 0.18, ease: "easeOut" }}
          className="flex max-h-[80vh] w-full flex-col overflow-hidden rounded-lg border border-line bg-ink-900 shadow-[var(--shadow-drawer)] md:w-[960px]"
        >
          <header className="flex flex-wrap items-center gap-2 border-b border-line px-3.5 py-2.5">
            <div className="flex items-center gap-2">
              <PackageOpen className="h-4 w-4 text-brand-300" />
              <h2 className="text-[13.5px] font-semibold text-text-strong">
                {t("market.title")}
              </h2>
              <Badge tone="dim">{settings?.market_source_label ?? t("market.title")}</Badge>
            </div>
            <div className="ml-1 w-[300px]">
              <SearchInput
                value={query}
                onChange={setQuery}
                placeholder={t("market.searchPlaceholder")}
              />
            </div>
            <div className="flex items-center gap-1 rounded-md border border-line p-0.5">
              {SORTS.map((item) => (
                <button
                  key={item.key}
                  type="button"
                  onClick={() => setSort(item.key)}
                  aria-pressed={sort === item.key}
                  className={clsx(
                    "cursor-pointer rounded px-1.5 py-[3px] text-[11.5px] transition-colors",
                    sort === item.key
                      ? "bg-surface-active text-text-strong"
                      : "text-text-dim hover:text-text-strong"
                  )}
                >
                  {t(item.label)}
                </button>
              ))}
            </div>
            <label className="flex cursor-pointer items-center gap-1.5 text-[11px] text-text-muted">
              <Toggle checked={safeOnly} onChange={setSafeOnly} />
              {t("market.safeOnly")}
            </label>
            <button
              type="button"
              onClick={onClose}
              aria-label={t("market.close")}
              className="ml-auto cursor-pointer rounded p-1 text-text-dim transition-colors hover:bg-surface-hover hover:text-text-strong"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </header>

          <div className="flex min-h-0 flex-1">
            <div className="flex w-[330px] shrink-0 flex-col border-r border-line">
              <div className="flex items-center justify-between px-3 py-1.5 text-[11.5px] text-text-dim">
                <span>
                  {loading
                    ? t("market.loading")
                    : t("market.count", { count: visible.length })}
                </span>
                {(loading || loadingMore) && <Loader2 className="h-3 w-3 animate-spin" />}
              </div>
              <div className="min-h-0 flex-1 overflow-y-auto" data-market-list>
                {error && items.length === 0 ? (
                  <div className="mx-3 my-2 rounded-md border border-state-danger/30 bg-state-danger/10 px-2.5 py-2 text-[11px] leading-relaxed text-state-danger">
                    {error}
                    <button
                      type="button"
                      onClick={() => setReloadKey((v) => v + 1)}
                      className="mt-1.5 flex cursor-pointer items-center gap-1 text-[11.5px] text-text-strong transition-colors hover:text-brand-300"
                    >
                      <RefreshCw className="h-3 w-3" />
                      {t("market.retry")}
                    </button>
                  </div>
                ) : visible.length === 0 && !loading ? (
                  <div className="px-3 py-6">
                    <Empty title={t("market.emptyTitle")} hint={t("market.emptyHint")} />
                  </div>
                ) : (
                  <>
                    {error && (
                      <div className="mx-3 my-2 rounded-md border border-state-danger/30 bg-state-danger/10 px-2.5 py-2 text-[11px] leading-relaxed text-state-danger">
                        {t("market.loadMoreFailed", { error })}
                      </div>
                    )}
                    <ul>
                      {visible.map((item) => {
                        const active = item.slug === activeSlug;
                        return (
                          <li key={item.slug}>
                            <button
                              type="button"
                              data-market-item={item.slug}
                              onClick={() => setActiveSlug(item.slug)}
                              className={clsx(
                                "relative block w-full cursor-pointer border-l-2 px-3 py-2 text-left transition-colors",
                                active
                                  ? "border-brand-500 bg-surface-hover"
                                  : "border-transparent hover:bg-surface-2"
                              )}
                            >
                              <div className="flex items-center gap-1.5">
                                <span className="min-w-0 flex-1 truncate text-[12px] text-text-strong">
                                  {item.display_name}
                                </span>
                                {item.malware_blocked ? (
                                  <Badge tone="danger">{t("market.badge.malicious")}</Badge>
                                ) : item.suspicious ? (
                                  <Badge tone="warn">{t("market.badge.suspicious")}</Badge>
                                ) : item.verdict === "clean" ? (
                                  <Badge tone="ok">clean</Badge>
                                ) : null}
                              </div>
                              <p className="mt-0.5 line-clamp-2 text-[11.5px] leading-relaxed text-text-dim">
                                {item.summary || "—"}
                              </p>
                              <div className="mt-1 flex items-center gap-2 text-[11px] text-text-dim">
                                <span className="mono">@{item.owner}</span>
                                <span className="flex items-center gap-1">
                                  <Download className="h-2.5 w-2.5" />
                                  {compact(item.downloads)}
                                </span>
                                {item.updated_at && <span>{timeAgo(item.updated_at)}</span>}
                              </div>
                            </button>
                          </li>
                        );
                      })}
                    </ul>
                    <div className="px-3 pb-3 pt-1">
                      {canLoadMore ? (
                        <button
                          type="button"
                          data-market-more
                          disabled={loadingMore}
                          onClick={loadMore}
                          className="flex w-full cursor-pointer items-center justify-center gap-1.5 rounded-md border border-line py-1.5 text-[11px] text-text-muted transition-colors hover:bg-surface-2 hover:text-text-strong disabled:cursor-progress disabled:text-text-dim"
                        >
                          {loadingMore ? (
                            <Loader2 className="h-3 w-3 animate-spin" />
                          ) : (
                            <ChevronDown className="h-3 w-3" />
                          )}
                          {loadingMore
                            ? t("market.loading")
                            : t("market.loadMore", { count: items.length })}
                        </button>
                      ) : capped ? (
                        <p className="text-center text-[11px] leading-relaxed text-text-dim">
                          {t("market.capReached", { cap: BROWSE_CAP })}
                        </p>
                      ) : visible.length > 0 ? (
                        <p className="text-center text-[11px] text-text-dim">
                          {t("market.noMore")}
                        </p>
                      ) : null}
                    </div>
                  </>
                )}
              </div>
            </div>

            <div className="min-w-0 flex-1 overflow-y-auto px-4 py-3">
              {detailLoading && !detail ? (
                <p className="text-[11.5px] text-text-dim">{t("market.loadingDetail")}</p>
              ) : detail ? (
                <MarketDetailView detail={detail} />
              ) : (
                <p className="text-[11.5px] text-text-dim">{t("market.pickOne")}</p>
              )}
            </div>
          </div>

          <footer className="flex flex-wrap items-center gap-2 border-t border-line px-3.5 py-2.5">
            <span className="text-[11px] text-text-dim">{t("market.installTo")}</span>
            <div className="flex flex-wrap items-center gap-1.5">
              {agents.map((agent) => {
                const info = agentInfo(agent.id);
                const active = picked.includes(agent.id);
                return (
                  <button
                    key={agent.id}
                    type="button"
                    data-market-agent={agent.id}
                    aria-pressed={active}
                    disabled={!agent.available}
                    onClick={() => toggleAgent(agent.id)}
                    title={
                      agent.available
                        ? `${info.name} · ${agent.skills_dir ?? ""}`
                        : (agent.reason ?? t("market.agentUnavailable"))
                    }
                    className={clsx(
                      "flex items-center gap-1.5 rounded-md border px-2 py-[3px] text-[11px] transition-colors",
                      !agent.available
                        ? "cursor-not-allowed border-line text-text-dim opacity-45"
                        : active
                          ? "cursor-pointer border-brand-500/40 bg-brand-500/15 text-brand-300"
                          : "cursor-pointer border-line text-text-muted hover:bg-surface-2 hover:text-text-strong"
                    )}
                  >
                    <span className={clsx("flex items-center rounded border px-1", info.accent)}>
                      <AgentIcon id={agent.id} className="h-3.5 w-3.5" />
                    </span>
                    {info.name}
                  </button>
                );
              })}
              {agents.length > 0 && availableAgents.length === 0 && (
                <span className="text-[11.5px] text-state-warn">{t("market.noAgent")}</span>
              )}
            </div>
            <div className="ml-auto flex items-center gap-2">
              <span className="text-[11.5px] text-text-dim">
                {picked.length > 0
                  ? t("market.linkTo", { count: picked.length })
                  : t("market.pickFirst")}
              </span>
              <Button
                variant="primary"
                size="sm"
                disabled={!detail || picked.length === 0 || installing}
                onClick={install}
              >
                {installing ? (
                  <>
                    <Loader2 className="h-3 w-3 animate-spin" />
                    {t("market.preparing")}
                  </>
                ) : (
                  t("market.installToCount", { count: picked.length })
                )}
              </Button>
            </div>
          </footer>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}

/** 详情：元信息 + 安全结论 + 正文（纯文本，不解析 HTML）+ 文件清单。 */
function MarketDetailView({ detail }: { detail: MarketSkillDetail }) {
  const t = useT();
  const { skill } = detail;
  const blocked = skill.malware_blocked;
  const tone = blocked ? "danger" : skill.suspicious ? "warn" : "ok";

  return (
    <div className="space-y-2.5">
      <div>
        <div className="flex flex-wrap items-center gap-2">
          <h3 className="text-[15px] font-medium text-text-strong">{skill.display_name}</h3>
          {skill.official && (
            <Badge tone="brand">{t("market.badge.official")}</Badge>
          )}
          <Badge tone={tone}>
            {blocked
              ? t("market.badge.blocked")
              : skill.suspicious
                ? t("market.badge.suspicious")
                : t("market.badge.passed")}
          </Badge>
        </div>
        <p className="mt-1 text-[11px] text-text-dim">
          <span className="mono">@{skill.owner}</span>
          {skill.version && <span className="mono"> · v{skill.version}</span>}
          {detail.license && <span> · {detail.license}</span>}
          {skill.updated_at && (
            <span>{t("market.updatedAt", { time: timeAgo(skill.updated_at) })}</span>
          )}
          {skill.installs > 0 && (
            <span>{t("market.installs", { count: skill.installs })}</span>
          )}
        </p>
      </div>

      <div
        className={clsx(
          "flex items-start gap-1.5 rounded-md border px-2.5 py-1.5 text-[11.5px] leading-relaxed",
          blocked
            ? "border-state-danger/30 bg-state-danger/10 text-state-danger"
            : skill.suspicious
              ? "border-state-warn/30 bg-state-warn/10 text-state-warn"
              : "border-state-ok/30 bg-state-ok/10 text-state-ok"
        )}
      >
        {blocked || skill.suspicious ? (
          <ShieldAlert className="mt-[1px] h-3 w-3 shrink-0" />
        ) : (
          <ShieldCheck className="mt-[1px] h-3 w-3 shrink-0" />
        )}
        <span>
          {detail.moderation_summary ??
            (blocked ? t("market.blockedNote") : t("market.noSuspicious"))}
          {detail.reason_codes.length > 0 && `（${detail.reason_codes.join("、")}）`}
        </span>
      </div>

      {skill.topics.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {skill.topics.map((topic) => (
            <Badge key={topic} tone="dim">
              {topic}
            </Badge>
          ))}
        </div>
      )}

      <div>
        <p className="mb-1 text-[11px] text-text-dim">SKILL.md</p>
        <pre className="max-h-[38vh] overflow-y-auto whitespace-pre-wrap break-words rounded-md border border-line bg-ink-950/60 px-2.5 py-2 font-mono text-[11.5px] leading-relaxed text-text-muted">
          {detail.description || skill.summary || t("market.noBody")}
        </pre>
      </div>

      {detail.files.length > 0 && (
        <div>
          <p className="mb-1 text-[11px] text-text-dim">
            {t("market.filesIncluded", { count: detail.files.length })}
            {bytes(detail.files.reduce((sum, file) => sum + file.size, 0))}
          </p>
          <ul className="flex flex-wrap gap-1">
            {detail.files.slice(0, 12).map((file) => (
              <li
                key={file.path}
                className="mono rounded border border-line bg-surface-2 px-1.5 py-[2px] text-[11px] text-text-dim"
                title={`${file.content_type ?? "unknown"} · ${bytes(file.size)}`}
              >
                {file.path}
              </li>
            ))}
            {detail.files.length > 12 && (
              <li className="text-[11px] text-text-dim">
                {t("market.moreFiles", { count: detail.files.length - 12 })}
              </li>
            )}
          </ul>
        </div>
      )}
    </div>
  );
}
