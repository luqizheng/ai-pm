import { http } from "@/api/base";

/**
 * AI Team 总览与执行监控（UI-2，U-1=C 现有表只读聚合，零迁移）。
 *
 * 后端契约：pm-api `ai_team_monitor.rs`（裁决
 * `docs/project-ai-team-v2/ai-pm-ai-team-ui-monitor-adjudication-pack-v1.md`
 * §〇；事件表挂 Phase 4，先聚合 governance.audit_log）——
 * - GET `/ai-team/overview`：roles / runs / tasks / provider 四组计数与绑定态。
 * - GET `/ai-team/monitor`：runs（最近 50 条 task_run JOIN task）+ events
 *   （最近 100 条 run_dispatch 审计）。V1 不做分页（`before`/limit 挂后续批次）。
 * 读守卫：项目可见（非成员 / 跨租户 / 不存在一律 404，错误体 `{message}`）。
 */

// ── Overview ──────────────────────────────────────────────────────────

/** Role 计数（disabled 只统计项目启用行中的停用；未启用模板不计） */
export interface AiTeamRolesSummary {
  enabled: number;
  disabled: number;
  /** 全局模板总数（V1 固定 5） */
  templates: number;
}

/** Run 计数（近 24h 锚 = COALESCE(finished_at, created_at)） */
export interface AiTeamRunsSummary {
  /** 活跃 Run：queued / claimed / running */
  active: number;
  succeeded_24h: number;
  failed_24h: number;
}

/** 任务阻塞计数 */
export interface AiTeamTasksSummary {
  awaiting_decision: number;
  blocked: number;
}

/**
 * Provider 绑定态（复用现行解析语义，不查密）：
 * 项目指定严格不回退——指定了但 Provider 不可用时 bound=false 且仍回填名字；
 * 未指定时回退 Team 首个 enabled+有模型（继承）。
 */
export interface AiTeamProviderBinding {
  bound: boolean;
  provider_name: string | null;
}

export interface AiTeamOverview {
  roles: AiTeamRolesSummary;
  runs: AiTeamRunsSummary;
  tasks: AiTeamTasksSummary;
  provider: AiTeamProviderBinding;
}

/** AI Team 总览计数聚合 */
export function getProjectAiTeamOverview(projectId: string) {
  return http.get<AiTeamOverview>(
    `/v1/pm/projects/${projectId}/ai-team/overview`
  );
}

// ── Monitor ───────────────────────────────────────────────────────────

/** 执行 Run 明细行（delivery.task_run JOIN delivery.task，created_at DESC） */
export interface AiTeamRunRow {
  run_id: string;
  task_id: string;
  task_title: string;
  /** queued / claimed / running / succeeded / failed / timed_out / cancelled */
  status: string;
  /** 执行阶段（dispatch / started / …；排队中为 null） */
  stage: string | null;
  /** delivery / review */
  run_kind: string | null;
  attempt_no: number;
  executor_kind: string | null;
  /** human / spawn */
  execution_mode: string | null;
  release_reason: string | null;
  /** N-1 调度保护：none / human_wait / start_wait / timed_out */
  scheduling_shield: string;
  claimed_at: string | null;
  /** 终态结果（未终态为 null） */
  outcome: string | null;
  /** 被替代终止时间（B3 隔离/围栏证据；一般终止为 null） */
  terminated_at: string | null;
  /** 展示用完成时间锚 = COALESCE(terminated_at, finished_at) */
  finished_at: string | null;
  created_at: string;
}

/** run_dispatch 审计事件行（created_at DESC，最多 100 条） */
export interface AiTeamEventRow {
  /** claim_task_run / skip_task_run / reclaim_task_run /
   * task_ready_skipped / task_ready_assignee_resolved */
  action: string;
  /** claim/skip/reclaim = run_id；task_ready_* = task_id */
  target_id: string;
  /** JOIN 出的目标任务标题（run 目标经 task_run 二跳；解析不到为 null） */
  target_title: string | null;
  /** actor 摘要 = principal 显示名（缺失为 null） */
  actor: string | null;
  /** COALESCE(after_data.release_reason, after_data.reason) */
  reason: string | null;
  created_at: string;
}

export interface AiTeamMonitor {
  runs: AiTeamRunRow[];
  events: AiTeamEventRow[];
}

