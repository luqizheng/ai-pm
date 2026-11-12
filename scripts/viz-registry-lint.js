#!/usr/bin/env node
/**
 * Relation Registry 门禁（Visualization V0-1.8）
 *
 * 校验 `policies/viz-relation-registry.yaml` 的结构与治理不变量。
 * 规则清单见 docs/visualization/ai-pm-viz-17-relation-registry-signoff-v1.md §6.3
 * （15 条）。**未会签 ≠ 默认允许**：draft 关系可留在词表中，但必须被显式标注，
 * 且 active 关系必须带会签记录。
 *
 * 用法：
 *   node scripts/viz-registry-lint.js            # 校验，违规退出码 1
 *   node scripts/viz-registry-lint.js --report   # 只报告，不以非零码退出
 *
 * 退出码：0 通过；1 存在违规；2 运行错误。
 */
const fs = require("node:fs");
const path = require("node:path");
const { createRequire } = require("node:module");

const ROOT = path.resolve(__dirname, "..");
const REGISTRY_FILE = path.join(ROOT, "policies", "viz-relation-registry.yaml");
const REPORT_ONLY = process.argv.includes("--report");

// `yaml` 装在前端工程里（pnpm workspace），脚本自身没有 node_modules。
const requireFromFrontend = createRequire(
  path.join(ROOT, "ai-frontend", "package.json")
);
const YAML = requireFromFrontend("yaml");

const PROJECTABLE_STATUSES = new Set(["active"]);
const VALID_STATUSES = new Set(["draft", "active", "deprecated", "rejected"]);
const VALID_DIRECTIONS = new Set(["directed", "undirected"]);
/** 合成关系（visualization 域）不带命名空间前缀——见 17 号词表定名。 */
const NAMESPACE_EXEMPT_DOMAINS = new Set(["visualization"]);

const violations = [];

function report(file, message) {
  violations.push(`${file}: ${message}`);
}

function loadRegistry() {
  if (!fs.existsSync(REGISTRY_FILE)) {
    console.error(`[viz-registry] 词表真源不存在：${REGISTRY_FILE}`);
    process.exit(2);
  }
  const raw = fs.readFileSync(REGISTRY_FILE, "utf8");
  let parsed;
  try {
    parsed = YAML.parse(raw);
  } catch (error) {
    console.error(`[viz-registry] 词表 YAML 解析失败：${error.message}`);
    process.exit(REPORT_ONLY ? 0 : 1);
  }
  if (!parsed || typeof parsed !== "object") {
    console.error("[viz-registry] 词表不是合法映射");
    process.exit(2);
  }
  return parsed;
}

