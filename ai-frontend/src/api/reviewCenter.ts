import { getCommandCenter, type ActionItem } from "@/api/command-center";

/**
 * 人工审核中心（R01–R12，docs/ui/review-center）聚合读取层。
 *
 * 原则（D5/D9）：
 * - 聚合真源 = command-center 读模型的 `human_action_summary` 块（R09 单源，
 *   与指挥中心面板共用同一份数据，不另造后端端点）；
 * - 具体决策走各领域既有 API（discovery / pc / delivery-review），本文件不重实现。
 */

/** 条目稳定键：kind+id 复合标识（id 跨表不保证全局唯一，V1.0 §2.3）。 */
export function reviewItemKey(item: ActionItem): string {
  return `${item.kind}:${item.id}`;
}

/** 条目 → 队列页签（finding 两 kind 同属「发现」页签）。 */
export function reviewTabOf(
  item: ActionItem
): "finding" | "proposal" | "delivery" | "other" {
  if (
    item.kind === "finding_pending" ||
    item.kind === "finding_revision_requested"
  ) {
    return "finding";
  }
  if (item.kind === "proposal_pending") return "proposal";
  if (item.kind === "delivery_pending") return "delivery";
  return "other";
}

export interface HumanActionBlock {
  status: "ok" | "unavailable";
  asOf: string | null;
  /** `null` = 块不可用（不得当 0，契约：不可用块不得伪造数字）。 */
  total: number | null;
  items: ActionItem[];
  errorCode?: string;
  errorMessage?: string;
}

/** 聚合读取：command-center 六块一次拉取，取 human_action_summary 块。 */
export async function loadHumanActionBlock(
  projectId: string
): Promise<HumanActionBlock> {
  const center = await getCommandCenter(projectId);
  const block = center.blocks.human_action_summary;
  return {
    status: block.status,
    asOf: block.as_of || null,
    total: block.status === "ok" ? (block.data?.total ?? 0) : null,
    items: block.data?.items ?? [],
    errorCode: block.error?.code,
    errorMessage: block.error?.message
  };
}
