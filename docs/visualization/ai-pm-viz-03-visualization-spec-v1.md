# Visualization Specification V1.0 设计

> 状态：草案 · 日期：2026-11-12 · 本文档定义 AI-PM 自有的图形中间格式；所有渲染技术（SeeCode/G6/降级表格）只是它的编译目标。

## 1. 设计目标

| 目标 | 手段 |
|---|---|
| 中立于渲染技术 | Spec 只含语义与意图，零渲染器字段；渲染器专属参数放 Adapter 编译层 |
| 可版本化 | Spec 本身是不可变 JSON 文本，content_hash 寻址 |
| 可溯源 | 每个节点/边强制携带 source_ref |
| 可校验 | JSON Schema + 引用完整性校验，模型输出先校验后入库 |
| 可降级 | 任何 Spec 至少能退化为结构化表格渲染（契约测试强制） |

## 2. 三个概念：Spec / Artifact / Slot

| 概念 | 定义 | 类比 |
|---|---|---|
| **VizSpec** | 一份不可变 JSON：图类型 + 节点/边 + 布局意图 + 溯源。纯数据，无状态 | `document_revision` 的内容 |
| **VizArtifact** | Spec 的持久化封装：版本号、状态机（draft/published/stale/rejected）、source_hash、生成方式、审批记录 | `design_revision` + `design_approval` |
| **VizSlot** | 图形的业务挂点：`project_id + slot_key`（如 `architecture-map`、`req-trace:{requirement_id}`、`task-dag`）。一个 slot 有多版本 | PageIndex 中的 document_section 定位 |

## 3. Spec 结构（四段式）

```jsonc
{
  "spec_version": "1.0",
  "slot_key": "req-trace:R-042",
  "view": {                          // ① 视图意图：渲染器据此选 Adapter 模板
    "kind": "trace-graph",           // 词汇表见 §5，共 8 种
    "focus": ["R-042"],              // 焦点对象（accent）
    "lenses": [ { "name": "coverage", "match": { "edge_type": "covers" } } ],
    "layout_hint": "layered-lr"      // 意图而非坐标；渲染器可忽略
  },
  "nodes": [                         // ② 节点：业务对象投影
    {
      "id": "R-042",
      "label": "批量导入执行器",
      "kind": "requirement",         // 受控词表：requirement/design/adr/task/module/service/agent/event/...
      "status": "confirmed",         // 透传原领域状态，展示层语义
      "source_ref": {                // ③ 溯源（强制字段，缺失即 E 级校验失败）
        "domain": "delivery", "type": "requirement",
        "id": "R-042", "revision_id": "…", "content_hash": "…",
        "locator": { "doc_path": "delivery/R-042.md", "section": "acceptance" }
      }
    }
  ],
  "edges": [                         // ④ 边：关系投影
    {
      "id": "e1", "from": "R-042", "to": "D-017",
      "rel": "refined-by",           // 受控词表：见 §5.2
      "source_ref": { "domain": "design", "type": "design_requirement", "id": "…", "locator": {} }
    }
  ],
  "meta": {
    "title": "R-042 追踪图",
    "lang": "zh-CN",
    "budget": "balanced",            // strict ≤30 / balanced ≤150 / faithful ≤500（节点数）
    "nature": "business",            // business | explanatory（解释图：AI 假设内容，DV-13）
    "generated_by": { "mode": "ai-draft", "conversation_id": "…", "message_id": "…" },
    "provenance": {                  // 来源快照（DV-7，Q4 裁决）
      "source_domain": "delivery",
      "source_object_ids": ["R-042", "D-017"],
      "source_revision_refs": ["R-042@rev12", "D-017@rev3"],
      "source_snapshot_hash": "sha256:…"
    },
    "renderer_id": "seecode",
    "renderer_version": "0.6.3",
    "source_hash": "sha256:…"        // 输入对象指纹集合的哈希（stale 判定）
  }
}
```

要点：
1. **无坐标、无颜色、无渲染器参数**。`layout_hint` 是意图枚举，Adapter 可自由决定。
2. **source_ref 与 `kb.memory_ref` 同构**（domain/type/id/revision_id/locator），复用既有形态，降低概念成本。
3. **source_hash** = 对 spec 引用的每个 (target_id, revision_id, content_hash) 排序拼接后取哈希。任何源对象 revision 变化 → source_hash 失配 → stale。

## 4. 校验规则（分级，沿用 SeeCode E/W 理念）

| 级别 | 规则（摘要） | 行为 |
|---|---|---|
| E（拒绝入库） | schema 违规；node/edge 引用不存在的对象 ID（对当前项目上下文校验）；source_ref 缺失或 revision 不存在；超 budget；kind/rel 不在受控词表；未知顶层字段 | 生成失败，返回 `{code,at,fix}` 供模型修补（≤3 轮） |
| W（入库+警示） | 无边节点；重复 label；edge 指向无权对象（服务端裁剪后产生悬空）；layout_hint 冲突 | 渲染但 UI 显式标注 |
| I（提示） | focus 未命中；lens 无匹配 | 静默或徽标 |

权限前置：校验阶段即按调用者权限过滤对象；**被裁剪的引用转为 W 级悬空边**，不允许静默丢弃后渲染出"看起来完整"的图。

## 5. 受控词表 V1.0

### 5.1 view.kind（8 种，V1 封顶）

