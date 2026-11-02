import { common } from "./common";
import { demo } from "./demo";
import { nav } from "./nav";
import { pages } from "./pages";
import { toast } from "./toast";

/**
 * 简体中文是文案的事实基准：`MessageKey` 由它推导，英文与繁体必须与它齐平。
 */
export const zhHans = { ...common, ...nav, ...pages, ...toast, ...demo } as const;
