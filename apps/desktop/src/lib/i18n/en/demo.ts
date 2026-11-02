import type { demo as HansDemo } from "../zh-Hans/demo";

export const demo = {} satisfies Record<keyof typeof HansDemo, string>;
