/**
 * S05 · 执行与协作视图契约（docs/scenarios/s05/ui 页面结构 + 总体设计 §3/§5/§6）。
 *
 * 领域语义裁决（防状态混淆，全部页面强制）：
 * - **Task ≠ Run ≠ Agent ≠ Delivery ≠ Review**：五层分列呈现，禁止用单一
 *   「状态」字段混排。
 * - **Run 七态**：queued / claimed / running / succeeded / failed /
 *   timed_out / cancelled；`attempt_no` 按 `run_kind`（delivery / review）
 *   分轨编号——展示必须带 kind。
 * - **完成三态分离**：Agent 声称完成（Run succeeded / Delivery submitted）
 *   ≠ AI-PM 检查通过（review verdict）≠ 人工正式验收（Task completed）。
 * - **waiting reason 五值**原样呈现，不合并成「等待中」：
 *   needs_info / needs_decision / needs_approval / external_dependency /
 *   no_executor_available。
 * - **review verdict 三值**：approved / changes_requested / inconclusive；
 *   未评审时为空——**不造「rejected」第四值**。
 * - **任务级一次性授权 ≠ ACP-07 Run 级 Deadline 重授权**：后者是 Run 级
 *   事实（`task_run_execution_authorization`）。
 * - **Device readiness 六态**：installed / connected / authenticated /
 *   ready / degraded / failed。
 */

export type Tone =
  | "neutral"
  | "info"
  | "success"
  | "warning"
  | "danger"
  | "accent";

/** Run 七态 → 语义档。 */
const RUN_TONES: Record<string, Tone> = {
  queued: "neutral",
  claimed: "info",
  running: "info",
  succeeded: "success",
  failed: "danger",
  timed_out: "danger",
  cancelled: "neutral"
};

/** Task 十三态 → 语义档。 */
const TASK_TONES: Record<string, Tone> = {
  draft: "neutral",
  ready: "success",
  claimed: "info",
  running: "info",
  awaiting_decision: "warning",
  blocked: "warning",
  submitted: "info",
  reviewing: "info",
  completed: "success",
  changes_requested: "warning",
  failed: "danger",
  cancelled: "neutral",
  superseded: "neutral"
};

/** waiting reason 五值 → 语义档（每值独立，不合并）。 */
const WAITING_TONES: Record<string, Tone> = {
  needs_info: "warning",
  needs_decision: "warning",
  needs_approval: "warning",
  external_dependency: "info",
  no_executor_available: "danger"
};

/** review verdict 三值 → 语义档（无第四值）。 */
const VERDICT_TONES: Record<string, Tone> = {
  approved: "success",
  changes_requested: "warning",
  inconclusive: "neutral"
};

/** Device readiness 六态 → 语义档。 */
const READINESS_TONES: Record<string, Tone> = {
  installed: "neutral",
  connected: "info",
  authenticated: "info",
  ready: "success",
  degraded: "warning",
  failed: "danger"
};

/** Worker 命令状态 → 语义档。 */
const COMMAND_TONES: Record<string, Tone> = {
  issued: "neutral",
  delivered: "info",
  acked: "success",
  failed: "danger"
};

function pick(
  map: Record<string, Tone>,
  value: string | null | undefined
): Tone {
  if (!value) return "neutral";
  return map[value] ?? "neutral";
}

export const runTone = (value: string | null | undefined) =>
  pick(RUN_TONES, value);
export const taskTone = (value: string | null | undefined) =>
  pick(TASK_TONES, value);
export const waitingTone = (value: string | null | undefined) =>
  pick(WAITING_TONES, value);
export const verdictTone = (value: string | null | undefined) =>
  pick(VERDICT_TONES, value);
export const readinessTone = (value: string | null | undefined) =>
  pick(READINESS_TONES, value);
export const commandTone = (value: string | null | undefined) =>
  pick(COMMAND_TONES, value);

/** UUID 短形。 */
export function shortId(id: string | null | undefined): string {
  if (!id) return "";
  return id.length > 8 ? id.slice(0, 8) : id;
}

/** 时间格式化。 */
export function formatDateTime(value: string | null | undefined): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  });
}

/**
 * 时长（秒）→ 可读文本。
 *
 * 用 `finished_at - started_at`；两者缺一即无时长（不拿当前时间补）。
 */
export function formatDuration(
  startedAt: string | null | undefined,
  finishedAt: string | null | undefined
): string {
  if (!startedAt || !finishedAt) return "";
  const start = new Date(startedAt).getTime();
  const end = new Date(finishedAt).getTime();
  if (Number.isNaN(start) || Number.isNaN(end) || end < start) return "";
  const seconds = Math.round((end - start) / 1000);
  const minutes = Math.floor(seconds / 60);
  if (minutes < 1) return `${seconds}s`;
  const hours = Math.floor(minutes / 60);
  if (hours < 1) return `${minutes}m`;
  return `${hours}h ${minutes % 60}m`;
}

/** 秒数 → 可读时长（用于概览的平均时长）。 */
export function secondsToText(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined) return "";
  const minutes = Math.round(seconds / 60);
  if (minutes < 1) return `${Math.round(seconds)}s`;
  const hours = Math.floor(minutes / 60);
  if (hours < 1) return `${minutes}m`;
  return `${hours}h ${minutes % 60}m`;
}

/** 比率 → 百分比文本（null = 无分母，返回空）。 */
export function rateToPercent(rate: number | null | undefined): string {
  if (rate === null || rate === undefined) return "";
  return `${Math.round(rate * 100)}%`;
}

/** Run 的展示标签：`delivery #3` / `review #1`。 */
export function runLabel(
  runKind: string | null | undefined,
  attemptNo: number | null | undefined,
  kindLabels: { delivery: string; review: string }
): string {
  const kind = runKind === "review" ? kindLabels.review : kindLabels.delivery;
  return attemptNo ? `${kind} #${attemptNo}` : kind;
}
