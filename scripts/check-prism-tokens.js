#!/usr/bin/env node
/**
 * PRISM 设计约束门禁
 *
 * 规则 1：组件源码禁止裸色值 —— 颜色只能来自 tokens.css。
 *         覆盖 hex / rgb() / rgba() / hsl() / hwb() / lab() / lch() / oklab() / oklch() / color()
 *         以及 CSS 具名色（white / red / …）。
 * 规则 2：源码引用的 CSS 变量必须在 tokens.css 中已定义。
 * 规则 3：布局/排版属性禁止裸 px（>2px 即违规；≤2px 为 hairline/微调级豁免）。
 * 规则 4：动效属性禁止裸时长（ms/s），必须引用 --duration-* token。
 * 规则 5：@media 断点必须 ∈ 白名单（1440 超宽 / 1100 页面收窄 / 880 紧凑 / 640 浮层窄屏）；
 *         views/design/ 展示页自带画框断点，豁免。
 *
 * 用法：
 *   node scripts/check-prism-tokens.js              # 扫描默认范围
 *   node scripts/check-prism-tokens.js --report     # 只报告，不以非零码退出
 *   node scripts/check-prism-tokens.js --changed-from <ref> # 只扫描该 ref 之后变更的源码
 *
 * 退出码：0 通过；1 存在违规。
 */
const { readFileSync, readdirSync, statSync } = require("node:fs");
const { join, relative, resolve, sep } = require("node:path");
const { execFileSync } = require("node:child_process");

const ROOT = join(__dirname, "..");
/** 活跃前端工程 = ai-frontend（2026-11-04 重组后唯一工程；旧 frontend/ 冻结只读） */
const FRONTEND_SRC = join(ROOT, "ai-frontend", "src");
/** token 真源（--ws-* + 组件消费的别名/原语，单文件） */
const TOKENS_FILE = join(FRONTEND_SRC, "styles", "tokens.css");

/**
 * 扫描范围：ai-frontend 的 src 全量（walk 跳过 *.test.ts）。
 * 旧 frontend/ 工程已冻结，不再纳入。
 *
 * `--scan-root <dir>` 可覆盖扫描根，仅供本脚本的自测使用。
 */
const DEFAULT_SCAN_ROOTS = [FRONTEND_SRC];

const scanRootIndex = process.argv.indexOf("--scan-root");
const SCAN_ROOTS =
  scanRootIndex >= 0 && process.argv[scanRootIndex + 1]
    ? [resolve(process.argv[scanRootIndex + 1])]
    : DEFAULT_SCAN_ROOTS;

/**
 * CI 增量门禁：只检查本次 PR/push 变更的活跃源码文件。
 * 不把历史遗留违规归咎于无关 PR；不带参数时仍可全量扫描以治理技术债。
 */
const changedFromIndex = process.argv.indexOf("--changed-from");
let SCAN_FILES = SCAN_ROOTS.flatMap((dir) => walk(dir));
if (changedFromIndex >= 0) {
  const changedFrom = process.argv[changedFromIndex + 1];
  if (!changedFrom) {
    console.error("[check-prism-tokens] --changed-from 必须提供 Git ref");
    process.exit(2);
  }
  const changedPaths = execFileSync(
    "git",
    ["diff", "--name-only", "--diff-filter=ACMR", changedFrom + "...HEAD"],
    { cwd: ROOT, encoding: "utf8" }
  )
    .split(/\r?\n/)
    .filter(
      (file) =>
        file.startsWith("ai-frontend/src/") &&
        /\.(vue|css|ts|tsx)$/.test(file) &&
        !/\.test\.ts$/.test(file)
    );
  SCAN_FILES = changedPaths.map((file) => resolve(ROOT, file));
}

/** 允许出现裸色值/裸尺寸的文件（token 定义处本身） */
const ALLOW_RAW_COLOR_FILES = new Set([TOKENS_FILE]);

/** 允许引用未在 tokens.css 中定义的变量前缀（如第三方库变量；ws- 为工作区 token 层） */
const ALLOWED_VAR_PREFIXES = ["el-", "reka-", "vi-", "ws-"];

/** CSS 具名色 —— 出现在颜色类属性上或在值中单独出现时即为裸色值 */
const NAMED_COLORS = [
  "aqua", "aquamarine", "beige", "black", "blue", "brown", "chartreuse", "chocolate",
  "coral", "crimson", "cyan", "darkblue", "darkgreen", "darkred", "fuchsia", "gold",
  "gray", "green", "grey", "indigo", "ivory", "khaki", "lavender", "lime", "linen",
  "magenta", "maroon", "navy", "olive", "orange", "orchid", "pink", "plum", "purple",
  "red", "salmon", "silver", "skyblue", "tan", "teal", "tomato", "turquoise", "violet",
  "wheat", "white", "yellow"
];

/** 允许的 CSS 关键字（非颜色） */
const ALLOWED_KEYWORDS = new Set([
  "transparent",
  "currentcolor",
  "currentColor",
  "inherit",
  "initial",
  "unset",
  "none",
  "auto"
]);

