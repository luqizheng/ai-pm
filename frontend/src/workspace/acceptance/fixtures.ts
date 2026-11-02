/**
 * 交付验收工作台（T-070）演示数据 —— 与 workspace/fixtures.ts 同一家族的（放在 acceptance/ 子目录，文件名保持 fixtures.ts 以命中 i18n 硬编码门禁的演示数据豁免）
 * 浏览器侧 mock 数据（由 fixtures.ts 末尾 re-export，测试也从那里取）。
 *
 * 数据形状与真后端完全同构：/v1/pm/acceptance-overview、
 * /tasks/{id}/review-findings、/tasks/{id}/criterion-verdicts、
 * /tasks/{id}/review-approvals、/tasks/{id}/delivery（snake_case）。
 * 徽标 / 动作区状态一律消费「后端字段」，页面零推导（Q1/Q2/Q3）。
 * `project_id` 必须等于 demoProjectId（fixtures.test.ts 钉住一致性）。
 */
import type { AcceptanceOverview, TaskDeliveryRow } from "@/api/acceptance";
import type { TaskRun } from "@/api/execution";
import type {
  CriterionVerdict,
  ReviewApproval,
  ReviewFinding
} from "@/api/review";

/** 单个待验收任务的全部证据（mock 分支按 task_id 取用）。 */
export interface AcceptanceDetailFixture {
  delivery: TaskDeliveryRow[];
  findings: ReviewFinding[];
  verdicts: CriterionVerdict[];
  approvals: ReviewApproval[];
  /** 该任务的 task_run（delivery / review 分轨），对应 /task-runs?task_id=。 */
  runs: TaskRun[];
}

/** 构造一条 task_run 行（与 /v1/pm/task-runs 列表项同构）。 */
function run(
  id: string,
  taskId: string,
  taskTitle: string,
  attemptNo: number,
  runKind: "delivery" | "review",
  status: string,
  extra: Partial<TaskRun> = {}
): TaskRun {
  return {
    id,
    task_id: taskId,
    task_title: taskTitle,
    attempt_no: attemptNo,
    run_kind: runKind,
    status,
    outcome: status === "succeeded" || status === "failed" ? status : null,
    agent_id: null,
    executor_kind: runKind === "review" ? "platform" : "codex",
    executor_session: runKind === "review" ? null : `sess-${id}`,
    lease_expires_at: null,
    created_at: "2026-11-01T08:00:00Z",
    started_at: "2026-11-01T08:01:00Z",
    finished_at: status === "running" ? null : "2026-11-01T08:20:00Z",
    failure_category: null,
    failure_reason: null,
    ...extra
  };
}

export const acceptanceOverview: AcceptanceOverview = {
  project_id: "proj-worldmap",
  counts: { pending: 5, eligible: 1, blocked: 2 },
  total: 5,
  tasks: [
    {
      task_id: "task-forest-v02",
      title: "森林分布逻辑 v0.2 实现",
      status: "reviewing",
      badge: "blocked",
      delivery_summary: {
        expected_repos: 2,
        delivered_repos: 2,
        missing_repos: [],
        integration_status: "passed"
      },
      blocker_count: 1,
      latest_delivery_run_id: "run-forest-d1",
      latest_attempt_no: 1,
      review_status: "awaiting_decision",
      pending_approval_id: "appr-forest-1",
      created_at: "2026-11-01T09:30:00Z"
    },
    {
      task_id: "task-river-perf",
      title: "河流生成性能回归",
      status: "submitted",
      badge: "blocked",
      delivery_summary: {
        expected_repos: 1,
        delivered_repos: 1,
        missing_repos: [],
        integration_status: "failed"
      },
      blocker_count: 0,
      latest_delivery_run_id: "run-river-d2",
      latest_attempt_no: 2,
      review_status: "none",
      pending_approval_id: null,
      created_at: "2026-11-01T08:10:00Z"
    },
    {
      task_id: "task-biome-spec",
      title: "生物群系规格起草",
      status: "submitted",
      badge: "missing",
      delivery_summary: {
        expected_repos: 0,
        delivered_repos: 0,
        missing_repos: [],
        integration_status: null
      },
      blocker_count: 0,
      latest_delivery_run_id: null,
      latest_attempt_no: null,
      review_status: "none",
      pending_approval_id: null,
      created_at: "2026-11-01T07:45:00Z"
    },
    {
      task_id: "task-cloud-docs",
      title: "云层文档同步",
      status: "submitted",
      badge: "missing",
      delivery_summary: {
        expected_repos: 2,
        delivered_repos: 1,
        missing_repos: ["shared-contracts"],
        integration_status: "passed"
      },
      blocker_count: 0,
      latest_delivery_run_id: "run-cloud-d1",
      latest_attempt_no: 1,
      review_status: "running",
      pending_approval_id: null,
      created_at: "2026-11-01T06:20:00Z"
    },
    {
      task_id: "task-terrain-shader",
      title: "地形渲染架构评审",
      status: "reviewing",
      badge: "eligible",
      delivery_summary: {
        expected_repos: 2,
        delivered_repos: 2,
        missing_repos: [],
        integration_status: "passed"
      },
      blocker_count: 0,
      latest_delivery_run_id: "run-terrain-d3",
      latest_attempt_no: 3,
      review_status: "awaiting_decision",
      pending_approval_id: "appr-terrain-1",
      created_at: "2026-11-01T05:05:00Z"
    }
  ]
};

