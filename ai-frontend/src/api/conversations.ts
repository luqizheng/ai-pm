import { http } from "@/api/base";

/**
 * Project conversation REST read/write models（V2 D-4 会话两表 + REST，2026-11-08）。
 *
 * 后端契约：pm-api `/v1/pm/projects/{project_id}/conversations*`（5393e70 0060）——
 * - 会话属主 = 创建时的 principal；他人会话与不存在会话同形 404（防探测）；
 * - `POST /conversations/{id}/messages` role 恒 `user`（body 不收 role）：
 *   assistant / system 的生产者是内部 AI 链（后续批次），REST 只存储；
 * - 消息列表 `(created_at, id)` ASC；`before` 为向后 keyset 游标
 *   （`created_at < before`），`after`(+`after_id`) 为向前增量游标
 *   （`created_at > after OR (created_at = after AND id > after_id)`，
 *   Batch G 前端轮询契约面），limit 默认 200、clamp [1, 500]；
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
  /** DK-A：线程最近一次升级指向的访谈会话；null = 纯 chat 线程。 */
  discovery_session_id?: string | null;
  /** Batch 2：线程级上下文锚点；null = 无锚点。 */
  context_anchor?: ConversationContextAnchor | null;
}

export type ConversationMessageRole = "user" | "assistant" | "system" | string;

export interface ConversationMessage {
  id: string;
  conversation_id: string;
  role: ConversationMessageRole;
  content: string;
  /** 服务端缺省 `markdown`；REST 写 user 消息，assistant 由 worker 链写入。 */
  content_type: string;
  metadata: unknown;
  created_at: string;
}

/**
 * 线程级上下文锚点（Batch 2，0003）。
 *
 * 建会话时定格，消息级不收（D-IC-1）。服务端校验：project_id 须与目标
 * 项目一致、repository_name 须为本项目真实仓库、default_branch 须为该
 * 仓库默认分支——非法值 400。
 */
export interface ConversationContextAnchor {
  project_id?: string;
  project_name?: string;
  repository_name?: string;
  default_branch?: string;
  repository_role?: string;
}

export interface CreateConversationPayload {
  title?: string;
  /** 上下文锚点；缺省 = 无锚点（存量线程）。 */
  context?: ConversationContextAnchor;
}

export interface AppendMessagePayload {
  content: string;
  /** 缺省 `markdown`。 */
  content_type?: string;
}

/** DK-A 升级交接面（服务端 `HandoffView`）：边界消息 + 撤销窗口。 */
export interface ConversationHandoff {
  boundary_message_id: string;
  discovery_session_id: string;
  /** explicit | implicit_continue | implicit_intent */
  trigger: string;
  /** requirement_intake | kb_correction | project_onboarding | … */
  intent: string | null;
  created_session: boolean;
  /** 「已切换到需求澄清」等服务端措辞（展示优先用它）。 */
  notice: string;
  /** 30 分钟撤销窗口截止（RFC3339）。 */
  undo_until: string;
}

/**
 * 发送消息的落点信封（DK-A）：消息字段平铺 + 落点判定。
 * - `landed = "chat"`：消息落本线程（conversation_message），形状与
 *   `ConversationMessage` 一致；
 * - `landed = "interview"`：消息落 discovery 段——此时平铺字段为
 *   DiscoveryMessageView（id/session_id/speaker/source/content/seq/created_at），
 *   且带 `handoff`。
 */
export interface SendMessageOutcome extends ConversationMessage {
  landed?: "chat" | "interview" | string;
  discovery_session_id?: string | null;
  handoff?: ConversationHandoff | null;
}

export interface ListMessagesQuery {
  /** RFC3339 keyset 游标：只取 `created_at < before` 的更早消息。 */
  before?: string;
  /** RFC3339 增量游标：只取 `created_at > after` 的更新消息（Batch G 轮询）。 */
  after?: string;
  /**
   * 同微秒决胜键：`created_at = after AND id > after_id`（须与 `after`
   * 成对给出，单独给会 400）。
   */
  after_id?: string;
  /** 服务端 clamp [1, 500]，缺省 200。 */
  limit?: number;
  /**
   * Batch H 首屏「最新窗口」：true 时服务端按 `(created_at, id)` DESC 取
   * limit 行再反转为 ASC 返回（即最新的 limit 条）；缺省 false（旧行为：
   * ASC 取最旧 limit 条）。与 before/after 任一同给会 400。
   */
  latest?: boolean;
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
  return http.post<SendMessageOutcome>(
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

// ── Batch 3C：交互写回（生命周期终态权威 + 信封回读）──────────────────

/** 交互写回记录（服务端权威终态；`recorded ≠ 生效`）。 */
export interface InteractionRecord {
  id: string;
  message_id: string;
  conversation_id: string;
  action: string;
  status: string;
  reason?: string | null;
  option_id?: string | null;
  answer?: unknown;
  decided_by: string;
  /** 服务端生成的人类可读记录号（INT-XXXXXXXX）。 */
  reference: string;
  created_at: string;
  /** true = 幂等重放（非新建，200 返回既有记录）。 */
  replayed: boolean;
}

export interface SubmitInteractionPayload {
  action: "accept" | "reject" | "submit" | "defer" | "decision";
  reason?: string;
  option_id?: string;
  answer?: unknown;
}

export function submitInteraction(
  projectId: string,
  conversationId: string,
  messageId: string,
  body: SubmitInteractionPayload
) {
  return http.post<InteractionRecord>(
    `/v1/pm/projects/${projectId}/conversations/${conversationId}/messages/${messageId}/interaction`,
    body
  );
}
