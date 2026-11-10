import { http } from "@/api/base";

/**
 * S03 设计与技术决策读模型（pm-api `pm_designs.rs`，docs/ui/s03 页面结构）。
 *
 * 领域语义（总体设计 §3/§4/§5/§6）：
 * - **TechnicalDesign = 稳定身份 + DesignRevision 不可变版本**；正文只在
 *   draft 态可改，进入 pending_confirmation 即冻结（§4.2）。
 * - **Revision 五态**：draft / pending_confirmation / approved / rejected /
 *   superseded；「当前正式版本」= design.current_revision_id 指向的那一版。
 * - **ADR 六态**：proposed / under_review / accepted / rejected / superseded /
 *   deprecated；已接受的 ADR 内容不可改，只能新建版本并显式替代。
 * - **需求关联必须绑定修订**（§3.4）：`is_current=false` 表示需求已被新版本
 *   取代而设计仍引用旧版 → 影响分析的重点，不可当已覆盖。
 * - **完成 ≠ 批准**：设计「评审通过」是 revision state=approved；实施基线
 *   冻结是另一件事（Readiness Gate 五条件全过 + 真人冻结）。
 *
 * 写路径（创建/提交/请求批准/裁决/冻结）一律走后端命令，前端只调不改状态。
 */

/** 设计修订五态。 */
export type DesignRevisionState =
  | "draft"
  | "pending_confirmation"
  | "approved"
  | "rejected"
  | "superseded";

/** ADR 六态。 */
export type AdrStatus =
  | "proposed"
  | "under_review"
  | "accepted"
  | "rejected"
  | "superseded"
  | "deprecated";

/** 审批三 kind（design.design_approval CHECK）。 */
export type DesignApprovalKind =
  | "design_revision"
  | "adr_acceptance"
  | "baseline_freeze";

/** 审批状态（pending / approved / rejected）。 */
export type DesignApprovalStatus = "pending" | "approved" | "rejected";

/** 评审问题严重度三值。 */
export type FindingSeverity = "info" | "warning" | "blocker";

/** 设计列表行（`design.design` + 当前/最新修订指针）。 */
export interface DesignEntry {
  id: string;
  project_id: string;
  code: string;
  title: string;
  lifecycle_status: string;
  row_revision: number;
  current_revision_id: string | null;
  /** 当前正式版本的版本号（未批准过则为 null）。 */
  current_version: number | null;
  /** 当前正式版本的状态。 */
  current_state: string | null;
  /** 最新版本号（可能有未批准的新版本）。 */
  latest_version: number | null;
  /** 最新版本状态。 */
  latest_state: string | null;
  created_at: string;
  updated_at: string;
}

/** 设计修订（不可变版本内容）。 */
export interface DesignRevision {
  id: string;
  design_id: string;
  version: number;
  state: DesignRevisionState;
  content: Record<string, unknown>;
  content_hash: string;
  source: string;
  analysis_run_id: string | null;
  base_current_revision_id: string | null;
  created_by: string;
  created_by_name: string | null;
  submitted_at: string | null;
  approved_by: string | null;
  approved_by_name: string | null;
  approved_at: string | null;
  created_at: string;
}

/** 设计 ↔ 需求关联（绑定到具体需求修订）。 */
export interface DesignRequirementLink {
  design_revision_id: string;
  requirement_id: string;
  requirement_revision_id: string;
  requirement_code: string | null;
  requirement_version: number | null;
  requirement_revision_state: string | null;
  /** 绑定的是否为该需求当前正式版本；false = 已被取代（需影响分析）。 */
  is_current: boolean | null;
  created_at: string;
}

/** 评审问题登记。 */
export interface ReviewFinding {
  id: string;
  design_revision_id: string;
  finding_kind: string;
  severity: FindingSeverity | string;
  body: string;
  status: string;
  resolution_note: string | null;
  raised_by: string;
  resolved_by: string | null;
  created_at: string;
  resolved_at: string | null;
}

/** 审批行。 */
export interface DesignApproval {
  id: string;
  kind: DesignApprovalKind;
  design_id: string | null;
  adr_id: string | null;
  baseline_id: string | null;
  target_id: string;
  target_hash: string;
  status: DesignApprovalStatus;
  requested_by: string;
  decided_by: string | null;
  decision_reason: string | null;
  decided_at: string | null;
  created_at: string;
}

