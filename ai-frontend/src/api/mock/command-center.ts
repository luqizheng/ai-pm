import type { CommandCenter } from "../command-center";

/**
 * 项目总控台聚合读模型的 dev mock（G-16）。
 *
 * pm-api 的 `GET /v1/pm/projects/{project_id}/command-center` 已实现
 * （apps/pm-api/src/command_center.rs:37-40），但 mock 层此前没有对应夹具，
 * 导致 VITE_USE_MOCK=1 下该端点 404、S01-01 页面无法自测。
 *
 * 本夹具按后端**真实派生规则**构造（crates/pm-application/src/command_center.rs:630-762）：
 *   repository_access = 有 pending 仓库 → in_progress
 *   code_analysis     = 最新作业 analyzing → in_progress
 *   project_cognition = 有会话 + 待确认发现 → waiting_for_human（G-01）
 *   dev_environment   = 设备从未自检 → undetermined
 *   human_actions     = 有待办 → waiting_for_human（G-01 七态）
 *   completion        = 无项目类型配置 → not_determinable
 *
 * 覆盖的展示形态：进行中 / 未开始 / 尚无法判断 / 待人工处理 /
 * AI 执行记录 + 待确认提案 + 风险 + 交接态。
 */

const AS_OF = "2026-11-04T09:12:00Z";

const PROJECT_ID = "prj-shield";