/** 只在这些属性上检查具名色，避免 font-family 等误报 */
const COLOR_PROPS = [
  "color",
  "background",
  "background-color",
  "border",
  "border-color",
  "border-top",
  "border-right",
  "border-bottom",
  "border-left",
  "border-top-color",
  "border-right-color",
  "border-bottom-color",
  "border-left-color",
  "outline",
  "outline-color",
  "fill",
  "stroke",
  "box-shadow",
  "text-shadow",
  "text-decoration-color",
  "caret-color",
  "accent-color",
  "column-rule-color"
];

/** 规则 3：禁止裸 px 的布局/排版属性（prop 已小写；驼峰名为 ts 对象字面量防御项） */
const SIZE_PROPS = new Set([
  "gap", "row-gap", "column-gap",
  "padding", "padding-top", "padding-right", "padding-bottom", "padding-left",
  "padding-inline", "padding-block", "padding-inline-start", "padding-inline-end",
  "margin", "margin-top", "margin-right", "margin-bottom", "margin-left",
  "margin-inline", "margin-block", "margin-inline-start", "margin-inline-end",
  "top", "right", "bottom", "left",
  "inset", "inset-inline", "inset-block",
  "width", "height",
  "border-width", "border-top-width", "border-right-width",
  "border-bottom-width", "border-left-width",
  "font-size", "fontsize",
  "line-height", "lineheight",
  "letter-spacing", "letterspacing"
]);

/** 规则 3：裸 px 阈值 —— ≤2px 视为 hairline/微调级（1px 描边、2px 缝隙），豁免 */
const RAW_PX_MAX = 2;

/** 规则 4：禁止裸时长的动效属性 */
const DURATION_PROPS = new Set([
  "transition", "transition-duration", "transition-delay",
  "animation", "animation-duration", "animation-delay",
  "transitionduration", "transitiondelay", "animationduration", "animationdelay"
]);

/** 规则 5：@media 断点白名单
 *  —— 页面布局 {1440 超宽, 1100 页面收窄, 880 紧凑} + 浮层/组件窄屏 {640}
 *  + 1240（壳三栏页专属：侧栏+上下文栏+页内双栏并存，需更早收窄） */
const BREAKPOINT_WHITELIST = new Set([1440, 1240, 1100, 880, 640]);

/** 规则 5：断点白名单豁免路径（设计系统展示页自带画框/样板断点） */
const BREAKPOINT_EXEMPT = "/views/design/";

const tokens = readFileSync(TOKENS_FILE, "utf8");
const definedTokens = new Set(
  [...tokens.matchAll(/--([a-z0-9-]+)\s*:/g)].map((m) => m[1])
);

const HEX_RE = /(?<![a-zA-Z0-9])#([0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})(?![a-zA-Z0-9])/g;
const COLOR_FN_RE = /\b(rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\s*\(/g;
const VAR_RE = /var\(\s*--([a-z0-9-]+)/g;
/** 裸 px（数字字面量 + px 单位；不匹配 var()/token 名中的数字） */
const RAW_PX_RE = /(?<![\w.])(\d+(?:\.\d+)?)px\b/g;
/** 裸时长（数字字面量 + ms/s 单位） */
const RAW_DURATION_RE = /(?<![\w.])(\d+(?:\.\d+)?)(ms|s)\b/g;
/** @media 断点 */
const MEDIA_WIDTH_RE = /@media[^{]*\((?:min|max)-width\s*:\s*(\d+(?:\.\d+)?)px/g;

const reportOnly = process.argv.includes("--report");

function walk(dir, out = []) {
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return out;
  }
  for (const entry of entries) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      walk(full, out);
    } else if (/\.(vue|css|ts|tsx)$/.test(entry) && !/\.test\.ts$/.test(entry)) {
      out.push(full);
    }
  }
  return out;
}

/**
 * 逐字符剥离注释，保留换行以维持行号对应。
 * 块注释跨行时，其中的内容整段被忽略（不会被误判为裸色值）。
 */
function stripCommentsKeepingLines(source) {
  let out = "";
  let i = 0;
  const n = source.length;
  while (i < n) {
    const two = source.slice(i, i + 2);
    if (two === "/*") {
      let j = i + 2;
      while (j < n && source.slice(j, j + 2) !== "*/") j++;
      const chunk = source.slice(i, Math.min(j + 2, n));
      out += chunk.replace(/[^\n]/g, " ");
      i = j + 2;
    } else if (two === "//") {
      let j = i;
      while (j < n && source[j] !== "\n") j++;
      out += source.slice(i, j).replace(/[^\n]/g, " ");
      i = j;
    } else {
      out += source[i];
      i++;
    }
  }
  return out;
}

/** 从一行里取出属性名 → 属性值 */
function declarations(line) {
  const pairs = [];
  const re = /([a-zA-Z-]+)\s*:\s*([^;{}]+)/g;
  let m;
  while ((m = re.exec(line)) !== null) {
    pairs.push({ prop: m[1].toLowerCase(), value: m[2].trim() });
  }
  return pairs;
}

/** 剥离 var(...) 引用后检查裸 px（>RAW_PX_MAX 即违规） */
function rawPxValues(value) {
  const bare = value.replace(/var\([^)]*\)/g, "var()");
  const out = [];
  for (const m of bare.matchAll(RAW_PX_RE)) {
    if (parseFloat(m[1]) > RAW_PX_MAX) out.push(m[0]);
  }
  return out;
}

/** 剥离 var(...) 引用后检查裸时长（ms/s） */
function rawDurationValues(value) {
  const bare = value.replace(/var\([^)]*\)/g, "var()");
  return [...bare.matchAll(RAW_DURATION_RE)].map((m) => m[0]);
}

