import { http } from "@/api/base";

/**
 * D-5 · 仓库分析报告（一次分析运行 = 一份报告，聚合 KB 修订 + PC 修订 + PC 提案）。
 *
 * 状态机：analyzing（分析中）→ analyzed（待审理）→ closed（已关闭，门释放）；
 * failed / cancelled 终态不占门。analyzing/analyzed 期间该仓库禁再分析
 * （analyze 409 report_open）。
 */

export type AnalysisReportStatus =
  | "analyzing"
  | "analyzed"
  | "closed"
  | "failed"
  | "cancelled";

export interface AnalysisReportRow {
  id: string;
  repository_id: string;
  repository_name: string;
  commit_sha: string;
  status: AnalysisReportStatus;
  stage_detail: string | null;
  error_code: string | null;
  error_detail: string | null;
  created_at: string;
  updated_at: string;
  closed_at: string | null;
  kb_revision_id: string | null;
  pc_source_id: string | null;
  /** D-2 运行口径：incremental（默认）/ full。 */
  run_mode: "incremental" | "full";
  /** 增量起点（全量/首次为 null）。 */
  since_commit: string | null;
  /** 未决 PC 提案数（draft/submitted）。 */
  pending_proposals: number;
  /** KB 报告草稿是否待确认。 */
  has_pending_kb: boolean;
}

export interface ReportKbRevision {
  revision_id: string;
  document_id: string;
  doc_key: string;
  revision_no: number;
  status: "draft" | "published" | "rejected";
  analysis_mode: string | null;
  content: string | null;
}

export interface ReportProposalRow {
  proposal_id: string;
  title: string;
  status: string;
  expected_version_no: number;
  member_count: number;
  created_at: string;
}

export interface ReportPcRevisionRow {
  object_id: string;
  object_type: string;
  display_name: string;
  revision_no: number;
  change_summary: string | null;
  content: string | null;
}

export interface AnalysisReportDetail extends AnalysisReportRow {
  kb_revision: ReportKbRevision | null;
  proposals: ReportProposalRow[];
  pc_revisions: ReportPcRevisionRow[];
}

export interface CloseReportOutcome {
  report_id: string;
  status: "closed";
  withdrawn_proposals: number;
  rejected_kb: boolean;
  last_analyzed_commit: string;
}

/** 项目分析报告列表（状态分区；repository_id/status 可选过滤）。 */
export function listAnalysisReports(
  projectId: string,
  opts: { repositoryId?: string; status?: AnalysisReportStatus } = {}
) {
  return http.get<{ items: AnalysisReportRow[] }>("/v1/pm/analysis-reports", {
    project_id: projectId,
    ...(opts.repositoryId ? { repository_id: opts.repositoryId } : {}),
    ...(opts.status ? { status: opts.status } : {})
  });
}

/** 报告详情聚合（头 + KB 修订全文 + PC 提案集 + PC 修订集）。 */
export function getAnalysisReport(reportId: string, projectId: string) {
  return http.get<AnalysisReportDetail>(`/v1/pm/analysis-reports/${reportId}`, {
    project_id: projectId
  });
}

/**
 * 关闭报告（D-5 门释放动作；interactive 真人门）：自动驳回未决提案、拒绝 KB
 * 草稿，并固化分析锚点（last_analyzed_commit）。关闭后该仓库可再次分析。
 */
export function closeAnalysisReport(reportId: string, projectId: string) {
  return http.post<CloseReportOutcome>(
    `/v1/pm/analysis-reports/${reportId}/close`,
    {},
    { params: { project_id: projectId } }
  );
}

/** D-4 · 业务身份综合（人工触发；LLM 汇聚访谈答案+仓库产物 → 三提案）。 */
export function synthesizeIdentity(projectId: string) {
  return http.post<{
    profile_proposal_id: string | null;
    vision_proposal_id: string | null;
    goal_proposal_id: string | null;
    goal_proposal_status: string | null;
    uncertainty_notes: string[];
  }>(
    `/v1/pm/projects/${projectId}/cognition/identity-synthesis`,
    {},
    { params: { project_id: projectId } }
  );
}