export const commandCenter: CommandCenter = {
  project_id: PROJECT_ID,
  as_of: AS_OF,
  blocks: {
    project_summary: {
      status: "ok",
      as_of: AS_OF,
      data: {
        project_id: PROJECT_ID,
        code: "SHIELD",
        name: "星河客服中台",
        lifecycle: "planning",
        archived: false,
        customer_name: "星河集团",
        goal: {
          present: false,
          pending_proposal: false
        },
        repository_count: 3,
        verified_repository_count: 2,
        member_count: 8,
        last_activity_at: "2026-11-04T09:08:00Z"
      }
    },
    initialization_progress: {
      status: "ok",
      as_of: AS_OF,
      data: {
        items: [
          {
            key: "project_basics",
            title: "项目基本信息",
            status: "completed",
            required: true,
            source_state: "planning"
          },
          {
            key: "project_goal",
            title: "项目目标",
            status: "not_started",
            required: true
          },
          {
            key: "repository_access",
            title: "仓库接入",
            status: "in_progress",
            required: false,
            detail: "已关联 3 个仓库，其中 2 个已验证"
          },
          {
            key: "code_analysis",
            title: "代码分析",
            status: "in_progress",
            required: false,
            source_state: "analyzing"
          },
          {
            key: "project_cognition",
            title: "项目认知",
            status: "waiting_for_human",
            required: false,
            detail: "共 2 个发现会话"
          },
          {
            key: "dev_environment",
            title: "开发环境",
            status: "undetermined",
            required: false,
            detail: "已绑定 1 台设备"
          },
          {
            key: "human_actions",
            title: "人工处理",
            status: "waiting_for_human",
            required: true,
            detail: "3 项待人工处理"
          }
        ],
        required_items: ["project_basics", "project_goal", "human_actions"],
        completion: "not_determinable",
        expanded: true
      }
    },
    work_summary: {
      status: "ok",
      as_of: AS_OF,
      data: {
        task_total: 0,
        task_by_status: {},
        requirement_total: 0,
        requirement_approved: 0,
        active_run_count: 0,
        active_runs: [],
        recent_runs: [],
        delivery: { delivered: 0, partial: 0, missing: 0, no_contract: 0 }
      }
    },
    human_action_summary: {
      status: "ok",
      as_of: AS_OF,
      data: {
        total: 3,
        items: [
          {
            kind: "finding_pending",
            domain: "discovery",
            id: "fnd-301",
            project_id: PROJECT_ID,
            title: "工单状态机与领域模型描述不一致",
            detail: "AI 从代码推断出两处状态定义差异，待人工裁决。",
            source_state: "PENDING",
            updated_at: "2026-11-04T09:05:00Z",
            action_hint: "查看发现并确认或拒绝",
            link: {
              kind: "discovery_finding",
              params: { finding_id: "fnd-301" }
            }
          },
          {
            kind: "proposal_pending",
            domain: "knowledge",
            id: "prop-77",
            project_id: PROJECT_ID,
            title: "项目架构概览提案待审批",
            detail: "AI 已生成架构概览文档提案，尚未提交为正式知识。",
            source_state: "redline_pending",
            updated_at: "2026-11-04T09:02:00Z",
            action_hint: "审阅红线差异并确认",
            link: {
              kind: "document_proposal",
              params: { proposal_id: "prop-77" }
            }
          },
          {
            kind: "finding_revision_requested",
            domain: "discovery",
            id: "fnd-288",
            project_id: PROJECT_ID,
            title: "认证中间件部署方式待补充信息",
            detail: "人类曾要求补充部署拓扑，尚未收到答复。",
            source_state: "PENDING",
            updated_at: "2026-11-03T18:20:00Z",
            action_hint: "补充缺失信息",
            link: {
              kind: "discovery_finding",
              params: { finding_id: "fnd-288" }
            }
          }
        ]
      }
    },
    risk_summary: {
      status: "ok",
      as_of: AS_OF,
      data: {
        total: 1,
        items: [
          {
            kind: "repository_unreachable",
            domain: "repository",
            source_id: "repo-infra",
            project_id: PROJECT_ID,
            title: "基础设施仓库最近核验失败",
            source_state: "unreachable",
            severity: "medium",
            occurred_at: "2026-11-04T08:40:00Z",
            suggestion: "检查 SSH Key 是否仍然有效，然后重新核验。",
            link: {
              kind: "repository",
              params: { repository_id: "repo-infra" }
            }
          }
        ]
      }
    },
    recent_activity: {
      status: "ok",
      as_of: AS_OF,
      data: {
        total: 3,
        items: [
          {
            id: "act-1",
            activity_type: "analysis_succeeded",
            source_type: "repo_import",
            source_id: "imp-9f1",
            occurred_at: "2026-11-04T09:08:00Z",
            actor: { kind: "ai_pm", name: "AI-PM" },
            summary: "AI-PM 完成前端仓库的首轮分析",
            dedup_key: "analysis-succeeded-imp-9f1"
          },
          {
            id: "act-2",
            activity_type: "repository_verified",
            source_type: "repository",
            source_id: "repo-backend",
            occurred_at: "2026-11-04T08:52:00Z",
            actor: { kind: "human", name: "林岸" },
            summary: "林岸确认了后端仓库的当前 Commit",
            dedup_key: "repo-verified-repo-backend"
          },
          {
            id: "act-3",
            activity_type: "analysis_failed",
            source_type: "repo_import",
            source_id: "imp-9e7",
            occurred_at: "2026-11-04T08:30:00Z",
            actor: { kind: "ai_pm", name: "AI-PM" },
            summary: "AI-PM 对基础设施仓库的首轮分析失败",
            dedup_key: "analysis-failed-imp-9e7"
          }
        ]
      }
    }
  }
};

/**
 * `GET /v1/pm/projects/{project_id}/command-center` 的 mock 响应。
 *
 * 只认 `/v1/pm/projects/<id>/command-center`；其余路径不拦截。
 */
export function resolveCommandCenterMock(
  method: string,
  pathname: string
): { status: number; body: unknown } | undefined {
  if (method !== "GET") return undefined;
  const matched = pathname.match(
    /^\/v1\/pm\/projects\/([^/]+)\/command-center$/
  );
  if (!matched) return undefined;

  // 任意项目 id 都返回同一份夹具；project_id 回显请求的项目。
  const projectId = decodeURIComponent(matched[1]);
  return {
    status: 200,
    body: { ...commandCenter, project_id: projectId }
  };
}
