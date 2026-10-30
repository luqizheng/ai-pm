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

export type TaskStatus = "done" | "doing" | "todo";

export interface GoalTask {
  id: string;
  name: string;
  status: TaskStatus;
  owner: string;
}

export interface Goal {
  id: string;
  name: string;
  description: string;
  progress: number;
  milestones: Milestone[];
  tasks: GoalTask[];
}

export const goals: Goal[] = [
  {
    id: "goal-terrain",
    name: "地形渲染方案定稿",
    description: "确定 GPU Shader 或 CPU 预计算路线，完成性能基准并合入主干。",
    progress: 40,
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
      { id: "gt-3", name: "双方案对比报告", status: "todo", owner: "元构" }
    ]
  },
  {
    id: "goal-worldgen",
    name: "世界生成管线 V2",
    description: "森林/水系/地貌程序化生成达到目标帧率，支持实时编辑。",
    progress: 65,
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
    ]
  },
  {
    id: "goal-data",
    name: "高保数据模块交付",
    description: "地理高保数据接入、校验与版本管理，支撑编辑器主流程。",
    progress: 85,
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
        status: "doing",
        owner: "质量验证 Agent"
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

export interface Decision {
  id: string;
  title: string;
  question: string;
  status: "pending" | "decided";
  options: DecisionOption[];
  evidence: string[];
  impact: string;
  conclusion?: string;
}

export const decisions: Decision[] = [
  {
    id: "dec-terrain",
    title: "地形渲染架构评审",
    question: "地形渲染采用 GPU Shader 还是 CPU 预计算方案？",
    status: "pending",
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
    impact: "影响地形/森林/水系全部渲染管线与后续 2 个迭代的排期"
  },
  {
    id: "dec-data-version",
    title: "高保数据版本策略",
    question: "高保数据按整包还是分块（tile）版本管理？",
    status: "decided",
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
    conclusion: "采用分块 tile 版本 + 边界校验（2026-10-30 评审通过）"
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

export interface KnowledgeDoc {
  id: string;
  title: string;
  version: string;
  updatedAt: string;
  source: string;
  body: string[];
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
    ]
  }
];

/* -------------------------------------------------------------------------
 * 产物验收（产品/交付物）
 * ---------------------------------------------------------------------- */

export type TestStatus = "passed" | "failed" | "running";
export type ApprovalStatus = "pending" | "approved" | "rejected";

export interface Artifact {
  id: string;
  name: string;
  version: string;
  type: string;
  testStatus: TestStatus;
  approval: ApprovalStatus;
  updatedAt: string;
  owner: string;
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
    owner: "高保数据 Agent"
  },
  {
    id: "art-terrain-shader",
    name: "地形渲染 Shader 原型",
    version: "v0.3.0",
    type: "渲染原型",
    testStatus: "running",
    approval: "pending",
    updatedAt: "今天 09:12",
    owner: "地形渲染 Agent"
  },
  {
    id: "art-forest",
    name: "森林生成原型",
    version: "v0.2.0",
    type: "程序化生成",
    testStatus: "failed",
    approval: "rejected",
    updatedAt: "昨天 19:30",
    owner: "森林生成 Agent"
  },
  {
    id: "art-editor-alpha",
    name: "世界地图编辑器 Alpha",
    version: "Build 12",
    type: "产品构建",
    testStatus: "passed",
    approval: "approved",
    updatedAt: "前天 17:00",
    owner: "质量验证 Agent"
  }
];

/** 右栏默认 AI 建议（V1.1 右侧上下文栏图面） */
export const contextAdvice = "先验证 Shader 原型性能，再做正式架构选择";

/** 右栏两个大占位区（V1.1 图面） */
export const contextPlaceholders = ["批量原型验证", "补充证据"];
