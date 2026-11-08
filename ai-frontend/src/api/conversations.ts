import { http } from "@/api/base";

/**
 * Project conversation REST read/write models（V2 D-4 会话两表 + REST，2026-11-08）。
 *
 * 后端契约：pm-api `/v1/pm/projects/{project_id}/conversations*`（5393e70 0060）——
 * - 会话属主 = 创建时的 principal；他人会话与不存在会话同形 404（防探测）；
 * - `POST /conversations/{id}/messages` role 恒 `user`（body 不收 role）：
 *   assistant / system 的生产者是内部 AI 链（后续批次），REST 只存储；
 * - 消息列表 `created_at` ASC，`before` 为 RFC3339 keyset 游标
 *   （`created_at < before`），limit 默认 200、clamp [1, 500]；
 * - 边界：title ≤ 500 字符、content ≤ 100k 字符（超限 400）。
 * 错误体 `{message}`（@digua/http 原样透传）。
 */

export type ConversationStatus = "active" | string;

export interface Conversation {
  id: string;
  tenant_id: string;
  project_id: string;
  principal_id: string;
  title: string;
  status: ConversationStatus;
  created_at: string;
  updated_at: string;
  /** 最近一条消息时间；尚无消息时为 null（列表按 updated_at DESC 排序）。 */
  last_message_at: string | null;
}

export type ConversationMessageRole = "user" | "assistant" | "system" | string;

export interface ConversationMessage {
  id: string;
  conversation_id: string;
  role: ConversationMessageRole;
  content: string;
  /** 服务端缺省 `markdown`；当前 REST 只写 user 消息。 */
  content_type: string;
  metadata: unknown;
  created_at: string;
}

export interface CreateConversationPayload {
  title?: string;
}

export interface AppendMessagePayload {
  content: string;
  /** 缺省 `markdown`。 */
  content_type?: string;
}

export interface ListMessagesQuery {
  /** RFC3339 keyset 游标：只取 `created_at < before` 的更早消息。 */
  before?: string;
  /** 服务端 clamp [1, 500]，缺省 200。 */
  limit?: number;
}

export function createConversation(
  projectId: string,
  body: CreateConversationPayload = {}
) {
  return http.post<Conversation>(
    `/v1/pm/projects/${projectId}/conversations`,
    body
  );
}

export function listConversations(projectId: string) {
  return http.get<Conversation[]>(`/v1/pm/projects/${projectId}/conversations`);
}

export function appendMessage(
  projectId: string,
  conversationId: string,
  body: AppendMessagePayload
) {
  return http.post<ConversationMessage>(
    `/v1/pm/projects/${projectId}/conversations/${conversationId}/messages`,
    body
  );
}

export function listMessages(
  projectId: string,
  conversationId: string,
  query: ListMessagesQuery = {}
) {
  return http.get<ConversationMessage[]>(
    `/v1/pm/projects/${projectId}/conversations/${conversationId}/messages`,
    query
  );
}
