/**
 * 任务详情演示数据（S04-UI-03，dev mock 专用）。
 *
 * 后端无 GET 单任务读端点（goal/constraints/deliverables/criteria 仅
 * PATCH 可写、REST 不可读），本页以 fixtures 演示 S04-UI-03 的完整
 * 信息模型与审批交互；所有动作只改本页本地状态，不回写后端。
 * fixtures 豁免域（i18n 硬编码门禁豁免），仅 mock-preview 路由引入。
 */

export interface DetailTraceFixture {
  kind: string;
  label: string;
  version: string;
  status: string;
}

export interface DetailDependencyFixture {
  code: string;
  title: string;
  status: string;
  direction: "upstream" | "downstream";
}

export interface DetailCriterionFixture {
  id: string;
  description: string;
  method: string;
  evidence: string;
  mandatory: boolean;
}

export interface DetailHistoryFixture {
  id: string;
  actor: "ai" | "human";
  action: string;
  detail: string;
  at: string;
}

export interface DetailSuggestionFixture {
  id: string;
  section: string;
  text: string;
}

export interface TaskDetailFixture {
  id: string;
  title: string;
  status: string;
  priority: string;
  approval: "human_confirmed" | "none";
  objective: string;
  background: string;
  scope: string;
  outOfScope: string;
  expected: string;
  traces: DetailTraceFixture[];
  dependencies: DetailDependencyFixture[];
  repo: string;
  module: string;
  instructions: string;
  constraints: string;
  context: string;
  criteria: DetailCriterionFixture[];
  requiredDeliverables: string[];
  optionalDeliverables: string[];
  history: DetailHistoryFixture[];
  suggestions: DetailSuggestionFixture[];
}

export const taskDetailFixture: TaskDetailFixture = {
  id: "11111111-1111-1111-1111-111111111111",
  title: "实现登录端点",
  status: "draft",
  priority: "high",
  approval: "none",
  objective:
    "在 core-api 提供POST /v1/auth/login 端点：校验凭据、签发访问令牌并写入审计日志。",
  background:
    "REQ-201 定稿后，认证服务进入实施；DES-114 §3 要求令牌统一由后端签发，前端不解析会话。",
  scope: "登录端点、凭据校验、令牌签发、失败限流计数。",
  outOfScope: "注册流程、第三方 OAuth、多因素认证。",
  expected: "有效凭据返回 200 与访问令牌；无效凭据返回 401 且不泄露原因细节。",
  traces: [
    {
      kind: "requirement",
      label: "REQ-201 · 用户登录",
      version: "v3",
      status: "approved"
    },
    {
      kind: "design",
      label: "DES-114 · 认证服务设计 §3",
      version: "v1",
      status: "approved"
    },
    {
      kind: "adr",
      label: "ADR-007 · 会话安全",
      version: "v1",
      status: "accepted"
    },
    {
      kind: "baseline",
      label: "BASE · 实施基线",
      version: "v1.2",
      status: "approved"
    }
  ],
  dependencies: [
    {
      code: "T-001.1",
      title: "定义认证请求模型",
      status: "draft",
      direction: "upstream"
    },
    {
      code: "T-003",
      title: "补充认证测试",
      status: "draft",
      direction: "downstream"
    }
  ],
  repo: "core-api",
  module: "services/auth",
  instructions:
    "按 DES-114 §3 实现：1) 复用 T-001.1 的请求模型；2) 凭据校验走既有 identity 服务；3) 令牌签发遵循 ADR-007 的密钥约定；4) 失败计数复用 redis 限流器。",
  constraints: "不得在日志记录明文凭据；所有错误响应复用全局错误契约。",
  context: "DES-114 全文、ADR-007、identity 服务 README。",
  criteria: [
    {
      id: "AC-1",
      description: "给定有效凭据，登录返回 200 与访问令牌",
      method: "automated_test",
      evidence: "集成测试断言响应结构与状态码",
      mandatory: true
    },
    {
      id: "AC-2",
      description: "无效凭据返回 401 且不泄露原因细节",
      method: "manual",
      evidence: "评审记录 + 手工验证截图",
      mandatory: true
    },
    {
      id: "AC-3",
      description: "连续 5 次失败触发限流",
      method: "automated_test",
      evidence: "限流集成测试",
      mandatory: false
    }
  ],
  requiredDeliverables: ["代码变更（core-api）", "单元测试", "集成测试"],
  optionalDeliverables: ["接口文档更新"],
  history: [
    {
      id: "h1",
      actor: "ai",
      action: "创建任务",
      detail: "由实施基线 BASE-v1.2 拆解生成（提案评审通过）",
      at: "2026-11-09T10:00:00Z"
    },
    {
      id: "h2",
      actor: "human",
      action: "更新验收标准",
      detail: "补充 AC-2 的证据要求",
      at: "2026-11-09T14:30:00Z"
    },
    {
      id: "h3",
      actor: "human",
      action: "状态变更",
      detail: "draft → draft（重新打开）",
      at: "2026-11-10T08:00:00Z"
    }
  ],
  suggestions: [
    {
      id: "s1",
      section: "s04.tasks.detail.sectionCriteria",
      text: "验收标准未定义过期令牌的处理方式，建议补充「过期令牌返回 401 并触发刷新提示」。"
    },
    {
      id: "s2",
      section: "s04.tasks.detail.sectionSpec",
      text: "执行说明缺少限流阈值来源，建议引用网关限流策略的版本号。"
    }
  ]
};
