import type {
  ActionRequestPayload,
  AiIdentity,
  AiEvidence,
  ArtifactPayload,
  ConversationMessage,
  DecisionPayload,
  DiffPayload,
  ExecutionResultPayload,
  InformationalPayload,
  ProposalPayload,
  QuestionPayload,
  RecommendationPayload
} from "./types";

/**
 * 目录页 / 演示用的真实研发项目样例数据。
 *
 * 场景：Atlas Platform「地图编辑器 · 世界地图 v2.0」迭代。
 * 内容为设计样例（不接后端），用于展示组件的真实信息密度。
 */

export const aiIdentity: AiIdentity = {
  name: "AI-PM",
  role: "研发负责人",
  avatarLabel: "AI"
};

export const humanReviewer: AiIdentity = {
  name: "Alex Chen",
  role: "项目负责人"
};

export const projectEvidence: AiEvidence = {
  summary:
    "结论基于 3 份需求文档、2 个已合并 PR 与最近 7 天的执行记录；森林生成与地形渲染的耦合度在 v1.4 后显著上升。",
  sources: [
    {
      id: "src-req-128",
      label: "需求文档：世界地图渲染 v2.0",
      kind: "requirement",
      meta: "REQ-128 · v1.4 · 3 天前"
    },
    {
      id: "src-arch-31",
      label: "架构说明：地形渲染管线",
      kind: "doc",
      meta: "ARCH-31 · v2 · 上周"
    },
    {
      id: "src-pr-128",
      label: "PR-128：引入 GPU Shader 预计算",
      kind: "code",
      meta: "已合并 · 6 天前"
    },
    {
      id: "src-run-77",
      label: "执行记录：森林分布生成任务",
      kind: "execution",
      meta: "RUN-77 · 失败 1 次后重试"
    }
  ],
  related: [
    { id: "obj-task-204", label: "实现森林分布逻辑", kind: "任务" },
    { id: "obj-dec-12", label: "地形渲染架构决策", kind: "决策" },
    { id: "obj-req-128", label: "世界地图渲染 v2.0", kind: "需求" }
  ]
};

/* ------------------------------------------------------------------ */
/* 02 Informational                                                    */
/* ------------------------------------------------------------------ */

export const informationalFixture: InformationalPayload = {
  contentMarkdown: [
    "本周迭代共 12 个任务，5 个执行中，2 个待人工决策。整体风险集中在**地形渲染**：",
    "",
    "- GPU Shader 预计算方案已合并，但缺少移动端回退路径；",
    "- 森林分布生成依赖地形网格数据，当前接口尚未冻结；",
    "- 需求 REQ-128 的验收标准与实现范围存在 1 处冲突（见下）。",
    "",
    "| 模块 | 进度 | 风险 |",
    "| --- | --- | --- |",
    "| 地形渲染 | 72% | 高 |",
    "| 森林生成 | 45% | 中 |",
    "| 编辑器交互 | 88% | 低 |",
    "",
    "> 建议先解冻森林生成接口，再推进渲染管线联调。"
  ].join("\n"),
  sources: [
    {
      id: "src-req-128",
      label: "需求文档：世界地图渲染 v2.0",
      kind: "requirement",
      meta: "REQ-128 · v1.4"
    },
    {
      id: "src-pr-128",
      label: "PR-128：引入 GPU Shader 预计算",
      kind: "code",
      meta: "已合并"
    }
  ]
};

/* ------------------------------------------------------------------ */
/* 03 Recommendation                                                   */
/* ------------------------------------------------------------------ */

export const recommendationFixture: RecommendationPayload = {
  summary:
    "建议将「森林生成」拆分为「地形网格接口」与「分布算法」两个任务，先解冻接口阻塞。",
  explanationMarkdown: [
    "当前 `ForestGenerator` 同时承担地形采样与分布规则，导致：",
    "",
    "1. 地形网格接口变更会直接阻塞森林任务（已发生 2 次）；",
    "2. 单任务周期 5 天，超过迭代剩余窗口；",
    "3. 无法独立验证分布算法，回归成本高。",
    "",
    "拆分后，接口任务可先行解冻，分布算法在稳定数据上进行。"
  ].join("\n"),
  benefits: [
    { label: "解除阻塞", detail: "地形接口可当天冻结，森林任务不再排队" },
    { label: "可独立验证", detail: "分布算法可基于固定网格快照做回归" },
    { label: "周期可控", detail: "两个任务均落在 2–3 天窗口内" }
  ],
  risks: [
    {
      label: "任务数增加",
      severity: "low",
      detail: "迭代任务数 12 → 13，需同步更新甘特图"
    },
    {
      label: "接口契约返工",
      severity: "medium",
      detail: "若网格精度标准后续调整，接口需再改一轮"
    }
  ],
  recommendedAction: "拆分任务"
};

