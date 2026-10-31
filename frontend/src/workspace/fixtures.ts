/**
 * 工作区演示数据 —— 总控台 V1.1（docs/ai-pm-ui-console-v1-1.md）图面文案照录
 * + 全页签 mock 数据（用户裁决 2026-10-31：先写 mock 数据，完成所有相关页面）。
 *
 * 与 src/api/mock/fixtures.ts（BFF 契约活文档，node 侧加载）不同：
 * 这里是浏览器侧纯前端演示数据，零后端改动；P1 对接真数据时整体替换。
 * 数据彼此一致：4 Agent（1 等待审批）、3 项待处理、高保数据模块待验收。
 */

export const demoProjectId = "proj-worldmap";

export interface DemoProject {
  id: string;
  name: string;
  iteration: string;
}

export const demoProject: DemoProject = {
  id: demoProjectId,
  name: "世界地图编辑器",
  iteration: "Iteration 2.0"
};

/** 待处理类型：决策（橙）/ 验收（蓝）/ 阻塞（红）—— V1.1 图面 */
export type AttentionKind = "decision" | "acceptance" | "blocker";

export interface AttentionItem {
  id: string;
  kind: AttentionKind;
  title: string;
  description: string;
  advice: string;
}

export const attentionItems: AttentionItem[] = [
  {
    id: "att-terrain-review",
    kind: "decision",
    title: "地形渲染架构评审",
    description: "GPU Shader 与 CPU 预计算方案",
    advice: "先验证 Shader 原型性能，再做正式架构选择"
  },
  {
    id: "att-data-acceptance",
    kind: "acceptance",
    title: "高保数据模块验收",
    description: "测试已通过，等待产物审查",
    advice: "按验收清单核对产物，通过后合入主线"
  },
  {
    id: "att-forest-perf",
    kind: "blocker",
    title: "森林生成性能页",
    description: "缺少目标帧率与视距范围定义",
    advice: "先补目标帧率与视距范围定义，再解除阻塞"
  }
];

export interface Briefing {
  author: string;
  body: string;
  suggestion: string;
}

export const briefing: Briefing = {
  author: "元构",
  body: "迭代整体正常，但存在一个架构决策和一个性能阻塞 issue，可能影响后续开发。",
  suggestion: "优先确认地形渲染方案，再推进相关实现。"
};

export interface MetricCard {
  id: "tasks" | "interventions" | "agents";
  value: number;
  caption: string;
  sub: string;
}

export const metricCards: MetricCard[] = [
  {
    id: "tasks",
    value: 12,
    caption: "迭代任务",
    sub: "5 项执行中"
  },
  {
    id: "interventions",
    value: 3,
    caption: "需要介入",
    sub: "1 项最优先"
  },
  {
    id: "agents",
    value: 4,
    caption: "活跃 Agent",
    sub: "1 等待审批"
  }
];

export interface DynamicEntry {
  id: string;
  kind: "change" | "failure" | "decision";
  text: string;
  time: string;
}

export const dynamics: DynamicEntry[] = [
  {
    id: "dyn-1",
    kind: "change",
    text: "高保数据模块全部测试通过，进入产物验收",
    time: "今天 10:24"
  },
  {
    id: "dyn-2",
    kind: "failure",
    text: "森林生成性能测试未达标，已创建阻塞项",
    time: "今天 09:51"
  },
  {
    id: "dyn-3",
    kind: "decision",
    text: "地形渲染架构评审进入决策流程（GPU Shader vs CPU 预计算）",
    time: "昨天 18:03"
  },
  {
    id: "dyn-4",
    kind: "change",
    text: "迭代 2.0 计划刷新，新增 3 项任务",
    time: "昨天 16:40"
  }
];

/* -------------------------------------------------------------------------
 * 目标计划：项目 → 目标 → 里程碑 + 任务树（mock，含依赖叙事）
 * ---------------------------------------------------------------------- */

export type MilestoneStatus = "done" | "active" | "pending";

export interface Milestone {
  id: string;
  name: string;
  due: string;
  status: MilestoneStatus;
}

export type TaskStatus =
  | "done"
  | "doing"
  | "todo"
  | "blocked"
  | "awaiting-acceptance"
  | "proposed";

