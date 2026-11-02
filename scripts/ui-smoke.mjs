#!/usr/bin/env node
/**
 * 前端 UI 断言（演示模式）——把「靠人眼看」的那几条变成可复现的 PASS/FAIL。
 *
 * 覆盖：
 *   1. 链接策略「目录联接」在当前平台置灰（macOS 置灰 + 说明，Windows 可选）；
 *   2. 技能详情里的同一处下拉与说明；
 *   3. 右键菜单守卫：技能行出我们的菜单，输入框放行系统原生菜单；
 *   4. 右键菜单的「链接策略 → 目录联接」在 macOS 上置灰并给出原因。
 *
 * 依赖一个已经跑起来的演示站点与一个可连的 CDP 端点（脚本自己不开浏览器：
 * 本机 Chrome 无头起不来时用 `moli serve --layout`，它会监听 9222）。
 *
 *   npm --prefix apps/desktop run dev &
 *   moli serve --layout &
 *   node scripts/ui-smoke.mjs
 *
 *   # 换端口 / 换机器
 *   node scripts/ui-smoke.mjs --url http://localhost:5273 --cdp http://127.0.0.1:9222
 *
 * 没有全局 WebSocket（Node < 21）或连不上 CDP 时打印「跳过」并 exit 0，
 * 这样 `check-macos.sh` / `check-windows.ps1` 可以在没准备浏览器的环境里安全带上它。
 */

const args = process.argv.slice(2);
const argOf = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};
const BASE = argOf("--url", "http://localhost:5273").replace(/\/$/, "");
const CDP = argOf("--cdp", "http://127.0.0.1:9222").replace(/\/$/, "");
const TIMEOUT = Number(argOf("--timeout", "15000"));

const results = [];
function check(name, fn) {
  results.push({ name, fn });
}
function assert(cond, detail) {
  if (!cond) throw new Error(detail);
}

class Cdp {
  constructor(ws) {
    this.ws = ws;
    this.seq = 0;
    this.pending = new Map();
    ws.addEventListener("message", (event) => {
      const msg = JSON.parse(
        typeof event.data === "string" ? event.data : event.data.toString()
      );
      const entry = this.pending.get(msg.id);
      if (!entry) return;
      this.pending.delete(msg.id);
      if (msg.error) entry.reject(new Error(`${msg.error.message}`));
      else entry.resolve(msg.result);
    });
  }

  static async connect() {
    const targets = await (await fetch(`${CDP}/json/list`)).json();
    const page = targets.find((t) => t.type === "page");
    if (!page) throw new Error("CDP 上没有 page target");
    const ws = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((resolve, reject) => {
      ws.addEventListener("open", resolve, { once: true });
      ws.addEventListener("error", () => reject(new Error("CDP WebSocket 连接失败")), {
        once: true,
      });
    });
    return new Cdp(ws);
  }

  send(method, params = {}) {
    const id = ++this.seq;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }

  async eval(expression) {
    const out = await this.send("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (out.exceptionDetails) {
      throw new Error(
        out.exceptionDetails.exception?.description ??
          out.exceptionDetails.text ??
          "页面表达式抛错"
      );
    }
    return out.result?.value;
  }

  /**
   * 每次都带一个变化的查询串再导航：`#hash` 之间切换是同文档跳转，不会重建 React 状态，
   * 上一条用例残留的菜单/弹层会把下一条断言带脏。
   */
  async goto(url) {
    const [path, hash = ""] = url.split("#");
    const sep = path.includes("?") ? "&" : "?";
    const fresh = `${path}${sep}t=${Date.now()}${hash ? `#${hash}` : ""}`;
    await this.send("Page.navigate", { url: fresh });
    await this.waitFor(() => document.readyState === "complete", `页面加载超时 ${fresh}`);
  }

  /** `expr` 是返回布尔值的 JS 表达式（不是 CSS 选择器）。 */
  async waitFor(expr, hint) {
    const code = typeof expr === "function" ? `(${expr})()` : String(expr);
    const deadline = Date.now() + TIMEOUT;
    while (Date.now() < deadline) {
      if (await this.eval(code)) return;
      await new Promise((r) => setTimeout(r, 120));
    }
    throw new Error(`等待超时：${hint ?? code}`);
  }
}