const violations = [];

for (const file of SCAN_FILES) {
  if (ALLOW_RAW_COLOR_FILES.has(file)) continue;
  const rel = relative(ROOT, file).split(sep).join("/");
  const source = readFileSync(file, "utf8");
  const cleaned = stripCommentsKeepingLines(source);
  const designExempt = rel.includes(BREAKPOINT_EXEMPT);

  cleaned.split("\n").forEach((line, index) => {
    if (!line.trim()) return;
    const lineNo = index + 1;

    // 规则 5：@media 断点白名单
    for (const m of line.matchAll(MEDIA_WIDTH_RE)) {
      if (designExempt) continue;
      const bp = parseFloat(m[1]);
      if (!BREAKPOINT_WHITELIST.has(bp)) {
        violations.push(
          `${rel}:${lineNo} 断点 ${bp}px 不在白名单 {1440, 1240, 1100, 880, 640} —— 请统一页面断点（design 展示页豁免）`
        );
      }
    }

    // 规则 2：变量引用完整性
    for (const m of line.matchAll(VAR_RE)) {
      const name = m[1];
      if (definedTokens.has(name)) continue;
      if (ALLOWED_VAR_PREFIXES.some((p) => name.startsWith(p))) continue;
      violations.push(`${rel}:${lineNo} 引用了未定义变量 --${name}`);
    }

    // 规则 1a：整行裸值（模板字符串、类名之外的自由文本）
    const decls = declarations(line);
    const isDeclaration = decls.length > 0 && decls[0].prop !== undefined;

    if (!isDeclaration) {
      const hex = line.match(HEX_RE);
      if (hex) {
        violations.push(
          `${rel}:${lineNo} 裸色值 ${hex.join(", ")} —— 请改用 tokens.css 变量`
        );
      }
      const fn = line.match(COLOR_FN_RE);
      if (fn) {
        violations.push(
          `${rel}:${lineNo} 裸色值 ${fn.join(", ")} —— 请改用 tokens.css 变量`
        );
      }
      return;
    }

    // 规则 1b：声明值里的裸色值
    for (const { prop, value } of decls) {
      const hex = value.match(HEX_RE);
      if (hex) {
        violations.push(
          `${rel}:${lineNo} 裸色值 ${hex.join(", ")} —— 请改用 tokens.css 变量`
        );
      }
      const fn = value.match(COLOR_FN_RE);
      if (fn) {
        violations.push(
          `${rel}:${lineNo} 裸色值 ${fn.join(", ")} —— 请改用 tokens.css 变量`
        );
      }
      // 具名色：值整体是颜色名，或颜色类属性里出现颜色名
      const lowered = value.toLowerCase();
      const wholeValueIsColor =
        NAMED_COLORS.includes(lowered) && !ALLOWED_KEYWORDS.has(lowered);
      const colorProp = COLOR_PROPS.includes(prop);
      const namedInValue = NAMED_COLORS.find(
        (c) => new RegExp(`(^|[\s,(])${c}($|[\s,;)])`).test(lowered)
      );
      if (wholeValueIsColor || (colorProp && namedInValue)) {
        violations.push(
          `${rel}:${lineNo} 裸色值 "${namedInValue ?? lowered}" —— 请改用 tokens.css 变量`
        );
      }

      // 规则 3：布局/排版属性裸 px（design 展示页自带样板尺寸，豁免）
      if (SIZE_PROPS.has(prop) && !designExempt) {
        for (const px of rawPxValues(value)) {
          violations.push(
            `${rel}:${lineNo} 裸 px "${px}"（${prop}）—— 请改用 tokens.css 尺寸 token`
          );
        }
      }

      // 规则 4：动效属性裸时长（design 展示页豁免）
      if (DURATION_PROPS.has(prop) && !designExempt) {
        const durations = rawDurationValues(value);
        if (durations.length > 0) {
          violations.push(
            `${rel}:${lineNo} 裸时长 ${durations.join(", ")}（${prop}）—— 请改用 tokens.css --duration-* token`
          );
        }
      }
    }
  });
}

if (violations.length > 0) {
  console.error(`[check-prism-tokens] 违规 ${violations.length} 条：`);
  for (const v of violations) console.error("  " + v);
  if (!reportOnly) process.exit(1);
} else {
  console.log(`[check-prism-tokens] 通过：tokens 已定义 ${definedTokens.size} 个`);
}