/** 项目级审批收件箱行（带目标展示标识与发起/裁决人名）。 */
export interface DesignApprovalInboxRow extends DesignApproval {
  requested_by_name: string | null;
  decided_by_name: string | null;
  /** 目标展示码（设计 code / ADR code / 基线 code）。 */
  target_code: string | null;
  /** 目标展示标题。 */
  target_title: string | null;
}

/** 设计详情（设计 + 全部修订 + 审批）。 */
export interface DesignDetail {
  design: DesignEntry;
  revisions: DesignRevision[];
  approvals: DesignApproval[];
}

/** ADR 列表行。 */
export interface AdrEntry {
  id: string;
  project_id: string;
  code: string;
  title: string;
  status: AdrStatus;
  /** 是否阻塞实施（Readiness Gate 条件 3 的输入）。 */
  blockers_implementation: boolean;
  current_revision_id: string | null;
  current_version: number | null;
  superseded_by_adr_id: string | null;
  row_revision: number;
  created_at: string;
  updated_at: string;
}

/** ADR 修订（提交后内容冻结）。 */
export interface AdrRevision {
  id: string;
  adr_id: string;
  version: number;
  content: Record<string, unknown>;
  state: string;
  content_hash: string;
  created_by: string;
  created_at: string;
}

/** ADR → 设计版本引用（影响关系）。 */
export interface AdrDesignLink {
  adr_revision_id: string;
  design_id: string;
  design_revision_id: string;
  design_code: string | null;
  design_revision_version: number | null;
}

/** ADR → 需求修订引用。 */
export interface AdrRequirementLink {
  adr_revision_id: string;
  requirement_id: string;
  requirement_revision_id: string;
  requirement_code: string | null;
  requirement_revision_version: number | null;
}

/** ADR 详情（ADR + 修订 + 关联 + 审批）。 */
export interface AdrDetail {
  adr: AdrEntry;
  revisions: AdrRevision[];
  design_links: AdrDesignLink[];
  requirement_links: AdrRequirementLink[];
  approvals: DesignApproval[];
}

/** 实施基线（冻结的实施依据快照）。 */
export interface ImplementationBaseline {
  id: string;
  project_id: string;
  code: string;
  status: string;
  content: Record<string, unknown>;
  content_hash: string;
  /** 冻结时的 Readiness Gate 判定快照。 */
  readiness: Record<string, unknown>;
  frozen_by: string;
  frozen_at: string;
  superseded_by_baseline_id: string | null;
  created_at: string;
}

/** Readiness Gate 五条件之一。 */
export interface GateCondition {
  condition: string;
  passed: boolean;
  details: string[];
}

/** Readiness Gate 判定报告（dry-run，不改任何状态）。 */
export interface GateReport {
  ready: boolean;
  design_revision_ids: string[];
  conditions: GateCondition[];
}

/** 基线 → 任务交接行（以本基线为实施依据的任务）。 */
export interface BaselineTaskRow {
  id: string;
  title: string;
  status: string;
  priority: string;
  created_at: string;
}

/** S03 工作空间项目级聚合（四页共用上下文与计数）。 */
export interface DesignWorkspace {
  design_count: number;
  adr_count: number;
  baseline_count: number;
  pending_approval_count: number;
  open_finding_count: number;
  linked_requirement_count: number;
  stale_requirement_count: number;
  /** 设计按当前修订状态计数。 */
  design_states: Record<string, number>;
  /** ADR 按生命周期状态计数。 */
  adr_states: Record<string, number>;
}

/** 分析页：需求覆盖行。 */
export interface AnalysisRequirementLink {
  design_id: string;
  design_revision_id: string;
  requirement_id: string;
  requirement_revision_id: string;
  requirement_code: string | null;
  requirement_title: string | null;
  requirement_version: number | null;
  requirement_state: string | null;
  is_current: boolean | null;
}

/** 分析页：影响关系行（本设计被哪些 ADR 引用）。 */
export interface AnalysisAdrLink {
  design_id: string;
  adr_revision_id: string;
  adr_id: string;
  adr_code: string | null;
  adr_title: string | null;
  adr_status: string | null;
  adr_version: number | null;
}

/** 分析页：单个设计的分析行。 */
export interface DesignAnalysisRow {
  design_id: string;
  code: string;
  title: string;
  lifecycle_status: string;
  current_revision_id: string | null;
  current_version: number | null;
  current_state: string | null;
  requirement_count: number;
  /** 引用了已被取代的需求修订的绑定数。 */
  stale_requirement_count: number;
  open_finding_count: number;
  requirements: AnalysisRequirementLink[];
  adrs: AnalysisAdrLink[];
}

