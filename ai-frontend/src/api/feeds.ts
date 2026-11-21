import { http } from "@/api/base";

/**
 * 统一信息流客户端（PA-05-08/09 批次 C）。
 *
 * My Feed = Project Membership ∪ Product Membership 的去重活动集；
 * Product/Project Feed 是同一 Activity 数据的另两个视图；read-state 只动
 * Read（Read/Notified/Processed/Approved 四概念互不等同）。
 */

export type FeedTargetType = "task" | "requirement" | "adr";
export type FeedActorType = "human" | "agent";

export interface FeedItem {
  id: string;
  item_id: string;
  target_type: FeedTargetType;
  target_id: string;
  item_title: string;
  activity_type: string;
  actor_type: FeedActorType;
  actor_id: string | null;
  summary: string;
  detail: Record<string, unknown> | null;
  project_id: string | null;
  product_id: string | null;
  created_at: string;
  read_at: string | null;
}

export async function fetchMyFeed(limit = 50, offset = 0): Promise<FeedItem[]> {
  const res = await http.get<{ items: FeedItem[] }>(
    `/v1/pm/feeds/my?limit=${limit}&offset=${offset}`
  );
  return res.items ?? [];
}

export async function fetchProjectFeed(
  projectId: string,
  limit = 50,
  offset = 0
): Promise<FeedItem[]> {
  const res = await http.get<{ items: FeedItem[] }>(
    `/v1/pm/feeds/projects/${projectId}?limit=${limit}&offset=${offset}`
  );
  return res.items ?? [];
}

export async function fetchProductFeed(
  productId: string,
  limit = 50,
  offset = 0
): Promise<FeedItem[]> {
  const res = await http.get<{ items: FeedItem[] }>(
    `/v1/pm/feeds/products/${productId}?limit=${limit}&offset=${offset}`
  );
  return res.items ?? [];
}

/** 批量标记已读/未读；返回受影响行数 */
export async function markFeedRead(
  itemIds: string[],
  read = true
): Promise<number> {
  const res = await http.post<{ updated: number }>("/v1/pm/feeds/read-state", {
    item_ids: itemIds,
    read
  });
  return res.updated;
}
