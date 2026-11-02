#!/usr/bin/env node
/**
 * 本地化覆盖检查。
 *
 * 三件事：
 * 1. 三语词典齐平：`zh-Hans` 是 key 的事实基准，`en` / `zh-Hant` 少一条或多一条都报出来
 *    （正常情况 `tsc` 已经挡住了，这个脚本给「改词典但没跑 tsc」的场景兜底）；
 * 2. 前端残留中文：`apps/desktop/src` 里还带中文字符串字面量 / JSX 文本的文件；
 * 3. 后端残留中文：`crates/*` 里还在字符串字面量中写中文的行（注释不算，`l10n/catalog.rs`
 *    是词典本身，跳过）。
 *
 * 用法：
 *   node scripts/i18n-check.mjs            # 只报告
 *   node scripts/i18n-check.mjs --strict   # 有残留就 exit 1（CI / 收尾自检用）
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

// 必须用 fileURLToPath：URL.pathname 在 Windows 上是 `/F:/Dev/AgentHub/`，
// 与 join() 拼出来会变成 `F:\F:\Dev\AgentHub\...`（ENOENT）。
const ROOT = fileURLToPath(new URL("..", import.meta.url));
const STRICT = process.argv.includes("--strict");
const ALL = process.argv.includes("--all");

/** 允许保留中文的文件：词典本身、以及这一版还没迁完的清单（迁完就从这里删掉）。 */
const TS_ALLOWED = [
  "apps/desktop/src/lib/i18n/",
  // 命令面板的检索关键词：中英双语都要能命中，翻成单一语言反而难用
  "apps/desktop/src/lib/commands.ts",
  // 演示数据：模拟的是「用户自己的技能名 / 项目名 / 终端输出」这类内容，
  // 按不翻译清单（知识型专有名词、远端原文）处理，不进词典。
  "apps/desktop/src/lib/demo.ts",
  "apps/desktop/src/lib/terminal-demo.ts",
];
const RS_ALLOWED = [
  "crates/agenthub-core/src/l10n/catalog.rs",
  "crates/agenthub-core/src/l10n/locale.rs",
  // 供应商品牌名（智谱 GLM / 通义千问 / 腾讯云混元…）：按「产品名与厂商名不翻译」处理。
  // 这些是数据表里的 name 字段，不是界面句式。
  "crates/agenthub-core/src/model/supplier.rs",
];

const HAN = /[\u4e00-\u9fff]/;

function walk(dir, exts, out = []) {
  for (const name of readdirSync(dir)) {
    if (name === "node_modules" || name === "target" || name.startsWith(".")) continue;
    const full = join(dir, name);
    const stat = statSync(full);
    if (stat.isDirectory()) walk(full, exts, out);
    else if (exts.some((ext) => name.endsWith(ext))) out.push(full);
  }
  return out;
}

function rel(full) {
  // 一律归成 POSIX 分隔符：白名单是按正斜杠写的，Windows 上 relative() 给的是反斜杠，
  // 直接比会让白名单与「排除 tests」全部失效（会把词典自己算成残留）。
  return relative(ROOT, full).split(sep).join("/");
}

/**
 * 逐行剥掉注释：块注释（`/** … *\/`）与行尾 `//` 都算，避免把 JSDoc 里的
 * 中文解释统计成「未本地化文案」。
 *
 * 返回 `{ code, inBlock }`，`inBlock` 供下一行继续使用。
 */
function stripComments(line, inBlock) {
  let code = "";
  let block = inBlock;
  let index = 0;
  while (index < line.length) {
    if (block) {
      const end = line.indexOf("*/", index);
      if (end < 0) return { code, inBlock: true };
      block = false;
      index = end + 2;
      continue;
    }
    const start = line.indexOf("/*", index);
    const slash = line.indexOf("//", index);
    if (slash >= 0 && (start < 0 || slash < start)) {
      code += line.slice(index, slash);
      return { code, inBlock: false };
    }
    if (start < 0) {
      code += line.slice(index);
      return { code, inBlock: false };
    }
    code += line.slice(index, start);
    block = true;
    index = start + 2;
  }
  return { code, inBlock: block };
}

