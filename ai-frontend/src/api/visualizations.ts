/**
 * Visualization 域 API。
 *
 * 后端契约：docs/visualization/ai-pm-viz-13-v0-2-data-model-design-v1.md §6.2。
 * 端点前缀 `/v1/pm/visualizations`（仓库惯例：pm-api 资源全量 `/v1/pm/` 前缀，
 * 无 `/api` 前缀——见 apps/pm-api/src/routes.rs 的路由注册注释）。
 *
 * 边界：
 * - **零写路径**：本文件只提供读接口与产物获取；图形不产生业务事实
 *   （ADR-VIZ-005 / AC-16）。
 * - **产物不经 iframe**：SVG 以 `<img>` 引用（blob object URL），产物零脚本执行
 *   （docs/visualization/ai-pm-viz-15-v0-6-security-review-v1.md）。
 */

import { http } from "./base";
import type { ListResponse, Paged } from "./types";
import type { VizArtifact, VizLiveResponse, VizSpec } from "@/lib/viz/types";
import { toLiveSpec, toVizArtifact } from "@/lib/viz/specAdapter";

/** 槽位摘要（槽位列表用，不返回 spec 全文）。 */
export interface VizSlotSummary {
  slot_key: string;
  view_kind: string;
  state: VizArtifact["state"];
  nature: VizArtifact["nature"];
  version_no: number;
  artifact_id: string;
  updated_at: string;
}

/** Artifact 列表查询参数。 */
export interface VizArtifactQuery {
  slot_key?: string;
  state?: VizArtifact["state"];
  page?: number;
  size?: number;
}

/** 槽位列表。 */
export async function listSlots(
  projectId: string
): Promise<ListResponse<VizSlotSummary>> {
  return http.get<ListResponse<VizSlotSummary>>(
    `/v1/pm/visualizations/projects/${projectId}/slots`
  );
}

/** Artifact 详情（含 spec 与来源信息）。 */
export async function getArtifact(artifactId: string): Promise<VizArtifact> {
  return http.get<VizArtifact>(`/v1/pm/visualizations/artifacts/${artifactId}`);
}

/** 槽位历史版本（分页）。 */
export async function listArtifacts(
  projectId: string,
  query: VizArtifactQuery = {}
): Promise<Paged<VizArtifact>> {
  const params: Record<string, string | number> = {};
  if (query.slot_key) params.slot_key = query.slot_key;
  if (query.state) params.state = query.state;
  if (query.page) params.page = query.page;
  if (query.size) params.size = query.size;
  const response = await http.get<Paged<unknown>>(
    `/v1/pm/visualizations/projects/${projectId}/artifacts`,
    { params }
  );
  return {
    ...response,
    items: (response.items ?? []).map(dto => toVizArtifact(dto))
  };
}

/**
 * 提交 Spec → 创建 draft Artifact。
 *
 * 校验失败由后端返回 400（错误体只带 `message`）；完整问题列表见 draft 的
 * `spec` 字段与校验报告，不通过错误通道承载（仓库错误体约定：只带 message）。
 */
export async function createDraft(
  projectId: string,
  spec: VizSpec
): Promise<VizArtifact> {
  const dto = await http.post<unknown>(
    `/v1/pm/visualizations/projects/${projectId}/artifacts`,
    spec
  );
  return toVizArtifact(dto);
}

/** 审批决定：批准 / 拒绝（拒绝必带 reason）。 */
export async function decideApproval(
  approvalId: string,
  body: { action: "approve" | "reject"; reason?: string }
): Promise<void> {
  await http.post<void>(
    `/v1/pm/visualizations/approvals/${approvalId}/decision`,
    body
  );
}

/** 请求审批。 */
export async function requestApproval(artifactId: string): Promise<void> {
  await http.post<void>(
    `/v1/pm/visualizations/artifacts/${artifactId}/approval-requests`
  );
}

/** 触发 stale 重算（同步）。 */
export async function refreshArtifact(artifactId: string): Promise<void> {
  await http.post<void>(
    `/v1/pm/visualizations/artifacts/${artifactId}/refresh`
  );
}

/**
 * 取渲染产物（SVG）并转成 object URL。
 *
 * 为什么必须转：`<img src>` 无法带 Bearer 头，而后端产物接口需要鉴权——
 * 沿用 `src/api/base/self-service.ts` 的既有范式（SDK 下载字节 → Blob → objectURL）。
 *
 * 调用方负责在卸载时 `URL.revokeObjectURL`。
 */
export async function fetchArtifactSvgUrl(artifactId: string): Promise<string> {
  const response = await http.get<Blob>(
    `/v1/pm/visualizations/artifacts/${artifactId}/outputs/svg`,
    { responseType: "blob" }
  );
  return URL.createObjectURL(new Blob([response], { type: "image/svg+xml" }));
}

/**
 * 取渲染产物字节（任意格式）。
 *
 * 走 SDK 而非裸 fetch——产物接口需要 Bearer 头，裸 fetch 会丢鉴权。
 */
export async function fetchArtifactOutputBlob(
  artifactId: string,
  format: "svg" | "png" | "html"
): Promise<Blob> {
  return http.get<Blob>(
    `/v1/pm/visualizations/artifacts/${artifactId}/outputs/${format}`,
    { responseType: "blob" }
  );
}

/** Live 图查询（在线交互图，P1）。 */
export async function getLiveGraph(
  projectId: string,
  viewKind: string,
  params: Record<string, string> = {}
): Promise<VizLiveResponse> {
  return http.get<VizLiveResponse>(
    `/v1/pm/visualizations/projects/${projectId}/live/${viewKind}`,
    { params }
  );
}

/** Live 图 → 前端 VizSpec（走唯一转换点）。 */
export async function getLiveSpec(
  projectId: string,
  viewKind: string,
  params: Record<string, string> = {}
): Promise<VizSpec> {
  const response = await getLiveGraph(projectId, viewKind, params);
  return toLiveSpec(response);
}