/** CSS 选择器 → 返回布尔值的 JS 表达式（`waitFor` 只吃表达式）。 */
const sel = (selector) => `!!document.querySelector(${JSON.stringify(selector)})`;

const js = {
  /** 平台默认策略：`[data-platform-strategy]` 上的值 */
  platformStrategy: `document.querySelector('[data-platform-strategy]')?.getAttribute('data-platform-strategy')`,
  strategyButton: (value) =>
    `(() => {
       const el = document.querySelector('[data-default-strategy="${value}"]');
       return el ? { disabled: el.disabled, text: el.textContent.trim() } : null;
     })()`,
  /** 点开第一条技能行的详情 */
  openFirstSkillDetail: `(() => {
      const row = document.querySelector('[data-ctx="skill-row"]');
      if (!row) return false;
      row.click();
      return true;
    })()`,
  junctionOption: `(() => {
      const select = document.querySelector('[data-strategy-select]');
      if (!select) return null;
      const option = [...select.options].find((o) => o.value === 'junction');
      return option ? { disabled: option.disabled, text: option.textContent.trim() } : null;
    })()`,
  hasJunctionNote: `!!document.querySelector('[data-junction-unsupported]')`,
  dispatchContextMenu: (selector) => `(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return false;
      el.dispatchEvent(new MouseEvent('contextmenu', {
        bubbles: true, cancelable: true, clientX: 120, clientY: 120,
      }));
      return true;
    })()`,
  /** 用量趋势：x 轴刻度（含首尾标记） */
  usageTicks: `(() => {
      const axis = document.querySelector('[data-usage-axis]');
      if (!axis) return null;
      return [...axis.querySelectorAll('[data-usage-tick]')].map((el) => ({
        label: el.textContent.trim(),
        edge: el.getAttribute('data-usage-tick-edge'),
      }));
    })()`,
  usageBars: `document.querySelectorAll('[data-usage-bar]').length`,
  /** 在第 index / count 列内横向 fraction（0~1）的位置发一次 mousemove */
  moveTrend: (index, count, fraction) => `(() => {
      const el = document.querySelector('[data-usage-trend]');
      if (!el) return false;
      const rect = el.getBoundingClientRect();
      if (rect.width <= 0) return false;
      const x = rect.left + ((${index} + ${fraction}) / ${count}) * rect.width;
      const y = rect.top + rect.height / 2;
      el.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, clientX: x, clientY: y }));
      return true;
    })()`,
  tooltip: `(() => {
      const el = document.querySelector('[data-usage-tooltip]');
      if (!el) return null;
      return {
        at: el.getAttribute('data-usage-tooltip-at'),
        head: el.textContent.trim().split('\\n')[0],
        count: document.querySelectorAll('[data-usage-tooltip]').length,
      };
    })()`,
  poolBadge: (attr, id) => `(() => {
      const el = document.querySelector('[${attr}="${id}"]');
      return el ? el.textContent.trim() : null;
    })()`,
  clickAgentRail: (agent) => `(() => {
      const el = document.querySelector('[data-agent-rail="${agent}"]');
      if (!el) return false;
      el.click();
      return true;
    })()`,
  codexEnv: `(() => {
      const el = document.querySelector('[data-codex-env]');
      if (!el) return null;
      return {
        kind: el.getAttribute('data-codex-env-kind'),
        variable: el.getAttribute('data-codex-env-var'),
        text: el.textContent.trim(),
        warn: document.querySelector('[data-codex-env-warn]')?.textContent.trim() ?? null,
      };
    })()`,
  menuOpen: `!!document.querySelector('[data-context-menu]')`,
  closeMenu: `(() => {
      document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      return true;
    })()`,
  menuItem: (id) => `(() => {
      const el = document.querySelector('[data-menu-item="${id}"]');
      return el ? { disabled: el.getAttribute('aria-disabled') === 'true' || el.hasAttribute('disabled'), text: el.textContent.trim() } : null;
    })()`,
};

