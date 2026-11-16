/**
 * 后端 payload → 前端模型转换（**唯一转换点**）。
 *
 * 依据 16 号 V0-8 设计 §10 V0-8.2：组件不得直接消费后端原始 DTO——
 * 所有形状差异在这里收敛一次，组件只面对 `VizSpec` / `VizArtifact`。
 *
 * 纪律：
 * - **不做静默默认**：字段缺失即抛 `VizContractError`（调用方负责降级），
 *   不能用空字符串/空数组把契约破损掩盖过去；
 * - 字段命名保持后端 snake_case 直传（不改名），避免"看起来一致其实语义不同"；
 * - 权限裁剪在服务端完成——转换层**不**做过滤，也不补齐无权对象。
 */
import type {
  VizArtifact,
  VizBudget,
  VizEdge,
  VizLiveResponse,
  VizNature,
  VizNode,
  VizSpec
} from "./types";

/** 契约破损（字段缺失或类型不符）。 */
export class VizContractError extends Error {
  constructor(
    message: string,
    readonly field: string
  ) {
    super(message);
    this.name = "VizContractError";
  }
}

function requireString(value: unknown, field: string): string {
  if (typeof value !== "string") {
    throw new VizContractError(`field ${field} missing or not a string`, field);
  }
  return value;
}

function optionalString(value: unknown, field: string): string | undefined {
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "string") {
    throw new VizContractError(`field ${field} is not a string`, field);
  }
  return value;
}

function requireArray(value: unknown, field: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new VizContractError(`field ${field} missing or not an array`, field);
  }
  return value;
}

function optionalNature(value: unknown, field: string): VizNature | undefined {
  if (value === undefined || value === null) return undefined;
  if (value !== "business" && value !== "explanatory") {
    throw new VizContractError(
      `field ${field} has invalid value: ${String(value)}`,
      field
    );
  }
  return value;
}

function optionalBudget(value: unknown, field: string): VizBudget | undefined {
  if (value === undefined || value === null) return undefined;
  if (value !== "strict" && value !== "balanced" && value !== "faithful") {
    throw new VizContractError(
      `field ${field} has invalid value: ${String(value)}`,
      field
    );
  }
  return value;
}

/** 后端节点 DTO → 前端节点。 */
export function toVizNode(dto: unknown, path = "node"): VizNode {
  const raw = (dto ?? {}) as Record<string, unknown>;
  const id = requireString(raw.id, `${path}.id`);
  const label = requireString(raw.label, `${path}.label`);
  const kind = requireString(raw.kind, `${path}.kind`);
  const node: VizNode = { id, label, kind };
  const status = optionalString(raw.status, `${path}.status`);
  if (status !== undefined) node.status = status;
  const nature = optionalNature(raw.nature, `${path}.nature`);
  if (nature !== undefined) node.nature = nature;
  if (raw.source_ref !== undefined && raw.source_ref !== null) {
    const src = raw.source_ref as Record<string, unknown>;
    node.source_ref = {
      domain: requireString(src.domain, `${path}.source_ref.domain`),
      type: requireString(src.type, `${path}.source_ref.type`),
      id: requireString(src.id, `${path}.source_ref.id`)
    };
    const revision = optionalString(
      src.revision_id,
      `${path}.source_ref.revision_id`
    );
    if (revision !== undefined) node.source_ref.revision_id = revision;
    const hash = optionalString(
      src.content_hash,
      `${path}.source_ref.content_hash`
    );
    if (hash !== undefined) node.source_ref.content_hash = hash;
    if (src.locator !== undefined) node.source_ref.locator = src.locator;
  }
  if (raw.derived !== undefined && raw.derived !== null) {
    const d = raw.derived as Record<string, unknown>;
    node.derived = {
      direct: d.direct === true,
      via: requireArray(d.via, `${path}.derived.via`).map(String),
      semantic_confidence: requireString(
        d.semantic_confidence,
        `${path}.derived.semantic_confidence`
      )
    };
  }
  return node;
}

/** 后端边 DTO → 前端边。 */
export function toVizEdge(dto: unknown, path = "edge"): VizEdge {
  const raw = (dto ?? {}) as Record<string, unknown>;
  const edge: VizEdge = {
    id: requireString(raw.id, `${path}.id`),
    from: requireString(raw.from, `${path}.from`),
    to: requireString(raw.to, `${path}.to`),
    relation: requireString(raw.relation, `${path}.relation`)
  };
  const label = optionalString(raw.label, `${path}.label`);
  if (label !== undefined) edge.label = label;
  const nature = optionalNature(raw.nature, `${path}.nature`);
  if (nature !== undefined) edge.nature = nature;
  if (raw.derived !== undefined && raw.derived !== null) {
    const d = raw.derived as Record<string, unknown>;
    edge.derived = {
      direct: d.direct === true,
      via: requireArray(d.via, `${path}.derived.via`).map(String),
      semantic_confidence: requireString(
        d.semantic_confidence,
        `${path}.derived.semantic_confidence`
      )
    };
  }
  return edge;
}

