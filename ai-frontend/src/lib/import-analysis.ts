import type { ImportListItem, ImportStatus } from "@/api/types";

/**
 * 仓库分析按钮的共享判定（S2 仓库设置页 / S3 总控台 / S4 详情面板单源）。
 *
 * 冷启动导入（`POST /v1/pm/projects/{id}/imports`）与手动增量分析
 * （`POST /v1/pm/repositories/{id}/analyze`）共用同一套进行中/重发语义，
 * 避免两个视图各写一份状态过滤与错误兜底。
 */

/** 进行中导入状态（尚未落终态）。 */
export const IN_PROGRESS_IMPORT_STATUSES: readonly ImportStatus[] = [
  "queued",
  "cloning",
  "reading",
  "analyzing"
];

export function isImportInProgress(status: ImportStatus): boolean {
  return IN_PROGRESS_IMPORT_STATUSES.includes(status);
}

/** 首个进行中导入（无则 null）。 */
export function findActiveImport(
  items: ImportListItem[]
): ImportListItem | null {
  return items.find(item => isImportInProgress(item.status)) ?? null;
}

/** failed / cancelled 可重发（复用主按钮文案，不设单独 retry 按钮）。 */
export function hasRetryableImport(items: ImportListItem[]): boolean {
  return items.some(
    item => item.status === "failed" || item.status === "cancelled"
  );
}

/** 防双击的幂等键（调用方用 ref 持有，成功/失败后清除）。 */
export function makeIdempotencyKey(): string {
  const cryptoApi = globalThis.crypto;
  if (cryptoApi && typeof cryptoApi.randomUUID === "function") {
    return cryptoApi.randomUUID();
  }
  return `imp-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

/**
 * 导入失败 error_code → i18n key（`s01.repositories.analysis.fail.<key>`）。
 * 未知码走 generic 兜底，把码与 detail 一起透出。
 */
const IMPORT_FAILURE_KEYS: Record<string, string> = {
  model_configuration: "modelConfiguration",
  model_timeout: "modelTimeout",
  model_unavailable: "modelUnavailable",
  bad_output: "badOutput",
  no_grounded_output: "noGroundedOutput",
  no_verified_repository: "noVerifiedRepository",
  all_repositories_failed: "allRepositoriesFailed",
  git_timeout: "gitTimeout",
  git_auth_failed: "gitAuthFailed",
  git_not_found: "gitNotFound",
  repo_too_large: "repoTooLarge",
  git_failed: "gitFailed",
  partial: "partial",
  job_failed: "jobFailed"
};

/** 导入失败原因 → 本地化文案；无 error_code（未失败）返回空串。 */
export function describeImportFailure(
  t: Translate,
  item: { error_code?: string | null; error_detail?: string | null }
): string {
  const code = item.error_code ?? "";
  if (!code) return "";
  const key = IMPORT_FAILURE_KEYS[code];
  if (key) {
    const base = t(`s01.repositories.analysis.fail.${key}`) as string;
    return item.error_detail ? `${base}（${item.error_detail}）` : base;
  }
  return t("s01.repositories.analysis.fail.generic", {
    code,
    detail: item.error_detail ?? ""
  }) as string;
}

/** 后端错误体 `{error}` → 错误码；取不到返回空串。 */
export function importErrorCode(error: unknown): string {
  const data = (error as { data?: unknown } | null)?.data;
  if (data && typeof data === "object") {
    const code = (data as { error?: unknown }).error;
    if (typeof code === "string" && code) return code;
  }
  return "";
}

/** 兜底错误文案（后端 message 或 HTTP 状态文案）。 */
export function importErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  const message = (error as { message?: unknown } | null)?.message;
  return typeof message === "string" ? message : "";
}

type Translate = (key: string, params?: Record<string, unknown>) => string;

/**
 * 后端错误码 → i18n 兜底文案。
 * 前端无 requirement count 端点，「项目非空 / 已 apply」只能靠错误码兜底；
 * 未知码把码与 message 一起透出，不伪装成成功。
 */
export function describeImportError(t: Translate, error: unknown): string {
  const code = importErrorCode(error);
  if (code === "project_not_empty") {
    return t("s01.repositories.analysis.error.projectNotEmpty");
  }
  if (code === "already_applied") {
    return t("s01.repositories.analysis.error.alreadyApplied");
  }
  if (code === "import_in_progress" || code === "analysis_in_progress") {
    return t("s01.repositories.analysis.error.inProgress");
  }
  if (code === "analysis_no_anchor") {
    return t("s01.repositories.analysis.error.noAnchor");
  }
  if (!code) {
    // 无稳定错误码（S1 仓库端点的 {message} 型错误体）不塞占位码，避免「（-）」噪音。
    return t("s01.repositories.analysis.error.failed", {
      message: importErrorMessage(error)
    });
  }
  return t("s01.repositories.analysis.error.generic", {
    code,
    message: importErrorMessage(error)
  });
}