const forestDelivery: TaskDeliveryRow[] = [
  {
    task_id: "task-forest-v02",
    project_id: "proj-worldmap",
    repository_id: "repo-gen",
    repository_name: "worldmap-gen",
    write_access: true,
    delivery_required: true,
    latest_run_id: "run-forest-d1",
    latest_attempt_no: 1,
    integration_status: "passed",
    latest_submitted_run_id: "run-forest-d1",
    base_commit: "b19c4e7",
    commit_sha: "9f2e1ab",
    changed_files: 12,
    test_evidence: {
      submitted_at: "2026-11-01T09:12:00Z",
      tests: [{ name: "distribute::plain", passed: true }]
    },
    branch: "review/forest-v02",
    commit_author: "forest-agent",
    commit_authored_at: "2026-11-01T09:12:00Z",
    expected_delivery_count: 2,
    actual_delivery_count: 2,
    delivery_state: "delivered"
  },
  {
    task_id: "task-forest-v02",
    project_id: "proj-worldmap",
    repository_id: "repo-contracts",
    repository_name: "shared-contracts",
    write_access: true,
    delivery_required: true,
    latest_run_id: "run-forest-d1",
    latest_attempt_no: 1,
    integration_status: "passed",
    latest_submitted_run_id: "run-forest-d1",
    base_commit: "77aa90c",
    commit_sha: "5d6f0c2",
    changed_files: 3,
    test_evidence: {},
    branch: "review/forest-v02",
    commit_author: "forest-agent",
    commit_authored_at: "2026-11-01T09:11:00Z",
    expected_delivery_count: 2,
    actual_delivery_count: 2,
    delivery_state: "delivered"
  }
];

const forestFindings: ReviewFinding[] = [
  {
    id: "find-1",
    review_run_id: "run-forest-r1",
    review_attempt_no: 1,
    review_run_status: "succeeded",
    criterion_id: null,
    kind: "contract_violation",
    severity: "blocker",
    code_location: "worldmap-gen/src/forest.rs:88-102",
    message: "山地用例海拔权重未接入，与验收标准 #1 冲突",
    evidence_ref: "run-forest-r1/logs/blocker-1",
    created_at: "2026-11-01T09:20:00Z"
  },
  {
    id: "find-2",
    review_run_id: "run-forest-r1",
    review_attempt_no: 1,
    review_run_status: "succeeded",
    criterion_id: null,
    kind: "style",
    severity: "warning",
    code_location: "worldmap-gen/src/forest.rs:14",
    message: "规则源命名与 docs 规范不一致",
    evidence_ref: null,
    created_at: "2026-11-01T09:20:05Z"
  }
];

const forestVerdicts: CriterionVerdict[] = [
  {
    criterion_id: "crit-forest-1",
    ordinal: 1,
    text: "10km² 森林生成时间 ≤ 3s",
    verification_kind: "automated_test",
    evaluated: true,
    review_run_id: "run-forest-r1",
    review_attempt_no: 1,
    verdict: "failed",
    evidence: { reason: "mountain case oversubscribed" },
    decided_by: null,
    decided_at: null,
    verdict_created_at: "2026-11-01T09:19:00Z"
  },
  {
    criterion_id: "crit-forest-2",
    ordinal: 2,
    text: "人工确认平原视觉样张",
    verification_kind: "manual",
    evaluated: false,
    review_run_id: null,
    review_attempt_no: null,
    verdict: null,
    evidence: null,
    decided_by: null,
    decided_at: null,
    verdict_created_at: null
  }
];

