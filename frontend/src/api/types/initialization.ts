/**
 * S01 项目初始化域类型 —— 与 `docs/s01-backend-api-requirements-v1.md` 对齐。
 *
 * 覆盖四个切片的 DTO：
 *   S01-01 基线确认 → `BaselineProposal` / `ProjectBaseline` / `BaselineRisk`
 *   S01-02 项目创建 → `ProjectProfile` / `InitializationState`
 *   S01-03 仓库管理 → `RepositoryV2` / `GitProviderAuthorization`
 *   S01-04 AI-PM 认知 → `CognitionRun` / `CognitionItem` / `ProjectKnowledge`
 *
 * 命名与字段一律 snake_case，与后端 JSON 一致（不做前端驼峰化）。
 */

/* ───────────────────────── S01-02 项目创建 ───────────────────────── */

/** S01-02 §8.2：初始化生命周期。与 `project.status` 分离，两者不得共用字段。 */
export type InitializationStatus =
  | "not_started"
  | "in_progress"
  | "waiting_for_human"
  | "blocked"
  | "completed";

export type ProjectStatus = "active" | "suspended" | "archived";

/** S01-02 §3.4：可选项目背景信息。不阻止创建，仅供 AI-PM Cognition 使用。 */
export interface ProjectProfile {
  project_id: string;
  project_type: string | null;
  business_domain: string | null;
  expected_outcome: string | null;
  initial_context: string | null;
  updated_at: string;
}

/** S01-02 §5.2：创建前确认摘要所需。 */
export interface ProjectCreatePreview {
  name: string;
  code: string;
  code_available: boolean;
  owner_principal_id: string;
  owner_display_name: string;
  customer_id: string | null;
  customer_name: string | null;
  description: string | null;
  background: {
    project_type: string | null;
    business_domain: string | null;
    expected_outcome: string | null;
    initial_context: string | null;
  };
}

/** S01-02 §3.2：Project Key 建议。 */
export interface ProjectCodeSuggestion {
  code: string;
  available: boolean;
  conflict_code: string | null;
}

/** S01-02 §8.1：初始化工作空间的聚合读模型。 */
export interface InitializationState {
  project_id: string;
  project_status: ProjectStatus;
  init_status: InitializationStatus;
  /** S01-01 §2：当前唯一生效基线指针，首次确认前为 null。 */
  current_baseline_id: string | null;
  baseline_version: number | null;
  repository_count: number;
  primary_repository_id: string | null;
  cognition_run_count: number;
  pending_cognition_items: number;
  pending_baseline_proposals: number;
  updated_at: string;
}

/* ───────────────────────── S01-03 仓库管理 ───────────────────────── */

/** S01-03 §3.3：Repository Role。项目管理属性，不代表访问权限等级。 */
export type RepositoryRoleV2 =
  | "primary"
  | "supporting"
  | "infrastructure"
  | "test";

export type GitProvider = "github" | "gitlab" | "other";

/** S01-03 §7：核验项与状态。 */
export type VerificationStatus = "pending" | "passed" | "warning" | "failed";

export type RepositoryConnectionStatus =
  | "pending"
  | "connected"
  | "failed"
  | "disconnected";

export type VerificationCheckCode =
  | "repository_identity"
  | "access_permission"
  | "branch_availability"
  | "commit_availability"
  | "repository_metadata";

export interface VerificationCheck {
  code: VerificationCheckCode;
  status: VerificationStatus;
  detail: string | null;
}

/**
 * S01-03 §4 + §6。
 *
 * 严格区分四个概念：`default_branch`（仓库自身默认分支）、
 * `selected_branch`（项目采用分支）、`selected_commit`（项目确认采用的版本）、
 * `head_commit_sha`（分支当前 HEAD）。
 */
export interface RepositoryV2 {
  id: string;
  project_id: string;
  name: string;
  url: string;
  provider: GitProvider;
  repo_owner: string | null;
  role: RepositoryRoleV2;
  default_branch: string | null;
  selected_branch: string | null;
  selected_commit: string | null;
  connection_status: RepositoryConnectionStatus;
  verification_status: VerificationStatus;
  last_verified_at: string | null;
  last_scanned_at: string | null;
  head_commit_sha: string | null;
  description: string | null;
  revision: number;
  created_at: string;
  updated_at: string;
}