export interface GoalTask {
  id: string;
  name: string;
  status: TaskStatus;
  owner: string;
}

export interface Goal {
  id: string;
  /** V1.2：目标编码（GOAL-001 …） */
  code: string;
  name: string;
  description: string;
  progress: number;
  /** V1.2：进度依据——不得是无依据的 AI 估算百分比 */
  progressBasis: string;
  /** V1.2：依赖链（依赖图视图的数据源） */
  dependencies: string[];
  milestones: Milestone[];
  tasks: GoalTask[];
  /** V1.2：AI 提议的计划（草案），未批准不进执行 */
  proposedTasks: GoalTask[];
}

export const goals: Goal[] = [
  {
    id: "goal-terrain",
    code: "GOAL-001",
    name: "地形渲染方案定稿",
    description: "确定 GPU Shader 或 CPU 预计算路线，完成性能基准并合入主干。",
    progress: 40,
    progressBasis: "基于计划权重",
    dependencies: ["地形架构决策", "地形渲染实现", "系统集成测试"],
    milestones: [
      {
        id: "ms-1",
        name: "Shader 原型验证",
        due: "Iteration 2.0 W2",
        status: "done"
      },
      {
        id: "ms-2",
        name: "架构评审通过",
        due: "Iteration 2.0 W3",
        status: "active"
      },
      {
        id: "ms-3",
        name: "方案合入主干",
        due: "Iteration 2.0 W4",
        status: "pending"
      }
    ],
    tasks: [
      {
        id: "gt-1",
        name: "GPU Shader 原型性能基准",
        status: "done",
        owner: "地形渲染 Agent"
      },
      {
        id: "gt-2",
        name: "CPU 预计算原型性能基准",
        status: "doing",
        owner: "地形渲染 Agent"
      },
      {
        id: "gt-3",
        name: "双方案对比报告",
        status: "blocked",
        owner: "元构"
      }
    ],
    proposedTasks: [
      {
        id: "pt-1",
        name: "新增：视距三档性能采样任务",
        status: "proposed",
        owner: "地形渲染 Agent"
      }
    ]
  },
  {
    id: "goal-worldgen",
    code: "GOAL-002",
    name: "世界生成管线 V2",
    description: "森林/水系/地貌程序化生成达到目标帧率，支持实时编辑。",
    progress: 65,
    progressBasis: "基于计划权重",
    dependencies: ["地形渲染实现", "森林分布逻辑", "性能回归"],
    milestones: [
      {
        id: "ms-4",
        name: "森林生成逻辑可用",
        due: "Iteration 2.0 W2",
        status: "done"
      },
      {
        id: "ms-5",
        name: "森林生成性能达标",
        due: "Iteration 2.0 W4",
        status: "active"
      },
      {
        id: "ms-6",
        name: "管线 V2 整体联调",
        due: "Iteration 3.0 W2",
        status: "pending"
      }
    ],
    tasks: [
      {
        id: "gt-4",
        name: "AI 实现森林分布逻辑",
        status: "done",
        owner: "森林生成 Agent"
      },
      {
        id: "gt-5",
        name: "目标帧率与视距范围定义",
        status: "todo",
        owner: "元构"
      },
      {
        id: "gt-6",
        name: "性能回归测试",
        status: "todo",
        owner: "质量验证 Agent"
      }
    ],
    proposedTasks: [
      {
        id: "pt-2",
        name: "新增：LOD 分级采样策略任务",
        status: "proposed",
        owner: "森林生成 Agent"
      }
    ]
  },
  {
    id: "goal-data",
    code: "GOAL-003",
    name: "高保数据模块交付",
    description: "地理高保数据接入、校验与版本管理，支撑编辑器主流程。",
    progress: 85,
    progressBasis: "基于计划权重",
    dependencies: ["高保数据校验", "产物验收"],
    milestones: [
      {
        id: "ms-7",
        name: "数据接入与校验",
        due: "Iteration 2.0 W1",
        status: "done"
      },
      {
        id: "ms-8",
        name: "产物验收通过",
        due: "Iteration 2.0 W3",
        status: "active"
      }
    ],
    tasks: [
      {
        id: "gt-7",
        name: "高保数据模块全部测试",
        status: "done",
        owner: "高保数据 Agent"
      },
      {
        id: "gt-8",
        name: "产物审查与合入",
        status: "awaiting-acceptance",
        owner: "质量验证 Agent"
      }
    ],
    proposedTasks: [
      {
        id: "pt-3",
        name: "新增：tile 边界一致性抽查",
        status: "proposed",
        owner: "高保数据 Agent"
      }
    ]
  }
];

