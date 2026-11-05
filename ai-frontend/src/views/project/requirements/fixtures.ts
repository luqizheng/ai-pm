/**
 * Requirements Workspace 演示数据（mock 先行，S02-UI-01 §6.1 示例记录）。
 *
 * 本文件为 fixtures（i18n 硬编码门禁豁免）：需求标题/描述/来源摘录等
 * **领域数据**走这里；界面文案（区块标题/列头/动作）走
 * `locales/<locale>/pm.yaml` 的 `requirements:` 命名空间。
 * 领域模型见 `requirement-entry.ts`；类型仅 functional / non_functional。
 */

import type {
  AiSuggestion,
  AnalysisFinding,
  DiscussionEntry,
  RequirementEntry,
  RevisionRecord,
  SourceItem
} from "./requirement-entry";

/** 演示需求集合（REQ-001 ~ REQ-004 + 两条基线内需求） */
export const requirementsFixture: RequirementEntry[] = [
  {
    id: "req-001",
    code: "REQ-001",
    title: "User authentication",
    description:
      "用户可使用邮箱与密码登录平台，会话超时自动登出；连续登录失败按安全策略锁定账户。",
    type: "functional",
    priority: "high",
    status: "pending_confirmation",
    source_label: "用户请求",
    created_at: "2026-10-29 10:05",
    updated_at: "2026-11-04 16:20",
    specification: {
      business_rationale:
        "平台是多人协作环境，没有可靠的账号认证就无法界定操作者与权限，也无法满足客户的安全审计要求。",
      expected_outcome:
        "用户能安全登录平台并保持会话，异常登录尝试被有效拦截。",
      scope: "账号密码登录、会话管理、登录失败锁定；覆盖 Web 端。",
      explicit_exclusions:
        "不含第三方 OAuth 登录（另立需求）；不含组织级 SSO 对接。"
    },
    acceptance_criteria: [
      {
        id: "ac-001-1",
        statement: "用户使用邮箱与密码登录成功后进入项目工作区。"
      },
      {
        id: "ac-001-2",
        statement: "连续 5 次登录失败后账户锁定 15 分钟并给出提示。"
      },
      {
        id: "ac-001-3",
        statement: "会话空闲 30 分钟后自动登出并要求重新认证。"
      }
    ],
    revisions: [
      {
        id: "rev-001-2",
        version: 2,
        state: "pending_confirmation",
        summary: "补充登录失败锁定与会话超时策略，重写验收标准。",
        author: "AI 分析助手",
        created_at: "2026-11-04 16:20",
        markdown:
          "# 用户认证\n\n- 用户使用邮箱与密码登录平台。\n- 连续 5 次登录失败后账户锁定 15 分钟。\n- 会话空闲 30 分钟后自动登出并要求重新认证。",
        origin: "ai_assisted",
        audit: {
          initiated_by: "AI 分析助手",
          reviewed_by: "林悦",
          reason: "响应 2026-11-03 需求评审会结论：锁定与会话超时需可测。"
        },
        priority: "high",
        type: "functional",
        criteria: [
          {
            id: "ac-001-1",
            statement: "用户使用邮箱与密码登录成功后进入项目工作区。"
          },
          {
            id: "ac-001-2",
            statement: "连续 5 次登录失败后账户锁定 15 分钟并给出提示。"
          },
          {
            id: "ac-001-3",
            statement: "会话空闲 30 分钟后自动登出并要求重新认证。"
          }
        ]
      },
      {
        id: "rev-001-1",
        version: 1,
        state: "superseded",
        summary: "初始拆解：账号密码登录基础能力。",
        author: "林悦",
        created_at: "2026-10-29 10:05",
        markdown: "# 用户认证\n\n- 用户使用邮箱与密码登录平台。",
        origin: "human_edit",
        decision: {
          kind: "confirmed",
          actor: "林悦",
          acted_at: "2026-10-30 09:00"
        },
        audit: {
          initiated_by: "林悦",
          reason: "项目冷启动时从原始用户请求拆解基础登录能力。"
        },
        priority: "high",
        type: "functional",
        criteria: [
          {
            id: "ac-001-v1-1",
            statement: "用户使用邮箱与密码登录成功后进入项目工作区。"
          }
        ]
      }
    ],
    sources: [
      {
        id: "src-001-1",
        kind: "user_request",
        title: "原始用户请求",
        detail:
          "「平台需要支持账号登录，密码错误太多次要拦一下，别让人一直试。」",
        ref: "PMR-2026-10-27"
      },
      {
        id: "src-001-2",
        kind: "knowledge",
        title: "认证与会话安全方案",
        detail: "会话超时、失败锁定阈值与提示文案的安全设计约定。",
        ref: "KB-AUTH-012"
      },
      {
        id: "src-001-3",
        kind: "discussion",
        title: "需求评审会（2026-11-03）",
        detail: "评审结论：锁定阈值取 5 次 / 15 分钟，需在验收标准中可测。",
        ref: "MIN-1103-02"
      }
    ],
    discussions: [
      {
        id: "dsc-001-1",
        author: "林悦",
        body: "锁定与会话超时两条标准可以测，确认前请 AI 再核对一遍与安全方案的偏差。",
        created_at: "2026-11-04 09:12"
      }
    ],
    analysis: [
      {
        id: "ana-001-1",
        kind: "completeness",
        severity: "medium",
        body: "缺少「忘记密码」找回路径，建议补充独立验收标准。"
      },
      {
        id: "ana-001-2",
        kind: "ambiguity",
        severity: "low",
        body: "「拦一下」已细化为锁定阈值，无遗留歧义。"
      }
    ],
    ai_suggestions: [
      {
        id: "ais-001-1",
        body: "建议将「OAuth 第三方登录」拆分为独立需求，避免本需求范围膨胀。",
        created_at: "2026-11-04 16:22"
      }
    ],
    related: [
      {
        kind: "objective",
        code: "OBJ-01",
        title: "平台基础能力可用"
      },
      {
        kind: "task",
        code: "TASK-210",
        title: "实现登录页与会话管理"
      }
    ]
  },
  {
    id: "req-002",
    code: "REQ-002",
    title: "System response time",
    description:
      "核心接口在 500 并发下 P95 响应时间不超过 300ms，页面首屏 LCP 不超过 2s。",
    type: "non_functional",
    priority: "high",
    status: "confirmed",
    source_label: "项目目标",
    created_at: "2026-10-30 15:30",
    updated_at: "2026-11-02 11:40",
    specification: {
      business_rationale:
        "500 人并发是企业客户明确的服务水平承诺，响应迟缓会直接导致验收失败。",
      expected_outcome: "核心接口与页面在压测环境达到既定 SLA，可持续稳定。",
      scope: "列表类接口与核心页面首屏；以压测环境口径为准。",
      explicit_exclusions: "不含弱网优化与离线能力。"
    },
    acceptance_criteria: [
      {
        id: "ac-002-1",
        statement: "列表类接口在 500 并发持续 10 分钟下 P95 ≤ 300ms。"
      },
      {
        id: "ac-002-2",
        statement: "核心页面首屏 LCP ≤ 2s（生产规格环境）。"
      }
    ],
    revisions: [
      {
        id: "rev-002-2",
        version: 2,
        state: "approved",
        summary: "并入 PRP-006：SLA 分位数口径标注（10 分钟采样窗口）。",
        author: "AI 分析助手",
        created_at: "2026-11-02 09:35",
        markdown:
          "# 系统响应时间\n\n- 列表类接口在 500 并发持续 10 分钟窗口下 P95 ≤ 300ms。\n- 首屏 LCP ≤ 2s。",
        origin: "ai_assisted",
        decision: {
          kind: "confirmed",
          actor: "沈舟",
          acted_at: "2026-11-02 09:40"
        },
        audit: {
          initiated_by: "AI 分析助手",
          reviewed_by: "沈舟",
          reason: "并入 PRP-006 评审确认的分位数口径，提升 SLA 可复现性。"
        },
        priority: "high",
        type: "non_functional",
        criteria: [
          {
            id: "ac-002-1",
            statement: "列表类接口在 500 并发持续 10 分钟下 P95 ≤ 300ms。"
          },
          {
            id: "ac-002-2",
            statement: "核心页面首屏 LCP ≤ 2s（生产规格环境）。"
          }
        ]
      },
      {
        id: "rev-002-1",
        version: 1,
        state: "superseded",
        summary: "自项目性能目标拆解，含可测 SLA。",
        author: "林悦",
        created_at: "2026-10-30 15:30",
        markdown:
          "# 系统响应时间\n\n- 列表类接口 500 并发 P95 ≤ 300ms。\n- 首屏 LCP ≤ 2s。",
        origin: "human_edit",
        decision: {
          kind: "confirmed",
          actor: "林悦",
          acted_at: "2026-10-30 16:00"
        },
        audit: {
          initiated_by: "林悦",
          reason: "自项目性能目标拆解，形成可测 SLA 初始版本。"
        },
        priority: "high",
        type: "non_functional",
        criteria: [
          {
            id: "ac-002-v1-1",
            statement: "列表类接口在 500 并发下 P95 响应时间不超过 300ms。"
          },
          {
            id: "ac-002-v1-2",
            statement: "核心页面首屏 LCP ≤ 2s（生产规格环境）。"
          }
        ]
      }
    ],
    sources: [
      {
        id: "src-002-1",
        kind: "objective",
        title: "项目目标（性能）",
        detail: "「系统要跟得上 500 人并发使用，不能明显卡顿。」",
        ref: "OBJ-01"
      },
      {
        id: "src-002-2",
        kind: "discovery",
        title: "代码发现证据",
        detail:
          "列表接口现存分页查询在深分页场景全表扫描，需索引优化佐证可行性。",
        ref: "commit 8f2c1aa"
      }
    ],
    discussions: [
      {
        id: "dsc-002-1",
        author: "沈舟",
        body: "SLA 数值与压测环境已对齐，可以纳入基线。",
        created_at: "2026-11-02 11:35"
      }
    ],
    analysis: [
      {
        id: "ana-002-1",
        kind: "completeness",
        severity: "low",
        body: "未定义超载时的降级目标，可在后续修订补充。"
      }
    ],
    ai_suggestions: [],
    related: [
      {
        kind: "objective",
        code: "OBJ-01",
        title: "平台基础能力可用"
      }
    ]
  },
  {
    id: "req-003",
    code: "REQ-003",
    title: "Support multiple repositories",
    description: "一个项目可绑定多个代码仓库，并可指定主仓库与文档仓库角色。",
    type: "functional",
    priority: "medium",
    status: "draft",
    source_label: "干系人讨论",
    created_at: "2026-11-03 14:05",
    updated_at: "2026-11-03 14:05",
    specification: {
      business_rationale:
        "客户研发资产分散在多个仓库，单仓库绑定无法支撑完整的发现与执行链路。",
      expected_outcome:
        "项目可声明多个仓库及各自角色，发现与任务可按仓库范围执行。",
      scope: "仓库绑定、角色标注（main / docs）、发现范围选择。",
      explicit_exclusions: "不含仓库间依赖推断（归项目认知能力）。"
    },
    acceptance_criteria: [
      {
        id: "ac-003-1",
        statement: "项目可添加不少于 2 个仓库并分别设置角色（main / docs）。"
      },
      {
        id: "ac-003-2",
        statement: "发现任务可选择单个仓库或全部仓库作为分析范围。"
      }
    ],
    revisions: [
      {
        id: "rev-003-1",
        version: 1,
        state: "draft",
        summary: "访谈结论草稿，待补齐多仓库冲突处理策略。",
        author: "AI 分析助手",
        created_at: "2026-11-03 14:05",
        markdown:
          "# 多仓库支持\n\n- 项目可绑定多个代码仓库。\n- 可指定主仓库与文档仓库角色。"
      }
    ],
    sources: [
      {
        id: "src-003-1",
        kind: "discussion",
        title: "干系人访谈（2026-10-28）",
        detail: "「我们前端、后端、文档分三个库，平台最好都能挂上。」",
        ref: "MIN-1028-01"
      }
    ],
    discussions: [],
    analysis: [
      {
        id: "ana-003-1",
        kind: "duplication",
        severity: "medium",
        body: "与 Discovery 模块的「仓库分析范围」能力存在重叠，需澄清边界。"
      }
    ],
    ai_suggestions: [
      {
        id: "ais-003-1",
        body: "建议明确多仓库场景下的默认分支与主仓库选举规则后再提交确认。",
        created_at: "2026-11-03 14:10"
      }
    ],
    related: [
      {
        kind: "task",
        code: "TASK-188",
        title: "仓库绑定与角色配置"
      }
    ]
  },
  {
    id: "req-004",
    code: "REQ-004",
    title: "API response consistency",
    description:
      "所有 REST 接口的错误响应遵循统一错误码与信封结构，错误码枚举与文档保持一致。",
    type: "non_functional",
    priority: "medium",
    status: "conflict",
    source_label: "代码发现",
    created_at: "2026-10-31 09:50",
    updated_at: "2026-11-04 10:15",
    specification: {
      business_rationale:
        "错误响应形态不一致导致前端与 SDK 重复适配，持续抬升联调成本。",
      expected_outcome:
        "适用范围内错误响应形态统一，错误码可被接口文档穷举核验。",
      scope: "平台层与网关端点（适用范围待冲突裁决确认）；错误码引用文档枚举。",
      explicit_exclusions: "业务端点裸 payload 约定暂不变更（冲突裁决中）。"
    },
    acceptance_criteria: [
      {
        id: "ac-004-1",
        statement: "错误响应统一为 {code, message} 信封结构。"
      },
      {
        id: "ac-004-2",
        statement: "错误码枚举与接口文档逐项一致。"
      }
    ],
    revisions: [
      {
        id: "rev-004-2",
        version: 2,
        state: "draft",
        summary: "检出与既有 pm-api 契约冲突，修订暂缓提交。",
        author: "AI 分析助手",
        created_at: "2026-11-04 10:15",
        markdown:
          "# API 响应一致性\n\n- 错误响应统一 {code, message} 信封。\n- 错误码枚举与文档一致。\n\n> 冲突：pm-api 业务端点契约约定返回裸 payload（无信封）。",
        origin: "ai_assisted",
        audit: {
          initiated_by: "AI 分析助手",
          reviewed_by: "沈舟",
          reason: "代码发现检出信封形态不一致；修订暂缓，待冲突裁决后提交。"
        },
        priority: "medium",
        type: "non_functional",
        criteria: [
          {
            id: "ac-004-1",
            statement: "错误响应统一为 {code, message} 信封结构。"
          },
          {
            id: "ac-004-2",
            statement: "错误码枚举与接口文档逐项一致。"
          }
        ]
      },
      {
        id: "rev-004-1",
        version: 1,
        state: "superseded",
        summary: "初始版本：统一错误信封。",
        author: "林悦",
        created_at: "2026-10-31 09:50",
        markdown: "# API 响应一致性\n\n- 错误响应统一 {code, message} 信封。",
        origin: "human_edit",
        decision: {
          kind: "confirmed",
          actor: "林悦",
          acted_at: "2026-10-31 10:30"
        },
        audit: {
          initiated_by: "林悦",
          reason: "平台层错误信封规范的初始需求化。"
        },
        priority: "medium",
        type: "non_functional",
        criteria: [
          {
            id: "ac-004-v1-1",
            statement: "错误响应统一为 {code, message} 信封结构。"
          }
        ]
      }
    ],
    sources: [
      {
        id: "src-004-1",
        kind: "discovery",
        title: "代码发现证据",
        detail: "抽样 12 个端点：9 个返回裸 payload，3 个带信封，形态不一致。",
        ref: "DISC-045"
      },
      {
        id: "src-004-2",
        kind: "knowledge",
        title: "API 设计规范",
        detail: "平台层错误信封与错误码分段的既有约定。",
        ref: "KB-API-003"
      }
    ],
    discussions: [
      {
        id: "dsc-004-1",
        author: "沈舟",
        body: "业务端点裸 payload 是有意的约定，冲突需要人工裁决范围。",
        created_at: "2026-11-04 10:20"
      }
    ],
    analysis: [
      {
        id: "ana-004-1",
        kind: "conflict",
        severity: "high",
        body: "与 pm-api「业务端点返回裸 payload（无信封）」契约直接冲突，需裁决适用范围。",
        related: "pm-api 契约约定"
      },
      {
        id: "ana-004-2",
        kind: "duplication",
        severity: "low",
        body: "错误码分段已在《API 设计规范》定义，需求正文引用即可，不必重复枚举。"
      }
    ],
    ai_suggestions: [
      {
        id: "ais-004-1",
        body: "建议把适用范围限定为「平台层与网关端点」，业务端点维持裸 payload 约定。",
        created_at: "2026-11-04 10:18"
      }
    ],
    related: [
      {
        kind: "requirement",
        code: "REQ-002",
        title: "System response time",
        target_id: "req-002"
      }
    ]
  }
];