const forestApprovals: ReviewApproval[] = [
  {
    id: "appr-forest-1",
    task_id: "task-forest-v02",
    review_run_id: "run-forest-r1",
    delivery_run_id: "run-forest-d1",
    kind: "manual_review",
    status: "pending",
    decided_by: null,
    decision_reason: null,
    decided_at: null,
    created_at: "2026-11-01T09:22:00Z",
    task_title: "森林分布逻辑 v0.2 实现"
  }
];

const terrainApprovals: ReviewApproval[] = [
  {
    id: "appr-terrain-1",
    task_id: "task-terrain-shader",
    review_run_id: "run-terrain-r2",
    delivery_run_id: "run-terrain-d3",
    kind: "manual_review",
    status: "pending",
    decided_by: null,
    decision_reason: null,
    decided_at: null,
    created_at: "2026-11-01T07:58:00Z",
    task_title: "地形渲染架构评审"
  },
  {
    id: "appr-terrain-0",
    task_id: "task-terrain-shader",
    review_run_id: "run-terrain-r1",
    delivery_run_id: "run-terrain-d2",
    kind: "review_degraded",
    status: "rejected",
    decided_by: "user-leo",
    decision_reason: "GPU 方案证据不足，退回补性能对照",
    decided_at: "2026-11-01T06:40:00Z",
    created_at: "2026-11-01T06:30:00Z",
    task_title: "地形渲染架构评审"
  }
];