/** 后端 Spec DTO → 前端 VizSpec。 */
export function toVizSpec(dto: unknown, path = "spec"): VizSpec {
  const raw = (dto ?? {}) as Record<string, unknown>;
  const view = (raw.view ?? {}) as Record<string, unknown>;
  const meta = (raw.meta ?? {}) as Record<string, unknown>;
  const spec: VizSpec = {
    spec_version: requireString(raw.spec_version, `${path}.spec_version`),
    slot_key: requireString(raw.slot_key, `${path}.slot_key`),
    view: {
      kind: requireString(view.kind, `${path}.view.kind`),
      focus: requireArray(view.focus ?? [], `${path}.view.focus`).map(String),
      lenses: Array.isArray(view.lenses)
        ? (view.lenses as VizSpec["view"]["lenses"])
        : [],
      layout_hint: optionalString(view.layout_hint, `${path}.view.layout_hint`)
    },
    nodes: requireArray(raw.nodes, `${path}.nodes`).map((n, i) =>
      toVizNode(n, `${path}.nodes[${i}]`)
    ),
    edges: requireArray(raw.edges, `${path}.edges`).map((e, i) =>
      toVizEdge(e, `${path}.edges[${i}]`)
    ),
    meta: {
      budget: optionalBudget(meta.budget, `${path}.meta.budget`) ?? "balanced"
    }
  };
  const title = optionalString(meta.title, `${path}.meta.title`);
  if (title !== undefined) spec.meta.title = title;
  const lang = optionalString(meta.lang, `${path}.meta.lang`);
  if (lang !== undefined) spec.meta.lang = lang;
  const nature = optionalNature(meta.nature, `${path}.meta.nature`);
  if (nature !== undefined) spec.meta.nature = nature;
  const registryVersion = optionalString(
    meta.registry_version,
    `${path}.meta.registry_version`
  );
  if (registryVersion !== undefined)
    spec.meta.registry_version = registryVersion;
  return spec;
}

/** 后端 Artifact DTO → 前端 Artifact。 */
export function toVizArtifact(dto: unknown, path = "artifact"): VizArtifact {
  const raw = (dto ?? {}) as Record<string, unknown>;
  const state = raw.state;
  if (
    state !== "draft" &&
    state !== "published" &&
    state !== "stale" &&
    state !== "rejected"
  ) {
    throw new VizContractError(
      `field ${path}.state has invalid value: ${String(state)}`,
      `${path}.state`
    );
  }
  const nature = optionalNature(raw.nature, `${path}.nature`) ?? "business";
  return {
    id: requireString(raw.id, `${path}.id`),
    project_id: requireString(raw.project_id, `${path}.project_id`),
    slot_key: requireString(raw.slot_key, `${path}.slot_key`),
    version_no: Number(raw.version_no ?? 1),
    state,
    nature,
    spec: toVizSpec(raw.spec, `${path}.spec`),
    spec_hash: requireString(raw.spec_hash, `${path}.spec_hash`),
    source_hash: requireString(raw.source_hash, `${path}.source_hash`),
    created_at: requireString(raw.created_at, `${path}.created_at`)
  };
}

/** 后端 Live 响应 → 前端 VizSpec（响应封装拆出核心）。 */
export function toLiveSpec(response: VizLiveResponse, path = "live"): VizSpec {
  const data = response?.data;
  if (!data) {
    throw new VizContractError("live response missing data", `${path}.data`);
  }
  return {
    spec_version: requireString(data.spec_version, `${path}.data.spec_version`),
    slot_key: "",
    view: {
      kind: requireString(data.diagram_type, `${path}.data.diagram_type`),
      focus: [],
      lenses: []
    },
    nodes: requireArray(data.nodes, `${path}.data.nodes`).map((n, i) =>
      toVizNode(n, `${path}.data.nodes[${i}]`)
    ),
    edges: requireArray(data.edges, `${path}.data.edges`).map((e, i) =>
      toVizEdge(e, `${path}.data.edges[${i}]`)
    ),
    meta: { budget: "balanced" }
  };
}