/** 页头演示项目信息 */
export const requirementsHeaderFixture = {
  projectName: "Atlas Platform"
};

/** 当前演示用户（领域数据，i18n 豁免区） */
export const currentAuthorFixture = "林悦";

/** AI 协作提案演示池（点击建议动作后注入，只进提案通道） */
export const aiProposalPool: Record<string, AiSuggestion[]> = {
  completeness: [
    {
      id: "ais-new-1",
      body: "检测到 REQ-003 验收标准未覆盖「移除仓库」场景，建议补充一条可测陈述。",
      created_at: "2026-11-05 09:00"
    }
  ],
  missing: [
    {
      id: "ais-new-2",
      body: "目标 OBJ-01 涉及「通知触达」，尚无对应需求条目，建议补充通知渠道需求。",
      created_at: "2026-11-05 09:00"
    }
  ],
  duplication: [
    {
      id: "ais-new-3",
      body: "REQ-003 与 Discovery 仓库范围能力高度重叠，建议合并为一条需求并互相引用。",
      created_at: "2026-11-05 09:00"
    }
  ],
  conflict: [
    {
      id: "ais-new-4",
      body: "REQ-004 与 pm-api 裸 payload 契约冲突未解决，确认基线前建议先裁决适用范围。",
      created_at: "2026-11-05 09:00"
    }
  ],
  acceptance: [
    {
      id: "ais-new-5",
      body: "REQ-001 第二条验收标准可补充分位数口径（如 P95），提升可测性。",
      created_at: "2026-11-05 09:00"
    }
  ],
  summary: [
    {
      id: "ais-new-6",
      body: "近 7 日需求变更：新增 1 条、修订 2 条、确认 1 条；冲突 1 条待裁决。",
      created_at: "2026-11-05 09:00"
    }
  ]
};