/** S01-03 §6.2：Branch HEAD 变化提示。`head_changed=true` 时只提示，不自动更新。 */
export interface RepositoryVerificationOutcome {
  repository_id: string;
  status: string;
  verification_status: VerificationStatus;
  default_branch: string | null;
  head_commit_sha: string | null;
  selected_branch: string | null;
  selected_commit: string | null;
  /** 当前 HEAD 与项目采用版本不一致 —— 前端据此提示用户确认更新。 */
  head_changed: boolean;
  checks: VerificationCheck[];
  verified_at: string;
  error_code: string | null;
  error_detail: string | null;
}

/**
 * S01-03 §5.2：Provider 授权。
 *
 * **凭证不回显** —— 只有 provider / scopes / status / expires_at。
 */
export interface GitProviderAuthorization {
  provider: GitProvider;
  scopes: string[];
  status: "active" | "expired" | "revoked";
  expires_at: string | null;
  authorized_at: string | null;
}

/* ───────────────────────── S01-01 基线确认 ───────────────────────── */

/** S01-01 §3.1：提案状态机。 */
export type BaselineProposalStatus =
  | "generating"
  | "pending_review"
  | "needs_revision"
  | "rejected"
  | "confirmed"
  | "expired"
  | "failed";

/** S01-01 §4.1：证据来源类型。代码事实与 AI 推断必须视觉可区分。 */
export type EvidenceSourceType =
  | "code_fact"
  | "git_commit"
  | "doc_fact"
  | "ai_inference";

export interface BaselineSource {
  source_type: EvidenceSourceType;
  label: string;
  detail: string | null;
  /** 可定位证据（文件路径 / commit / 文档节）。 */
  locator: string | null;
}

/** S01-01 §4.2：差异类型。 */
export type BaselineDiffType = "added" | "modified" | "removed" | "unchanged";

export interface BaselineDiffEntry {
  diff_type: BaselineDiffType;
  section: string;
  field: string | null;
  before: string | null;
  after: string | null;
  reason: string | null;
  locator: string | null;
}

/** S01-01 §4.3：风险等级。 */
export type BaselineRiskLevel = "P0" | "P1" | "P2";

/**
 * S01-01 §4.3：`ai_status` 与 `human_decision` 是两个独立语义，
 * 前者不得被渲染成「已解决」。
 */
export interface BaselineRisk {
  risk_id: string;
  risk_level: BaselineRiskLevel;
  risk_title: string;
  risk_description: string;
  ai_status: string | null;
  human_decision: string | null;
  evidence_ref: string | null;
  decided_by: string | null;
  decided_at: string | null;
  decision_note: string | null;
}

export interface BaselineRepositoryRef {
  repository_id: string;
  repository_name: string;
  repository_url: string;
  branch_name: string | null;
  commit_sha: string | null;
}

/** S01-01 §2：基线提案 + 提案版本。 */
export interface BaselineProposal {
  id: string;
  project_id: string;
  proposal_version: number;
  status: BaselineProposalStatus;
  /** 为什么生成这份基线。 */
  generation_reason: string;
  analysis_scope: string[];
  sources: BaselineSource[];
  repositories: BaselineRepositoryRef[];
  /** 完整基线内容，按章节。 */
  content: BaselineSection[];
  diff: BaselineDiffEntry[];
  risks: BaselineRisk[];
  /** 生成时的仓库基准，用于判断是否过期。 */
  repository_baselines: BaselineRepositoryRef[];
  created_at: string;
  updated_at: string;
  confirmed_at: string | null;
  rejection_reason: string | null;
  failure_reason: string | null;
}

export interface BaselineSection {
  key: string;
  title: string;
  body: string;
}

/** S01-01 §2：正式基线，一经确认不可覆盖。 */
export interface ProjectBaseline {
  id: string;
  project_id: string;
  version: number;
  content: BaselineSection[];
  repositories: BaselineRepositoryRef[];
  proposal_id: string | null;
  proposal_version: number | null;
  confirmed_by: string | null;
  confirmed_by_name: string | null;
  confirmed_at: string;
  superseded_at: string | null;
  created_at: string;
}

