# AI-PM Visualization V0-1 技术设计（VizSpec 类型 · 校验器 · Relation Registry）V1.0

> 状态：设计稿（证据齐备，可进入实现） · 日期：2026-11-12
> 上游：[ADR V1.0.1](ai-pm-viz-09-architecture-decision-record-v1.md)（ADR-VIZ-003/004/005）· [P0 验收矩阵](ai-pm-viz-10-p0-acceptance-matrix-v1.md)（AC-1 / AC-4 / AC-21）· [D3 Spec 概念设计](ai-pm-viz-03-visualization-spec-v1.md)
> 落地约定：本文所有"仓库事实"均来自 2026-11-12 对 `D:\kudi-project\templates\ai-pm` 的实证（路径:行号随附），与既有文档冲突处见 §10 更正清单。

## 1. 范围与非目标

**范围（V0-1 交付物）**

| 交付物 | 内容 |
|---|---|
| VizSpec 类型 | pm-domain 的 Rust 类型定义 + serde 形态 + 版本字段 |
| 校验器 | 纯函数、分级问题列表（Blocker/Warning/Info）、问题码表、可选修复建议 |
| Relation Registry | 受控关系词表（命名空间字符串）+ 治理字段 + lint 规则 + owner 会签记录 |
| source_hash 规范 | 指纹计算输入、规范化顺序、编码（算法与实现位置） |
| Golden 语料 | 固定 spec 样例 + 规范化结果快照（Golden Tests 基线） |

**非目标**：迁移与建表（属 V0-2，本任务零迁移）；Adapter 实现（V0-4/V0-9）；API 端点（V0-2）；前端组件（V0-8）；权限过滤实现（V0-3，V0-1 只定义校验器所需的上下文接口）。

## 2. 仓库落点

| 位置 | 内容 | 依据 |
|---|---|---|
| `crates/pm-domain/src/visualization/mod.rs` | 模块真源 doc comment + `pub mod` 列表 + 底部 `pub use` 再导出 | 参照 `crates/pm-application/src/review/mod.rs:35-44`（16 处同构子目录） |
| `crates/pm-domain/src/visualization/spec.rs` | VizSpec / VizNode / VizEdge / VizView / VizMeta 类型 | — |
| `crates/pm-domain/src/visualization/source_ref.rs` | SourceRef（与 `kb.memory_ref` 同构） | `migrations/0001_baseline.sql:2914` |
| `crates/pm-domain/src/visualization/nature.rs` | Nature 枚举（business/explanatory）+ DerivedTrace 标记 | ADR-VIZ-009 R-06 |
| `crates/pm-domain/src/visualization/validation/mod.rs` | 校验器入口、ValidationReport、Severity | 参照 `crates/pm-application/src/review/pipeline.rs:45-60`（Severity{Info,Warning,Blocker}） |
| `crates/pm-domain/src/visualization/validation/codes.rs` | 问题码常量表（E_/W_/I_ 前缀） | 仓内无 `{code,at,fix}` 先例，须新造 |
| `crates/pm-domain/src/visualization/registry.rs` | Registry 数据结构与查询（不含文件解析） | — |
| `crates/pm-domain/src/lib.rs` | 登记 `pub mod visualization;` | `crates/pm-domain/src/lib.rs:3-19` + `:21-39` 惯例 |
| `crates/pm-application/src/visualization/registry_loader.rs` | Registry 文件加载（YAML → Registry） | `serde_yaml` 仅 pm-application 有（`crates/pm-application/src/policy_engine.rs:89,145,284`） |
| `policies/viz-relation-registry.yaml` | Registry 真源文件 | 仓库级 YAML 配置先例 `policies/ai-pm-ai-lead-v2.yaml` |

**依赖变更（仅一处）**：`crates/pm-domain/Cargo.toml` 增 `sha2.workspace = true`（现依赖仅 chrono/serde/serde_json/sqlx/thiserror/uuid，`crates/pm-domain/Cargo.toml:7-13`）。按根 `Cargo.toml:33-98` 的 `[workspace.dependencies]` + `xxx.workspace = true` 惯例登记。