/** 三语词典齐平。 */
function checkDictionaries() {
  const base = join(ROOT, "apps/desktop/src/lib/i18n/zh-Hans");
  const domains = readdirSync(base).filter((name) => name.endsWith(".ts") && name !== "index.ts");
  const problems = [];
  const countKeys = (file) => {
    const text = readFileSync(file, "utf8");
    return new Set([...text.matchAll(/^\s*"([^"]+)"\s*:/gm)].map((m) => m[1]));
  };
  let total = 0;
  for (const domain of domains) {
    const hans = countKeys(join(base, domain));
    total += hans.size;
    for (const lang of ["en", "zh-Hant"]) {
      const other = countKeys(join(ROOT, `apps/desktop/src/lib/i18n/${lang}/${domain}`));
      for (const key of hans) {
        if (!other.has(key)) problems.push(`${lang}/${domain} 缺 ${key}`);
      }
      for (const key of other) {
        if (!hans.has(key)) problems.push(`${lang}/${domain} 多出 ${key}（zh-Hans 没有）`);
      }
    }
  }
  return { total, domains: domains.length, problems };
}

/**
 * 开发者可见文案：`console.*` 与 `throw new Error(...)`。
 *
 * 这些只进终端与堆栈，不进界面；保持中文反而方便维护者 grep，
 * 统计时跳过，避免把它们算成「漏翻」。
 */
function isDeveloperOnly(code) {
  return /\bconsole\.(log|warn|error|info|debug)\s*\(/.test(code) || /\bnew Error\s*\(/.test(code);
}

/** 前端残留中文（只报文件与行数，行内容留给人去看）。 */
function checkFrontend() {
  const files = walk(join(ROOT, "apps/desktop/src"), [".ts", ".tsx"]).filter(
    (file) => !TS_ALLOWED.some((prefix) => rel(file).startsWith(prefix))
  );
  const hits = [];
  for (const file of files) {
    const lines = readFileSync(file, "utf8").split("\n");
    let count = 0;
    let inBlock = false;
    for (const line of lines) {
      const stripped = stripComments(line, inBlock);
      inBlock = stripped.inBlock;
      if (HAN.test(stripped.code) && !isDeveloperOnly(stripped.code)) count += 1;
    }
    if (count > 0) hits.push({ file: rel(file), count });
  }
  hits.sort((a, b) => b.count - a.count);
  return hits;
}

/** 后端残留中文（字符串字面量里，注释与词典不算）。 */
function checkBackend() {
  const files = walk(join(ROOT, "crates"), [".rs"]).filter(
    (file) =>
      !RS_ALLOWED.some((suffix) => rel(file).endsWith(suffix)) &&
      !rel(file).includes("/tests/") &&
      // `mod tests;` 拆出去的测试模块同样是测试代码（断言里的中文不是界面文案）
      !rel(file).endsWith("/tests.rs")
  );
  const hits = [];
  for (const file of files) {
    // `#[cfg(test)]` 之后是测试模块：里面的中文是**测试数据**（供应商名「中转站」、
    // 用户自起的模型名、断言里的样例注释），不进界面，跟 `console.*` 同理不计入。
    // 仓库约定是测试模块放在文件末尾，所以直接按这个分隔符截断。
    const source = readFileSync(file, "utf8").split("#[cfg(test)]")[0];
    const lines = source.split("\n");
    let count = 0;
    let inBlock = false;
    for (const raw of lines) {
      const stripped = stripComments(raw, inBlock);
      inBlock = stripped.inBlock;
      // 只看双引号里的中文：注释与标识符里的中文与本地化无关
      for (const match of stripped.code.matchAll(/"([^"\\]|\\.)*"/g)) {
        if (HAN.test(match[0])) {
          count += 1;
          break;
        }
      }
    }
    if (count > 0) hits.push({ file: rel(file), count });
  }
  hits.sort((a, b) => b.count - a.count);
  return hits;
}

const dict = checkDictionaries();
const fe = checkFrontend();
const be = checkBackend();
const feTotal = fe.reduce((sum, item) => sum + item.count, 0);
const beTotal = be.reduce((sum, item) => sum + item.count, 0);

console.log(`词典：${dict.domains} 个域 / ${dict.total} 条 key`);
if (dict.problems.length > 0) {
  console.log(`  ✗ 三语不齐 ${dict.problems.length} 处`);
  for (const problem of dict.problems.slice(0, 20)) console.log(`    - ${problem}`);
} else {
  console.log("  ✓ 三语齐平");
}

// 默认只列前 15 个文件；加 `--all` 列全部（迁移中途按文件推进时用）
const LIMIT = ALL ? 999 : 15;

console.log(`\n前端残留中文：${fe.length} 个文件 / ${feTotal} 行`);
for (const item of fe.slice(0, LIMIT)) console.log(`  ${String(item.count).padStart(4)}  ${item.file}`);
if (fe.length > LIMIT) console.log(`  …… 其余 ${fe.length - LIMIT} 个文件`);

console.log(`\n后端残留中文：${be.length} 个文件 / ${beTotal} 行（字符串字面量内）`);
for (const item of be.slice(0, LIMIT)) console.log(`  ${String(item.count).padStart(4)}  ${item.file}`);
if (be.length > LIMIT) console.log(`  …… 其余 ${be.length - LIMIT} 个文件`);

// --strict 的判定口径：词典三语齐平 + 后端残留为 0。
// 前端那几行是**有意保留**的（示例内容 / 比对哨兵 / 结构占位），
// 只报告不判失败——否则会逼着人去改这些内容，把哨兵和示例指令一起翻掉。
const failed = STRICT && (dict.problems.length > 0 || beTotal > 0);
if (failed) {
  const reasons = [];
  if (dict.problems.length > 0) reasons.push(`词典三语不齐 ${dict.problems.length} 处`);
  if (beTotal > 0) reasons.push(`后端残留 ${beTotal} 行（分布在 ${be.length} 个文件）`);
  console.log(`\n--strict：${reasons.join("；")}`);
  process.exit(1);
}
if (STRICT) {
  console.log("\n--strict：通过（词典齐平，后端残留 0）");
  if (feTotal > 0) console.log(`  注：前端 ${feTotal} 行为有意保留（示例内容 / 比对哨兵 / 结构占位），不计入失败`);
}