/** S01-01 §4.4：确认记录，永久保留。 */
export interface BaselineConfirmation {
  id: string;
  baseline_id: string;
  baseline_version: number;
  proposal_id: string;
  proposal_version: number;
  confirmed_by: string;
  confirmed_by_name: string | null;
  confirmed_at: string;
  confirmation_note: string | null;
  repository_baselines: BaselineRepositoryRef[];
  resulting_baseline_version: number;
}

/** S01-01 §5：当前基线总览。 */
export interface BaselineOverview {
  current: ProjectBaseline | null;
  history: ProjectBaseline[];
  pending_proposals: number;
}

/* ───────────────────────── S01-04 AI-PM 认知 ───────────────────────── */

/** S01-04 §5.1：Cognition Run 状态。 */
export type CognitionRunStatus =
  | "pending"
  | "collecting"
  | "analyzing"
  | "completed"
  | "partially_completed"
  | "failed"
  | "cancelled";

/** S01-04 §5.2：Cognition Item 状态。 */
export type CognitionItemStatus =
  | "proposed"
  | "confirmed"
  | "rejected"
  | "investigation_required"
  | "deferred"
  | "superseded";

/** S01-04 §3：六个认知领域。 */
export type CognitionDomain =
  | "project_identity"
  | "repository_understanding"
  | "domain_understanding"
  | "architecture_understanding"
  | "development_convention"
  | "project_evolution";

/** S01-04 §7.1：知识变更类型。 */
export type KnowledgeChangeType =
  | "add"
  | "modify"
  | "retire"
  | "relationship"
  | "conflict";

/** S01-04 §6.1：逐项处理动作。无「全部通过」。 */
export type CognitionAction =
  | "confirm"
  | "edit_and_confirm"
  | "reject"
  | "request_investigation"
  | "defer";

export interface CognitionRun {
  id: string;
  project_id: string;
  status: CognitionRunStatus;
  scope: string[];
  repository_baselines: BaselineRepositoryRef[];
  inputs: string[];
  capability_version: string | null;
  item_count: number;
  confirmed_count: number;
  error_message: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
}

export interface CognitionItem {
  id: string;
  run_id: string;
  domain: CognitionDomain;
  content: string;
  evidence: BaselineSource[];
  evidence_type: EvidenceSourceType;
  confidence: number;
  has_conflict: boolean;
  status: CognitionItemStatus;
  change_type: KnowledgeChangeType | null;
  related_knowledge_id: string | null;
  ai_rationale: string | null;
  decided_by: string | null;
  decided_at: string | null;
  decision_note: string | null;
  created_at: string;
}

/** S01-04 §5.3：Knowledge 状态。 */
export type KnowledgeStatus = "active" | "superseded" | "retired";

export interface ProjectKnowledge {
  id: string;
  project_id: string;
  category: string;
  content: string;
  source_references: BaselineSource[];
  confirmed_by: string | null;
  confirmed_by_name: string | null;
  confirmed_at: string;
  version: number;
  status: KnowledgeStatus;
  supersedes: string | null;
  /** S01-04 §8.1：核心认知默认注入 Agent Context，须人类确认。 */
  is_core: boolean;
  created_at: string;
}

/** S01-04 §4.1：自动准备结果。此阶段不启动正式 Run。 */
export interface CognitionPreparation {
  project_info_complete: boolean;
  missing_project_fields: string[];
  repositories: Array<{
    repository_id: string;
    name: string;
    authorized: boolean;
    accessible: boolean;
    collected_documents: number;
    missing: string[];
  }>;
  collected_count: number;
  missing_materials: Array<{
    code: string;
    message: string;
    /** 是否阻断正式分析。无仓库不阻断。 */
    blocking: boolean;
  }>;
  analysis_scope: CognitionDomain[];
}

/** S01-04 §8.3：Agent Context 四层结构。 */
export interface AgentContext {
  base_context: { knowledge: ProjectKnowledge[] };
  task_context: { knowledge: ProjectKnowledge[] };
  repository_context: {
    repository_id: string | null;
    branch: string | null;
    commit: string | null;
    knowledge: ProjectKnowledge[];
  };
  execution_context: {
    run_id: string | null;
    task_id: string | null;
    state: string;
  };
  generated_at: string;
}
