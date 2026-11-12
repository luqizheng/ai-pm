/**
 * VizSpec 的 TypeScript 形态——与后端契约同源。
 *
 * 后端定义：`crates/pm-domain/src/visualization/spec.rs`（V0-1）。
 * 契约文档：docs/visualization/ai-pm-viz-03-visualization-spec-v1.md。
 *
 * 硬约束（ADR-VIZ-003）：Spec **零渲染器专属字段**。本文件不得出现
 * `row`/`col`/`skin`/`motion` 等渲染器参数——由 `scripts/viz-spec-lint.js` 守卫。
 */

/** 图形性质：正式业务事实 / AI 生成的解释性内容。 */
export type VizNature = "business" | "explanatory";

/** 复杂度预算档位。 */
export type VizBudget = "strict" | "balanced" | "faithful";

/** 来源引用（与后端 `kb.memory_ref` 同构）。 */
export interface VizSourceRef {
  domain: string;
  type: string;
  id: string;
  revision_id?: string;
  locator?: unknown;
  content_hash?: string;
}

/** 间接推导标记（推导方式维度，与 nature 正交）。 */
export interface VizDerivedTrace {
  direct: boolean;
  via: string[];
  semantic_confidence: string;
}

/** 图中节点。 */
export interface VizNode {
  id: string;
  label: string;
  kind: string;
  status?: string;
  nature?: VizNature;
  source_ref?: VizSourceRef;
  derived?: VizDerivedTrace;
}

/** 图中边。 */
export interface VizEdge {
  id: string;
  from: string;
  to: string;
  relation: string;
  label?: string;
  nature?: VizNature;
  derived?: VizDerivedTrace;
}

/** 透镜（按条件过滤高亮）。 */
export interface VizLens {
  name: string;
  match: Record<string, unknown>;
}

/** 视图意图。 */
export interface VizView {
  kind: string;
  focus?: string[];
  lenses?: VizLens[];
  layout_hint?: string;
}

/** 来源快照。 */
export interface VizProvenance {
  source_domain: string;
  source_object_ids: string[];
  source_revision_refs?: string[];
  source_snapshot_hash?: string;
}

/** 图的元信息。 */
export interface VizMeta {
  title?: string;
  lang?: string;
  budget: VizBudget;
  nature?: VizNature;
  registry_version?: string;
  provenance?: VizProvenance;
}

/** 一份完整的图形描述。 */
export interface VizSpec {
  spec_version: string;
  slot_key: string;
  view: VizView;
  nodes: VizNode[];
  edges: VizEdge[];
  meta: VizMeta;
}

/** Artifact 生命周期状态（ADR-VIZ-006）。 */
export type VizArtifactState = "draft" | "published" | "stale" | "rejected";

/** Artifact 元数据（后端 `viz.artifact`）。 */
export interface VizArtifact {
  id: string;
  project_id: string;
  slot_key: string;
  version_no: number;
  state: VizArtifactState;
  nature: VizNature;
  spec: VizSpec;
  spec_hash: string;
  source_hash: string;
  renderer_id?: string;
  renderer_version?: string;
  created_at: string;
  published_at?: string;
  /** 生成该图时的词表版本——落后于服务端当前版本即提示。 */
  registry_version?: string;
}

/** Live 响应封装（ADR-VIZ-006 / DV-6：统一图形数据模型 + 独立响应封装）。 */
export interface VizLiveResponse {
  data: {
    spec_version: string;
    diagram_type: string;
    nodes: VizNode[];
    edges: VizEdge[];
  };
  meta: {
    source_revision?: string;
    generated_at: string;
    /** current = 版本一致；mixed-revision = 混合时点（须提示用户）。 */
    consistency: "current" | "mixed-revision";
    truncated: boolean;
    truncation_reason?: string | null;
    source_revisions?: Array<{
      domain: string;
      type: string;
      id: string;
      revision_id?: string;
      content_hash?: string;
    }>;
  };
}

/** 一致性状态（前端展示用）。 */
export type VizConsistency = "current" | "mixed-revision";
