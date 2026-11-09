/**
 * 任务总览 · AI 规划建议演示数据（S04-UI-01 §8，dev mock 专用）。
 *
 * 仅由 TasksOverviewView 在 VITE_USE_MOCK=1 下**动态**引入；发布构建该
 * 分支被静态剔除，本文件不进 dist（build/release-guard.ts 硬失败兜底）。
 * 演示文案为 fixtures 豁免域（i18n 硬编码门禁豁免 fixtures.ts）。
 */

export interface OverviewInsightFixture {
  id: string;
  title: string;
  detail: string;
  source: string;
  impact: string;
}

export function overviewInsights(): OverviewInsightFixture[] {
  return [
    {
      id: "insight-1",
      title: "3 个任务可能遗漏自实施基线",
      detail:
        "BASE-v1.2 的「令牌刷新」小节没有对应任务；建议拆解页复核登录 API 的子任务覆盖。",
      source: "BASE-v1.2 §4.2",
      impact: "覆盖缺口可能导致验收阶段返工"
    },
    {
      id: "insight-2",
      title: "2 个任务可并行执行",
      detail: "登录表单（前端）与令牌校验（core-api）互不依赖，可并行排期。",
      source: "任务依赖图",
      impact: "预计缩短关键路径 2 天"
    },
    {
      id: "insight-3",
      title: "1 个任务存在未解决的依赖",
      detail: "「限流策略」依赖的网关任务仍处于阻塞状态，建议先解决上游。",
      source: "任务依赖图",
      impact: "阻塞会顺延下游 3 个任务"
    }
  ];
}
