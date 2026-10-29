/**
 * AI-PM 业务领域类型 —— 字段与 ai-pm/migrations 0001–0004 对齐（snake_case）。
 *
 * 状态字段在库里多为无约束 TEXT，前端按 pm.status.* i18n 渲染、
 * 按 statusVariant() 映射 Badge 语义色。
 */

export interface ListResponse<T> {
  items: T[];
  total: number;
}

export interface Paged<T> extends ListResponse<T> {
  page: number;
  size: number;
}

/** core.customer */
export interface Customer {
  id: string;
  name: string;
  status: string;
  created_at: string;
}

/** core.project */
export interface Project {
  id: string;
  customer_id: string;
  code: string;
  name: string;
  description: string | null;
  status: string;
  revision: number;
  created_at: string;
  updated_at: string;
}

/** core.principal */
export type PrincipalKind = "human" | "dev_agent" | "ai_pm" | "service";

export interface Principal {
  id: string;
  kind: PrincipalKind;
  display_name: string;
  is_active: boolean;
  created_at: string;
}

/** delivery.requirement */
export interface Requirement {
  id: string;
  project_id: string;
  code: string;
  title: string;
  status: string;
  current_revision_id: string | null;
  row_revision: number;
  created_at: string;
}

/** delivery.task */
export interface Task {
  id: string;
  project_id: string;
  milestone_id: string | null;
  title: string;
  description: string | null;
  status: string;
  priority: string;
  assignee_id: string | null;
  revision: number;
  created_at: string;
  updated_at: string;
}

/** governance.approval_request */
export interface ApprovalRequest {
  id: string;
  project_id: string;
  proposal_id: string;
  status: string;
  required_role: string;
  decided_by: string | null;
  decision_reason: string | null;
  decided_at: string | null;
  created_at: string;
}

/** governance.audit_log */
export interface AuditLog {
  id: string;
  project_id: string;
  actor_id: string;
  action: string;
  target_type: string;
  target_id: string | null;
  created_at: string;
}