/* ------------------------------------------------------------------ */
/* 06/07 Proposal                                                      */
/* ------------------------------------------------------------------ */

export const proposalFixture: ProposalPayload = {
  proposalType: "架构变更",
  changes: [
    {
      label: "地形数据来源",
      before: "任务内直接调用 TerrainService.sample()",
      after: "改为消费冻结的网格快照 TerrainSnapshot"
    },
    {
      label: "森林分布入口",
      before: "ForestGenerator.generate(seed)",
      after: "ForestGenerator.generate(snapshot, rules)"
    },
    {
      label: "回归测试",
      before: "依赖实时地形服务",
      after: "使用 3 份固定快照做金标准比对"
    }
  ],
  impact: [
    "影响 2 个任务、1 个已合并 PR 的接口调用",
    "执行器镜像需重新构建（约 6 分钟）",
    "不影响已发布的地图编辑器 1.x"
  ],
  related: [
    { id: "obj-req-128", label: "世界地图渲染 v2.0", kind: "需求" },
    { id: "obj-task-204", label: "实现森林分布逻辑", kind: "任务" },
    { id: "obj-arch-31", label: "地形渲染管线", kind: "架构文档" }
  ],
  version: "v1.2 → v1.3",
  canSubmitForApproval: true
};

export const proposalDiffFixture: DiffPayload = {
  path: "docs/architecture/terrain-render-pipeline.md",
  hunks: [
    {
      header: "@@ -12,7 +12,9 @@",
      lines: [
        { kind: "context", text: "## 数据来源" },
        {
          kind: "remove",
          text: "- 任务运行时直接调用 TerrainService.sample() 获取高程。"
        },
        {
          kind: "add",
          text: "+ 任务运行时消费已冻结的 TerrainSnapshot（只读）。"
        },
        { kind: "add", text: "+ 快照由地形模块在冻结节点统一产出。" },
        { kind: "context", text: "" },
        { kind: "context", text: "## 失败重试" },
        {
          kind: "remove",
          text: "- 失败后重新采样地形，结果可能不一致。"
        },
        {
          kind: "add",
          text: "+ 失败后复用同一快照重试，保证结果可复现。"
        }
      ]
    }
  ]
};

/* ------------------------------------------------------------------ */
/* 08/09/10 Question                                                   */
/* ------------------------------------------------------------------ */

export const questionFixture: QuestionPayload = {
  groupTitle: "森林生成方案确认",
  questions: [
    {
      id: "q-scope",
      kind: "single",
      label: "森林生成的首发范围",
      help: "影响本轮迭代的验收范围。",
      required: true,
      options: [
        {
          value: "full",
          label: "完整分布算法",
          description: "包含密度、树种与边缘衰减"
        },
        { value: "basic", label: "基础分布", description: "仅密度与随机种子" },
        { value: "defer", label: "本轮不做", description: "顺延至下一迭代" }
      ]
    },
    {
      id: "q-biomes",
      kind: "multi",
      label: "需要覆盖的生物群系",
      required: true,
      options: [
        { value: "forest", label: "温带森林" },
        { value: "rainforest", label: "雨林" },
        { value: "taiga", label: "针叶林" },
        { value: "mangrove", label: "红树林" }
      ]
    },
    {
      id: "q-seed",
      kind: "number",
      label: "默认随机种子",
      help: "留空则由地形快照哈希生成。",
      placeholder: "例如 20261105"
    },
    {
      id: "q-deadline",
      kind: "date",
      label: "期望冻结日期",
      required: true
    },
    {
      id: "q-note",
      kind: "textarea",
      label: "补充说明",
      help: "可填写验收口径、已知限制等。",
      placeholder: "选填"
    }
  ]
};

/* ------------------------------------------------------------------ */
/* 11/12 Decision                                                      */
/* ------------------------------------------------------------------ */

