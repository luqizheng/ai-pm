/**
 * 任务拆解演示数据（S04-UI-02，dev mock 专用）。
 *
 * AI 拆解接口尚未接线（crates/pm-ai 的 decompose 仅 worker 侧触发），
 * 本页以 fixtures 演示「生成 → 人工评审 → 批准」交互模型；提案只存在于
 * 本页会话，不写回后端。文件为 fixtures 豁免域（i18n 硬编码门禁豁免），
 * 仅由 mock-preview 条件注册的路由动态引入，不进发布 dist。
 */

export interface BreakdownBaselineFixture {
  name: string;
  version: string;
  status: string;
  scope: string;
  requirements: string[];
  designs: string[];
  adrs: string[];
}

export interface BreakdownTraceFixture {
  requirement: string;
  design: string;
  adr: string;
  baseline: string;
}

export interface BreakdownTaskFixture {
  code: string;
  parentId: string | null;
  featureId: string;
  title: string;
  priority: string;
  effort: string;
  repo: string;
  deps: number;
  why: string;
  trace: BreakdownTraceFixture;
  missing: string;
  overlap: string;
  subtaskSuggestions: string[];
  risks: string[];
  criteria: string[];
}

export interface BreakdownFeatureFixture {
  id: string;
  title: string;
}

export const breakdownBaseline: BreakdownBaselineFixture = {
  name: "BASE-v1.2 · 用户认证域",
  version: "v1.2",
  status: "approved",
  scope: "覆盖登录 API、令牌校验与前端登录流；约束见 ADR-007（会话安全基线）。",
  requirements: ["REQ-201 · 用户登录"],
  designs: ["DES-114 · 认证服务设计"],
  adrs: ["ADR-007 · 会话安全"]
};

export const breakdownFeatures: BreakdownFeatureFixture[] = [
  { id: "f-auth", title: "用户认证（User Authentication）" }
];

export function breakdownProposedTasks(): BreakdownTaskFixture[] {
  const trace: BreakdownTraceFixture = {
    requirement: "REQ-201 · 用户登录（approved v3）",
    design: "DES-114 §3 认证流程",
    adr: "ADR-007 · 会话安全",
    baseline: "BASE-v1.2"
  };
  return [
    {
      code: "T-001",
      parentId: null,
      featureId: "f-auth",
      title: "实现认证 API",
      priority: "high",
      effort: "3d",
      repo: "core-api",
      deps: 0,
      why: "DES-114 §3 要求由后端统一签发与校验令牌，前端不得自行解析会话。",
      trace,
      missing: "未定义过期令牌的刷新策略",
      overlap: "与既有 T-010（会话中间件）存在边界重叠，需在执行说明中划清",
      subtaskSuggestions: ["请求模型定义", "登录端点", "令牌校验"],
      risks: ["密钥轮换期间的双写窗口"],
      criteria: [
        "给定有效凭据，登录返回 200 与访问令牌",
        "无效凭据返回 401 且不泄露原因细节"
      ]
    },
    {
      code: "T-001.1",
      parentId: "T-001",
      featureId: "f-auth",
      title: "定义认证请求模型",
      priority: "normal",
      effort: "0.5d",
      repo: "core-api",
      deps: 0,
      why: "T-001 的前置：先冻结请求/响应契约再实现端点。",
      trace,
      missing: "缺少字段级校验规则",
      overlap: "无",
      subtaskSuggestions: [],
      risks: [],
      criteria: ["请求模型含字段校验规则并通过单元测试"]
    },
    {
      code: "T-001.2",
      parentId: "T-001",
      featureId: "f-auth",
      title: "实现登录端点",
      priority: "high",
      effort: "1.5d",
      repo: "core-api",
      deps: 1,
      why: "REQ-201 的核心服务端行为。",
      trace,
      missing: "无",
      overlap: "无",
      subtaskSuggestions: [],
      risks: ["登录限流策略归属未定（网关 or 服务内）"],
      criteria: ["连续 5 次失败触发限流", "成功登录写入审计日志"]
    },
    {
      code: "T-001.3",
      parentId: "T-001",
      featureId: "f-auth",
      title: "实现令牌校验",
      priority: "urgent",
      effort: "1d",
      repo: "core-api",
      deps: 1,
      why: "ADR-007 要求所有受保护端点统一走令牌校验中间件。",
      trace,
      missing: "无",
      overlap: "无",
      subtaskSuggestions: [],
      risks: [],
      criteria: ["过期令牌返回 401", "篡改签名的令牌被拒绝并告警"]
    },
    {
      code: "T-002",
      parentId: null,
      featureId: "f-auth",
      title: "实现前端登录流",
      priority: "high",
      effort: "2d",
      repo: "web-app",
      deps: 1,
      why: "REQ-201 的用户界面侧闭环。",
      trace,
      missing: "无",
      overlap: "无",
      subtaskSuggestions: ["登录表单", "接入认证 API"],
      risks: [],
      criteria: ["登录成功进入工作台", "失败提示不泄露服务端细节"]
    },
    {
      code: "T-002.1",
      parentId: "T-002",
      featureId: "f-auth",
      title: "构建登录表单",
      priority: "normal",
      effort: "1d",
      repo: "web-app",
      deps: 0,
      why: "T-002 的界面基础。",
      trace,
      missing: "无",
      overlap: "无",
      subtaskSuggestions: [],
      risks: [],
      criteria: ["表单校验先行于提交", "键盘可达（Tab 顺序完整）"]
    },
    {
      code: "T-002.2",
      parentId: "T-002",
      featureId: "f-auth",
      title: "接入认证 API",
      priority: "high",
      effort: "1d",
      repo: "web-app",
      deps: 2,
      why: "对接 T-001.2 的登录端点。",
      trace,
      missing: "无",
      overlap: "无",
      subtaskSuggestions: [],
      risks: [],
      criteria: ["令牌存取符合 ADR-007 的会话安全约束"]
    },
    {
      code: "T-003",
      parentId: null,
      featureId: "f-auth",
      title: "补充认证测试",
      priority: "normal",
      effort: "1.5d",
      repo: "core-api",
      deps: 3,
      why: "基线验收要求认证域具备单元与集成测试。",
      trace,
      missing: "缺少集成测试环境约定",
      overlap: "无",
      subtaskSuggestions: ["API 单元测试", "认证集成测试"],
      risks: [],
      criteria: ["核心路径覆盖率 ≥ 80%", "集成测试覆盖登录→受保护端点全链路"]
    }
  ];
}
