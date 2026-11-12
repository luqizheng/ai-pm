#!/usr/bin/env node
/**
 * VizSpec 字段纯净性门禁（Visualization V0-1.8）
 *
 * 依据 ADR-VIZ-003：**Spec 不得包含任何渲染器专属字段**——这是防第三方技术
 * 绑定的前提（业务领域与统一 Spec 层都不应感知 SeeCode / G6 的内部参数）。
 *
 * 检查两类问题：
 *   1. 渲染器保留字（row / col / skin / motion / theme / positions …）出现在 Spec 中；
 *   2. 顶层或节点/边出现**未知字段**（不在自有字段白名单内）。
 *
 * 扫描对象：仓内所有 VizSpec 形态的 JSON（黄金语料 + 约定命名的 spec 文件）。
 *
 * 用法：
 *   node scripts/viz-spec-lint.js            # 校验，违规退出码 1
 *   node scripts/viz-spec-lint.js --report   # 只报告，不以非零码退出
 *
 * 退出码：0 通过；1 存在违规；2 运行错误。
 */
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.resolve(__dirname, "..");
const REPORT_ONLY = process.argv.includes("--report");

/** 渲染器专属保留字——出现即视为 Spec 与渲染技术耦合。 */
const RENDERER_RESERVED = new Set([
  "row",
  "col",
  "column",
  "grid",
  "x",
  "y",
  "px",
  "py",
  "width",
  "height",
  "skin",
  "motion",
  "theme",
  "palette",
  "animation",
  "transition",
  "layout",
  "position",
  "positions",
  "nodes_px",
  "shape",
  "fontFamily",
  "fontSize",
  "bg",
  "background"
]);

/** VizSpec 自有字段白名单（顶层）。 */
const TOP_LEVEL_ALLOWED = new Set([
  "spec_version",
  "slot_key",
  "view",
  "nodes",
  "edges",
  "meta"
]);

const VIEW_ALLOWED = new Set([
  "kind",
  "focus",
  "lenses",
  "layout_hint"
]);

const NODE_ALLOWED = new Set([
  "id",
  "label",
  "kind",
  "status",
  "nature",
  "source_ref",
  "derived"
]);

const EDGE_ALLOWED = new Set([
  "id",
  "from",
  "to",
  "relation",
  "label",
  "nature",
  "derived"
]);

const META_ALLOWED = new Set([
  "title",
  "lang",
  "budget",
  "nature",
  "registry_version",
  "provenance"
]);

const SOURCE_REF_ALLOWED = new Set([
  "domain",
  "type",
  "id",
  "revision_id",
  "locator",
  "content_hash"
]);

const DERIVED_ALLOWED = new Set(["direct", "via", "semantic_confidence"]);

const violations = [];

function report(file, message) {
  violations.push(`${file}: ${message}`);
}

/** 递归收集所有键路径（用于渲染器保留字检查）。 */
function collectKeys(value, prefix = "", out = []) {
  if (value === null || typeof value !== "object") return out;
  if (Array.isArray(value)) {
    value.forEach((item, index) => collectKeys(item, `${prefix}[${index}]`, out));
    return out;
  }
  for (const [key, child] of Object.entries(value)) {
    const path = prefix ? `${prefix}.${key}` : key;
    out.push({ path, key });
    collectKeys(child, path, out);
  }
  return out;
}

function checkUnknown(file, obj, allowed, where) {
  if (!obj || typeof obj !== "object" || Array.isArray(obj)) return;
  for (const key of Object.keys(obj)) {
    if (!allowed.has(key)) {
      report(file, `${where} 出现未知字段：${key}`);
    }
  }
}

function checkSpec(file, spec) {
  // 规则 1：渲染器保留字
  for (const { path, key } of collectKeys(spec)) {
    if (RENDERER_RESERVED.has(key)) {
      report(file, `渲染器专属字段不得进入 Spec：${path}`);
    }
  }

  // 规则 2：未知字段（按层级白名单）
  checkUnknown(file, spec, TOP_LEVEL_ALLOWED, "顶层");
  checkUnknown(file, spec.view, VIEW_ALLOWED, "view");
  checkUnknown(file, spec.meta, META_ALLOWED, "meta");

  (spec.nodes ?? []).forEach((node, index) => {
    checkUnknown(file, node, NODE_ALLOWED, `nodes[${index}]`);
    checkUnknown(file, node?.source_ref, SOURCE_REF_ALLOWED, `nodes[${index}].source_ref`);
    checkUnknown(file, node?.derived, DERIVED_ALLOWED, `nodes[${index}].derived`);
  });

  (spec.edges ?? []).forEach((edge, index) => {
    checkUnknown(file, edge, EDGE_ALLOWED, `edges[${index}]`);
    checkUnknown(file, edge?.derived, DERIVED_ALLOWED, `edges[${index}].derived`);
  });
}

/** 收集待检查的 Spec 文件：黄金语料 + 约定命名的 spec 文件。 */
function collectSpecFiles() {
  const files = [];
  const goldenDir = path.join(
    ROOT,
    "crates",
    "pm-application",
    "tests",
    "viz_golden",
    "cases"
  );
  if (fs.existsSync(goldenDir)) {
    for (const name of fs.readdirSync(goldenDir)) {
      if (name.endsWith(".json")) files.push(path.join(goldenDir, name));
    }
  }
  for (const dir of ["specs", "docs/specs"]) {
    const full = path.join(ROOT, dir);
    if (!fs.existsSync(full)) continue;
    for (const name of fs.readdirSync(full)) {
      if (/\.spec\.json$/.test(name)) files.push(path.join(full, name));
    }
  }
  return files.sort();
}

function main() {
  const files = collectSpecFiles();
  if (files.length === 0) {
    console.log("[viz-spec] 无 VizSpec 样例可校验");
    process.exit(0);
  }

  for (const file of files) {
    const rel = path.relative(ROOT, file);
    let parsed;
    try {
      parsed = JSON.parse(fs.readFileSync(file, "utf8"));
    } catch (error) {
      report(rel, `不是合法 JSON：${error.message}`);
      continue;
    }
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      report(rel, "顶层不是对象");
      continue;
    }
    checkSpec(rel, parsed);
  }

  if (violations.length === 0) {
    console.log(`[viz-spec] ✓ ${files.length} 份 Spec：字段纯净性通过`);
    process.exit(0);
  }
  for (const violation of violations) {
    console.error(`[viz-spec] ${violation}`);
  }
  console.error(`[viz-spec] ✗ ${violations.length} 项违规`);
  process.exit(REPORT_ONLY ? 0 : 1);
}

main();