check("设置页：macOS 上「目录联接」置灰并说明原因", async (cdp) => {
  await cdp.goto(`${BASE}/?platform=macos#settings`);
  await cdp.waitFor(sel("[data-platform-strategy]"), "设置页默认链接策略没渲染出来");
  const platform = await cdp.eval(js.platformStrategy);
  assert(platform === "symlink", `macOS 演示的平台默认应解析为 symlink，实际 ${platform}`);
  const junction = await cdp.eval(js.strategyButton("junction"));
  assert(junction, "设置页找不到目录联接按钮");
  assert(junction.disabled, "macOS 上目录联接必须置灰");
  assert(junction.text.includes("仅 Windows"), `置灰按钮应说明原因，实际「${junction.text}」`);
  const symlink = await cdp.eval(js.strategyButton("symlink"));
  assert(symlink && !symlink.disabled, "符号链接不该被置灰");
});

check("设置页：Windows 上「目录联接」可选", async (cdp) => {
  await cdp.goto(`${BASE}/?platform=windows#settings`);
  await cdp.waitFor(sel("[data-platform-strategy]"), "设置页默认链接策略没渲染出来");
  const platform = await cdp.eval(js.platformStrategy);
  assert(platform === "junction", `Windows 演示的平台默认应是 junction，实际 ${platform}`);
  const junction = await cdp.eval(js.strategyButton("junction"));
  assert(junction && !junction.disabled, "Windows 上目录联接必须可选");
  assert(!junction.text.includes("仅 Windows"), "Windows 上不该出现「仅 Windows」后缀");
});

check("技能详情：macOS 上目录联接置灰并给出说明", async (cdp) => {
  await cdp.goto(`${BASE}/?platform=macos#skills`);
  await cdp.waitFor(sel('[data-ctx="skill-row"]'), "技能列表没渲染出来");
  assert(await cdp.eval(js.openFirstSkillDetail), "点不开第一条技能详情");
  await cdp.waitFor(sel("[data-strategy-select]"), "技能详情里的链接策略下拉没出现");
  const option = await cdp.eval(js.junctionOption);
  assert(option, "下拉里没有 junction 选项");
  assert(option.disabled, "macOS 上目录联接选项必须 disabled");
  assert(option.text.includes("仅 Windows"), `选项文案应说明原因，实际「${option.text}」`);
  assert(await cdp.eval(js.hasJunctionNote), "应显示「目录联接仅 Windows 支持」的说明");
});

check("技能详情：Windows 上目录联接可选", async (cdp) => {
  await cdp.goto(`${BASE}/?platform=windows#skills`);
  await cdp.waitFor(sel('[data-ctx="skill-row"]'), "技能列表没渲染出来");
  assert(await cdp.eval(js.openFirstSkillDetail), "点不开第一条技能详情");
  await cdp.waitFor(sel("[data-strategy-select]"), "技能详情里的链接策略下拉没出现");
  const option = await cdp.eval(js.junctionOption);
  assert(option && !option.disabled, "Windows 上目录联接选项不该置灰");
  assert(!(await cdp.eval(js.hasJunctionNote)), "Windows 上不该出现「仅 Windows」说明");
});

check("右键菜单：技能行出我们的菜单", async (cdp) => {
  await cdp.goto(`${BASE}/?platform=macos#skills`);
  await cdp.waitFor(sel('[data-ctx="skill-row"]'), "技能列表没渲染出来");
  assert(await cdp.eval(js.dispatchContextMenu('[data-ctx="skill-row"]')), "找不到技能行");
  await cdp.waitFor(js.menuOpen, "技能行的右键菜单没弹出来");
  const item = await cdp.eval(js.menuItem("skill:strategy"));
  assert(item, "菜单里没有「链接策略」子菜单入口");
  await cdp.eval(js.closeMenu);
});

check("右键菜单：输入框放行系统原生菜单", async (cdp) => {
  await cdp.goto(`${BASE}/?platform=macos#skills`);
  await cdp.waitFor(sel("input"), "页面没有可测的输入框");
  assert(await cdp.eval(js.dispatchContextMenu("input")), "找不到输入框");
  // 给 React 的 setState 一点时间；这里断言「没有弹出我们的菜单」
  await new Promise((r) => setTimeout(r, 250));
  assert(!(await cdp.eval(js.menuOpen)), "输入框上不该弹出自绘菜单（应放行系统菜单）");
});