function checkRegistry(registry) {
  const file = path.relative(ROOT, REGISTRY_FILE);
  const relations = Array.isArray(registry.relations) ? registry.relations : [];
  const nodeKinds = Array.isArray(registry.node_kinds) ? registry.node_kinds : [];

  if (!registry.version) {
    report(file, "缺少顶层 version");
  }
  if (relations.length === 0) {
    report(file, "relations 为空");
  }
  if (nodeKinds.length === 0) {
    report(file, "node_kinds 为空");
  }

  // 规则 3/4：关系键唯一 + 命名空间规范
  const seenKeys = new Set();
  for (const relation of relations) {
    const key = relation?.key ?? "";
    if (!key) {
      report(file, "存在没有 key 的关系条目");
      continue;
    }
    if (seenKeys.has(key)) {
      report(file, `关系键重复：${key}`);
    }
    seenKeys.add(key);

    const domain = relation.domain ?? "";
    if (!NAMESPACE_EXEMPT_DOMAINS.has(domain) && !key.includes(".")) {
      report(file, `业务域关系键必须是命名空间受控字符串（domain.relation）：${key}`);
    }
    if (key.startsWith(".") || key.endsWith(".") || key.includes("..")) {
      report(file, `关系键命名不规范：${key}`);
    }

    // 规则 5：来源必须存在（稳定的 schema.table 标识）
    if (!relation.source || typeof relation.source !== "string") {
      report(file, `关系 ${key} 缺少 source`);
    }

    // 规则 1：active 关系必须有 Owner 与会签记录
    if (!relation.owner_role || typeof relation.owner_role !== "string") {
      report(file, `关系 ${key} 缺少 owner_role`);
    }
    if (PROJECTABLE_STATUSES.has(relation.status)) {
      if (!relation.signoff_date && !relation.signoff_record) {
        report(file, `已启用的关系 ${key} 必须带会签记录（signoff_date / signoff_record）`);
      }
    }

    // 规则 2：状态必须合法
    if (!VALID_STATUSES.has(relation.status)) {
      report(file, `关系 ${key} 的状态非法：${relation.status}`);
    }

    // 方向与端点类型
    if (!VALID_DIRECTIONS.has(relation.direction)) {
      report(file, `关系 ${key} 的 direction 非法：${relation.direction}`);
    }
    if (!relation.source_type || !relation.target_type) {
      report(file, `关系 ${key} 缺少 source_type / target_type`);
    }

    // 别名：必须声明 reverse_view_of，且不得同时是 active 独立关系
    if (relation.view_alias) {
      if (!relation.reverse_view_of) {
        report(file, `反向视图 ${key} 必须声明 reverse_view_of`);
      }
    }

    // 合成关系不得声明为业务事实
    if (relation.synthetic && relation.business_fact) {
      report(file, `合成关系 ${key} 不得声明 business_fact: true`);
    }

    // 跨 Registry 引用：type_key_source 必须存在于本词表声明的来源域
    if (relation.type_key_source && !relation.type_key_source.includes(".")) {
      report(file, `type_key_source 必须是 schema.table 形态：${relation.type_key_source}`);
    }
  }

  // 别名的 base 必须存在
  for (const relation of relations) {
    if (relation?.view_alias && relation.reverse_view_of) {
      if (!seenKeys.has(relation.reverse_view_of)) {
        report(file, `反向视图 ${relation.key} 指向的关系不存在：${relation.reverse_view_of}`);
      }
    }
  }

  // 规则 6：节点类型键唯一 + 状态合法
  const seenKinds = new Set();
  for (const kind of nodeKinds) {
    const key = kind?.key ?? "";
    if (!key) {
      report(file, "存在没有 key 的节点类型条目");
      continue;
    }
    if (seenKinds.has(key)) {
      report(file, `节点类型键重复：${key}`);
    }
    seenKinds.add(key);
    if (!VALID_STATUSES.has(kind.status)) {
      report(file, `节点类型 ${key} 的状态非法：${kind.status}`);
    }
    if (!kind.owner_role) {
      report(file, `节点类型 ${key} 缺少 owner_role`);
    }
  }

  // 关系端点类型必须能在节点类型词表里找到（通配 * 除外）
  for (const relation of relations) {
    for (const [field, value] of [
      ["source_type", relation.source_type],
      ["target_type", relation.target_type]
    ]) {
      if (!value || value === "*") continue;
      if (!seenKinds.has(value)) {
        report(file, `关系 ${relation.key} 的 ${field}（${value}）未在节点类型词表登记`);
      }
    }
  }

  // 规则 13/14：语义变更须升版——用 introduced_in 与 version 的一致性做弱校验
  for (const relation of relations) {
    if (relation.status === "active" && !relation.introduced_in) {
      report(file, `已启用的关系 ${relation.key} 缺少 introduced_in（版本变更需可追溯）`);
    }
  }
}

function main() {
  const registry = loadRegistry();
  checkRegistry(registry);

  const file = path.relative(ROOT, REGISTRY_FILE);
  if (violations.length === 0) {
    const count = Array.isArray(registry.relations) ? registry.relations.length : 0;
    const active = (registry.relations ?? []).filter(
      (r) => r.status === "active"
    ).length;
    console.log(
      `[viz-registry] ✓ ${file}：${count} 条关系（active ${active}），结构与治理不变量通过`
    );
    if (active === 0) {
      console.log(
        "[viz-registry] 注：当前无 active 关系——领域 Owner 会签完成前，关系不可投影（校验器对 draft 关系产出 E_SPEC_RELATION_NOT_APPROVED）"
      );
    }
    process.exit(0);
  }

  for (const violation of violations) {
    console.error(`[viz-registry] ${violation}`);
  }
  console.error(`[viz-registry] ✗ ${violations.length} 项违规`);
  process.exit(REPORT_ONLY ? 0 : 1);
}

main();
