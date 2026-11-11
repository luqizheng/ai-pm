import { http } from "@/api/base";

/**
 * 会话上下文条目 / 交互记录 / 候选对象（B4 后端契约 CX3 的前端读面 · B5 对接）。
 *
 * **权威数据源**：右栏 Actions / History 不再用时间线客户端推断充当权威状态
 * （契约 B5 硬要求）——交互与审批一律读 `interactions`；Context Tab 的对象
 * 列表读 `context-items`；Add Context 选择器读 `context-candidates`（服务端
 * 聚合，前端不得分别请求五个业务域再自行拼接权限）。
 */

/** 归属轴存储态（Available 是候选查询结果，不入库——DR-C7 修正口径）。 */
export type ContextItemAttachment = "attached" | "detached";

/** 生命周期轴。 */
export type ContextItemLifecycle =
  | "active"
  | "inactive"
  | "pending"
  | "unavailable";

/** 会话上下文条目（0019 / GET context-items）。 */
export interface ContextItem {
  id: string;
  conversation_id: string;
  object_type: string;
  object_id: string;
  object_code: string | null;
  /** 版本引用（按类型解释；task 只有 row_version）。 */
  revision_id: string | null;
  version: number | null;
  content_hash: string | null;
  row_version: number | null;
  attachment: ContextItemAttachment;
  lifecycle: ContextItemLifecycle;
  /** 可写性属性（独立于 lifecycle；不随状态流转切换）。 */
  read_only: boolean;
  source: string;
  unavailable_reason: string | null;
  added_by: string;
  added_at: string;
  updated_at: string;
}

/** 交互记录（0005 表 · GET interactions · Actions/History 权威读源）。 */
export interface InteractionEntry {
  id: string;
  conversation_id: string;
  message_id: string;
  /** accept / reject / submit / defer / decision。 */
  action: string;
  /** accepted / rejected / deferred / submitted / resolved。 */
  status: string;
  reason: string | null;
  option_id: string | null;
  answer: unknown;
  decided_by: string;
  decided_by_name: string | null;
  /** 服务端生成的人类可读记录号（INT-XXXXXXXX）。 */
  reference: string;
  created_at: string;
  /** 关联消息正文摘要（前 160 字符）。 */
  message_excerpt: string | null;
}

/** 候选对象（GET context-candidates · Add Context 选择器服务端聚合）。 */
export interface ContextCandidate {
  object_type: string;
  object_id: string;
  code: string;
  title: string;
  summary: string;
  version_label: string | null;
  /** 对当前会话而言是否已附加（detached 的历史不算，可再次附加）。 */
  attached: boolean;
}

export function listContextItems(
  projectId: string,
  conversationId: string,
  query: { include_detached?: boolean } = {}
) {
  return http.get<{ items: ContextItem[] }>(
    `/v1/pm/projects/${projectId}/conversations/${conversationId}/context-items`,
    query
  );
}

export function listInteractions(projectId: string, conversationId: string) {
  return http.get<{ interactions: InteractionEntry[] }>(
    `/v1/pm/projects/${projectId}/conversations/${conversationId}/interactions`
  );
}

export function listContextCandidates(
  projectId: string,
  query: {
    type: string;
    q?: string;
    page?: number;
    size?: number;
    conversation_id?: string;
  }
) {
  return http.get<{ items: ContextCandidate[]; total: number }>(
    `/v1/pm/projects/${projectId}/context-candidates`,
    query
  );
}
