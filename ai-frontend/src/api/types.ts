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

/**
 * core.customer（0017 客户主数据）。
 * status: active 正常 / archived 已归档 / placeholder 未归类（旧 bootstrap 按项目 code 隐式生成）。
 */
export interface Customer {
  id: string;
  code: string | null;
  name: string;
  short_name: string | null;
  industry: string | null;
  notes: string | null;
  parent_id: string | null;
  status: string;
  project_count: number;
  created_at: string;
  updated_at: string;
}

/** core.project */
export interface Project {
  id: string;
  customer_id: string;
  /** 冗余客户显示名，读端点 JOIN 取得 */
  customer_name: string;
  /** 客户状态，用于把未归类客户渲染成提示而非真实客户名 */
  customer_status: string;
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

/** delivery.requirement（0039 起 status 列改名 lifecycle_status，S02D-04 三态） */
export interface Requirement {
  id: string;
  project_id: string;
  code: string;
  title: string;
  lifecycle_status: string;
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

/** delivery.requirement_revision（content 目前为 {markdown}，V1） */
export interface RequirementRevision {
  id: string;
  requirement_id: string;
  project_id: string;
  version: number;
  content: { markdown?: string } & Record<string, unknown>;
  state: "draft" | "approved" | "superseded";
  content_hash: string;
  created_by: string;
  approved_by: string | null;
  created_at: string;
}
/** integration.project_repository（0008/0009）：项目 ↔ 代码仓库的关系 */
export type RepositoryRole = "main" | "docs" | "service" | "infra";
export type RepositoryStatus =
  | "pending"
  | "verified"
  | "unreachable"
  | "archived";

export interface Repository {
  id: string;
  project_id: string;
  name: string;
  url: string;
  role: RepositoryRole;
  default_branch: string | null;
  description: string | null;
  ssh_key_id: string | null;
  status: RepositoryStatus;
  head_commit_sha: string | null;
  last_verified_at: string | null;
  last_error_code: string | null;
  last_error_detail: string | null;
  revision: number;
  created_at: string;
  updated_at: string;
}

/** integration.project_ssh_key（迁移 0022 多把模型）：只回公钥与指纹，私钥永不回显。
 * `id` 供仓库绑定引用；`is_default` = 未绑定仓库的回退目标（每项目至多一把）。 */
export interface SshKey {
  id: string;
  project_id: string;
  label: string;
  public_key: string;
  fingerprint: string;
  status: "active" | "revoked";
  is_default: boolean;
  created_at: string;
}

export type ImportStatus =
  | "queued"
  | "cloning"
  | "reading"
  | "analyzing"
  | "draft_ready"
  | "proposed"
  | "applied"
  | "rejected"
  | "failed"
  | "cancelled";

export interface Evidence {
  kind: "doc" | "commit";
  repo_id: string;
  ref: string;
  detail: string;
}

export interface DraftRequirement {
  key: string;
  code: string;
  title: string;
  markdown: string;
  priority: string;
  confidence: "low" | "medium" | "high";
  evidence: Evidence[];
}

export interface DraftTask {
  key: string;
  title: string;
  description: string;
  priority: string;
  milestone_key: string | null;
  requirement_key: string | null;
  assignee_role: "ai_pm" | "architect" | "developer" | "tester" | null;
  confidence: "low" | "medium" | "high";
  evidence: Evidence[];
}

export interface DraftMilestone {
  key: string;
  name: string;
  planned_at: string | null;
}

export interface ProjectDraft {
  schema_version: number;
  profile: {
    suggested_code: string;
    suggested_name: string;
    summary: string;
    domain: string;
    tech_stack: string[];
  };
  milestones: DraftMilestone[];
  requirements: DraftRequirement[];
  tasks: DraftTask[];
  risks: { description: string; severity: string; evidence: Evidence[] }[];
  unknowns: { question: string; why_blocking: string }[];
  coverage: {
    repos: {
      repository_id: string;
      head_commit_sha: string;
      commits_analyzed: number;
      docs_analyzed: number;
      truncated: boolean;
    }[];
  };
}

/** 一次分析轮次（I6-B R-4-1：成功与失败都留档）。 */
export interface ImportAttempt {
  attempt_no: number;
  status: string;
  commit_sha: string | null;
  error_code: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface ImportSource {
  repository_id: string;
  label: string;
  head_commit_sha: string | null;
  commit_count: number;
  doc_count: number;
  size_bytes: number;
  truncated: boolean;
  error_code: string | null;
  /** 该仓库的分析轮次历史。 */
  attempts?: ImportAttempt[];
}

export interface ProjectImport {
  id: string;
  project_id: string;
  status: ImportStatus;
  progress: number;
  stage_detail: string | null;
  model_name: string;
  draft: ProjectDraft | null;
  draft_revision: number;
  dropped_items: number;
  error_code: string | null;
  error_detail: string | null;
  analysis_run_id: string | null;
  created_at: string;
  proposed_at: string | null;
  applied_at: string | null;
  sources: ImportSource[];
}

export interface ImportListItem {
  id: string;
  status: ImportStatus;
  progress: number;
  model_name: string;
  draft_revision: number;
  analysis_run_id: string | null;
  created_at: string;
  proposed_at: string | null;
  applied_at: string | null;
}

export interface ProposalRef {
  kind: string;
  proposal_id: string;
  approval_id: string;
  item_count: number;
}

export interface ProposedImport {
  outcome: "proposed" | "already_applied";
  proposals?: ProposalRef[];
  analysis_run_id?: string;
}