export const decisionFixture: DecisionPayload = {
  background:
    "地形渲染存在两条可行路线：GPU Shader 预计算与 CPU 预计算。两者在移动端表现差异明显，需要在冻结接口前定稿。",
  impact: [
    "决定地形渲染管线的主路径，影响 3 个任务排期",
    "移动端包体预计增加 1.8MB（Shader 方案）",
    "若选 CPU 方案，桌面端首屏地图加载预计 +400ms"
  ],
  allowComment: true,
  options: [
    {
      id: "opt-gpu",
      label: "GPU Shader 预计算",
      summary: "桌面端最优，移动端需回退路径",
      pros: ["桌面端帧率 +18%", "与现有 Vulkan 管线一致"],
      cons: ["移动端需额外回退实现", "包体 +1.8MB"],
      recommended: true
    },
    {
      id: "opt-cpu",
      label: "CPU 预计算",
      summary: "实现简单，跨端一致",
      pros: ["无平台差异", "包体不变"],
      cons: ["首屏加载 +400ms", "大场景内存占用 +12%"]
    },
    {
      id: "opt-hybrid",
      label: "混合方案",
      summary: "按平台分流，维护两套管线",
      pros: ["各端表现最优"],
      cons: ["维护成本最高", "本轮迭代无法完成"]
    }
  ]
};

/* ------------------------------------------------------------------ */
/* 13 Artifact                                                         */
/* ------------------------------------------------------------------ */

export const artifactFixture: ArtifactPayload = {
  artifactType: "requirement-doc",
  version: {
    label: "v1.4",
    createdAt: "2026-11-03T10:20:00+08:00",
    author: "AI-PM"
  },
  contentMarkdown: [
    "# 世界地图渲染 v2.0",
    "",
    "## 背景",
    "地图编辑器 1.x 的渲染管线无法支撑 4096×4096 的世界地图，首屏加载超过 3s。",
    "",
    "## 目标",
    "- 首屏加载 ≤ 1.2s（桌面端 1440p）",
    "- 支持地形、森林、河流三类图层叠加",
    "- 移动端提供降级路径",
    "",
    "## 验收标准",
    "1. 世界地图 4096² 在桌面端首屏 ≤ 1.2s；",
    "2. 森林图层与地形图层误差 ≤ 1px；",
    "3. 移动端降级后仍可编辑，帧率 ≥ 30fps。",
    "",
    "## 范围外",
    "- 动态天气系统",
    "- 多人在线协同编辑"
  ].join("\n"),
  diff: {
    path: "requirements/REQ-128-world-map-render.md",
    hunks: [
      {
        header: "@@ -18,6 +18,9 @@",
        lines: [
          { kind: "context", text: "## 验收标准" },
          { kind: "context", text: "1. 世界地图 4096² 桌面端首屏 ≤ 1.2s；" },
          {
            kind: "remove",
            text: "- 2. 森林图层与地形图层误差 ≤ 2px；"
          },
          {
            kind: "add",
            text: "+ 2. 森林图层与地形图层误差 ≤ 1px；"
          },
          { kind: "add", text: "+ 3. 移动端降级后帧率 ≥ 30fps。" }
        ]
      }
    ]
  },
  sources: [
    {
      id: "src-req-128",
      label: "需求文档：世界地图渲染 v2.0",
      kind: "requirement",
      meta: "REQ-128 · v1.4"
    }
  ],
  workflowLabel: "需求评审"
};

/* ------------------------------------------------------------------ */
/* 14 Action Request                                                   */
/* ------------------------------------------------------------------ */

export const actionRequestFixture: ActionRequestPayload = {
  action: "将 PR-128 合并到 main 分支",
  target: "atlas/map-editor · main",
  impact: [
    "触发 CI 全量回归（约 8 分钟）",
    "main 分支进入预发布通道，30 分钟后可部署",
    "影响 2 个正在进行中的任务，需要 rebase"
  ],
  permission: "仓库写权限 + 合并审批",
  riskLevel: "medium",
  statement:
    "我确认该 PR 已通过评审与回归测试，并了解合并后 main 分支的变更影响。",
  consequential: true
};

/* ------------------------------------------------------------------ */
/* 15 Execution Result                                                 */
/* ------------------------------------------------------------------ */