check("右键菜单：菜单内滚动不关闭、页面滚动才关闭", async (cdp) => {
  await cdp.goto(`${BASE}/?platform=macos#skills`);
  await cdp.waitFor(sel('[data-ctx="skill-row"]'), "技能列表没渲染出来");
  assert(await cdp.eval(js.dispatchContextMenu('[data-ctx="skill-row"]')), "找不到技能行");
  await cdp.waitFor(js.menuOpen, "技能行的右键菜单没弹出来");

  // 菜单内部滚动（长菜单要能往下翻）不能关
  assert(
    await cdp.eval(`(() => {
      const el = document.querySelector('[data-context-menu]');
      if (!el) return false;
      el.dispatchEvent(new Event('scroll'));
      return true;
    })()`),
    "找不到已打开的菜单"
  );
  await new Promise((r) => setTimeout(r, 150));
  assert(await cdp.eval(js.menuOpen), "菜单内滚动把菜单关掉了（长菜单没法翻）");

  // 页面滚动必须关：菜单是 fixed 定位，不关就悬在错误位置
  await cdp.eval(`(() => {
    const el = document.scrollingElement || document.documentElement;
    el.dispatchEvent(new Event('scroll'));
    return true;
  })()`);
  await new Promise((r) => setTimeout(r, 200));
  assert(!(await cdp.eval(js.menuOpen)), "页面滚动后菜单应关闭");
});

check("右键菜单：终端正文放行系统原生菜单", async (cdp) => {
  await cdp.goto(`${BASE}/?platform=macos#terminal`);
  const hasXterm = await cdp
    .waitFor(sel(".xterm"), "终端正文（.xterm）")
    .then(() => true)
    .catch(() => false);
  if (!hasXterm) {
    console.log("        （演示模式没渲染出 .xterm，跳过终端正文这一条）");
    return;
  }
  assert(await cdp.eval(js.dispatchContextMenu(".xterm")), "找不到 .xterm");
  await new Promise((r) => setTimeout(r, 250));
  assert(!(await cdp.eval(js.menuOpen)), "终端正文上不该弹出自绘菜单（应放行系统菜单）");
});

check("用量趋势：x 轴刻度数量与首尾标记", async (cdp) => {
  await cdp.goto(`${BASE}/?platform=macos#usage`);
  await cdp.waitFor(sel("[data-usage-axis]"), "趋势图的 x 轴没渲染出来");
  const bars = await cdp.eval(js.usageBars);
  assert(bars > 0, "演示数据里没有柱子，刻度断言无从谈起");
  const ticks = await cdp.eval(js.usageTicks);
  assert(Array.isArray(ticks) && ticks.length > 0, "刻度数组为空");
  assert(ticks.length <= 6, `刻度最多 6 个，实际 ${ticks.length}`);
  assert(
    ticks[0].edge === "first" && ticks[ticks.length - 1].edge === "last",
    `首尾刻度必须有 edge 标记，实际 ${JSON.stringify(ticks)}`
  );
  const labels = ticks.map((tick) => tick.label);
  assert(
    labels.every((label) => /^\d{2}-\d{2}$|^\d{2}:00$/.test(label)),
    `刻度文案格式不对：${labels.join(" / ")}`
  );
  assert(new Set(labels).size === labels.length, `刻度重复：${labels.join(" / ")}`);
  if (bars >= 6) {
    assert(ticks.length === 6, `柱子够多时应有 6 个刻度，实际 ${ticks.length}`);
  }
});