/* -------------------------------------------------------------------------
 * AI 团队：4 名 Agent（与总控台指标一致：1 等待审批）
 * ---------------------------------------------------------------------- */

export type AgentStatus = "working" | "waiting-approval" | "idle";

export interface Agent {
  id: string;
  name: string;
  role: string;
  status: AgentStatus;
  currentTask: string;
  completed: number;
  lastActive: string;
}

export const agents: Agent[] = [
  {
    id: "agent-yuangou",
    name: "元构",
    role: "AI 研发负责人",
    status: "idle",
    currentTask: "协调迭代 2.0，跟进架构评审与阻塞项",
    completed: 36,
    lastActive: "刚刚"
  },
  {
    id: "agent-terrain",
    name: "地形渲染 Agent",
    role: "客户端图形",
    status: "working",
    currentTask: "CPU 预计算原型性能基准",
    completed: 12,
    lastActive: "5 分钟前"
  },
  {
    id: "agent-forest",
    name: "森林生成 Agent",
    role: "程序化生成",
    status: "waiting-approval",
    currentTask: "森林分布逻辑 v0.2 提交待审批",
    completed: 9,
    lastActive: "20 分钟前"
  },
  {
    id: "agent-data",
    name: "高保数据 Agent",
    role: "数据管线",
    status: "working",
    currentTask: "高保数据模块产物审查材料准备",
    completed: 21,
    lastActive: "刚刚"
  }
];

/* -------------------------------------------------------------------------
 * 需求管理（研发模块）
 * ---------------------------------------------------------------------- */

export type RequirementStatus =
  | "draft"
  | "reviewing"
  | "confirmed"
  | "implementing"
  | "accepted";

export interface Requirement {
  id: string;
  title: string;
  description: string;
  status: RequirementStatus;
  priority: "P0" | "P1" | "P2";
  acceptance: string[];
  source: string;
}

export const requirements: Requirement[] = [
  {
    id: "REQ-101",
    title: "地形渲染性能指标定义",
    description: "明确目标帧率与视距范围，解除森林生成性能阻塞。",
    status: "reviewing",
    priority: "P0",
    acceptance: [
      "目标帧率 ≥ 60 FPS（中端 GPU）",
      "视距范围 2km / 5km / 10km 三档可配",
      "指标写入架构文档并评审通过"
    ],
    source: "森林生成性能阻塞复盘"
  },
  {
    id: "REQ-102",
    title: "森林分布逻辑 AI 实现",
    description: "按气候/海拔/湿度规则程序化生成森林分布，支持笔刷覆盖。",
    status: "implementing",
    priority: "P1",
    acceptance: [
      "分布规则可配置并实时预览",
      "10km² 场景生成时间 ≤ 3s",
      "与地形 LOD 协同无接缝"
    ],
    source: "世界生成管线 V2 规格"
  },
  {
    id: "REQ-103",
    title: "高保数据校验规则",
    description: "接入地理高保数据的完整性/一致性校验，失败可定位到字段。",
    status: "accepted",
    priority: "P1",
    acceptance: ["校验规则可配置", "失败报告定位到字段级", "全部测试通过"],
    source: "高保数据模块设计稿"
  },
  {
    id: "REQ-104",
    title: "双渲染架构对比报告",
    description: "GPU Shader 与 CPU 预计算在性能/功耗/可维护性上的对比结论。",
    status: "draft",
    priority: "P0",
    acceptance: [
      "同一场景集基准数据",
      "三个维度打分与推荐结论",
      "评审会材料就绪"
    ],
    source: "地形渲染架构评审"
  },
  {
    id: "REQ-105",
    title: "编辑器实时协作草案",
    description: "多人同时编辑同一地图的冲突策略与光标可见性草案。",
    status: "draft",
    priority: "P2",
    acceptance: ["冲突策略文档化", "原型演示一场"],
    source: "产品路线讨论"
  }
];