| kind | 形态 | 典型场景 |
|---|---|---|
| `capability-map` | 分组邻接图 | Product Cognition 能力地图 |
| `architecture-graph` | 分层组件图 | Project Cognition 技术架构 |
| `trace-graph` | 追踪链（layered） | Req→Design→ADR→Task |
| `dag` | 有向无环依赖 | Task dependency、设计依赖 |
| `sequence` | 时序泳道 | Agent 协作时序、审批流 |
| `dependency-graph` | 力导向网络 | 模块/Repo 依赖、风险传播 |
| `impact-tree` | 以焦点为根的影响树 | 变更影响分析 |
| `data-model` | ER 卡片图 | 数据模型关系 |

### 5.2 Relation Registry（DV-9：带命名空间的受控字符串，**非数据库全局 ENUM**）

格式 `domain.relation`，由 Registry 统一维护（schema 校验 + lint + **领域 owner 会签**）。理由：关系词表失守（depends_on/dependency/requires 混用）会让影响分析、覆盖检查、AI 推理逐渐失真。关系**语义**由业务领域 Owner 确认，Visualization 只负责映射与表达，不创造关系定义。

V1 首批词表：

| 词 | 语义 |
|---|---|
| `design.covers_requirement` / `requirement.refines_design` | Req↔Design |
| `design.decided_by_adr` | Design↔ADR |
| `task.implements_requirement` | Task↔Req |
| `task.depends_on_task` | Task 依赖 |
| `pc.*`（透传 pc.relationship 类型） | 产品认知关系 |
| `kb.supported_by_memory` | 知识溯源 |
| `derived_trace`（**合成边，非业务事实**） | 二跳拼接：`{kind:"derived_trace", direct:false, via:[{type,id}], semantic_confidence:"structurally_supported"}`（DV-8）——只声明路径结构完整，不夸大业务确定性 |

## 6. 版本与 Stale（与既有机制对齐）

```
VizArtifact { id, project_id, slot_key, version_no, state,
              spec (jsonb), spec_hash, source_hash, renderer_used,
              approved_by?, approval_id?, created_by, created_at }
```
- 状态机：`draft → published → stale`；`draft → rejected`（reject 必带 reason）。pending 审批每 slot 至多一条（部分唯一索引，对齐 `design_approval` 0010:286 先例）。
- stale 判定：读取时（或轻量后台任务）重算 source_hash 与当前库内对象指纹比对。**不自动重渲染**——重新生成走 AI 起草 + 审批全流程。
- 发布的 Artifact 不可变；对比/回看永远可用（派生产物，删除不伤权威）。

## 7. 防第三方绑定：契约测试

1. **多目标契约**：每种 `view.kind` 必须有 ≥2 个编译目标可用（Adapter A 在线 + Adapter B 产物 + Fallback Adapter=表格/Mermaid），CI 用固定 spec 样例跑多路并比对关键断言（节点数、边数、溯源可达）。
2. **Spec Golden Tests（与 SeeCode 无关，DV-11）**：固定输入样例 → 校验器输出与结构规范化结果快照比对。这是防供应商绑定的真正关键，不依赖任何第三方行为。
2. **字段纯净性门禁**：lint 检查 Spec JSON 不含渲染器保留字（`row/col/skin/motion` 等 SeeCode 字段、G6 layout 配置对象），出现即构建失败。
3. **降级达标**：任一 Adapter 不可用时，spec 必须可渲染为表格（nodes 表 + edges 表 + source_ref 列），Viewer 显示降级视图而非空白。

## 8. 与 SeeCode spec 的映射（Adapter B 编译器职责，非 Spec 内容）

| AI-PM Spec | → SeeCode spec |
|---|---|
| view.kind + layout_hint | type（trace-graph+layered-lr → flowchart/architecture 视 rel 密度选择） |
| nodes(label/kind/status) | nodes(label/sub/icon) |
| edges(rel) | edges(label) |
| view.focus | focal + primary |
| meta.budget | budget（strict/balanced/faithful 档位映射） |
| meta.lang | lang |
| 每节点 source_ref | evidence（file/line 仅当 locator 提供文件定位时；其余放 note） |

超预算处理：Adapter B 编译器对 `dag/dependency-graph` 做 top-K 聚焦裁剪（保留 focus 邻域），产物页脚注明"显示 N/M 节点，完整图请用在线视图"。

## 9. 开放问题（留评审决策）

1. `kind/rel` 词表 V1 是否需要各领域 owner 会签（建议：是，P0 内完成一次评审）。
2. ~~Live View 是否复用 VizSpec 结构~~ → **已裁决（DV-6，Q3）**：统一图形数据模型 + 独立响应封装。Live 响应 `{data: <VizSpec 核心>, meta: {source_revision, generated_at, consistency, truncated, …}}`；查询游标/分页/权限错误/缓存状态等 API 语义只进 meta，**不进 VizSpec**；Artifact 复用同一核心结构，附带 artifact_id/version/renderer/provenance。前端零二次转换，G6 与 SeeCode 消费同一标准数据，Live↔Artifact 可结构化 Diff。
3. spec 是否需要 Git 化——**V1 决策：不做，推迟安全（DV-7，Q4）**，附三个强制条件：① provenance 字段齐备（§3 meta，含 renderer_id/version）；② stale 可识别（旧图不得冒充当前事实）；③ 原 Spec 快照必须持久保留（防"只有渲染 HTML、丢失图形描述"）。将来正式交付物类图形（评审结论、已批准架构文件）进 Git 权威链时，按**正式文档**管理，而非全量 Git 化临时 Artifact。