/** 后端列表端点统一返回 `{ items: [...] }`；缺 items 视为空列表。 */
function items<T>(payload: { items?: T[] } | null | undefined): T[] {
  return payload?.items ?? [];
}

/** `GET /v1/pm/projects/{project_id}/designs`：设计列表。 */
export async function listDesigns(projectId: string) {
  const payload = await http.get<{ items: DesignEntry[] }>(
    `/v1/pm/projects/${projectId}/designs`
  );
  return items(payload);
}

/** `GET /v1/pm/designs/{design_id}`：设计详情（修订链 + 审批）。 */
export function getDesign(designId: string) {
  return http.get<DesignDetail>(`/v1/pm/designs/${designId}`);
}

/** `GET /v1/pm/designs/{design_id}/revisions/{revision_id}/requirements`：需求关联。 */
export async function listDesignRequirements(
  designId: string,
  revisionId: string
) {
  const payload = await http.get<{ items: DesignRequirementLink[] }>(
    `/v1/pm/designs/${designId}/revisions/${revisionId}/requirements`
  );
  return items(payload);
}

/** `GET /v1/pm/designs/{design_id}/revisions/{revision_id}/findings`：评审问题。 */
export async function listDesignFindings(designId: string, revisionId: string) {
  const payload = await http.get<{ items: ReviewFinding[] }>(
    `/v1/pm/designs/${designId}/revisions/${revisionId}/findings`
  );
  return items(payload);
}

/** `GET /v1/pm/projects/{project_id}/adrs`：ADR 列表。 */
export async function listAdrs(projectId: string) {
  const payload = await http.get<{ items: AdrEntry[] }>(
    `/v1/pm/projects/${projectId}/adrs`
  );
  return items(payload);
}

/** `GET /v1/pm/adrs/{adr_id}`：ADR 详情（修订链 + 关联 + 审批）。 */
export function getAdr(adrId: string) {
  return http.get<AdrDetail>(`/v1/pm/adrs/${adrId}`);
}

/** `GET /v1/pm/projects/{project_id}/implementation-baselines`：基线列表。 */
export async function listImplementationBaselines(projectId: string) {
  const payload = await http.get<{ items: ImplementationBaseline[] }>(
    `/v1/pm/projects/${projectId}/implementation-baselines`
  );
  return items(payload);
}

/** `GET /v1/pm/implementation-baselines/{baseline_id}`：基线详情。 */
export function getImplementationBaseline(baselineId: string) {
  return http.get<ImplementationBaseline>(
    `/v1/pm/implementation-baselines/${baselineId}`
  );
}

/** `GET /v1/pm/implementation-baselines/{baseline_id}/tasks`：基线 → 任务交接。 */
export async function listBaselineTasks(baselineId: string) {
  const payload = await http.get<{ items: BaselineTaskRow[] }>(
    `/v1/pm/implementation-baselines/${baselineId}/tasks`
  );
  return items(payload);
}

/**
 * `GET /v1/pm/projects/{project_id}/implementation-readiness`：门禁 dry-run。
 *
 * 纯读（不改任何状态）；缺省 `designRevisionIds` = 项目内全部已批准的设计
 * 版本，即「当前整体是否可进入 S04」。
 */
export function evaluateReadiness(
  projectId: string,
  designRevisionIds?: string[]
) {
  const params =
    designRevisionIds && designRevisionIds.length > 0
      ? { design_revision_ids: designRevisionIds.join(",") }
      : undefined;
  return http.get<GateReport>(
    `/v1/pm/projects/${projectId}/implementation-readiness`,
    params
  );
}

/** `GET /v1/pm/projects/{project_id}/design-approvals`：项目级审批收件箱。 */
export async function listDesignApprovals(projectId: string) {
  const payload = await http.get<{ items: DesignApprovalInboxRow[] }>(
    `/v1/pm/projects/${projectId}/design-approvals`
  );
  return items(payload);
}

/** `GET /v1/pm/projects/{project_id}/design-workspace`：四页共用项目级聚合。 */
export function getDesignWorkspace(projectId: string) {
  return http.get<DesignWorkspace>(
    `/v1/pm/projects/${projectId}/design-workspace`
  );
}

/** `GET /v1/pm/projects/{project_id}/design-analysis`：需求覆盖 + 影响关系。 */
export async function listDesignAnalysis(projectId: string) {
  const payload = await http.get<{ items: DesignAnalysisRow[] }>(
    `/v1/pm/projects/${projectId}/design-analysis`
  );
  return items(payload);
}
