import type { zhHans } from "../zh-Hans";
import { common } from "./common";
import { demo } from "./demo";
import { nav } from "./nav";
import { pages } from "./pages";
import { toast } from "./toast";

/** 繁体走台湾正体用词，不是简体的字形直转。 */
export const zhHant: Record<keyof typeof zhHans, string> = {
  ...common,
  ...nav,
  ...pages,
  ...toast,
  ...demo,
};
