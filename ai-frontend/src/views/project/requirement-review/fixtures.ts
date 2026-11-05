/**
 * 需求评审演示数据（mock 先行，S02-UI-03）。
 *
 * 本文件为 fixtures（i18n 硬编码门禁豁免）：提案标题/描述/证据摘录等
 * **领域数据**走这里；界面文案走 `locales/<locale>/pm.yaml` 的
 * `requirementReview:` 命名空间。契约见 `review-contract.ts`：
 * 提案评审状态独立于需求 Revision 生命周期，AI 提案必须经人类决策。
 */

import type { ReviewProposal } from "./review-contract";

/** 演示提案集合（PRP-001 ~ PRP-006：覆盖四种评审状态与三种来源） */
export const reviewProposalsFixture: ReviewProposal[] = [
  {
    id: "prp-001",
    code: "PRP-001",
    title: "Forgot password & account recovery",
    description:
      "用户可通过邮箱验证码重置密码；锁定账户到期自动解锁，必要时由管理员手动解锁。",
    type: "functional",
    priority: "high",
    status: "pending_review",
    source: "ai",
    source_detail: "AI 分析助手 · 2026-11-04 17:05",
    submitted_at: "2026-11-04 17:05",
    has_conflict: false,
    requirement_code: null,
    requirement_id: null,
    acceptance_criteria: [
      {
        id: "prp-001-ac-1",
        statement: "登录页提供「忘记密码」入口，输入注册邮箱后收到重置验证码。"
      },
      {
        id: "prp-001-ac-2",
        statement: "验证码 10 分钟内有效且一次性使用。"
      },
      {
        id: "prp-001-ac-3",
        statement: "重置成功后该账户所有旧会话立即失效。"
      }
    ],
    sources: [
      {
        id: "prp-001-src-1",
        kind: "user_request",
        title: "原始用户请求",
        detail: "「上次有人密码忘了进不去，得找人后台改，太麻烦了。」",
        ref: "PMR-2026-11-01"
      },
      {
        id: "prp-001-src-2",
        kind: "knowledge",
        title: "认证与会话安全方案",
        detail: "验证码有效期、一次性使用与会话失效的安全设计约定。",
        ref: "KB-AUTH-012"
      }
    ],
    analysis: [
      {
        id: "prp-001-ana-1",
        kind: "ambiguity",
        severity: "medium",
        body: "「管理员手动解锁」的操作者权限边界未定义，需补充授权角色。"
      },
      {
        id: "prp-001-ana-2",
        kind: "completeness",
        severity: "low",
        body: "未覆盖邮箱不可达（退信）时的备用找回路径。"
      }
    ],
    comparison: {
      existing_code: "REQ-001",
      blocks: [
        {
          id: "prp-001-cmp-1",
          kind: "duplication",
          proposalText: "涵盖登录失败锁定与会话超时的关联场景。",
          existingText: "REQ-001 已含连续失败锁定与会话超时标准。",
          note: "与既有认证需求存在场景重叠，确认后需在 REQ-001 标注范围边界。"
        },
        {
          id: "prp-001-cmp-2",
          kind: "added",
          proposalText: "邮箱验证码找回路径（全新能力，既有需求未覆盖）。",
          note: "补齐 REQ-001 分析中指出的「忘记密码」缺失项。"
        }
      ]
    }
  },
  {
    id: "prp-002",
    code: "PRP-002",
    title: "REQ-004 修订：适用范围限定为平台层与网关端点",
    description:
      "按冲突裁决建议，把统一错误信封的适用范围收窄到平台层与网关端点；业务端点维持裸 payload 约定并在文档标注豁免。",
    type: "non_functional",
    priority: "medium",
    status: "pending_review",
    source: "ai",
    source_detail: "AI 分析助手 · 2026-11-04 11:30",
    submitted_at: "2026-11-04 11:30",
    has_conflict: true,
    requirement_code: "REQ-004",
    requirement_id: "req-004",
    acceptance_criteria: [
      {
        id: "prp-002-ac-1",
        statement: "平台层与网关端点错误响应统一为 {code, message} 信封。"
      },
      {
        id: "prp-002-ac-2",
        statement: "业务端点维持裸 payload，接口文档逐项标注豁免。"
      },
      {
        id: "prp-002-ac-3",
        statement: "错误码枚举与接口文档逐项一致。"
      }
    ],
    sources: [
      {
        id: "prp-002-src-1",
        kind: "knowledge",
        title: "API 设计规范",
        detail: "平台层错误信封与错误码分段的既有约定。",
        ref: "KB-API-003"
      },
      {
        id: "prp-002-src-2",
        kind: "discovery",
        title: "代码发现证据",
        detail: "抽样 12 个端点：9 个返回裸 payload，3 个带信封，形态不一致。",
        ref: "DISC-045"
      }
    ],
    analysis: [
      {
        id: "prp-002-ana-1",
        kind: "conflict",
        severity: "high",
        body: "与 pm-api「业务端点返回裸 payload」契约的冲突尚未正式裁决，本提案即裁决载体。",
        related: "pm-api 契约约定"
      },
      {
        id: "prp-002-ana-2",
        kind: "completeness",
        severity: "low",
        body: "豁免端点清单未穷举，需在文档附完整列表。"
      }
    ],
    comparison: {
      existing_code: "REQ-004",
      blocks: [
        {
          id: "prp-002-cmp-1",
          kind: "modified",
          proposalText: "适用范围：平台层与网关端点统一错误信封。",
          existingText: "所有 REST 接口错误响应遵循统一信封结构。",
          note: "范围收窄是本次修订的核心变更。"
        },
        {
          id: "prp-002-cmp-2",
          kind: "added",
          proposalText: "业务端点维持裸 payload，并在接口文档逐项标注豁免。",
          note: "响应既有 pm-api 契约约定。"
        },
        {
          id: "prp-002-cmp-3",
          kind: "removed",
          existingText: "「所有 REST 接口」的全局统一表述（原文）。",
          note: "旧的全局表述被范围化表述替代。"
        },
        {
          id: "prp-002-cmp-4",
          kind: "duplication",
          proposalText: "错误码枚举与接口文档一致。",
          existingText: "《API 设计规范》已定义错误码分段（KB-API-003）。",
          note: "引用规范即可，需求正文不重复枚举。"
        }
      ]
    }
  },
  {
    id: "prp-003",
    code: "PRP-003",
    title: "Notification channels & delivery policy",
    description:
      "平台通知支持站内与邮件双渠道，用户可按类别配置接收偏好；关键安全事件强制双通道触达。",
    type: "functional",
    priority: "medium",
    status: "pending_review",
    source: "human",
    source_detail: "林悦 · 2026-11-04 15:40",
    submitted_at: "2026-11-04 15:40",
    has_conflict: false,
    requirement_code: null,
    requirement_id: null,
    acceptance_criteria: [
      {
        id: "prp-003-ac-1",
        statement: "通知按类别投递到用户配置的渠道，偏好修改即时生效。"
      },
      {
        id: "prp-003-ac-2",
        statement: "安全类事件强制站内 + 邮件双通道投递。"
      },
      {
        id: "prp-003-ac-3",
        statement: "邮件投递失败自动重试并在站内标记未送达。"
      }
    ],
    sources: [
      {
        id: "prp-003-src-1",
        kind: "discussion",
        title: "干系人访谈（2026-11-01）",
        detail: "「邮件也要能收到关键提醒，不然离线就错过了。」",
        ref: "MIN-1101-02"
      },
      {
        id: "prp-003-src-2",
        kind: "objective",
        title: "项目目标（触达）",
        detail: "OBJ-01 要求关键信息可靠触达所有项目成员。",
        ref: "OBJ-01"
      }
    ],
    analysis: [
      {
        id: "prp-003-ana-1",
        kind: "ambiguity",
        severity: "medium",
        body: "「关键安全事件」的事件清单未定义，需在规格中穷举。"
      }
    ],
    comparison: {
      existing_code: null,
      blocks: []
    }
  },
  {
    id: "prp-004",
    code: "PRP-004",
    title: "REQ-002 修订：补充过载降级目标",
    description:
      "在 SLA 之外补充超载降级目标：过载时核心接口返回明确限流响应，而不是无响应挂起。",
    type: "non_functional",
    priority: "high",
    status: "changes_requested",
    source: "ai",
    source_detail: "AI 分析助手 · 2026-11-03 10:20",
    submitted_at: "2026-11-03 10:20",
    has_conflict: false,
    requirement_code: "REQ-002",
    requirement_id: "req-002",
    acceptance_criteria: [
      {
        id: "prp-004-ac-1",
        statement: "过载时列表接口返回 429 与 Retry-After，不出现无响应挂起。"
      },
      {
        id: "prp-004-ac-2",
        statement: "降级期间核心写操作可用性不低于 99%。"
      }
    ],
    sources: [
      {
        id: "prp-004-src-1",
        kind: "objective",
        title: "项目目标（性能）",
        detail: "「系统要跟得上 500 人并发使用，不能明显卡顿。」",
        ref: "OBJ-01"
      }
    ],
    analysis: [
      {
        id: "prp-004-ana-1",
        kind: "completeness",
        severity: "low",
        body: "限流阈值需要压测数据支撑，当前为占位值。"
      }
    ],
    comparison: {
      existing_code: "REQ-002",
      blocks: [
        {
          id: "prp-004-cmp-1",
          kind: "added",
          proposalText:
            "过载降级目标：429 + Retry-After；降级期写可用性 ≥ 99%。",
          note: "回应 REQ-002 分析中「未定义降级目标」的完备性发现。"
        }
      ]
    },
    decision: {
      kind: "changes_requested",
      reviewer: "沈舟",
      decided_at: "2026-11-03 16:05",
      comment: "方向正确；先补限流阈值依据（压测报告），重新提交后再议。"
    }
  },
  {
    id: "prp-005",
    code: "PRP-005",
    title: "Inter-repository dependency inference",
    description: "自动推断多仓库之间的调用与依赖关系，并展示项目内仓库拓扑图。",
    type: "functional",
    priority: "low",
    status: "rejected",
    source: "ai",
    source_detail: "AI 分析助手 · 2026-11-02 09:15",
    submitted_at: "2026-11-02 09:15",
    has_conflict: false,
    requirement_code: null,
    requirement_id: null,
    acceptance_criteria: [
      {
        id: "prp-005-ac-1",
        statement: "检出跨仓库调用关系并生成拓扑视图。"
      }
    ],
    sources: [
      {
        id: "prp-005-src-1",
        kind: "discovery",
        title: "代码发现证据",
        detail: "多仓库项目样本中检出 6 组跨仓库 import 引用。",
        ref: "DISC-041"
      }
    ],
    analysis: [
      {
        id: "prp-005-ana-1",
        kind: "duplication",
        severity: "high",
        body: "与项目认知（Cognition）的跨仓库关系能力完全重叠，见 REQ-003 明确排除项。"
      }
    ],
    comparison: {
      existing_code: "REQ-003",
      blocks: [
        {
          id: "prp-005-cmp-1",
          kind: "duplication",
          proposalText: "自动推断仓库间依赖并展示拓扑。",
          existingText:
            "REQ-003 明确排除项：「不含仓库间依赖推断（归项目认知能力）」。",
          note: "按既有裁决归 Cognition，不作为独立需求。"
        }
      ]
    },
    decision: {
      kind: "rejected",
      reviewer: "林悦",
      decided_at: "2026-11-02 14:30",
      comment:
        "仓库依赖推断归项目认知能力（Cognition），不作为独立需求；拒绝理由留档，后续同类提案参考本记录。"
    }
  },
  {
    id: "prp-006",
    code: "PRP-006",
    title: "REQ-002 修订：SLA 分位数口径标注",
    description: "在 SLA 陈述中显式标注分位数口径（P95），并注明压测环境规格。",
    type: "non_functional",
    priority: "low",
    status: "confirmed",
    source: "ai",
    source_detail: "AI 分析助手 · 2026-11-01 11:00",
    submitted_at: "2026-11-01 11:00",
    has_conflict: false,
    requirement_code: "REQ-002",
    requirement_id: "req-002",
    acceptance_criteria: [
      {
        id: "prp-006-ac-1",
        statement:
          "SLA 陈述显式包含分位数口径与采样窗口（如 10 分钟窗口 P95）。"
      }
    ],
    sources: [
      {
        id: "prp-006-src-1",
        kind: "objective",
        title: "项目目标（性能）",
        detail: "性能承诺需要可复现的测量口径。",
        ref: "OBJ-01"
      }
    ],
    analysis: [
      {
        id: "prp-006-ana-1",
        kind: "ambiguity",
        severity: "low",
        body: "原文「P95 响应时间」未标注采样窗口，已在本提案补齐。"
      }
    ],
    comparison: {
      existing_code: "REQ-002",
      blocks: [
        {
          id: "prp-006-cmp-1",
          kind: "modified",
          proposalText: "列表类接口在 500 并发持续 10 分钟窗口下 P95 ≤ 300ms。",
          existingText: "列表类接口在 500 并发下 P95 响应时间不超过 300ms。",
          note: "补齐采样窗口，口径可复现。"
        }
      ]
    },
    decision: {
      kind: "confirmed",
      reviewer: "沈舟",
      decided_at: "2026-11-02 09:40",
      comment: "口径清晰可测，已并入 REQ-002 修订待确认修订集。"
    }
  }
];

/** 当前演示用户（决策记录署名用，i18n 豁免区） */
export const reviewReviewerFixture = "林悦";