/* -------------------------------------------------------------------------
 * 决策中心（双方案对比 + 证据）
 * ---------------------------------------------------------------------- */

export interface DecisionOption {
  name: string;
  pros: string[];
  cons: string[];
}

export interface DecisionDimension {
  name: string;
  /** 每个候选方案在该维度上的取值（顺序与 options 对齐） */
  values: string[];
}

export interface Decision {
  id: string;
  title: string;
  question: string;
  status: "pending" | "decided";
  /** V1.2：问题背景 */
  background: string;
  options: DecisionOption[];
  /** V1.2：比较维度 */
  dimensions: DecisionDimension[];
  evidence: string[];
  /** V1.2：AI 推荐理由 */
  recommendation: string;
  /** V1.2：风险 */
  risks: string[];
  impact: string;
  conclusion?: string;
  /** V1.2：决策人与批准时间 */
  decidedBy?: string;
  approvedAt?: string;
  /** V1.2：后续验证结果 */
  verification?: string[];
  /** V1.2：需求变化触发的重新评估提示（系统主动提示，不让 Agent 按旧决策执行） */
  reassess?: { reason: string; triggeredBy: string };
}

export const decisions: Decision[] = [
  {
    id: "dec-terrain",
    title: "地形渲染架构评审",
    question: "地形渲染采用 GPU Shader 还是 CPU 预计算方案？",
    status: "pending",
    background:
      "迭代 1.0 的渲染路线在低端 GPU 上出现过兼容性回退；迭代 2.0 需要在架构层面定稿，否则地形/森林/水系三条管线都无法推进。",
    dimensions: [
      {
        name: "运行时性能",
        values: ["1080p 中档 72 FPS 实测", "帧率稳定但重算等待 1.8s/km²"]
      },
      {
        name: "低端机兼容",
        values: ["核显用例未跑全", "无 GPU 依赖，兼容性好"]
      },
      { name: "可维护性", values: ["Shader 调试成本高", "逻辑可单测"] }
    ],
    options: [
      {
        name: "GPU Shader",
        pros: ["运行时零预计算等待", "编辑实时反馈好", "业界主流路线"],
        cons: ["低端 GPU 兼容风险", "Shader 调试成本高"]
      },
      {
        name: "CPU 预计算",
        pros: ["帧率稳定可预测", "低端机友好", "逻辑易测试"],
        cons: ["编辑后需重算（秒级等待）", "内存占用更高"]
      }
    ],
    evidence: [
      "GPU Shader 原型基准（已完成）：1080p 中档 72 FPS",
      "CPU 预计算基准（进行中）：预计重算 1.8s / km²",
      "历史决策：迭代 1.0 曾因 Shader 兼容性回退一次"
    ],
    impact: "影响地形/森林/水系全部渲染管线与后续 2 个迭代的排期",
    recommendation:
      "在核显兼容用例补齐前维持双原型并行；若 CPU 重算压进 1s/km² 以内，建议预计算优先。",
    risks: [
      "若仓促定稿 GPU 路线，可能重演迭代 1.0 的兼容性回退",
      "双线并行消耗两个 Agent 的算力配额"
    ]
  },
  {
    id: "dec-data-version",
    title: "高保数据版本策略",
    question: "高保数据按整包还是分块（tile）版本管理？",
    status: "decided",
    background:
      "高保数据接入后，任意字段修改都会触发数据更新；版本管理粒度直接决定编辑器加载与协作体验。",
    dimensions: [
      { name: "下载体积", values: ["整包约 380MB", "tile 均值 6MB"] },
      {
        name: "实现复杂度",
        values: ["简单，一致性易保证", "边界一致性需额外校验"]
      }
    ],
    options: [
      {
        name: "整包版本",
        pros: ["实现简单", "一致性易保证"],
        cons: ["任意修改触发整包下载"]
      },
      {
        name: "分块 tile 版本",
        pros: ["增量下载小", "并行编辑友好"],
        cons: ["边界一致性需要额外校验"]
      }
    ],
    evidence: [
      "10km² 整包约 380MB，tile 均值 6MB",
      "协作编辑需求（REQ-105）预期"
    ],
    impact: "决定数据管线与编辑器加载架构",
    recommendation:
      "协作编辑（REQ-105）已在路线图上，分块 tile 的增量与并行收益更大。",
    risks: ["tile 边界一致性缺陷可能导致跨块地物错位"],
    conclusion: "采用分块 tile 版本 + 边界校验（2026-10-30 评审通过）",
    decidedBy: "用户（PM）",
    approvedAt: "2026-10-30 18:03",
    verification: [
      "tile 下载体积实测 4-9MB，符合预期",
      "边界抽查抽样 50 块，未发现错位"
    ],
    reassess: {
      reason:
        "REQ-105 实时协作草案引入多人并行编辑，tile 边界校验的触发时机与粒度需要重新评估。",
      triggeredBy: "REQ-105"
    }
  }
];