/** 执行监控明细聚合（首屏 runs 50 / events 100，V1 无分页） */
export function getProjectAiTeamMonitor(projectId: string) {
  return http.get<AiTeamMonitor>(
    `/v1/pm/projects/${projectId}/ai-team/monitor`
  );
}

// ── Execution Policies（UI-3，U-2=A 五子页最后一块；零迁移）─────────────

/**
 * 后端契约：pm-api `ai_team_policies.rs` ——
 * - GET  `/ai-team/execution-policies`：四层现状 + composition 合成规则说明；
 * - PUT  `/ai-team/execution-policies/project`：body
 *   `{ require_human_approval: boolean | null }`（null = 删行恢复继承组织）；
 * - PUT  `/tasks/{task_id}/policy-override`：四个布尔字段至少一个非 null
 *   （全 null 继承），否则 400；清除走 DELETE。
 * 容器键 taskTypes / taskOverrides 按契约驼峰；行内字段 snake_case。
 * 读守卫 = 项目可见（非成员 404）；写守卫 = 租户 admin 或项目 owner
 * （否则 403）。**V1 写面限定**：organization（租户全局 '*'）与 task_type
 * （跨项目）两层只读，写入口留待显式裁决。
 */

/** scope 层行（organization / project；行缺失 = null = 不表态/继承上层） */
export interface AiTeamScopePolicy {
  require_human_approval: boolean;
  updated_by: string;
  updated_at: string;
}

/** task_type 层行（租户内跨项目全局口径，UI 注明） */
export interface AiTeamTaskTypePolicy {
  task_type: string;
  require_human_approval: boolean;
  updated_by: string;
  updated_at: string;
}

/** 任务级覆盖行（NULL 字段 = 继承上层） */
export interface AiTeamTaskPolicyOverride {
  task_id: string;
  task_title: string;
  auto_accept: boolean | null;
  auto_merge: boolean | null;
  auto_publish_knowledge: boolean | null;
  require_human_approval: boolean | null;
  updated_at: string;
}

/** 合成规则单条说明（key 稳定，text 为后端给的人类可读文案，UI 原样渲染） */
export interface AiTeamPolicyRule {
  key: string;
  text: string;
}

/** 无任何层表态时的有效策略（由后端 compose 对空层实时计算，非手抄常量） */
export interface AiTeamPolicyDefaults {
  auto_accept: boolean;
  auto_merge: boolean;
  auto_publish_knowledge: boolean;
  require_human_approval: boolean;
  additional_checks: unknown[];
}

export interface AiTeamPolicyComposition {
  layer_order: string[];
  defaults: AiTeamPolicyDefaults;
  rules: AiTeamPolicyRule[];
}

export interface AiTeamExecutionPolicies {
  organization: AiTeamScopePolicy | null;
  project: AiTeamScopePolicy | null;
  taskTypes: AiTeamTaskTypePolicy[];
  taskOverrides: AiTeamTaskPolicyOverride[];
  composition: AiTeamPolicyComposition;
}

/** Execution Policies 四层现状 + 合成规则说明 */
export function getProjectAiExecutionPolicies(projectId: string) {
  return http.get<AiTeamExecutionPolicies>(
    `/v1/pm/projects/${projectId}/ai-team/execution-policies`
  );
}

/** 设置项目层 require_human_approval；null = 清除（恢复继承组织）。
 * 成功返回写后的项目层行（null = 已恢复继承）。 */
export function setProjectExecutionPolicy(
  projectId: string,
  requireHumanApproval: boolean | null
) {
  return http.put<AiTeamScopePolicy | null>(
    `/v1/pm/projects/${projectId}/ai-team/execution-policies/project`,
    { require_human_approval: requireHumanApproval }
  );
}

/** 任务覆盖写输入：null = 该字段恢复继承；至少一个非 null */
export interface TaskPolicyOverrideInput {
  require_human_approval?: boolean | null;
  auto_accept?: boolean | null;
  auto_merge?: boolean | null;
  auto_publish_knowledge?: boolean | null;
}

/** UPSERT 任务覆盖（至少一个非 null 字段；204 无响应体） */
export function setTaskPolicyOverride(
  taskId: string,
  input: TaskPolicyOverrideInput
) {
  return http.put<void>(`/v1/pm/tasks/${taskId}/policy-override`, input);
}

/** 删除任务覆盖（恢复全继承；行不存在为幂等成功；204 无响应体） */
export function clearTaskPolicyOverride(taskId: string) {
  return http.delete<void>(`/v1/pm/tasks/${taskId}/policy-override`);
}