**不新建 crate**：pm-domain 已有子目录先例（`crates/pm-application/src/{review,pm_write,docproc_parse,discovery,delivery,kb_service}`），新 crate 会破坏依赖方向约定（pm-domain ← pm-application ← pm-infrastructure ← pm-api）。

## 3. 数据模型（Rust 类型草案）

```rust
// crates/pm-domain/src/visualization/spec.rs
use serde::{Deserialize, Serialize};

/// 图形性质：business=真实业务事实；explanatory=AI 生成的假设内容。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Nature {
    Business,
    Explanatory,
}
impl Nature {
    pub fn as_str(&self) -> &'static str { /* "business" | "explanatory" */ }
    pub fn parse(s: &str) -> Option<Self> { /* 成对提供，见 design.rs:36-56 */ }
}

pub type VizSpecVersion = String; // 形如 "1.0"，带兼容策略（§3.3）

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VizSpec {
    pub spec_version: VizSpecVersion,
    pub slot_key: String,
    pub view: VizView,
    pub nodes: Vec<VizNode>,
    pub edges: Vec<VizEdge>,
    pub meta: VizMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VizView {
    pub kind: String,          // 受控词表（§5.2）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus: Vec<String>,    // 节点 id
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lenses: Vec<VizLens>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VizNode {
    pub id: String,            // 图内唯一（§3.1）
    pub label: String,         // 纯文本（ADR-VIZ-010）
    pub kind: String,          // 受控词表
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nature: Option<Nature>,
    pub source_ref: SourceRef, // 必填（§4 E_SPEC_MISSING_SOURCE_REF）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived: Option<DerivedTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VizEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub relation: String,      // 命名空间受控字符串（§5）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nature: Option<Nature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived: Option<DerivedTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DerivedTrace {
    pub direct: bool,                       // 二跳拼接恒为 false
    pub via: Vec<String>,                   // 中间对象 id
    pub semantic_confidence: String,        // "structurally_supported"
}
```

**类型约定对齐仓库现状**：
- 枚举一律 `#[serde(rename_all="snake_case")]` + `as_str()`/`parse()` 成对（`crates/pm-domain/src/design.rs:15-19,36-56`）。
- ID 用 `pub type XId = uuid::Uuid`（`crates/pm-domain/src/ids.rs:3-12`）；**VizSpec 的 node/edge id 保持 String**，因为它跨领域引用多种业务 ID（Uuid 与业务自然键混用），但 `source_ref.id` 定义为字符串以容纳业务键（如 `R-042`）。
- `#[serde(default, skip_serializing_if = "…")]` 用于可选字段，保证序列化结果稳定（Golden 快照可比）。

### 3.1 ID 唯一性规则（Spec Contract，ADR-VIZ-003 增补项）

| 规则 | 级别 | 行为 |
|---|---|---|
| node.id 在图内唯一 | Blocker | E_SPEC_DUPLICATE_NODE_ID |
| edge.id 在图内唯一 | Blocker | E_SPEC_DUPLICATE_EDGE_ID |
| edge.from / edge.to 必须命中 node.id | Blocker | E_SPEC_EDGE_ENDPOINT_UNKNOWN |
| node.id 与 edge.id 命名空间可重叠（各自独立） | — | 允许（便于 `R-042` 既是节点也是边 id） |

