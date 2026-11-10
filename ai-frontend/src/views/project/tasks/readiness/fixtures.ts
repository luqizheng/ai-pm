/**
 * 执行准备演示数据（S04-UI-07，dev mock 专用）。
 *
 * 后端无准备度聚合与 S05 移交端点（仅 mark-ready 单点闸门），本目录
 * fixtures 演示质量门交互：检查表 / 问题处置（解决或留痕豁免）/ 移交
 * 确认。仅由 mock-preview 条件注册的路由动态引入，不进发布 dist。
 * fixtures 豁免域（i18n 硬编码门禁豁免）。
 */

export type ReadinessState = "ready" | "attention" | "blocked";
export type IssueState = "open" | "resolved" | "waived";
export type IssueSeverity = "high" | "medium" | "low";

export interface ReadinessPlanFixture {
  version: string;
  included: number;
  excluded: number;
  ready: number;
  attention: number;
  blocked: number;
}

export interface ReadinessIssueFixture {
  id: string;
  severity: IssueSeverity;
  code: string;
  reason: string;
  resolution: string;
  /** 映射到检查表条目（a1..f2） */
  checklistKey: string;
  /** 该问题未决时把检查表条目打成 block（否则 warn） */
  blocking: boolean;
}

export interface ReadinessTaskFixture {
  code: string;
  title: string;
  priority: string;
  state: ReadinessState;
  missing: string[];
  repo: string | null;
  deps: number;
}

export interface ReadinessChecklistItemFixture {
  key: string;
  /** 无未决映射问题时的基础态 */
  baseState: "pass" | "na";
  issueIds: string[];
  category: string;
}

export const readinessPlan: ReadinessPlanFixture = {
  version: "v3",
  included: 34,
  excluded: 2,
  ready: 22,
  attention: 8,
  blocked: 4
};

export const readinessIssuesFixture = (): ReadinessIssueFixture[] => [
  {
    id: "I-1",
    severity: "high",
    code: "T-008",
    reason: "任务没有分配目标仓库，无法生成执行上下文。",
    resolution: "在任务详情绑定已验证仓库（gateway）。",
    checklistKey: "d1",
    blocking: true
  },
  {
    id: "I-2",
    severity: "high",
    code: "T-009",
    reason: "验收标准缺失，无法独立判定交付是否合格。",
    resolution: "在任务详情补充可独立验证的验收标准。",
    checklistKey: "e1",
    blocking: false
  },
  {
    id: "I-3",
    severity: "medium",
    code: "T-011",
    reason: "前置任务 T-007 处于阻塞状态（等待 ADR-007 修订裁决）。",
    resolution: "先解决 T-007 的上游裁决，或将本任务改期。",
    checklistKey: "c2",
    blocking: true
  },
  {
    id: "I-4",
    severity: "medium",
    code: "T-015",
    reason: "引用的技术设计修订已不是当前批准的实施基线版本。",
    resolution: "在任务详情把追溯引用更新到 BASE-v1.2。",
    checklistKey: "b3",
    blocking: false
  },
  {
    id: "I-5",
    severity: "low",
    code: "T-010",
    reason: "交付物清单为空（未声明代码/测试/文档要求）。",
    resolution: "在任务详情定义必交付与可选交付物。",
    checklistKey: "e2",
    blocking: false
  }
];

export const readinessTasksFixture = (): ReadinessTaskFixture[] => [
  {
    code: "T-002",
    title: "实现令牌校验",
    priority: "urgent",
    state: "ready",
    missing: [],
    repo: "core-api",
    deps: 1
  },
  {
    code: "T-003",
    title: "实现登录 UI",
    priority: "high",
    state: "ready",
    missing: [],
    repo: "web-app",
    deps: 0
  },
  {
    code: "T-005",
    title: "实现用户档案 API",
    priority: "high",
    state: "ready",
    missing: [],
    repo: "core-api",
    deps: 1
  },
  {
    code: "T-008",
    title: "登录限流策略",
    priority: "high",
    state: "attention",
    missing: ["repository"],
    repo: null,
    deps: 1
  },
  {
    code: "T-009",
    title: "认证集成测试",
    priority: "high",
    state: "attention",
    missing: ["acceptance criteria"],
    repo: "core-api",
    deps: 2
  },
  {
    code: "T-015",
    title: "网关路由调整",
    priority: "normal",
    state: "attention",
    missing: ["baseline version"],
    repo: "gateway",
    deps: 1
  },
  {
    code: "T-010",
    title: "审计埋点",
    priority: "low",
    state: "attention",
    missing: ["deliverables"],
    repo: "core-api",
    deps: 0
  },
  {
    code: "T-011",
    title: "密钥轮换脚本",
    priority: "high",
    state: "blocked",
    missing: ["upstream T-007"],
    repo: "core-api",
    deps: 2
  }
];

/** 检查表：条目 key 对应文案 s04.tasks.readiness.<key> */
export const readinessChecklistFixture: ReadinessChecklistItemFixture[] = [
  { key: "a1", baseState: "pass", issueIds: [], category: "catDefinition" },
  { key: "a2", baseState: "pass", issueIds: [], category: "catDefinition" },
  { key: "a3", baseState: "pass", issueIds: [], category: "catDefinition" },
  { key: "b1", baseState: "pass", issueIds: [], category: "catTrace" },
  { key: "b2", baseState: "pass", issueIds: [], category: "catTrace" },
  { key: "b3", baseState: "pass", issueIds: ["I-4"], category: "catTrace" },
  { key: "c1", baseState: "pass", issueIds: [], category: "catDependency" },
  {
    key: "c2",
    baseState: "pass",
    issueIds: ["I-3"],
    category: "catDependency"
  },
  { key: "d1", baseState: "na", issueIds: ["I-1"], category: "catPreparation" },
  { key: "d2", baseState: "pass", issueIds: [], category: "catPreparation" },
  {
    key: "e1",
    baseState: "pass",
    issueIds: ["I-2"],
    category: "catAcceptance"
  },
  { key: "e2", baseState: "na", issueIds: ["I-5"], category: "catAcceptance" },
  { key: "f1", baseState: "pass", issueIds: [], category: "catApproval" },
  { key: "f2", baseState: "pass", issueIds: [], category: "catApproval" }
];

export interface ReadinessRecFixture {
  id: string;
  text: string;
  issueId?: string;
}

export function readinessRecsFixture(): ReadinessRecFixture[] {
  return [
    {
      id: "RA-1",
      text: "三个任务的验收标准不完整（T-002 缺过期令牌分支、T-009 全缺、T-003 缺键盘可达项）。",
      issueId: "I-2"
    },
    {
      id: "RA-2",
      text: "两个任务引用的技术设计修订已不是当前批准的实施基线（T-014、T-015）。",
      issueId: "I-4"
    },
    {
      id: "RA-3",
      text: "T-011 的前置 T-007 处于阻塞状态，移交前无法完成工程准备。",
      issueId: "I-3"
    }
  ];
}
