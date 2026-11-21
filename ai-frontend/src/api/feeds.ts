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

export interface FeedCounts {
  total: number;
  unread: number;
  attention: number;
}

export type FeedView = "all" | "unread" | "attention";

export interface MyFeedQuery {
  limit?: number;
  offset?: number;
  view?: FeedView;
  actor_type?: string;
  activity_type?: string;
  project_id?: string;
}

export async function fetchMyFeed(
  query: MyFeedQuery = {}
): Promise<{ items: FeedItem[]; counts?: FeedCounts }> {
  const params = new URLSearchParams();
  params.set("limit", String(query.limit ?? 50));
  params.set("offset", String(query.offset ?? 0));
  if (query.view && query.view !== "all") params.set("view", query.view);
  if (query.actor_type) params.set("actor_type", query.actor_type);
  if (query.activity_type) params.set("activity_type", query.activity_type);
  if (query.project_id) params.set("project_id", query.project_id);
  const res = await http.get<{ items: FeedItem[]; counts?: FeedCounts }>(
    `/v1/pm/feeds/my?${params.toString()}`
  );
  return { items: res.items ?? [], counts: res.counts };
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

/** 全部标记已读（My Feed 范围）；返回受影响行数 */
export async function markAllFeedRead(): Promise<number> {
  const res = await http.post<{ updated: number }>(
    "/v1/pm/feeds/mark-all-read",
    {}
  );
  return res.updated;
}

/** 屏 04：统一工作项详情（注册行 + 双向关联 + 最近活动） */
export interface WorkItemDetail {
  item: {
    id: string;
    target_type: FeedTargetType | "issue" | "risk" | "blocker";
    target_id: string;
    title: string;
    status: string;
    project_id: string | null;
    created_at: string;
    updated_at: string;
  };
  relations: Array<{
    id: string;
    direction: "outgoing" | "incoming";
    relation_type: string;
    other_item_id: string;
    other_type: string;
    other_title: string;
    other_status: string;
    origin: string;
    reason: string | null;
  }>;
  activities: FeedItem[];
}

export async function fetchWorkItemDetail(
  itemId: string
): Promise<WorkItemDetail> {
  return http.get<WorkItemDetail>(`/v1/pm/work-items/${itemId}`);
}

/** 屏 06 Logs 页签：task 事项的执行日志（task_run_event） */
export interface ExecutionLog {
  id: string;
  run_id: string;
  event_type: string;
  occurred_at: string;
  detail: Record<string, unknown> | null;
}

export async function fetchExecutionLogs(
  itemId: string,
  limit = 50,
  offset = 0
): Promise<ExecutionLog[]> {
  const res = await http.get<{ items: ExecutionLog[] }>(
    `/v1/pm/work-items/${itemId}/execution-logs?limit=${limit}&offset=${offset}`
  );
  return res.items ?? [];
}

/** 屏 05：单条活动详情（打开即标已读，仅调用者本人状态） */
export async function fetchActivityDetail(
  activityId: string
): Promise<FeedItem> {
  return http.get<FeedItem>(`/v1/pm/feeds/activities/${activityId}`);
}