check("用量趋势：同一根柱内移动不换列、浮层不闪", async (cdp) => {
  await cdp.goto(`${BASE}/?platform=macos#usage`);
  await cdp.waitFor(sel("[data-usage-axis]"), "趋势图的 x 轴没渲染出来");
  const bars = await cdp.eval(js.usageBars);
  assert(bars > 1, "柱子太少，测不了跨列对比");
  const target = Math.min(2, bars - 1);

  assert(await cdp.eval(js.moveTrend(target, bars, 0.3)), "找不到趋势图容器");
  await cdp.waitFor(js.tooltip, "悬停后浮层没出现");
  const first = await cdp.eval(js.tooltip);
  assert(first && first.count === 1, `浮层应只有一个，实际 ${first?.count}`);

  // 同一列里换个横向位置：命中列不变，浮层内容/位置档位也不该变
  assert(await cdp.eval(js.moveTrend(target, bars, 0.75)), "第二次移动失败");
  await new Promise((r) => setTimeout(r, 150));
  const second = await cdp.eval(js.tooltip);
  assert(second, "同列移动后浮层消失了");
  assert(
    second.head === first.head && second.at === first.at,
    `同一列内移动不该换列：${first.head}@${first.at} → ${second.head}@${second.at}`
  );
  assert(second.count === 1, "同一列内移动不该叠加多个浮层");

  // 换一列：内容必须跟着变
  assert(await cdp.eval(js.moveTrend(0, bars, 0.5)), "第三次移动失败");
  await new Promise((r) => setTimeout(r, 150));
  const third = await cdp.eval(js.tooltip);
  assert(third && third.head !== first.head, `换列后浮层应显示另一段时间：${third?.head}`);
});

check("供应商池：成员显示 ×权重 与 #优先级", async (cdp) => {
  await cdp.goto(`${BASE}/?platform=macos#provider`);
  await cdp.waitFor(sel("[data-agent-rail]"), "供应商页没渲染出来");
  await cdp.waitFor(sel('[data-pool-weight="relay"]'), "池卡片上没有权重徽标");
  const weight = await cdp.eval(js.poolBadge("data-pool-weight", "relay"));
  const priority = await cdp.eval(js.poolBadge("data-pool-priority", "relay"));
  assert(weight === "×2", `relay 的权重应是 ×2，实际 ${weight}`);
  assert(priority === "#1", `relay 的优先级应是 #1，实际 ${priority}`);
  assert(
    await cdp.eval(sel("[data-pool-legend]")),
    "「已添加」上方应有一行说明权重/优先级口径"
  );
});

check("供应商池：Codex 的 env_key 提示", async (cdp) => {
  await cdp.goto(`${BASE}/?platform=macos#provider`);
  await cdp.waitFor(sel("[data-agent-rail]"), "供应商页没渲染出来");
  assert(await cdp.eval(js.clickAgentRail("codex")), "左栏没有 Codex");
  await cdp.waitFor(sel("[data-codex-env]"), "Codex 的 env_key 提示没出现");
  const env = await cdp.eval(js.codexEnv);
  assert(env.kind === "gateway", `经网关时提示应是 gateway，实际 ${env.kind}`);
  assert(env.variable === "ANTHROPIC_API_KEY", `变量名不对：${env.variable}`);
  assert(
    env.text.includes("export ANTHROPIC_API_KEY=codex"),
    `应给出可直接复制的 export 行：${env.text}`
  );
  assert(env.warn && env.warn.includes("中转站"), `keychain 形式的供应商要给出警告：${env.warn}`);
});

async function main() {
  if (typeof WebSocket === "undefined") {
    console.log("跳过 UI 断言：当前 Node 没有全局 WebSocket（需要 Node ≥ 21）");
    return 0;
  }
  let cdp;
  try {
    cdp = await Cdp.connect();
  } catch (err) {
    console.log(`跳过 UI 断言：连不上 CDP（${CDP}）：${err.message}`);
    return 0;
  }

  let failed = 0;
  for (const item of results) {
    try {
      await item.fn(cdp);
      console.log(`  PASS  ${item.name}`);
    } catch (err) {
      failed += 1;
      console.log(`  FAIL  ${item.name}`);
      console.log(`        ${err.message}`);
    }
  }
  console.log(
    failed === 0
      ? `\nUI 断言全部通过（${results.length} 项）`
      : `\nUI 断言失败 ${failed}/${results.length} 项`
  );
  return failed === 0 ? 0 : 1;
}

main().then(
  (code) => process.exit(code),
  (err) => {
    console.error(`UI 断言异常：${err.stack ?? err}`);
    process.exit(1);
  }
);
