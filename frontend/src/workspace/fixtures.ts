/**
 * 工作区演示数据 —— 总控台 V1.1（docs/ai-pm-ui-console-v1-1.md）图面文案照录。
 *
 * 与 src/api/mock/fixtures.ts（BFF 契约活文档，node 侧加载）不同：
 * 这里是浏览器侧纯前端演示数据，P0 零后端改动；P1 对接真数据时整体替换。
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

export interface GoalEntry {
  id: string;
  name: string;
  milestone: string;
  progress: number;
}

export const goals: GoalEntry[] = [
  {
    id: "goal-terrain",
    name: "地形渲染方案定稿",
    milestone: "架构评审通过",
    progress: 40
  },
  {
    id: "goal-worldgen",
    name: "世界生成管线 V2",
    milestone: "森林生成性能达标",
    progress: 65
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

/** 右栏默认 AI 建议（V1.1 右侧上下文栏图面） */
export const contextAdvice = "先验证 Shader 原型性能，再做正式架构选择";

/** 右栏两个大占位区（V1.1 图面） */
export const contextPlaceholders = ["批量原型验证", "补充证据"];
