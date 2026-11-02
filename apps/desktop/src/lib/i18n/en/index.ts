import type { zhHans } from "../zh-Hans";
import { common } from "./common";
import { demo } from "./demo";
import { nav } from "./nav";
import { pages } from "./pages";
import { toast } from "./toast";

/**
 * 每个子文件的 key 集合都被 `satisfies` 钉在简体基准上，
 * 所以这里只要不整体漏掉一个域，编译期就能保证英文没有缺项。
 */
export const en: Record<keyof typeof zhHans, string> = {
  ...common,
  ...nav,
  ...pages,
  ...toast,
  ...demo,
};