/* -------------------------------------------------------------------------
 * 项目认知（文档树 + 阅读器）
 * ---------------------------------------------------------------------- */

export interface DocNode {
  id: string;
  name: string;
  children?: DocNode[];
}

export const docTree: DocNode[] = [
  {
    id: "grp-req",
    name: "需求文档",
    children: [
      { id: "doc-req-v2", name: "迭代 2.0 需求文档.md" },
      { id: "doc-req-perf", name: "性能指标定义（草案）.md" }
    ]
  },
  {
    id: "grp-arch",
    name: "架构概述",
    children: [
      { id: "doc-arch-render", name: "地形渲染架构备选方案.md" },
      { id: "doc-arch-pipeline", name: "世界生成管线 V2.md" }
    ]
  },
  { id: "doc-meeting", name: "评审会议纪要 1029.md" }
];

export type DocReview = "approved" | "ai-draft";

export interface DocReference {
  kind: "decision" | "requirement" | "artifact";
  id: string;
  title: string;
}

export interface KnowledgeDoc {
  id: string;
  title: string;
  version: string;
  updatedAt: string;
  source: string;
  body: string[];
  /** V1.2：审核状态——AI 摘要不得覆盖人工批准的正式事实 */
  review: DocReview;
  approvedFacts: string[];
  aiSummary: string;
  references: DocReference[];
}

export const knowledgeDocs: KnowledgeDoc[] = [
  {
    id: "doc-arch-render",
    title: "地形渲染架构备选方案",
    version: "v0.3",
    updatedAt: "今天 11:02",
    source: "架构评审 · 地形渲染架构评审",
    body: [
      "方案 A（GPU Shader）：把高度/法线/植被混合全部放到顶点与像素着色器，运行时零预计算等待，编辑反馈即时。基准场景 1080p 中档 GPU 实测 72 FPS，超出目标帧率。",
      "方案 B（CPU 预计算）：地形网格离线烘焙，运行时只做采样。帧率最稳，低端机友好；代价是每次编辑触发约 1.8s/km² 的重算等待。",
      "待决事项：低端 GPU（Intel 核显）兼容性尚未跑全用例；预计算内存曲线待测量。建议先补齐两组数据再进入评审。"
    ],
    review: "ai-draft",
    approvedFacts: [
      "迭代 1.0 渲染路线因低端 GPU 兼容性回退（评审会纪要 1029 批准存档）"
    ],
    aiSummary:
      "AI 维护摘要：两方案原型基准已完成一组对比数据，等待 CPU 侧内存曲线补齐后进入评审。",
    references: [
      { kind: "decision", id: "dec-terrain", title: "地形渲染架构评审" },
      { kind: "requirement", id: "REQ-104", title: "双渲染架构对比报告" }
    ]
  },
  {
    id: "doc-req-v2",
    title: "迭代 2.0 需求文档",
    version: "v1.4",
    updatedAt: "昨天 16:40",
    source: "需求管理 · 迭代计划",
    body: [
      "迭代 2.0 聚焦三件事：地形渲染架构定稿、森林生成性能达标、高保数据模块交付验收。",
      "本迭代新增 12 项任务，其中 5 项与渲染路径相关。架构决策未落定前，森林生成性能任务保持阻塞。",
      "验收基线沿用迭代 1.0 的性能回归集，新增视距三档（2/5/10km）配置化要求。"
    ],
    review: "approved",
    approvedFacts: [
      "迭代 2.0 范围 = 三件事（计划评审批准）",
      "验收基线沿用迭代 1.0 性能回归集 + 视距三档要求"
    ],
    aiSummary:
      "AI 维护摘要：当前 12 项任务中 5 项被架构决策阻塞，建议优先处理决策中心待办。",
    references: [
      { kind: "decision", id: "dec-terrain", title: "地形渲染架构评审" },
      { kind: "requirement", id: "REQ-101", title: "地形渲染性能指标定义" }
    ]
  },
  {
    id: "doc-meeting",
    title: "评审会议纪要 1029",
    version: "v1.0",
    updatedAt: "昨天 18:03",
    source: "项目知识 · 会议纪要",
    body: [
      "结论一：高保数据版本策略采用分块 tile + 边界校验，数据管线按此推进。",
      "结论二：地形渲染架构评审延后至基准数据补齐，阻塞项由 REQ-101 承接。",
      "行动项：元构整理双方案对比报告；地形渲染 Agent 补核显兼容用例。"
    ],
    review: "approved",
    approvedFacts: [
      "结论一：分块 tile + 边界校验（正式决策 dec-data-version）",
      "结论二：架构评审延后至基准数据补齐"
    ],
    aiSummary:
      "AI 维护摘要：两项行动项分别由元构与地形渲染 Agent 承接，进行中。",
    references: [
      { kind: "decision", id: "dec-data-version", title: "高保数据版本策略" },
      { kind: "requirement", id: "REQ-101", title: "地形渲染性能指标定义" }
    ]
  }
];