/** mock 分支的任务详情表（真后端按 task_id 并发拉四个端点）。 */
export const acceptanceDetails: Record<string, AcceptanceDetailFixture> = {
  "task-forest-v02": {
    delivery: forestDelivery,
    findings: forestFindings,
    verdicts: forestVerdicts,
    approvals: forestApprovals,
    runs: [
      run(
        "run-forest-r1",
        "task-forest-v02",
        "森林分布逻辑 v0.2 实现",
        1,
        "review",
        "succeeded"
      ),
      run(
        "run-forest-d1",
        "task-forest-v02",
        "森林分布逻辑 v0.2 实现",
        1,
        "delivery",
        "succeeded",
        {
          executor_session: "sess-forest-v02",
          lease_expires_at: "2026-11-01T09:45:00Z"
        }
      )
    ]
  },
  "task-river-perf": {
    delivery: [
      {
        task_id: "task-river-perf",
        project_id: "proj-worldmap",
        repository_id: "repo-gen",
        repository_name: "worldmap-gen",
        write_access: true,
        delivery_required: true,
        latest_run_id: "run-river-d2",
        latest_attempt_no: 2,
        integration_status: "failed",
        latest_submitted_run_id: "run-river-d2",
        base_commit: "b19c4e7",
        commit_sha: "c8811fe",
        changed_files: 5,
        test_evidence: {
          submitted_at: "2026-11-01T08:02:00Z",
          tests: [{ name: "perf::river_10km2", passed: false }]
        },
        branch: "review/river-perf",
        commit_author: "river-agent",
        commit_authored_at: "2026-11-01T08:02:00Z",
        expected_delivery_count: 1,
        actual_delivery_count: 1,
        delivery_state: "delivered"
      }
    ],
    findings: [],
    verdicts: [],
    approvals: [],
    runs: [
      run(
        "run-river-d2",
        "task-river-perf",
        "河流生成性能回归",
        2,
        "delivery",
        "failed",
        {
          failure_category: "validation",
          failure_reason: "perf::river_10km2 超时"
        }
      )
    ]
  },
  "task-biome-spec": {
    delivery: [],
    findings: [],
    verdicts: [],
    approvals: [],
    runs: []
  },
  "task-cloud-docs": {
    delivery: [
      {
        task_id: "task-cloud-docs",
        project_id: "proj-worldmap",
        repository_id: "repo-docs",
        repository_name: "worldmap-docs",
        write_access: false,
        delivery_required: true,
        latest_run_id: "run-cloud-d1",
        latest_attempt_no: 1,
        integration_status: "passed",
        latest_submitted_run_id: "run-cloud-d1",
        base_commit: "31fac8d",
        commit_sha: "a10b2c3",
        changed_files: 9,
        test_evidence: {},
        branch: "review/cloud-docs",
        commit_author: "docs-agent",
        commit_authored_at: "2026-11-01T06:15:00Z",
        expected_delivery_count: 2,
        actual_delivery_count: 1,
        delivery_state: "partial"
      },
      {
        task_id: "task-cloud-docs",
        project_id: "proj-worldmap",
        repository_id: "repo-contracts",
        repository_name: "shared-contracts",
        write_access: true,
        delivery_required: true,
        latest_run_id: "run-cloud-d1",
        latest_attempt_no: 1,
        integration_status: "passed",
        latest_submitted_run_id: "run-cloud-d1",
        base_commit: "77aa90c",
        commit_sha: null,
        changed_files: null,
        test_evidence: {},
        branch: null,
        commit_author: null,
        commit_authored_at: null,
        expected_delivery_count: 2,
        actual_delivery_count: 1,
        delivery_state: "partial"
      }
    ],
    findings: [],
    verdicts: [],
    approvals: [],
    runs: [
      run(
        "run-cloud-d1",
        "task-cloud-docs",
        "云层文档同步",
        1,
        "delivery",
        "succeeded"
      ),
      run(
        "run-cloud-r1",
        "task-cloud-docs",
        "云层文档同步",
        1,
        "review",
        "running"
      )
    ]
  },
  "task-terrain-shader": {
    delivery: [
      {
        task_id: "task-terrain-shader",
        project_id: "proj-worldmap",
        repository_id: "repo-gen",
        repository_name: "worldmap-gen",
        write_access: true,
        delivery_required: true,
        latest_run_id: "run-terrain-d3",
        latest_attempt_no: 3,
        integration_status: "passed",
        latest_submitted_run_id: "run-terrain-d3",
        base_commit: "9c02bb1",
        commit_sha: "e44f7a9",
        changed_files: 21,
        test_evidence: {
          submitted_at: "2026-11-01T07:50:00Z",
          tests: [{ name: "shader::gpu_bench", passed: true }]
        },
        branch: "review/terrain-shader",
        commit_author: "terrain-agent",
        commit_authored_at: "2026-11-01T07:50:00Z",
        expected_delivery_count: 2,
        actual_delivery_count: 2,
        delivery_state: "delivered"
      },
      {
        task_id: "task-terrain-shader",
        project_id: "proj-worldmap",
        repository_id: "repo-contracts",
        repository_name: "shared-contracts",
        write_access: true,
        delivery_required: true,
        latest_run_id: "run-terrain-d3",
        latest_attempt_no: 3,
        integration_status: "passed",
        latest_submitted_run_id: "run-terrain-d3",
        base_commit: "77aa90c",
        commit_sha: "0d9e8f1",
        changed_files: 2,
        test_evidence: {},
        branch: "review/terrain-shader",
        commit_author: "terrain-agent",
        commit_authored_at: "2026-11-01T07:49:00Z",
        expected_delivery_count: 2,
        actual_delivery_count: 2,
        delivery_state: "delivered"
      }
    ],
    findings: [],
    verdicts: [
      {
        criterion_id: "crit-terrain-1",
        ordinal: 1,
        text: "GPU 原型帧耗 ≤ 4ms（1080p）",
        verification_kind: "automated_test",
        evaluated: true,
        review_run_id: "run-terrain-r2",
        review_attempt_no: 2,
        verdict: "passed",
        evidence: { bench: "3.2ms p50" },
        decided_by: null,
        decided_at: null,
        verdict_created_at: "2026-11-01T07:55:00Z"
      },
      {
        criterion_id: "crit-terrain-2",
        ordinal: 2,
        text: "CPU 预计算对照报告归档",
        verification_kind: "artifact_exists",
        evaluated: true,
        review_run_id: "run-terrain-r2",
        review_attempt_no: 2,
        verdict: "awaiting_manual",
        evidence: {},
        decided_by: null,
        decided_at: null,
        verdict_created_at: "2026-11-01T07:55:10Z"
      }
    ],
    approvals: terrainApprovals,
    runs: [
      run(
        "run-terrain-r2",
        "task-terrain-shader",
        "地形渲染架构评审",
        2,
        "review",
        "succeeded"
      ),
      run(
        "run-terrain-d3",
        "task-terrain-shader",
        "地形渲染架构评审",
        3,
        "delivery",
        "succeeded",
        {
          executor_session: "sess-terrain-d3"
        }
      )
    ]
  }
};
