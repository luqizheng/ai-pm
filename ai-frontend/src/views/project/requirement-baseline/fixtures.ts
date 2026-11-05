/**
 * 需求基线管理演示数据（mock 先行，S02-UI-06）。
 *
 * 本文件为 fixtures（i18n 硬编码门禁豁免）：基线描述等领域数据走这里；
 * 界面文案走 `locales/<locale>/pm.yaml` 的 `requirementBaseline:` 命名空间。
 * 契约见 `baseline-contract.ts`：基线引用正式修订（R3）、历史保留原引用、
 * 仅 confirmed 需求可含入、建立必须人类确认。
 */

import { requirementsFixture } from "../requirements/fixtures";
import type {
  BaselineComparisonModified,
  BaselineComparisonResult,
  BaselineEntryItem,
  BaselineRecord
} from "./baseline-contract";

/**
 * 演示基线集合（BL-001 → BL-002）。
 * BL-001：初始基线（REQ-001 v1 当时定稿 + REQ-002 v1）；
 * BL-002：当前生效（REQ-001 修订待确认暂缓含入，REQ-002 并入 PRP-006 分位数口径）。
 * 与既有 `requirements/fixtures.ts` 的 baselineFixture（v2 / REQ-002）保持一致。
 */
export const baselinesFixture: BaselineRecord[] = [
  {
    id: "BL-002",
    version: 2,
    description:
      "REQ-001 修订待确认暂缓含入；REQ-002 并入 PRP-006 分位数口径。",
    established_at: "2026-11-02 12:00",
    established_by: "林悦",
    status: "active",
    entries: [
      {
        requirement_id: "req-002",
        requirement_code: "REQ-002",
        title: "System response time",
        type: "non_functional",
        priority: "high",
        revision_version: 2
      }
    ]
  },
  {
    id: "BL-001",
    version: 1,
    description: "初始需求基线：账号登录能力与性能 SLA。",
    established_at: "2026-10-31 10:00",
    established_by: "林悦",
    status: "superseded",
    entries: [
      {
        requirement_id: "req-001",
        requirement_code: "REQ-001",
        title: "User authentication",
        type: "functional",
        priority: "high",
        revision_version: 1
      },
      {
        requirement_id: "req-002",
        requirement_code: "REQ-002",
        title: "System response time",
        type: "non_functional",
        priority: "high",
        revision_version: 1
      }
    ]
  }
];

/** 当前生效基线（status = active） */
export function currentBaseline(
  baselines: BaselineRecord[]
): BaselineRecord | null {
  return baselines.find(baseline => baseline.status === "active") ?? null;
}

/**
 * 两基线比较（§7）：新增 / 移除 / 修改（含两版验收标准差异陈述）。
 * criteria 差异从需求域修订快照取数；快照缺失时按空集处理并允许标注。
 */
export function compareBaselines(
  previous: BaselineRecord,
  next: BaselineRecord
): BaselineComparisonResult {
  const prevMap = new Map(
    previous.entries.map(entry => [entry.requirement_code, entry])
  );
  const nextMap = new Map(
    next.entries.map(entry => [entry.requirement_code, entry])
  );

  const added = next.entries.filter(
    entry => !prevMap.has(entry.requirement_code)
  );
  const removed = previous.entries.filter(
    entry => !nextMap.has(entry.requirement_code)
  );

  const modified: BaselineComparisonModified[] = [];
  for (const entry of next.entries) {
    const prev = prevMap.get(entry.requirement_code);
    if (!prev || prev.revision_version === entry.revision_version) continue;

    const requirement = requirementsFixture.find(
      item => item.id === entry.requirement_id
    );
    const oldRevision = requirement?.revisions.find(
      revision => revision.version === prev.revision_version
    );
    const newRevision = requirement?.revisions.find(
      revision => revision.version === entry.revision_version
    );
    const oldStatements = new Set(
      (oldRevision?.criteria ?? []).map(criterion => criterion.statement)
    );
    const newStatements = new Set(
      (newRevision?.criteria ?? []).map(criterion => criterion.statement)
    );

    modified.push({
      requirement_code: entry.requirement_code,
      title: entry.title,
      old_revision: prev.revision_version,
      new_revision: entry.revision_version,
      criteria_removed: (oldRevision?.criteria ?? [])
        .filter(criterion => !newStatements.has(criterion.statement))
        .map(criterion => criterion.statement),
      criteria_added: (newRevision?.criteria ?? [])
        .filter(criterion => !oldStatements.has(criterion.statement))
        .map(criterion => criterion.statement)
    });
  }

  return { added, removed, modified };
}

/** 基线含入条目 → 需求域修订的 criteria 快照（准备向导 Step2 审查用） */
export function revisionCriteriaOf(
  entry: BaselineEntryItem
): { version: number; statements: string[]; state: string } | null {
  const requirement = requirementsFixture.find(
    item => item.id === entry.requirement_id
  );
  const revision = requirement?.revisions.find(
    revision => revision.version === entry.revision_version
  );
  if (!requirement || !revision) return null;
  return {
    version: revision.version,
    state: revision.state,
    statements: (revision.criteria ?? []).map(criterion => criterion.statement)
  };
}

/** 准备向导演示用户（决策署名，i18n 豁免区） */
export const baselineOwnerFixture = "林悦";
