import type { BadgeVariant } from "@/components/ui/badge";

/**
 * 业务状态 → Badge 语义色的映射。
 * 中文文案不在这里硬编码 —— 视图用 `t("pm.status." + value, String(value))` 走 i18n。
 * 入参收 unknown：壳 DataTable 的插槽 row 是 Record<string, unknown>。
 */

const successValues = new Set(["active", "approved", "done"]);
const infoValues = new Set(["planning", "in_progress", "in_review"]);
const warningValues = new Set(["pending", "high", "blocked"]);
const dangerValues = new Set(["rejected", "cancelled", "urgent"]);

export function statusVariant(value: unknown): BadgeVariant {
  const status = typeof value === "string" ? value : String(value ?? "");
  if (successValues.has(status)) return "success";
  if (dangerValues.has(status)) return "danger";
  if (warningValues.has(status)) return "warning";
  if (infoValues.has(status)) return "info";
  return "neutral";
}
