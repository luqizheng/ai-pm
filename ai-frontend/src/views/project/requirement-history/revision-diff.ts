/**
 * 修订版本比较工具（S02-UI-05 §5）。
 *
 * 行级 LCS diff：按行比较两版 Markdown 正文，输出按序行块
 * （unchanged / added / removed）；样式规则是「可读的行内 diff、
 * 避免过多颜色」——只用 success / danger 两类语义色，修改块由
 * 相邻的 removed + added 行对自然表达，不做字符级染色。
 * 另附元数据 diff（验收标准按陈述匹配、优先级/类型字段比较）。
 */

import type { AcceptanceCriterion } from "../requirements/requirement-entry";

export type DiffRowKind = "unchanged" | "added" | "removed";

export interface DiffRow {
  kind: DiffRowKind;
  text: string;
}

/** 行级 LCS diff（演示数据规模小，O(n·m) DP 足够） */
export function diffLines(previous: string, next: string): DiffRow[] {
  const a = previous.split("\n");
  const b = next.split("\n");
  const rows = a.length;
  const cols = b.length;

  // lcs[i][j] = a[i..] 与 b[j..] 的最长公共子序列长度
  const lcs: number[][] = Array.from({ length: rows + 1 }, () =>
    new Array<number>(cols + 1).fill(0)
  );
  for (let i = rows - 1; i >= 0; i -= 1) {
    for (let j = cols - 1; j >= 0; j -= 1) {
      lcs[i][j] =
        a[i] === b[j]
          ? lcs[i + 1][j + 1] + 1
          : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }

  const result: DiffRow[] = [];
  let i = 0;
  let j = 0;
  while (i < rows && j < cols) {
    if (a[i] === b[j]) {
      result.push({ kind: "unchanged", text: a[i] });
      i += 1;
      j += 1;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      result.push({ kind: "removed", text: a[i] });
      i += 1;
    } else {
      result.push({ kind: "added", text: b[j] });
      j += 1;
    }
  }
  while (i < rows) {
    result.push({ kind: "removed", text: a[i] });
    i += 1;
  }
  while (j < cols) {
    result.push({ kind: "added", text: b[j] });
    j += 1;
  }
  return result;
}

/** 相邻 unchanged 行的折叠阈值：超过该数量的连续 unchanged 行保留首尾 */
const CONTEXT_KEEP = 1;

export interface DiffRowWithContext extends DiffRow {
  /** 与差异区距离过远的上下文行被折叠（渲染为省略提示） */
  collapsed: boolean;
}

/**
 * 折叠远离差异区的 unchanged 行（§5 readable inline diff）：
 * 连续 unchanged 行只保留每个差异区前后 CONTEXT_KEEP 行，其余折叠。
 */
export function collapseContext(rows: DiffRow[]): DiffRowWithContext[] {
  const hasDiff = rows.some(row => row.kind !== "unchanged");
  if (!hasDiff) {
    return rows.slice(0, CONTEXT_KEEP * 2 + 1).map(row => ({
      ...row,
      collapsed: false
    }));
  }

  const keep = new Array<boolean>(rows.length).fill(false);
  rows.forEach((row, index) => {
    if (row.kind !== "unchanged") {
      for (let k = Math.max(0, index - CONTEXT_KEEP); k <= Math.min(rows.length - 1, index + CONTEXT_KEEP); k += 1) {
        keep[k] = true;
      }
    }
  });

  return rows.map((row, index) => ({
    ...row,
    collapsed: !keep[index]
  }));
}

/** 验收标准 diff：按陈述文本匹配（新增 / 删除 / 未变） */
export interface CriterionDiff {
  statement: string;
  kind: "unchanged" | "added" | "removed";
}

export function diffCriteria(
  previous: AcceptanceCriterion[],
  next: AcceptanceCriterion[]
): CriterionDiff[] {
  const prevStatements = new Set(previous.map(item => item.statement));
  const nextStatements = new Set(next.map(item => item.statement));
  const result: CriterionDiff[] = [];
  const seen = new Set<string>();

  for (const criterion of next) {
    if (!seen.has(criterion.statement)) {
      seen.add(criterion.statement);
      result.push({
        statement: criterion.statement,
        kind: prevStatements.has(criterion.statement) ? "unchanged" : "added"
      });
    }
  }
  for (const criterion of previous) {
    if (!seen.has(criterion.statement)) {
      seen.add(criterion.statement);
      result.push({
        statement: criterion.statement,
        kind: "removed"
      });
    }
  }
  return result;
}

/** 简单字段 diff：值不同即视为变更 */
export function fieldChanged<T>(previous: T | undefined, next: T | undefined): boolean {
  return previous !== next;
}