### 3.2 SourceRef（与 kb.memory_ref 同构）

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceRef {
    pub domain: String,          // delivery / design / pc / ai / kb …
    pub r#type: String,          // requirement / design_requirement / memory_ref …
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}
```
字段与 `kb.memory_ref` 的 `target_domain/target_type/target_id/target_revision_id/locator`（`migrations/0001_baseline.sql:2914`）一一对应，去掉 `ref_role`（角色属业务语义，由边关系表达）。

### 3.3 版本与兼容策略

- `spec_version` 形如 `"1.0"`（major.minor）。
- **兼容规则**：读取时 major 不同 → 拒绝（E_SPEC_VERSION_UNSUPPORTED）；major 相同 minor 更高 → 拒绝（防止旧代码误读新字段，fail-closed）；minor 相同 → 直接读取。
- **历史 Artifact 读取**：Artifact 存 spec 快照原文 + `spec_version`，读取路径不得依赖前后端同步升级（旧 Artifact 用旧 minor 读，新代码必须仍能读已发布的同 major 快照）。
- 版本升级必须附带迁移说明文档段落（写入 D3 的兼容附录）。

## 4. 校验器设计

### 4.1 形态：纯函数 + 分级问题列表

参照仓内两处先例：`crates/pm-application/src/review/pipeline.rs:45-60`（Severity{Info,Warning,Blocker} + findings）与 `crates/kb-pc-contracts/src/retrieval.rs:192-246`（ValidationCheck/Report + `all_passed()` 纯函数）。

```rust
// crates/pm-domain/src/visualization/validation/mod.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity { Info, Warning, Blocker }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VizProblem {
    pub code: String,        // E_/W_/I_ 前缀（codes.rs 常量）
    pub at: String,          // 定位串，如 "nodes[3].label" / "edges[1].from"
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>, // 可选修复建议（供 AI 修补循环消费）
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationReport {
    pub problems: Vec<VizProblem>,
}
impl ValidationReport {
    pub fn has_blockers(&self) -> bool { /* 任一 Blocker */ }
    pub fn blocking(&self) -> Vec<&VizProblem> { /* … */ }
}

/// 纯函数：无 IO、无 DB、无全局状态。
pub fn validate(spec: &VizSpec, ctx: &ValidationContext) -> ValidationReport { … }
```

### 4.2 ValidationContext（依赖注入，pm-domain 不得依赖 DB/应用层）

```rust
pub struct ValidationContext<'a> {
    pub registry: &'a RelationRegistry,                 // 词表
    pub kinds: &'a NodeKindVocabulary,                  // node kind 受控词表
    pub resolver: &'a dyn ObjectResolver,               // 存在性与权限判定
    pub limits: SpecLimits,                             // §7 尺寸上限
    pub allowed_slots: &'a [String],                    // 调用者可写的槽位白名单
}