/* -------------------------------------------------------------------------
 * 产物验收（产品/交付物）
 * ---------------------------------------------------------------------- */

export type TestStatus = "passed" | "failed" | "running";
export type ApprovalStatus = "pending" | "approved" | "rejected";

export type DiffLineType = "add" | "del" | "ctx";

export interface DiffLine {
  type: DiffLineType;
  text: string;
}

export interface DiffFile {
  path: string;
  diff: DiffLine[];
  comments: { line: number; author: string; text: string }[];
}

export interface TestReportItem {
  name: string;
  result: "passed" | "failed" | "skipped";
}

export type AcceptanceVerdict = "partial" | "meets" | "not-met";

export interface Artifact {
  id: string;
  name: string;
  version: string;
  type: string;
  testStatus: TestStatus;
  approval: ApprovalStatus;
  updatedAt: string;
  owner: string;
  /** V1.2：文件树 + 统一 Diff + 行级评论 */
  files: DiffFile[];
  /** V1.2：测试报告 */
  testReport: TestReportItem[];
  /** V1.2：需求验收标准核对 */
  acceptanceCriteria: { text: string; passed: boolean }[];
  /** V1.2：关联任务 */
  linkedTask: string;
  /** V1.2：整体判定——「部分检查通过」≠「符合验收标准」 */
  verdict: AcceptanceVerdict;
}

