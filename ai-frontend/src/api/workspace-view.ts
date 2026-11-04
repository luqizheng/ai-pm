/**
 * workspace 五页接真后端所需的读模型（与 api/types.ts 的经典页契约分开维护，
 * 避免共享文件被多条会话同时修改）。
 */

/** delivery.milestone */
export interface Milestone {
  id: string;
  project_id: string;
  name: string;
  planned_at: string | null;
  status: string;
  revision: number;
}

/**
 * 决策中心读模型：`governance.approval_request` 左连接 `ai.change_proposal`。
 * 提案缺失时 p.* 字段为 null（历史审批仍可读）。
 */
export interface DecisionRecord {
  id: string;
  project_id: string;
  status: string;
  required_role: string;
  decided_by: string | null;
  decision_reason: string | null;
  decided_at: string | null;
  created_at: string;
  proposal_id: string | null;
  proposal_status: string | null;
  target_type: string | null;
  command_type: string | null;
  rationale: string | null;
  risk_level: string | null;
  evidence: unknown[] | null;
  proposed_data: Record<string, unknown> | null;
  analysis_run_id: string | null;
}