/// 由应用层实现的 trait：pm-domain 定义接口，pm-application 连 DB 实现。
pub trait ObjectResolver {
    /// 对象是否存在且调用者可见（无权一律按"不存在"返回 → 见 §4.4）。
    fn exists_and_visible(&self, domain: &str, r#type: &str, id: &str) -> bool;
    /// 关系是否存在（用于 derived_trace 的 via 校验）。
    fn relation_exists(&self, from: &str, to: &str, relation: &str) -> bool;
}
```

### 4.3 问题码表（V1.0 首批）

| 码 | 级别 | 触发 | 定位 |
|---|---|---|---|
| `E_SPEC_VERSION_UNSUPPORTED` | Blocker | major/minor 不兼容 | `spec_version` |
| `E_SPEC_DUPLICATE_NODE_ID` | Blocker | node.id 重复 | `nodes[i].id` |
| `E_SPEC_DUPLICATE_EDGE_ID` | Blocker | edge.id 重复 | `edges[i].id` |
| `E_SPEC_EDGE_ENDPOINT_UNKNOWN` | Blocker | edge 端点无对应节点 | `edges[i].from` |
| `E_SPEC_MISSING_SOURCE_REF` | Blocker | 节点缺 source_ref | `nodes[i]` |
| `E_SPEC_UNKNOWN_OBJECT_REF` | Blocker | source_ref 指向不存在或无权对象 | `nodes[i].source_ref` |
| `E_SPEC_RELATION_NOT_REGISTERED` | Blocker | relation 未在 Registry 登记 | `edges[i].relation` |
| `E_SPEC_RELATION_KIND_MISMATCH` | Blocker | relation 的 source/target 类型与两端节点 kind 不匹配 | `edges[i]` |
| `E_SPEC_BUDGET_EXCEEDED` | Blocker | 节点/边数超 budget 档位 | `meta.budget` |
| `E_SPEC_PAYLOAD_TOO_LARGE` | Blocker | 序列化后超上限 | `meta` |
| `E_SPEC_LABEL_TOO_LONG` | Blocker | label 超长（§7） | `nodes[i].label` |
| `E_SPEC_LABEL_CONTROL_CHARS` | Blocker | label 含控制字符 | `nodes[i].label` |
| `E_SPEC_SLOT_NOT_ALLOWED` | Blocker | slot_key 不在调用者白名单 | `slot_key` |
| `E_SPEC_DERIVED_TRACE_INVALID` | Blocker | derived_trace.direct=true 或 via 为空或 via 元素非节点 | `edges[i].derived` |
| `E_SPEC_NATURE_MISMATCH` | Blocker | Edge.nature=business 但端点节点为 explanatory（或 Artifact=explanatory 却声明 business 边） | `edges[i].nature` |
| `W_SPEC_NODE_WITHOUT_EDGE` | Warning | 孤立节点 | `nodes[i].id` |
| `W_SPEC_DUPLICATE_LABEL` | Warning | 同 kind 同 label 重复 | `nodes[i].label` |
| `W_SPEC_DANGLING_EDGE` | Warning | 端点存在但被权限裁剪 | `edges[i]` |
| `W_SPEC_LAYOUT_HINT_CONFLICT` | Warning | layout_hint 与 kind 不匹配 | `view.layout_hint` |
| `W_SPEC_FOCUS_UNMATCHED` | Warning | focus 指向不存在的节点 | `view.focus[i]` |
| `W_SPEC_LENS_UNMATCHED` | Warning | lens 无可匹配节点/边 | `view.lenses[i]` |
| `I_SPEC_REGISTRY_VERSION_STALE` | Info | Registry 版本低于当前发布版本 | `meta` |

### 4.4 权限与裁剪语义（与 ADR-VIZ-009 一致）

- 校验阶段即以调用者权限过滤：`resolver.exists_and_visible()` 对无权对象返回 false → 命中 `E_SPEC_UNKNOWN_OBJECT_REF`（**不用"无权"字样**，避免探测差异；与仓库 404 同形惯例一致，`apps/pm-api/src/pm_requirement_read.rs` 先例）。
- 边两端任一无权 → 边整体裁剪（不留半边）→ 命中 `W_SPEC_DANGLING_EDGE` 供 UI 显式呈现，不静默丢弃。
- 计数类信息（节点数/边数/截断数）不得随无权对象变化而变化（§6 泄露矩阵要求）。

## 5. Relation Registry

### 5.1 文件真源：`policies/viz-relation-registry.yaml`

```yaml
version: 1
relations:
  - key: task.implements_requirement
    semantics: "任务实现需求"
    source_type: task
    target_type: requirement
    direction: directed
    domain: delivery
    owner: "delivery-owner"
    status: active            # active | deprecated
    introduced_in: "1.0"
  - key: derived_trace
    semantics: "经中间对象推导的间接关系（不代表直接业务关系）"
    source_type: "*"
    target_type: "*"
    direction: directed
    domain: visualization
    owner: "architecture-owner"
    status: active
    introduced_in: "1.0"
```

### 5.2 治理规则（ADR-VIZ-004 / R-09）

| 规则 | 落地方式 |
|---|---|
| 每个 relation 必须齐备 8 字段 | Registry 加载时校验，缺字段 → 加载失败（fail-fast，参照 `crates/pm-application/src/policy_engine.rs` 的策略加载语义） |
| 语义变更 = 新版本或新 relation | lint：已登记 relation 的 `semantics` 变更 → 需 `introduced_in` 递增 |
| 废弃 relation 保留历史解释 | `status: deprecated` 保留条目，不再用于新图，但旧 Artifact 的 relation 仍可解释 |
| 未知 relation 阻断 | `E_SPEC_RELATION_NOT_REGISTERED` |
| 类型不匹配阻断 | `E_SPEC_RELATION_KIND_MISMATCH`（source/target type 与两端 node.kind 不符；`*` 通配） |
| lint 检查未登记/类型错/方向反 | `scripts/viz-registry-lint.js`（§8） |
| **状态模型（V1.1 补 `rejected`）** | `draft`（未会签，**不可投影**）/ `active` / `deprecated`（曾 active 后被替代，保留历史解释）/ `rejected`（会签否决，从未启用）——**未会签 ≠ 默认允许** |
| **反向视图（R-08 裁决）** | `view_alias: true` + `reverse_view_of: <key>`：别名不新增业务事实、不参与底层关系计数；**同一底层事实不得同时产出两条边** |
| **合成关系（R-09）** | `synthetic: true` + `source: vizspec` + `business_fact: false`；必须校验 `derived` 元数据完整 |
| **跨 Registry 校验（R-06 裁决）** | `type_key_source: pc.relationship_type`；`type_key` 必须存在于 PC 权威注册表，方向/端点类型/`governance_level`/`owner_domain` 全部沿用 PC 定义；**PC 侧停用即不得投影** |
| **来源标识（V-06）** | 用稳定 `schema.table.column` 或领域注册表标识；**迁移文件行号仅作审查依据，不得作为唯一运行时来源标识** |
| **方向分离（R-05 裁决）** | Registry 语义方向 ≠ 视觉流向；语义方向单一事实源，视觉翻转须为显式 presentation transform（详见 [17 号](ai-pm-viz-17-relation-registry-signoff-v1.md) R-05 与 [14 号](ai-pm-viz-14-v0-3-projection-consistency-design-v1.md)） |

### 5.3 owner 会签记录

Registry 文件尾部维护 `signoffs:` 段（relation key → owner / 日期 / 结论）；**未会签的 relation 在 Registry 中 `status: draft`**，加载时可见但**校验器对 draft relation 产出 Blocker**（`E_SPEC_RELATION_NOT_APPROVED`），确保"会签未完成 = 不可用"，而非"默认放行"。

### 5.4 NodeKind 词表

同文件维护 `node_kinds:`（kind → 语义/领域/owner/状态），与 relation 的 source/target type 对齐；node.kind 未登记同样 Blocker（`E_SPEC_NODE_KIND_NOT_REGISTERED`）。首批 kind：requirement / design / adr / task / task_dependency / module / service / repository / agent / event / knowledge / concept / module_relationship / flow_step。

## 6. source_hash 规范（AC-4）

**编码**：小写 hex sha256，**不加 `sha256:` 前缀**（对齐 `crates/pm-application/src/pm_write/support.rs:3-11` `hex_sha256` 与 `crates/pm-application/src/review/anchor.rs:131-139`）。

**输入**（对齐 ADR-VIZ-006 的五要素）：

| 序 | 输入 | 规范化规则 |
|---|---|---|
| 1 | 来源业务对象 | 收集所有节点 `source_ref` 的 `(domain, type, id)`，去重 |
| 2 | 来源 revision | 对应 `revision_id`（缺失记空串占位） |
| 3 | Spec 核心内容 | `view` + `nodes` + `edges` 的规范化 JSON（不含 `meta.generated_at` 等易变字段） |
| 4 | 参与推导的关系版本 | `derived_trace.via` 元素 + relation key 组成的有序集合 |
| 5 | 影响图形解释的配置版本 | Registry `version` + budget 档位 + spec_version |

**算法**（对齐 `crates/pm-application/src/review/anchor.rs:93-114` 先例：先按 id 排序再哈希）：

```
1. 上述 5 组输入分别按既定排序键排序
2. 合并为规范化 JSON（键名固定顺序，serde_json::to_vec 字节流）
3. sha256 → 小写 hex（64 字符）
```

**实现位置**：`crates/pm-application/src/visualization/source_hash.rs`（pm-application 已有 sha2；pm-domain 保持无哈希依赖，除非后续决定把指纹下沉到 pm-domain）。

**stale 判定**：读取 Artifact 时重算当前 source_hash 与存档值比对；失配 → 状态置 stale（不改发布内容，ADR-VIZ-006）。

## 7. 规模与尺寸上限（AC-11 P0 部分）

| 项 | 上限 | 超限行为 |
|---|---|---|
| 节点数 | 按 budget 档位：strict ≤30 / balanced ≤150 / faithful ≤500 | 超档 → `E_SPEC_BUDGET_EXCEEDED` |
| 边数 | 同档位同步（见下表） | 同上 |
| 单 label 长度 | 120 字符 | `E_SPEC_LABEL_TOO_LONG` |
| 单节点文本总长（label+status） | 200 字符 | `E_SPEC_LABEL_TOO_LONG` |
| Spec 序列化大小 | 256 KB | `E_SPEC_PAYLOAD_TOO_LARGE` |
| edges[i].from/to 长度 | 128 字符 | `E_SPEC_LABEL_TOO_LONG`（定位到端点字段） |

budget 档位与边数上限：strict ≤30 节点 / ≤45 边；balanced ≤150 / ≤300；faithful ≤500 / ≤1000。

## 8. 测试与门禁落地（仓内无 CI workflow 的现实约束）

**关键事实**：仓内**无** `.github/`、`.gitlab-ci.yml`、Jenkinsfile、Makefile —— 所有"CI 任务"只能落成 `scripts/` 下本地脚本，由 `scripts/cargo-test.sh` / `scripts/test.sh` 串联（`scripts/cargo-test.sh` 已含 nextest + doctest + template1 预建 + NO_PROXY loopback）。

### 8.1 Golden Tests（AC-21 P0 基础部分）

**事实**：仓内无 `insta` / `expect-test` 等快照 crate → Golden 快照**手写文件比对**。

| 项 | 方案 |
|---|---|
| 语料位置 | `crates/pm-domain/src/visualization/validation/golden/`（`cases/*.json` + `expected/*.json`） |
| 比对方式 | 纯函数单测：读 case → validate → 序列化规范化报告（问题码 + 级别 + 定位，**不含 message 文案**）→ 与 expected 逐字节比对 |
| 比对粒度 | 只比 `{code, severity, at}` 三元组序列，避免文案变动造成噪声 |
| 文案稳定性 | message 文案不进快照；文案变更由 `scripts/viz-spec-lint.js` 之外的人工评审保障 |
| 失败信息 | 打印首个差异位置与两侧内容，便于定位 |

### 8.2 Adapter Contract Tests（AC-21，V0-4 承接）

固定 spec → Adapter 输入；断言八项（合法 HTML/SVG、零意外外网请求、无脚本注入、溯源保留、节点/边身份保留、失败可处理）。**不依赖 SeeCode**（ADR-VIZ-002），本任务只交付 Contract 侧骨架与语料。

### 8.3 脚本与门禁

| 脚本 | 作用 | 接线 |
|---|---|---|
| `scripts/viz-registry-lint.js` | Registry lint：未登记 relation、字段缺失、kind 不匹配、draft 未会签、语义变更未升版 | 与 `scripts/check-prism-tokens.js` 同风格（CommonJS node），由 `scripts/test.sh` 调用 |
| `scripts/viz-spec-lint.js` | Spec 字段纯净性：禁止渲染器保留字（`row/col/skin/motion/budget` 之外的自有字段白名单）、禁止未知顶层字段 | 同上 |
| `scripts/viz-no-write-path.sh` | 架构 lint：viz 模块不得 import 领域写 service（扫描 `crates/*/src/visualization/**` 的 use 语句） | 同上（AC-16 T-32） |

**注意**：仓内 `check-i18n-hardcode.js` 有基线文件惯例（`scripts/i18n-hardcode-baseline.json`）→ 若新增前端文案需同步维护基线（V0-8 阶段）。

### 8.4 测试运行约定

- **纯函数用例不需要 DB**：`crates/pm-domain` 无 `tests/` 目录 → 单测内联 `#[cfg(test)] mod tests` 放源文件末尾（先例 `crates/pm-domain/src/design.rs:178+`）。
- 需要 DB 的用例用 `#[sqlx::test(migrations = "../../migrations")]`（`apps/pm-api/tests/pm_lists.rs:53,77,128`），`DATABASE_URL` **必须 `127.0.0.1`**（AGENTS.md 规则 3）。
- **禁 `query!` 宏**：全仓零宏零 `.sqlx/`，一律运行时 `sqlx::query*` + `.bind()`（根 `Cargo.toml:47-54` 注释明文）。
- 迁移后必须 `cargo build --workspace --tests`（`docs/parallel-development-discipline.md` §1.3）——**V0-1 无迁移，但新 crate 模块仍需该步**。

## 9. API 错误映射（与仓库约定对齐）

**事实**：`ApiError` 在 `apps/pm-api/src/support.rs:14-27`，错误体仅 `{"message": …}`（模块 doc `:1-6` 明示不带 code）；`IntoResponse` 映射 400/409/404/403/501/500。

因此校验器的 `{code, at, fix}` 三元组**不直接出现在 HTTP 错误体**：

| 场景 | HTTP | 响应体 |
|---|---|---|
| 提交非法 Spec（用户/接口调用方） | 400 | `{"message": "spec 校验未通过：E_SPEC_RELATION_NOT_REGISTERED @ edges[2].relation；…"}`（message 内含前 N 条 code 与定位，不含结构化字段） |
| AI 生成链路的校验失败 | 不走 HTTP 错误 | 校验报告写入 draft Artifact / 渲染 job 结果，AI 修补循环消费结构化问题列表（内部通道，非 API 语义） |
| 结构化问题列表对外暴露 | — | 若未来需要 API 暴露分级报告，作为**正常 200 响应体**（`{report: {...}}`），不用错误通道承载 |

新错误类型若需引入，只能加在 `crates/pm-domain/src/error.rs`（仓内唯一错误文件，`CommandError`），并在 `apps/pm-api/src/support.rs` 补 `From` 映射。

## 10. 与既有文档的偏差更正（本次实证得出）

| # | 既有文档写法 | 实证结论 | 更正 |
|---|---|---|---|
| 1 | D3 §3 / D1 用 `"sha256:…"` | 仓内哈希为**裸小写 hex**，无前缀（`pm_write/support.rs:3-11`） | 统一改为 64 位小写 hex；本设计 §6 为准 |
| 2 | 10 号矩阵多处写"CI job（viz-golden / viz-detach / viz-no-write-path）" | 仓内**无 CI workflow**，无法新增 job | 改为**本地脚本 + `scripts/test.sh` 串联**；"CI"语义 = 本地门禁脚本 |
| 3 | 10 号矩阵 AC-2 写"≥2 渲染目标" | P0 不含 G6（ADR-VIZ-002/011） | P0 = 每 kind ≥1 主呈现 + ≥1 Fallback；G6 双目标完整语义在 P1（矩阵 §2 已分阶段，需同步本文 §10 引用） |
| 4 | 10 号矩阵提"JSON Schema 校验" | workspace 无 jsonschema/valico（仅 pm-mcp 有 schemars） | 校验以**手写 Rust 校验器**为准；如需对外发布 JSON Schema 作为文档资产，另行登记 |
| 5 | D3 提"Golden 快照测试" | 无 insta/expect-test | Golden 改为**手写文件比对**（本文 §8.1） |
| 6 | 10 号矩阵 §6 暂定取号 0055 | 0055 已被 `0055_device_name_active_unique.sql` 占用；跨分支最大 0064 | 编号不阻断 V0-1；收敛到 V0-2 建表前由 Migration Owner 在真实 master 双源核对（矩阵 §6.1 已记录） |

## 11. 实施任务清单（V0-1 拆分，可直接排期）

| # | 任务 | 产出 | 完成定义 |
|---|---|---|---|
| V0-1.1 | pm-domain 子目录骨架 | `crates/pm-domain/src/visualization/{mod.rs,spec.rs,source_ref.rs,nature.rs}` + `lib.rs` 登记 + Cargo 依赖 | `cargo build -p pm-domain` 通过；无未用导入（`scripts/clippy.sh` -D warnings） |
| V0-1.2 | 类型与 serde 形态 | §3 类型落地 + 序列化往返测试 | 内联单测：往返一致；枚举 as_str/parse 成对；optional 字段不产生 null |
| V0-1.3 | 问题码表与校验器骨架 | `validation/{mod.rs,codes.rs}` + `ValidationContext` + `ObjectResolver` trait | 骨架编译；`ValidationReport::has_blockers()` 行为正确 |
| V0-1.4 | 规则实现 | §4.3 全部码实现（纯函数；含 14 号 §6.2 增补的 `E_SPEC_NATURE_MISMATCH`） | 每个码至少 1 条触发用例 + 1 条不触发用例（内联单测） |
| V0-1.5 | Registry 文件与加载器 | `policies/viz-relation-registry.yaml` + `pm-application/src/visualization/registry_loader.rs` | 加载器：字段缺失 fail-fast；draft relation 产出 Blocker；deprecated 仍可解释 |
| V0-1.6 | source_hash 实现 | `pm-application/src/visualization/source_hash.rs` | 确定性：同输入两次哈希一致；排序无关（打乱输入顺序结果相同）；编码为裸小写 hex |
| V0-1.7 | Golden 语料 | `validation/golden/{cases,expected}` | ≥12 case（覆盖每个 E 码至少 1 条、每个 W 码至少 1 条）；全绿 |
| V0-1.8 | 脚本门禁 | `scripts/viz-registry-lint.js`、`scripts/viz-spec-lint.js`、`scripts/viz-no-write-path.sh` | 三脚本可独立运行；接入 `scripts/test.sh` |
| V0-1.9 | 文档回填 | 本文 + D3 兼容附录 + 索引表 | D3 §9 开放问题 1（词表会签）标记进行中；偏差更正清单同步到 10 号矩阵 |

**V0-1 完成定义**：AC-1 全绿（校验 + Golden）、AC-21 P0 基础部分绿、Registry 词表经领域 Owner 会签（或全部标 draft 并阻断使用）、三个 lint 脚本接入 test.sh。

## 12. 遗留待定项

| # | 待定 | 决策人 | 期限 |
|---|---|---|---|
| O-1 | 首批 Registry 词表条目与 owner（谁签） | Domain Owners | V0-1.5 前（否则全部 draft → 校验器阻断，符合"会签未完成=不可用"） |
| O-2 | pm-domain 是否下沉 sha2（现放 pm-application） | Architecture Owner | V0-1.6 前 |
| O-3 | `derived_trace` 是否进入 Registry 正式词表（现为 visualization 域自持） | Architecture Owner | V0-1.5 前 |
| O-4 | ObjectResolver 实现落在 pm-application 哪个模块（依赖倒置边界） | Backend Owner | V0-1.3 前 |
| O-5 | 文本长度上限是否与前端输入限制统一（前端另有硬编码中文检查，需共用常量） | Frontend Owner | V0-1.4 前 |