export const executionResultFixture: ExecutionResultPayload = {
  agentName: "codex-executor",
  taskRef: "RUN-77 / TASK-204",
  executionState: "partial",
  summary:
    "森林分布算法已实现并通过 12/14 条用例；2 条边缘用例因地形快照缺失失败。",
  changedFiles: [
    {
      path: "src/forest/generator.ts",
      status: "modified",
      additions: 86,
      deletions: 24
    },
    {
      path: "src/forest/rules.ts",
      status: "added",
      additions: 112,
      deletions: 0
    },
    {
      path: "test/forest/generator.spec.ts",
      status: "modified",
      additions: 64,
      deletions: 8
    }
  ],
  logs: [
    { time: "10:02:11", level: "info", text: "bootstrap workspace" },
    { time: "10:02:26", level: "info", text: "install deps (cached)" },
    { time: "10:03:02", level: "info", text: "run pnpm test:unit -- forest" },
    {
      time: "10:04:40",
      level: "warn",
      text: "snapshot 'coastline-03' missing"
    },
    { time: "10:04:41", level: "error", text: "2 specs failed (edge cases)" }
  ],
  warnings: [
    "地形快照 coastline-03 未纳入仓库，边缘用例无法复现",
    "生成器对空网格未做显式防御"
  ],
  failure: {
    code: "SPEC_FAILED",
    message: "2 条边缘用例失败：空网格与单点网格",
    retryable: true
  },
  diff: {
    path: "src/forest/generator.ts",
    hunks: [
      {
        header: "@@ -41,7 +41,12 @@",
        lines: [
          {
            kind: "context",
            text: "export function generate(snapshot: TerrainSnapshot) {"
          },
          {
            kind: "remove",
            text: "-  const cells = snapshot.cells.filter(c => c.height > 0);"
          },
          {
            kind: "add",
            text: "+  if (snapshot.cells.length === 0) {"
          },
          {
            kind: "add",
            text: "+    return { trees: [], warnings: ['EMPTY_GRID'] };"
          },
          { kind: "add", text: "+  }" },
          {
            kind: "add",
            text: "+  const cells = snapshot.cells.filter(c => c.height > 0);"
          }
        ]
      }
    ]
  }
};

/* ------------------------------------------------------------------ */
/* Execution failure-only（16 Error & Retry）                           */
/* ------------------------------------------------------------------ */

export const executionFailureFixture: ExecutionResultPayload = {
  ...executionResultFixture,
  executionState: "failed",
  summary: "执行器在拉取地形快照时超时，任务未开始。",
  changedFiles: [],
  logs: [
    { time: "10:02:11", level: "info", text: "bootstrap workspace" },
    {
      time: "10:02:40",
      level: "error",
      text: "fetch snapshot TIMEOUT after 30s"
    },
    { time: "10:02:41", level: "error", text: "run aborted" }
  ],
  warnings: [],
  failure: {
    code: "SNAPSHOT_TIMEOUT",
    message: "地形快照服务 30s 未响应",
    retryable: true
  }
};

/* ------------------------------------------------------------------ */
/* §7 Conversation                                                     */
/* ------------------------------------------------------------------ */

const message = (
  id: string,
  role: "user" | "ai",
  at: string,
  text?: string,
  interaction?: ConversationMessage["interaction"]
): ConversationMessage => ({ id, role, createdAt: at, text, interaction });

export const conversationFixture: ConversationMessage[] = [
  message(
    "m1",
    "user",
    "2026-11-05T10:01:00+08:00",
    "帮我看下这轮迭代的风险，重点森林生成。"
  ),
  message(
    "m2",
    "ai",
    "2026-11-05T10:01:12+08:00",
    "本轮迭代 12 个任务，风险集中在森林生成与地形渲染的接口耦合。"
  ),
  message("m3", "ai", "2026-11-05T10:01:14+08:00", undefined, {
    id: "int-reco-1",
    type: "recommendation",
    status: "presented",
    identity: aiIdentity,
    createdAt: "2026-11-05T10:01:14+08:00",
    title: "建议拆分森林生成任务",
    description: "先解冻地形网格接口，再实现分布算法。",
    conclusion: "拆分后本迭代可完成，且回归可复现。",
    evidence: projectEvidence,
    payload: recommendationFixture
  }),
  message(
    "m4",
    "user",
    "2026-11-05T10:03:02+08:00",
    "同意拆分，但先确认首发范围。"
  ),
  message("m5", "ai", "2026-11-05T10:03:20+08:00", undefined, {
    id: "int-question-1",
    type: "question",
    status: "presented",
    identity: aiIdentity,
    createdAt: "2026-11-05T10:03:20+08:00",
    title: "需要你确认 5 个问题",
    description: "问题会一次性提交，未完成答案自动保留草稿。",
    payload: questionFixture
  }),
  message(
    "m6",
    "user",
    "2026-11-05T10:06:40+08:00",
    "已提交，按基础分布 + 温带森林先做。"
  ),
  message(
    "m7",
    "ai",
    "2026-11-05T10:06:52+08:00",
    "收到。已记录你的选择，等待需求评审同步正式范围。"
  )
];