/** 冲突解决选项（演示用固定文案，走 i18n 注入展示标签） */
export const conflictResolutionOptions = [
  "scope_limit",
  "revise_draft",
  "defer"
] as const;

export type ConflictResolutionOption =
  (typeof conflictResolutionOptions)[number];

/** 类型收敛助手：RevisionRecord[] 的当前修订（version 最大） */
export function currentRevision(
  entry: RequirementEntry
): RevisionRecord | undefined {
  return [...entry.revisions].sort((a, b) => b.version - a.version)[0];
}

/** 来源图标语义 key（组件内映射 lucide 图标） */
export const sourceKindOrder: SourceItem["kind"][] = [
  "user_request",
  "discovery",
  "knowledge",
  "discussion",
  "objective"
];

/** 讨论列表排序（最新在前） */
export function sortDiscussions(entries: DiscussionEntry[]): DiscussionEntry[] {
  return [...entries].sort((a, b) => (a.created_at < b.created_at ? 1 : -1));
}

/** 分析发现排序：冲突 > 重复 > 歧义 > 完备性，同级按严重度 */
const analysisWeight: Record<AnalysisFinding["kind"], number> = {
  conflict: 0,
  duplication: 1,
  ambiguity: 2,
  completeness: 3
};
const severityWeight: Record<AnalysisFinding["severity"], number> = {
  high: 0,
  medium: 1,
  low: 2
};

export function sortAnalysis(entries: AnalysisFinding[]): AnalysisFinding[] {
  return [...entries].sort(
    (a, b) =>
      analysisWeight[a.kind] - analysisWeight[b.kind] ||
      severityWeight[a.severity] - severityWeight[b.severity]
  );
}
