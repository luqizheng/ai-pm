#!/usr/bin/env node
/**
 * i18n 硬编码门禁
 *
 * 前端文案必须走 `locales/<locale>/*.yaml`。本脚本扫描 `frontend/src` 的
 * 源码与模板文本，发现字符串/模板文本中的中文即报违规；注释不算文案，先剥离。
 *
 * 豁免范围：
 *   - 测试文件（`*.test.ts` / `*.spec.ts` / `__tests__/`）
 *   - 演示数据与 mock（`src/api/mock/`、任意层级的 `fixtures.ts`）
 *   - 行尾带 `i18n-ignore` 注释的行（个别技术性中文，例如正则/协议常量）
 *
 * 存量基线：`scripts/i18n-hardcode-baseline.json` 记录补门禁时已存在的违规
 * （按 `file|行内容` 指纹，行号漂移不影响）。默认只拦**新增**违规；清除存量后
 * 用 `--update-baseline` 收缩基线，或 `--strict` 全量校验。
 *
 * 用法：
 *   node scripts/check-i18n-hardcode.js                     # 校验新增违规
 *   node scripts/check-i18n-hardcode.js --strict            # 连存量一起校验
 *   node scripts/check-i18n-hardcode.js --update-baseline   # 以当前状态重写基线
 *   node scripts/check-i18n-hardcode.js --report            # 只报告，不以非零码退出
 *
 * 退出码：0 通过；1 存在违规；2 运行错误。
 */
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.resolve(__dirname, "..");
const SRC = path.join(ROOT, "frontend", "src");
const REPORT_ONLY = process.argv.includes("--report");
const STRICT = process.argv.includes("--strict");
const UPDATE_BASELINE = process.argv.includes("--update-baseline");
const BASELINE_FILE = path.join(__dirname, "i18n-hardcode-baseline.json");
const CJK = /[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]/;

const SOURCE_EXT = /\.(vue|ts|tsx|js|jsx)$/;

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (SOURCE_EXT.test(entry.name)) out.push(full);
  }
  return out;
}

function isExempt(relative) {
  const normalized = relative.split(path.sep).join("/");
  if (/\.(test|spec)\.(ts|tsx|js|jsx)$/.test(normalized)) return true;
  if (normalized.includes("__tests__/")) return true;
  if (normalized.startsWith("api/mock/")) return true;
  if (/(^|\/)fixtures\.(ts|js)$/.test(normalized)) return true;
  return false;
}

/**
 * 剥离 JS/TS 注释与 Vue 模板注释，保留字符串内容与模板文本。
 * 逐字符状态机：字符串里的 `//`、URL、正则不会被当成注释。
 */
function stripComments(text) {
  let out = "";
  let i = 0;
  let state = "code";

  while (i < text.length) {
    const ch = text[i];
    const next = text[i + 1];

    if (state === "code") {
      if (ch === "/" && next === "/") {
        state = "line";
        out += "  ";
        i += 2;
        continue;
      }
      if (ch === "/" && next === "*") {
        state = "block";
        out += "  ";
        i += 2;
        continue;
      }
      if (text.startsWith("<!--", i)) {
        state = "html";
        out += "    ";
        i += 4;
        continue;
      }
      if (ch === '"' || ch === "'" || ch === "`") {
        state = ch === '"' ? "double" : ch === "'" ? "single" : "template";
      }
      out += ch;
      i += 1;
      continue;
    }

    if (state === "line") {
      if (ch === "\n") {
        state = "code";
        out += ch;
      } else {
        out += " ";
      }
      i += 1;
      continue;
    }

    if (state === "block") {
      if (ch === "*" && next === "/") {
        state = "code";
        out += "  ";
        i += 2;
      } else {
        out += ch === "\n" ? "\n" : " ";
        i += 1;
      }
      continue;
    }

    if (state === "html") {
      if (text.startsWith("-->", i)) {
        state = "code";
        out += "   ";
        i += 3;
      } else {
        out += ch === "\n" ? "\n" : " ";
        i += 1;
      }
      continue;
    }

    // 字符串内部：保留原文，处理转义与结束引号。
    if (ch === "\\") {
      out += text.slice(i, i + 2);
      i += 2;
      continue;
    }
    if (
      (state === "single" && ch === "'") ||
      (state === "double" && ch === '"') ||
      (state === "template" && ch === "`")
    ) {
      state = "code";
    }
    out += ch;
    i += 1;
  }

  return out;
}

function main() {
  if (!fs.existsSync(SRC)) {
    console.error(`[i18n] 找不到源码目录：${SRC}`);
    process.exit(2);
  }

  const all = [];
  let scanned = 0;

  for (const file of walk(SRC)) {
    const relative = path.relative(SRC, file);
    if (isExempt(relative)) continue;
    scanned += 1;

    const raw = fs.readFileSync(file, "utf8").replace(/^\uFEFF/, "");
    const stripped = stripComments(raw);
    const rawLines = raw.split(/\r?\n/);
    const lines = stripped.split(/\r?\n/);

    lines.forEach((line, index) => {
      if (!CJK.test(line)) return;
      if (rawLines[index] && rawLines[index].includes("i18n-ignore")) return;
      const text = rawLines[index].trim().slice(0, 160);
      all.push({
        file: relative.split(path.sep).join("/"),
        line: index + 1,
        text,
        fingerprint: `${relative.split(path.sep).join("/")}|${text}`
      });
    });
  }

  if (UPDATE_BASELINE) {
    const payload = {
      note: "check-i18n-hardcode.js 存量基线；新增硬编码中文必须修，勿直接重跑本参数扩容",
      fingerprints: [...new Set(all.map((v) => v.fingerprint))].sort()
    };
    fs.writeFileSync(BASELINE_FILE, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
    console.log(`[i18n] 已写入基线 ${payload.fingerprints.length} 条：${BASELINE_FILE}`);
    return 0;
  }

  const baseline =
    !STRICT && fs.existsSync(BASELINE_FILE)
      ? new Set(JSON.parse(fs.readFileSync(BASELINE_FILE, "utf8")).fingerprints ?? [])
      : new Set();
  const violations = all.filter((v) => !baseline.has(v.fingerprint));
  const suppressed = all.length - violations.length;

  if (!violations.length) {
    console.log(
      `[i18n] ✓ ${scanned} 个源文件无新增硬编码中文` +
        (suppressed ? `（基线豁免存量 ${suppressed} 处）` : "")
    );
    return 0;
  }

  for (const v of violations) {
    console.log(`  ✗ ${v.file}:${v.line}  ${v.text}`);
  }
  console.error(
    `[i18n] ✗ ${violations.length} 处硬编码中文（${scanned} 个源文件）；改为 locales/*.yaml 文案` +
      (suppressed ? `；另有 ${suppressed} 处存量基线豁免` : "")
  );
  return REPORT_ONLY ? 0 : 1;
}

process.exit(main());