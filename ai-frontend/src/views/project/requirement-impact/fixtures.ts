/**
 * 变更影响分析演示数据（mock 先行，S02-UI-07）。
 *
 * 本文件为 fixtures（i18n 硬编码门禁豁免）。**红线：不虚构依赖**——
 * 影响对象全部锚定 `requirements/fixtures.ts` 中 REQ-001 的真实关系：
 *   - 验收标准快照：REQ-001 v1（1 条）→ v2（3 条，真实重写）；
 *   - TASK-210：REQ-001 related 中的真实任务（实现登录页与会话管理）；
 *   - KB-AUTH-012：REQ-001 sources 引用的知识文档（v2 策略引自该方案）；
 *   - BL-002：基线页 fixtures 中当前生效基线（**未含 REQ-001**，真实）；
 *   - M-01：无正式里程碑数据 → uncertain（待确认），不臆造关联。
 * 分析对象 = REQ-001 v2（pending_confirmation 的拟议修订）。
 */

import type {
  ChangeImpactAnalysis,
  ImpactFinding,
  ImpactTarget
} from "./impact-contract";

/** 影响对象（依赖图节点；逐条可溯） */
export const impactTargetsFixture: ImpactTarget[] = [
  {
    id: "tgt-ac-001",
    kind: "acceptance_criteria",
    code: "AC-001",
    title: "验收标准（3 条重写）",
    level: "direct",
    reason:
      "拟议修订把验收标准从 1 条重写为 3 条：新增登录失败锁定（5 次 / 15 分钟）与会话空闲超时（30 分钟）两条可测标准。"
  },
  {
    id: "tgt-task-210",
    kind: "task",
    code: "TASK-210",
    title: "实现登录页与会话管理",
    level: "direct",
    reason:
      "REQ-001 关联任务（来自需求关联项）。锁定与超时策略属于该任务的实现范围，需按 v2 扩展。"
  },
  {
    id: "tgt-kb-auth-012",
    kind: "knowledge_doc",
    code: "KB-AUTH-012",
    title: "认证与会话安全方案",
    level: "indirect",
    reason:
      "REQ-001 来源引用的知识文档。v2 的锁定阈值与超时口径引自该方案，方案需按定稿结论复核同步。"
  },
  {
    id: "tgt-baseline-bl-002",
    kind: "baseline",
    code: "BL-002",
    title: "当前生效基线",
    level: "potential",
    reason:
      "当前基线未含入 REQ-001（v1 曾在 BL-001，v2 待确认后暂缓含入）。变更确认且需求定稿后，建议纳入下一基线。"
  },
  {
    id: "tgt-milestone-m-01",
    kind: "milestone",
    code: "M-01",
    title: "平台基础能力可用（里程碑）",
    level: "uncertain",
    reason:
      "项目尚无正式里程碑记录与登录能力的明确绑定；是否存在里程碑级影响待确认，不臆造关联。"
  }
];

/** 分类影响发现（§5 A-D） */
export const impactFindingsFixture: ImpactFinding[] = [
  {
    id: "fnd-req-1",
    category: "requirement",
    level: "direct",
    body: "验收标准重写：AC-001 由 1 条扩展为 3 条，旧标准（仅登录成功）失效，需按新标准验收。",
    related: "AC-001"
  },
  {
    id: "fnd-req-2",
    category: "requirement",
    level: "uncertain",
    body: "未发现直接关联需求受影响：REQ-001 无需求层关联项，无跨需求依赖扩散。"
  },
  {
    id: "fnd-impl-1",
    category: "implementation",
    level: "direct",
    body: "TASK-210（实现登录页与会话管理）范围需覆盖登录失败锁定与会话超时的实现与测试。",
    related: "TASK-210"
  },
  {
    id: "fnd-impl-2",
    category: "implementation",
    level: "potential",
    body: "返工风险：若登录页已按 v1（仅登录成功）开工，锁定与超时部分需补充开发与用例。",
    related: "TASK-210"
  },
  {
    id: "fnd-doc-1",
    category: "documentation",
    level: "indirect",
    body: "KB-AUTH-012（认证与会话安全方案）需按定稿的阈值口径复核同步。",
    related: "KB-AUTH-012"
  },
  {
    id: "fnd-doc-2",
    category: "documentation",
    level: "uncertain",
    body: "未发现技术文档层面的直接关联；如存在接口规格文档需人工补充排查。"
  },
  {
    id: "fnd-bl-1",
    category: "baseline",
    level: "potential",
    body: "当前基线 BL-002 未含入 REQ-001（v1 曾在 BL-001）。变更确认且需求定稿后，需准备新基线纳入。",
    related: "BL-002"
  }
];

/** 变更影响分析主对象（分析对象 = REQ-001 v2 拟议修订） */
export const impactAnalysisFixture: ChangeImpactAnalysis = {
  id: "cia-001",
  requirement_id: "req-001",
  requirement_code: "REQ-001",
  title: "User authentication",
  type: "functional",
  priority: "high",
  baseline_id: "BL-002",
  baseline_version: null,
  proposed_revision: 2,
  status: "analyzed",
  change: {
    original_summary:
      "v1：用户使用邮箱与密码登录平台（基础登录能力，1 条验收标准）。",
    proposed_summary:
      "v2：补充登录失败锁定（连续 5 次失败锁定 15 分钟）与会话空闲超时（30 分钟自动登出），验收标准重写为 3 条。",
    reason: "响应 2026-11-03 需求评审会结论：锁定与会话超时需可测。",
    source: "ai_assisted",
    proposed_by: "AI 分析助手",
    submitted_at: "2026-11-04 16:20"
  },
  targets: impactTargetsFixture,
  findings: impactFindingsFixture,
  ai: {
    summary:
      "本次变更集中于本需求的策略扩展与验收标准重写：直接影响验收标准与实现任务 TASK-210，间接涉及安全方案文档复核；未发现跨需求依赖扩散。基线影响为潜在项（确认后纳入下一基线）。",
    risk: "low",
    evidence: [
      "修订差异：v1 → v2 新增锁定（5 次 / 15 分钟）与会话超时（30 分钟），验收标准 1 → 3 条（REQ-001 修订快照）。",
      "REQ-001 关联任务 TASK-210「实现登录页与会话管理」（需求关联项）。",
      "v2 的阈值与超时口径引自 KB-AUTH-012「认证与会话安全方案」（需求来源引用）。"
    ],
    confidence: "medium",
    recommendations: [
      "TASK-210 的任务范围与用例按 v2 复核，锁定与超时纳入验收。",
      "变更定稿后同步复核 KB-AUTH-012 的阈值口径。",
      "需求定稿后准备新基线，将 REQ-001 纳入含入清单。"
    ]
  }
};

/** 演示决策署名（领域数据，i18n 豁免区） */
export const impactDecisionOwnerFixture = "林悦";