export const artifacts: Artifact[] = [
  {
    id: "art-data-v1",
    name: "高保数据模块",
    version: "v1.0.0",
    type: "数据管线",
    testStatus: "passed",
    approval: "pending",
    updatedAt: "今天 10:24",
    owner: "高保数据 Agent",
    files: [
      {
        path: "pipeline/validate.rs",
        diff: [
          {
            type: "ctx",
            text: "fn validate_tile(tile: &Tile) -> Result<()> {"
          },
          { type: "add", text: "    let boundary = check_boundary(tile)?;" },
          {
            type: "add",
            text: '    ensure!(boundary.gaps.is_empty(), "tile 边界存在缺口");'
          },
          { type: "ctx", text: "    Ok(())" },
          { type: "del", text: "    // TODO: 边界校验待补" },
          { type: "ctx", text: "}" }
        ],
        comments: [
          {
            line: 2,
            author: "质量验证 Agent",
            text: "边界缺口已覆盖 50 块抽样，建议补充跨 LOD 用例。"
          }
        ]
      },
      {
        path: "pipeline/version.rs",
        diff: [
          {
            type: "ctx",
            text: "pub fn split_version(pkg: &Package) -> Vec<Tile> {"
          },
          { type: "add", text: "    tiles.sort_by_key(|t| t.coord);" },
          { type: "ctx", text: "    tiles" },
          { type: "ctx", text: "}" }
        ],
        comments: []
      }
    ],
    testReport: [
      { name: "校验规则 · 字段级定位", result: "passed" },
      { name: "tile 边界 · 50 块抽样", result: "passed" },
      { name: "性能验证 · 大场景导入", result: "skipped" }
    ],
    acceptanceCriteria: [
      { text: "校验规则可配置", passed: true },
      { text: "失败报告定位到字段级", passed: true },
      { text: "大场景导入性能达标", passed: false }
    ],
    linkedTask: "gt-8",
    verdict: "partial"
  },
  {
    id: "art-terrain-shader",
    name: "地形渲染 Shader 原型",
    version: "v0.3.0",
    type: "渲染原型",
    testStatus: "running",
    approval: "pending",
    updatedAt: "今天 09:12",
    owner: "地形渲染 Agent",
    files: [
      {
        path: "shaders/terrain.frag",
        diff: [
          { type: "ctx", text: "void main() {" },
          { type: "add", text: "  float slope = normalize(v_normal).y;" },
          { type: "add", text: "  // 岩壁与草坡混合权重" },
          {
            type: "ctx",
            text: "  fragColor = mix(grass, rock, step(0.6, slope));"
          },
          { type: "ctx", text: "}" }
        ],
        comments: []
      }
    ],
    testReport: [
      { name: "基准场景 · 1080p 中档", result: "passed" },
      { name: "核显兼容 · 用例 3/12", result: "skipped" }
    ],
    acceptanceCriteria: [
      { text: "同一场景集基准数据", passed: true },
      { text: "核显兼容用例全量通过", passed: false }
    ],
    linkedTask: "gt-1",
    verdict: "partial"
  },
  {
    id: "art-forest",
    name: "森林生成原型",
    version: "v0.2.0",
    type: "程序化生成",
    testStatus: "failed",
    approval: "rejected",
    updatedAt: "昨天 19:30",
    owner: "森林生成 Agent",
    files: [
      {
        path: "gen/forest.rs",
        diff: [
          {
            type: "ctx",
            text: "pub fn distribute(ctx: &GenContext) -> Forest {"
          },
          {
            type: "add",
            text: "    let rules = ClimateRules::from_config(&ctx.cfg);"
          },
          { type: "add", text: "    // TODO: 海拔权重未接入" },
          { type: "ctx", text: "    Forest::from(rules)" },
          { type: "ctx", text: "}" }
        ],
        comments: [
          {
            line: 3,
            author: "元构",
            text: "海拔权重缺失导致山地区域森林过量，补齐后再提审。"
          }
        ]
      }
    ],
    testReport: [
      { name: "分布规则 · 平原用例", result: "passed" },
      { name: "分布规则 · 山地用例", result: "failed" },
      { name: "生成时间 · 10km² ≤ 3s", result: "failed" }
    ],
    acceptanceCriteria: [
      { text: "分布规则可配置并实时预览", passed: true },
      { text: "10km² 场景生成时间 ≤ 3s", passed: false },
      { text: "与地形 LOD 协同无接缝", passed: false }
    ],
    linkedTask: "gt-4",
    verdict: "not-met"
  },
  {
    id: "art-editor-alpha",
    name: "世界地图编辑器 Alpha",
    version: "Build 12",
    type: "产品构建",
    testStatus: "passed",
    approval: "approved",
    updatedAt: "前天 17:00",
    owner: "质量验证 Agent",
    files: [
      {
        path: "app/build-manifest.json",
        diff: [
          { type: "ctx", text: "{" },
          { type: "add", text: '  "build": 12,' },
          { type: "add", text: '  "channel": "alpha",' },
          { type: "ctx", text: '  "modules": ["terrain", "forest", "data"]' },
          { type: "ctx", text: "}" }
        ],
        comments: []
      }
    ],
    testReport: [
      { name: "冒烟 · 主流程", result: "passed" },
      { name: "回归 · 迭代 1.0 基线", result: "passed" }
    ],
    acceptanceCriteria: [
      { text: "主流程冒烟通过", passed: true },
      { text: "迭代 1.0 回归集通过", passed: true }
    ],
    linkedTask: "gt-7",
    verdict: "meets"
  }
];

