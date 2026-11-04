#!/usr/bin/env node
/**
 * i18n 命名空间门禁
 *
 * 文案按命名空间分文件：`ai-frontend/locales/<locale>/<namespace>.yaml`。
 * 本脚本按 `src/lib/i18n.ts` 相同的展开规则（单键同名解包 + 命名空间点号前缀）
 * 计算每种语言的完整 key 集合，并要求：
 *   1. 除基准语言外，每种语言都不得缺少任何 key；
 *   2. 除基准语言外，每种语言也不得存在基准语言没有的死 key。
 *
 * 用法：
 *   node scripts/check-i18n-namespace.js            # 校验，违规退出码 1
 *   node scripts/check-i18n-namespace.js --report   # 只报告，不以非零码退出
 *
 * 退出码：0 通过；1 存在违规；2 运行错误。
 */
const fs = require("node:fs");
const path = require("node:path");
const { createRequire } = require("node:module");

const ROOT = path.resolve(__dirname, "..");
/** 活跃前端工程 = ai-frontend（2026-11-04 重组后唯一工程；旧 frontend/ 冻结只读） */
const LOCALES_DIR = path.join(ROOT, "ai-frontend", "locales");
const BASE_LOCALE = "zh-CN";
const REPORT_ONLY = process.argv.includes("--report");

// `yaml` 装在前端工程里（pnpm workspace），脚本自身没有 node_modules。
const requireFromFrontend = createRequire(
  path.join(ROOT, "ai-frontend", "package.json")
);
const YAML = requireFromFrontend("yaml");

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (/\.ya?ml$/.test(entry.name)) out.push(full);
  }
  return out;
}

function flatten(value, prefix, out) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    out.set(prefix, value);
    return;
  }
  for (const [key, child] of Object.entries(value)) {
    flatten(child, prefix ? `${prefix}.${key}` : key, out);
  }
}

function collect(locale) {
  const root = path.join(LOCALES_DIR, locale);
  const keys = new Map();
  const namespaces = new Set();
  if (!fs.existsSync(root)) return { keys, namespaces };

  for (const file of walk(root)) {
    const relative = path.relative(root, file).split(path.sep).join("/");
    const namespace = relative.replace(/\.ya?ml$/, "").split("/").join(".");
    const segments = namespace.split(".");
    const lastSegment = segments[segments.length - 1];

    const raw = fs.readFileSync(file, "utf8").replace(/^\uFEFF/, "");
    const content = YAML.parse(raw) ?? {};
    const keysAtTop = Object.keys(content);
    // 与 i18n.ts 一致：文件只有唯一顶层键且等于命名空间末段时解包一层。
    const unwrapped =
      keysAtTop.length === 1 && keysAtTop[0] === lastSegment
        ? content[lastSegment]
        : content;

    namespaces.add(namespace);
    flatten(unwrapped, namespace, keys);
  }
  return { keys, namespaces };
}

function main() {
  if (!fs.existsSync(LOCALES_DIR)) {
    console.error(`[i18n] 找不到 locales 目录：${LOCALES_DIR}`);
    process.exit(2);
  }

  const locales = fs
    .readdirSync(LOCALES_DIR, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort();

  if (!locales.includes(BASE_LOCALE)) {
    console.error(`[i18n] 缺少基准语言 ${BASE_LOCALE}`);
    process.exit(2);
  }

  const base = collect(BASE_LOCALE);
  const failures = [];
  const lines = [`[i18n] 基准 ${BASE_LOCALE}: ${base.namespaces.size} 个命名空间 / ${base.keys.size} 个 key`];

  for (const locale of locales) {
    if (locale === BASE_LOCALE) continue;
    const target = collect(locale);
    const missing = [...base.keys.keys()].filter((key) => !target.keys.has(key)).sort();
    const extra = [...target.keys.keys()].filter((key) => !base.keys.has(key)).sort();

    lines.push(
      `[i18n] ${locale}: ${target.namespaces.size} 个命名空间 / ${target.keys.size} 个 key`
    );
    if (missing.length) {
      failures.push(`${locale} 缺少 ${missing.length} 个 key`);
      for (const key of missing) lines.push(`  ✗ 缺少 ${key}`);
    }
    if (extra.length) {
      failures.push(`${locale} 多出 ${extra.length} 个死 key`);
      for (const key of extra) lines.push(`  ✗ 多出 ${key}（${BASE_LOCALE} 中不存在）`);
    }
  }

  // 同一命名空间的文件集两语言必须一致（防止整份文件漏翻）。
  for (const locale of locales) {
    if (locale === BASE_LOCALE) continue;
    const target = collect(locale);
    for (const namespace of base.namespaces) {
      if (!target.namespaces.has(namespace)) {
        failures.push(`${locale} 缺少命名空间文件 ${namespace}`);
      }
    }
    for (const namespace of target.namespaces) {
      if (!base.namespaces.has(namespace)) {
        failures.push(`${locale} 多出命名空间文件 ${namespace}`);
      }
    }
  }

  console.log(lines.join("\n"));
  if (!failures.length) {
    console.log("[i18n] ✓ 各语言 key 与命名空间完全对齐");
    return 0;
  }
  console.error(`[i18n] ✗ ${failures.join("；")}`);
  return REPORT_ONLY ? 0 : 1;
}

process.exit(main());