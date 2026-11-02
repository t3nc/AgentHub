import { VALID_NAV, type NavKey } from "./nav";

export interface Route {
  page: NavKey;
  /** `#changes?batch=cs-xxx` 里带的批次号；其它页面恒为 null */
  batch: string | null;
  /** `#terminal?cwd=...` 里带的工作目录：让「在此项目打开终端」可分享、可刷新复现 */
  cwd: string | null;
}

export interface RouteParams {
  batch?: string | null;
  cwd?: string | null;
}

/** 解析 `#skills` / `#changes?batch=cs-1` / `#terminal?cwd=%2Ftmp` 这类 hash。非法页面回落到总览。 */
export function parseHash(raw = window.location.hash): Route {
  const body = raw.replace(/^#/, "");
  const [pagePart, queryPart] = body.split("?");
  const page = (VALID_NAV as string[]).includes(pagePart)
    ? (pagePart as NavKey)
    : "dashboard";
  const params = new URLSearchParams(queryPart ?? "");
  const batch = params.get("batch");
  const cwd = params.get("cwd");
  return {
    page,
    batch: batch && batch.length > 0 ? batch : null,
    cwd: cwd && cwd.length > 0 ? cwd : null,
  };
}

/** 生成 hash。batch 只对变更历史有意义，cwd 只对终端有意义，其余参数自然被忽略。 */
export function buildHash(page: NavKey, params: RouteParams = {}): string {
  const query = new URLSearchParams();
  if (params.batch) query.set("batch", params.batch);
  if (params.cwd) query.set("cwd", params.cwd);
  const text = query.toString();
  return text ? `#${page}?${text}` : `#${page}`;
}