/* -------------------------------------------------------------------------
 * AI 执行中心（V1.2）：双栏执行视图数据。
 * 四类事件：thought（AI 思考摘要）/ tool（工具执行记录）/
 * state（真实业务状态变化）/ approval（人工审批请求）——不混在聊天气泡里。
 * 进度用「可验证阶段状态 + 已完成检查点 + 剩余工作」，不用 AI 估算百分比。
 * ---------------------------------------------------------------------- */

export type RunEventType = "thought" | "tool" | "state" | "approval";

export interface RunToolCall {
  name: string;
  input: string;
  output: string;
  durationMs: number;
}

export interface RunStep {
  id: string;
  seq: number;
  type: RunEventType;
  title: string;
  detail: string;
  status: "done" | "running" | "waiting";
  tool?: RunToolCall;
  logLines?: string[];
  codeChange?: { file: string; additions: number; deletions: number };
  checkpoint?: boolean;
}

export interface AgentRun {
  id: string;
  title: string;
  agent: string;
  goalCode: string;
  /** 可验证的阶段状态（非百分比） */
  phase: string;
  checkpoints: string[];
  remaining: string[];
  steps: RunStep[];
}

export const agentRun: AgentRun = {
  id: "run-forest-v02",
  title: "森林分布逻辑 v0.2 实现",
  agent: "森林生成 Agent",
  goalCode: "GOAL-002",
  phase: "实现完成 · 提交待人工审批（审批通过后进入性能回归）",
  checkpoints: [
    "气候规则引擎可用（gt-4 前置）",
    "平原用例全部通过",
    "代码已提交 review 分支"
  ],
  remaining: [
    "人工审批提交内容（当前阻塞点）",
    "海拔权重补齐（山地用例失败根因）",
    "10km² 生成时间 ≤ 3s 达标",
    "性能回归测试（gt-6）"
  ],
  steps: [
    {
      id: "step-1",
      seq: 1,
      type: "thought",
      title: "分析分布规则拆分",
      detail:
        "把森林分布拆为气候/海拔/湿度三个规则源，先实现气候源并保留扩展点，避免一次性引入全量复杂度。",
      status: "done",
      checkpoint: true
    },
    {
      id: "step-2",
      seq: 2,
      type: "tool",
      title: "读取生成配置模板",
      detail: "拉取 gen/config/climate.toml 作为规则源模板。",
      status: "done",
      tool: {
        name: "read_file",
        input: "gen/config/climate.toml",
        output: "42 行，含温度/湿度区间定义",
        durationMs: 120
      }
    },
    {
      id: "step-3",
      seq: 3,
      type: "state",
      title: "分布逻辑主实现合入工作区",
      detail:
        "Forest 生成主流程 + 气候规则源接入完成，业务状态从「未开始」变为「实现完成」。",
      status: "done",
      codeChange: { file: "gen/forest.rs", additions: 86, deletions: 12 },
      logLines: [
        "gen/forest.rs: ClimateRules::from_config 接入",
        "gen/forest.rs: Forest::from(rules) 主流程",
        "状态变更：forest.implementation = done"
      ],
      checkpoint: true
    },
    {
      id: "step-4",
      seq: 4,
      type: "tool",
      title: "运行平原用例",
      detail: "执行分布规则单元测试（平原子集）。",
      status: "done",
      tool: {
        name: "run_tests",
        input: "cargo test --plain",
        output: "18 passed, 0 failed",
        durationMs: 4200
      },
      logLines: [
        "test distribute::plain::ok ... ok",
        "test climate::regions ... ok"
      ]
    },
    {
      id: "step-5",
      seq: 5,
      type: "thought",
      title: "山地用例失败归因",
      detail:
        "山地用例失败：海拔权重未接入导致森林过量。判断为范围遗漏而非规则缺陷，需要补实现或先提交审批征询。",
      status: "done"
    },
    {
      id: "step-6",
      seq: 6,
      type: "approval",
      title: "v0.2 提交待人工审批",
      detail:
        "提交内容含已知缺口（海拔权重）。请求审批：A) 按现状合入 review 分支继续联调；B) 先补齐海拔权重再提交。",
      status: "waiting"
    }
  ]
};

/** 右栏默认 AI 建议（V1.1 右侧上下文栏图面） */
export const contextAdvice = "先验证 Shader 原型性能，再做正式架构选择";

/** 右栏两个大占位区（V1.1 图面） */
export const contextPlaceholders = ["批量原型验证", "补充证据"];
