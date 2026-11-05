/**
 * Project List 演示数据（mock 先行）。
 *
 * 本文件为 fixtures（i18n 硬编码门禁豁免）：项目名/描述等**领域数据**走这里；
 * 界面文案（指标/过滤/动作）走 `locales/<locale>/pm.yaml` 的 `projects:` 命名空间。
 * 领域模型见 `project-entry.ts`。
 *
 * 时间用相对当前时刻生成：保证「最近活跃 / 需要关注」指标在任何演示时点都有
 * 可见样本（需求同 requirements/fixtures 的演示口径）。
 */

import type { ProjectEntry } from "./project-entry";

/** 距今 days 天前的展示时间戳（固定时刻，保证快照稳定） */
function daysAgo(days: number, hour = "10:00"): string {
  const date = new Date(Date.now() - days * 24 * 60 * 60 * 1000);
  return `${date.toISOString().slice(0, 10)} ${hour}`;
}

/** 演示项目集合：覆盖 多仓库 / 无仓库 / 无主仓库 / 仓库不可达 / 已归档 */
export const projectEntriesFixture: ProjectEntry[] = [
  {
    id: "proj-001",
    code: "ATLAS",
    name: "Atlas Platform",
    description:
      "研发协作主平台：需求治理、AI 执行管线与知识空间的一体化底座。",
    status: "active",
    customer_name: "内部孵化和预研",
    repositories: {
      count: 2,
      primary_name: "atlas-backend",
      has_unreachable: false
    },
    updated_at: daysAgo(0, "16:20"),
    created_at: daysAgo(96, "09:00")
  },
  {
    id: "proj-002",
    code: "BILLING",
    name: "Billing Core",
    description: "计费核心：用量归集、账单生成与对账，支撑订阅与按量计费。",
    status: "active",
    customer_name: "Northwind Retail",
    repositories: {
      count: 1,
      primary_name: "billing-core",
      has_unreachable: true
    },
    updated_at: daysAgo(2, "11:40"),
    created_at: daysAgo(61, "14:00")
  },
  {
    id: "proj-003",
    code: "DOCS",
    name: "Docs Portal",
    description: "产品文档门户：双语内容发布与检索，面向外部开发者。",
    status: "active",
    customer_name: null,
    repositories: {
      count: 0,
      primary_name: null,
      has_unreachable: false
    },
    updated_at: daysAgo(1, "14:05"),
    created_at: daysAgo(9, "10:30")
  },
  {
    id: "proj-004",
    code: "MOBILE",
    name: "Mobile Companion",
    description: "移动端伴侣应用：审批与通知触达，配套 iOS / Android 客户端。",
    status: "active",
    customer_name: "Northwind Retail",
    repositories: {
      count: 3,
      primary_name: null,
      has_unreachable: false
    },
    updated_at: daysAgo(13, "17:45"),
    created_at: daysAgo(40, "09:20")
  },
  {
    id: "proj-005",
    code: "LEGACY",
    name: "Legacy Reporter",
    description: "旧报表系统：已由 Billing Core 取代，保留归档以供审计追溯。",
    status: "archived",
    customer_name: "内部孵化和预研",
    repositories: {
      count: 1,
      primary_name: "legacy-reporter",
      has_unreachable: false
    },
    updated_at: daysAgo(35, "18:00"),
    created_at: daysAgo(210, "09:00")
  }
];

/** 缺省数据加载器：返回演示集合的深拷贝（接真 API 时替换为 listProjects） */
export function loadProjectEntries(): Promise<ProjectEntry[]> {
  return Promise.resolve(
    structuredClone(projectEntriesFixture) as ProjectEntry[]
  );
}
