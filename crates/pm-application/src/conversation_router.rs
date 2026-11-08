//! DK-A 对话编排底座：载体判定 + 单向升级 + 撤销（设计
//! `docs/product/ai-pm-dialog-orchestration-design-v1.md` §2.3/§2.4，裁决 §13
//! D-DK-1=a+ / D-DK-2=加列（0065）/ D-DK-3=显式优先）。
//!
//! 口径（a+）：
//! - **线程目录唯一** = `ai.conversation`；访谈不另建列表（A5）。
//! - **正文按段落归属**：chat 段 → `ai.conversation_message`，interview 段 →
//!   `discovery.discovery_message`；同一条用户消息只落一处（A1，无双写）。
//! - **单向升级**：线程一旦链到未完结访谈，后续消息一律落访谈（A2）；访谈
//!   `COMPLETED` / `PAUSED` 后判定表按规则 5 回落 chat（A6，D-DK-1 附注）。
//!
//! 判定（服务端、写入用户消息之前，先命中先返回）：
//! 1. 线程已链且会话未 `COMPLETED/PAUSED` → interview 续接；
//! 2. 请求显式 `mode="interview"` / `discovery_session_id` → interview；
//! 3. （开关 on）本地变更意图规则命中 + 本人在本项目有未完结访谈 → 续接该访谈；
//! 4. （开关 on）本地规则命中 + LLM 意图分类 ≥ 阈值 → 新建会话并升级；
//! 5. 其余 → chat。
//!
//! 失败方向不对称：分类器无 provider / 超时 / 输出不合规 → **落 chat**（写入侧
//! fail-closed，绝不因模型抖动建会话）。开关 `AI_PM_CONVERSATION_ROUTER=off|on`
//! 默认 off（F22：新计费型生产者默认关，拼错值 WARN 回 off）。
//!
//! 原子性：升级四步（建/取会话 → 交接摘要 → 边界 system 消息 → 置链接）与
//! 用户消息落点同一事务，失败整体回滚，不留半升级状态。会话行 `FOR UPDATE`
//! 串行化同线程并发发送。discovery 写入全部经既有 discovery 服务的事务内
//! 变体（V-6 单写入），本模块零新增审计点（R5：0060 / discovery 采集层本就
//! 无审计）。
//!
//! 不变量（测试钉住）：landed=interview 的用户消息不进 `ai.conversation_message`，
//! 升级后线程最新消息为 system 边界行 → 天然脱离第七循环 `conversation_reply`
//! 的 pending 扫描集（判据=最新消息 role='user'）。

use std::fmt;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::ai_conversations::{
    db, insert_message_in, validate_content, ConversationError, MessageView,
};
use crate::discovery::{
    self, DiscoveryError, DiscoveryMessageView, SESSION_STATUS_COMPLETED,
    SESSION_STATUS_IN_PROGRESS, SESSION_STATUS_PAUSED,
};

/// 隐式判定开关环境变量（规则 3/4）。
pub const ROUTER_ENV: &str = "AI_PM_CONVERSATION_ROUTER";
/// 撤销升级窗口（§2.4「30 分钟窗口内有效」）。
pub const UNDO_WINDOW_MINUTES: i32 = 30;
/// 意图分类置信度阈值（规则 4）。
pub const INTENT_CONFIDENCE_THRESHOLD: f64 = 0.7;
/// 升级可见提示（§2.4 静默升级禁止条款 / A4）。
pub const HANDOFF_NOTICE: &str = "已切换到需求澄清";
/// 撤销后的边界提示。
pub const HANDOFF_UNDONE_NOTICE: &str = "已撤销切换，回到普通对话";
/// 显式进入访谈的 `mode` 取值。
pub const MODE_INTERVIEW: &str = "interview";

/// 本地「变更意图」规则集（规则 3/4 的前置闸：未命中不调用分类器，省 token）。
const CHANGE_INTENT_KEYWORDS: [&str; 22] = [
    "需要",
    "需求",
    "新增",
    "增加",
    "添加",
    "支持",
    "希望",
    "应该",
    "改成",
    "改为",
    "更正",
    "纠正",
    "不对",
    "错了",
    "功能",
    "要求",
    "need",
    "should",
    "feature",
    "support",
    "wrong",
    "incorrect",
];

/// 规则 3/4 开关。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterMode {
    /// 只有规则 1/2（用户显式进入访谈）生效。默认。
    Off,
    /// 规则 1~4 全部生效。
    On,
}

impl RouterMode {
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("on") => Self::On,
            Some("off") | Some("") | None => Self::Off,
            Some(other) => {
                tracing::warn!(
                    value = other,
                    "unknown {ROUTER_ENV} value (expected off|on); using default off"
                );
                Self::Off
            }
        }
    }

    pub fn from_env() -> Self {
        Self::parse(std::env::var(ROUTER_ENV).ok().as_deref())
    }
}

/// LLM 意图分类结果词表（JSON 契约 `intent` 字段同源）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    RequirementIntake,
    KbCorrection,
    ProjectOnboarding,
    ProjectQuery,
    ChitChat,
}

impl Intent {
    pub const ALL: [Intent; 5] = [
        Intent::RequirementIntake,
        Intent::KbCorrection,
        Intent::ProjectOnboarding,
        Intent::ProjectQuery,
        Intent::ChitChat,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Intent::RequirementIntake => "requirement_intake",
            Intent::KbCorrection => "kb_correction",
            Intent::ProjectOnboarding => "project_onboarding",
            Intent::ProjectQuery => "project_query",
            Intent::ChitChat => "chit_chat",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|i| i.as_str() == value)
    }

    /// 升级型意图 → discovery 会话 kind；非升级型 → None（落 chat）。
    pub fn session_kind(self) -> Option<&'static str> {
        match self {
            Intent::RequirementIntake => Some("TASK_DISCOVERY"),
            Intent::KbCorrection => Some("UPDATE"),
            Intent::ProjectOnboarding => Some("INITIAL_ONBOARDING"),
            Intent::ProjectQuery | Intent::ChitChat => None,
        }
    }
}

/// 分类器输出（已过 JSON 契约校验）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntentClassification {
    pub intent: Intent,
    pub confidence: f64,
}

/// 分类失败（任一变体都落 chat）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassifierError {
    /// 三级解析无可用 provider（或解析本身失败）。
    NoProvider(String),
    Timeout,
    Unavailable(String),
    /// 输出不合规（非 JSON / intent 不在词表 / confidence 越界）。
    BadOutput(String),
}

impl fmt::Display for ClassifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClassifierError::NoProvider(r) => write!(f, "no intent classifier provider: {r}"),
            ClassifierError::Timeout => write!(f, "intent classifier timed out"),
            ClassifierError::Unavailable(r) => write!(f, "intent classifier unavailable: {r}"),
            ClassifierError::BadOutput(r) => write!(f, "intent classifier bad output: {r}"),
        }
    }
}

/// 意图分类 LLM 端口（实现：`pm_ai::conversation_router_model`）。
#[async_trait::async_trait]
pub trait IntentClassifier: Send + Sync {
    async fn classify(
        &self,
        project_id: Uuid,
        content: &str,
    ) -> Result<IntentClassification, ClassifierError>;
}

/// 无分类器（测试 / 未接线）：恒 `NoProvider` → 落 chat。
pub struct DisabledClassifier;

#[async_trait::async_trait]
impl IntentClassifier for DisabledClassifier {
    async fn classify(
        &self,
        _project_id: Uuid,
        _content: &str,
    ) -> Result<IntentClassification, ClassifierError> {
        Err(ClassifierError::NoProvider("classifier disabled".into()))
    }
}

/// 本地变更意图判定（规则 3/4 前置闸）。纯函数、大小写不敏感。
pub fn has_local_change_intent(content: &str) -> bool {
    let lowered = content.to_lowercase();
    CHANGE_INTENT_KEYWORDS.iter().any(|k| lowered.contains(k))
}

/// 发送请求（REST body 映射）。
#[derive(Debug, Clone, Default)]
pub struct SendMessageRequest {
    pub content: String,
    pub content_type: Option<String>,
    /// 仅接受 `"interview"`（规则 2）。
    pub mode: Option<String>,
    /// 显式续接指定访谈（规则 2）。
    pub discovery_session_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Landed {
    Chat,
    Interview,
}

/// 升级触发方式（写进边界消息 metadata，供审计/UI）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffTrigger {
    /// 规则 2。
    Explicit,
    /// 规则 3：续接本人未完结访谈。
    ImplicitContinue,
    /// 规则 4：分类器命中，新建会话。
    ImplicitIntent,
}

/// 本次发送触发的升级（A4：提示 + 撤销截止）。
#[derive(Debug, Clone, Serialize)]
pub struct HandoffView {
    pub boundary_message_id: Uuid,
    pub discovery_session_id: Uuid,
    pub trigger: HandoffTrigger,
    pub intent: Option<Intent>,
    pub created_session: bool,
    pub notice: &'static str,
    pub undo_until: DateTime<Utc>,
}

/// 落点消息（序列化时平铺进响应体；chat 形状与旧端点完全一致）。
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum LandedMessage {
    Chat(MessageView),
    Interview(DiscoveryMessageView),
}

/// 唯一发送入口的结果。
#[derive(Debug, Clone, Serialize)]
pub struct SendOutcome {
    pub landed: Landed,
    pub discovery_session_id: Option<Uuid>,
    pub handoff: Option<HandoffView>,
    #[serde(flatten)]
    pub message: LandedMessage,
}

/// 撤销结果。
#[derive(Debug, Clone, Serialize)]
pub struct UndoOutcome {
    pub discovery_session_id: Uuid,
    pub session_status: String,
    pub boundary_message_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RouterError {
    #[error("conversation not found")]
    ConversationNotFound,
    #[error("discovery session not found")]
    SessionNotFound,
    #[error("{0}")]
    Validation(String),
    /// 状态冲突（上层 409）：静态码便于前端分支。
    #[error("{0}")]
    Conflict(&'static str),
    #[error("database error: {0}")]
    Db(String),
}

impl From<ConversationError> for RouterError {
    fn from(e: ConversationError) -> Self {
        match e {
            ConversationError::NotFound => RouterError::ConversationNotFound,
            ConversationError::Validation(m) => RouterError::Validation(m),
            ConversationError::Db(m) => RouterError::Db(m),
        }
    }
}

impl From<DiscoveryError> for RouterError {
    fn from(e: DiscoveryError) -> Self {
        match e {
            DiscoveryError::SessionNotFound => RouterError::SessionNotFound,
            DiscoveryError::ProjectNotFound => RouterError::ConversationNotFound,
            DiscoveryError::Validation(m) => RouterError::Validation(m),
            DiscoveryError::InvalidTransition { .. } => {
                RouterError::Conflict("discovery_session_not_undoable")
            }
            DiscoveryError::Db(e) => RouterError::Db(e.to_string()),
            other => RouterError::Db(other.to_string()),
        }
    }
}

impl From<sqlx::Error> for RouterError {
    fn from(e: sqlx::Error) -> Self {
        RouterError::Db(e.to_string())
    }
}

/// 落点计划（事务外判定；事务内按锁后状态复核）。
#[derive(Debug, Clone, PartialEq)]
enum Plan {
    Chat,
    Continue(Uuid),
    Upgrade {
        target: UpgradeTarget,
        trigger: HandoffTrigger,
        intent: Option<Intent>,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum UpgradeTarget {
    Existing(Uuid),
    New { kind: &'static str },
}

fn session_is_active(status: &str) -> bool {
    status != SESSION_STATUS_COMPLETED && status != SESSION_STATUS_PAUSED
}

/// 读线程链接与所链会话状态（带属主边界）。`None` = 会话不可见。
async fn read_link(
    conn: &mut sqlx::PgConnection,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
    lock: bool,
) -> Result<Option<(Option<Uuid>, Option<String>)>, RouterError> {
    let sql = if lock {
        "SELECT c.discovery_session_id FROM ai.conversation c \
         WHERE c.id = $1 AND c.tenant_id = $2 AND c.project_id = $3 AND c.principal_id = $4 \
         FOR UPDATE"
    } else {
        "SELECT c.discovery_session_id FROM ai.conversation c \
         WHERE c.id = $1 AND c.tenant_id = $2 AND c.project_id = $3 AND c.principal_id = $4"
    };
    let link: Option<Option<Uuid>> = sqlx::query_scalar(sql)
        .bind(conversation_id)
        .bind(tenant_id)
        .bind(project_id)
        .bind(principal_id)
        .fetch_optional(&mut *conn)
        .await?;
    let Some(link) = link else {
        return Ok(None);
    };
    let status = match link {
        Some(sid) => Some(
            discovery::get_session_in(&mut *conn, tenant_id, sid)
                .await?
                .status,
        ),
        None => None,
    };
    Ok(Some((link, status)))
}

/// 唯一发送入口（`POST /conversations/{cid}/messages`）：判定落点并原子写入。
#[allow(clippy::too_many_arguments)]
pub async fn send_message(
    pool: &PgPool,
    classifier: &dyn IntentClassifier,
    mode: RouterMode,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
    request: SendMessageRequest,
) -> Result<SendOutcome, RouterError> {
    let content_type = request
        .content_type
        .as_deref()
        .unwrap_or("markdown")
        .trim()
        .to_string();
    validate_content(&request.content, &content_type)?;
    if let Some(m) = request.mode.as_deref() {
        if m != MODE_INTERVIEW {
            return Err(RouterError::Validation(format!(
                "mode must be \"{MODE_INTERVIEW}\" when given"
            )));
        }
    }

    // ── 事务外判定（分类器调用不持行锁）──────────────────────────────
    let plan = {
        let mut conn = pool.acquire().await?;
        let (link, link_status) = read_link(
            &mut conn,
            tenant_id,
            project_id,
            principal_id,
            conversation_id,
            false,
        )
        .await?
        .ok_or(RouterError::ConversationNotFound)?;
        decide(
            &mut conn,
            classifier,
            mode,
            tenant_id,
            project_id,
            principal_id,
            &request,
            link,
            link_status.as_deref(),
        )
        .await?
    };

    // ── 事务内执行（锁线程行，按锁后状态复核规则 1）─────────────────────
    let mut tx = pool.begin().await?;
    let (link, link_status) = read_link(
        &mut tx,
        tenant_id,
        project_id,
        principal_id,
        conversation_id,
        true,
    )
    .await?
    .ok_or(RouterError::ConversationNotFound)?;
    // 并发：判定后另一请求已把线程升级 → 以锁后状态为准续接（不二次升级）。
    let plan = match (link, link_status.as_deref()) {
        (Some(sid), Some(status)) if session_is_active(status) => Plan::Continue(sid),
        _ => match plan {
            // 判定时会话活跃、锁后已完结 → 回落 chat（A6 同一语义）。
            Plan::Continue(_) => Plan::Chat,
            other => other,
        },
    };

    let outcome = match plan {
        Plan::Chat => {
            let message = insert_message_in(
                &mut tx,
                conversation_id,
                "user",
                &request.content,
                &content_type,
                serde_json::json!({}),
            )
            .await?;
            SendOutcome {
                landed: Landed::Chat,
                discovery_session_id: None,
                handoff: None,
                message: LandedMessage::Chat(message),
            }
        }
        Plan::Continue(session_id) => {
            let message =
                append_user_to_interview(&mut tx, tenant_id, conversation_id, session_id, &request)
                    .await?;
            SendOutcome {
                landed: Landed::Interview,
                discovery_session_id: Some(session_id),
                handoff: None,
                message: LandedMessage::Interview(message),
            }
        }
        Plan::Upgrade {
            target,
            trigger,
            intent,
        } => {
            let handoff = upgrade_in(
                &mut tx,
                tenant_id,
                project_id,
                principal_id,
                conversation_id,
                target,
                trigger,
                intent,
            )
            .await?;
            let message = append_user_to_interview(
                &mut tx,
                tenant_id,
                conversation_id,
                handoff.discovery_session_id,
                &request,
            )
            .await?;
            SendOutcome {
                landed: Landed::Interview,
                discovery_session_id: Some(handoff.discovery_session_id),
                handoff: Some(handoff),
                message: LandedMessage::Interview(message),
            }
        }
    };
    tx.commit().await?;
    Ok(outcome)
}

/// 判定表（§2.4）。只读；分类器在此调用（事务外）。
#[allow(clippy::too_many_arguments)]
async fn decide(
    conn: &mut sqlx::PgConnection,
    classifier: &dyn IntentClassifier,
    mode: RouterMode,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    request: &SendMessageRequest,
    link: Option<Uuid>,
    link_status: Option<&str>,
) -> Result<Plan, RouterError> {
    // 规则 1：已链且未完结 → 续接（只查列存在不够，必须看状态——A6）。
    if let (Some(sid), Some(status)) = (link, link_status) {
        if session_is_active(status) {
            return Ok(Plan::Continue(sid));
        }
    }
    // 规则 2：显式。
    if let Some(sid) = request.discovery_session_id {
        let session = discovery::get_session_in(&mut *conn, tenant_id, sid).await?;
        if session.project_id != project_id {
            // 跨项目会话按不存在处理（防探测）。
            return Err(RouterError::SessionNotFound);
        }
        if !session_is_active(&session.status) {
            return Err(RouterError::Conflict("discovery_session_not_active"));
        }
        return Ok(Plan::Upgrade {
            target: UpgradeTarget::Existing(sid),
            trigger: HandoffTrigger::Explicit,
            intent: None,
        });
    }
    if request.mode.as_deref() == Some(MODE_INTERVIEW) {
        return Ok(Plan::Upgrade {
            target: UpgradeTarget::New {
                kind: "TASK_DISCOVERY",
            },
            trigger: HandoffTrigger::Explicit,
            intent: None,
        });
    }
    if mode == RouterMode::Off || !has_local_change_intent(&request.content) {
        return Ok(Plan::Chat);
    }
    // 规则 3：本人在本项目的未完结访谈（最近活动者）。
    let open: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM discovery.discovery_session \
         WHERE tenant_id = $1 AND project_id = $2 AND created_by = $3 \
           AND status NOT IN ('COMPLETED', 'PAUSED') \
         ORDER BY updated_at DESC, id LIMIT 1",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(principal_id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(sid) = open {
        return Ok(Plan::Upgrade {
            target: UpgradeTarget::Existing(sid),
            trigger: HandoffTrigger::ImplicitContinue,
            intent: None,
        });
    }
    // 规则 4：分类器；任何失败都落 chat（fail-closed）。
    match classifier.classify(project_id, &request.content).await {
        Ok(c) if c.confidence >= INTENT_CONFIDENCE_THRESHOLD => match c.intent.session_kind() {
            Some(kind) => Ok(Plan::Upgrade {
                target: UpgradeTarget::New { kind },
                trigger: HandoffTrigger::ImplicitIntent,
                intent: Some(c.intent),
            }),
            None => Ok(Plan::Chat),
        },
        Ok(_) => Ok(Plan::Chat),
        Err(err) => {
            tracing::warn!(
                %project_id,
                error = %err,
                "conversation router intent classification failed; landing in chat \
                 (write-side fail-closed: no discovery session is created)"
            );
            Ok(Plan::Chat)
        }
    }
}

/// 用户消息落访谈段（经 discovery 服务）+ 刷新线程活动时间（列表排序）。
///
/// 先取会话行锁（C7 会话级追加互斥，与编排器 / 池版追加同一把锁），锁后复核
/// 会话仍活跃——判定时活跃、锁前被人工完结/暂停的极端竞态 → 409，不写。
async fn append_user_to_interview(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    conversation_id: Uuid,
    session_id: Uuid,
    request: &SendMessageRequest,
) -> Result<DiscoveryMessageView, RouterError> {
    let session = discovery::lock_session_in(tx, tenant_id, session_id).await?;
    if !session_is_active(&session.status) {
        return Err(RouterError::Conflict("discovery_session_not_active"));
    }
    let message =
        discovery::append_message_in_tx(tx, tenant_id, session_id, "user", "WEB", &request.content)
            .await?;
    touch_conversation(tx, conversation_id).await?;
    Ok(message)
}

async fn touch_conversation(
    conn: &mut sqlx::PgConnection,
    conversation_id: Uuid,
) -> Result<(), RouterError> {
    sqlx::query(
        "UPDATE ai.conversation SET updated_at = now(), last_message_at = now() WHERE id = $1",
    )
    .bind(conversation_id)
    .execute(&mut *conn)
    .await
    .map_err(db)?;
    Ok(())
}

/// 单向升级四步（§2.3(3)），调用方事务内（线程行已 `FOR UPDATE`；锁序
/// conversation → session）。
#[allow(clippy::too_many_arguments)]
async fn upgrade_in(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
    target: UpgradeTarget,
    trigger: HandoffTrigger,
    intent: Option<Intent>,
) -> Result<HandoffView, RouterError> {
    // ① 建 / 取会话（既有会话取行锁并复核，状态检查与后续追加原子）。
    let (session_id, created_session) = match target {
        UpgradeTarget::New { kind } => {
            let s =
                discovery::create_session_in(tx, tenant_id, project_id, "WEB", kind, principal_id)
                    .await?;
            (s.id, true)
        }
        UpgradeTarget::Existing(sid) => {
            let s = discovery::lock_session_in(tx, tenant_id, sid).await?;
            if s.project_id != project_id {
                return Err(RouterError::SessionNotFound);
            }
            if !session_is_active(&s.status) {
                return Err(RouterError::Conflict("discovery_session_not_active"));
            }
            (sid, false)
        }
    };

    // ② 交接摘要（摘要，不逐条复制旧轮次——复制即双写）。
    let row = sqlx::query(
        "SELECT c.title, \
                (SELECT count(*) FROM ai.conversation_message m \
                 WHERE m.conversation_id = c.id AND m.role IN ('user', 'assistant')) AS turns \
         FROM ai.conversation c WHERE c.id = $1",
    )
    .bind(conversation_id)
    .fetch_one(&mut **tx)
    .await?;
    let title: String = row.get("title");
    let turns: i64 = row.get("turns");
    let summary = handoff_summary(&title, turns, trigger, intent);
    // 新建会话的行由本事务插入，对其他事务不可见；既有会话已在 ① 取锁。
    discovery::append_message_in_tx(tx, tenant_id, session_id, "ai", "SYSTEM", &summary).await?;

    // ③ 边界 system 消息（UI 分隔条；metadata 带撤销所需事实）。
    let at: DateTime<Utc> = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut **tx)
        .await?;
    let metadata = serde_json::json!({
        "handoff": {
            "discovery_session_id": session_id,
            "at": at,
            "trigger": trigger,
            "intent": intent,
            "created_session": created_session,
        }
    });
    let boundary = insert_message_in(
        tx,
        conversation_id,
        "system",
        HANDOFF_NOTICE,
        "text",
        metadata,
    )
    .await?;

    // ④ 置链接。
    sqlx::query("UPDATE ai.conversation SET discovery_session_id = $2 WHERE id = $1")
        .bind(conversation_id)
        .bind(session_id)
        .execute(&mut **tx)
        .await?;

    Ok(HandoffView {
        boundary_message_id: boundary.id,
        discovery_session_id: session_id,
        trigger,
        intent,
        created_session,
        notice: HANDOFF_NOTICE,
        undo_until: boundary.created_at + chrono::Duration::minutes(i64::from(UNDO_WINDOW_MINUTES)),
    })
}

fn handoff_summary(
    title: &str,
    turns: i64,
    trigger: HandoffTrigger,
    intent: Option<Intent>,
) -> String {
    let title = if title.trim().is_empty() {
        "（未命名）"
    } else {
        title.trim()
    };
    let how = match (trigger, intent) {
        (HandoffTrigger::Explicit, _) => "用户显式进入需求澄清".to_string(),
        (HandoffTrigger::ImplicitContinue, _) => "检测到变更意图，续接未完结访谈".to_string(),
        (HandoffTrigger::ImplicitIntent, Some(i)) => {
            format!("意图分类命中 {}", i.as_str())
        }
        (HandoffTrigger::ImplicitIntent, None) => "意图分类命中".to_string(),
    };
    format!(
        "【交接摘要】本段访谈由对话线程「{title}」升级而来（{how}）。升级前该线程共 {turns} \
         条对话轮次，原文保留在对话线程中，未复制到访谈；触发升级的用户消息为本段下一条消息。"
    )
}

/// 撤销升级（A4）：30 分钟窗口内，把本次升级新建的会话置 `PAUSED` 并回退链接；
/// 续接既有会话（规则 2 指定 id / 规则 3）的升级只回退链接、不动该会话状态
/// （撤销的是「这次切换」，不是用户此前已在进行的访谈）。
pub async fn undo_handoff(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    principal_id: Uuid,
    conversation_id: Uuid,
) -> Result<UndoOutcome, RouterError> {
    let mut tx = pool.begin().await?;
    let (link, _) = read_link(
        &mut tx,
        tenant_id,
        project_id,
        principal_id,
        conversation_id,
        true,
    )
    .await?
    .ok_or(RouterError::ConversationNotFound)?;
    let Some(session_id) = link else {
        return Err(RouterError::Conflict("conversation_not_upgraded"));
    };
    let boundary = sqlx::query(
        "SELECT id, \
                COALESCE((metadata -> 'handoff' ->> 'created_session')::boolean, false) \
                    AS created_session, \
                (now() - created_at) <= make_interval(mins => $3) AS within_window \
         FROM ai.conversation_message \
         WHERE conversation_id = $1 AND role = 'system' \
           AND metadata -> 'handoff' ->> 'discovery_session_id' = $2::text \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(conversation_id)
    .bind(session_id)
    .bind(UNDO_WINDOW_MINUTES)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(RouterError::Conflict("handoff_boundary_missing"))?;
    let within: bool = boundary.get("within_window");
    if !within {
        return Err(RouterError::Conflict("undo_window_expired"));
    }
    let created_session: bool = boundary.get("created_session");

    let mut status = discovery::get_session_in(&mut tx, tenant_id, session_id)
        .await?
        .status;
    if created_session && status != SESSION_STATUS_PAUSED {
        if status != SESSION_STATUS_IN_PROGRESS {
            // 已被编排推进（NEEDS_USER_INPUT/REVIEW_REQUIRED/COMPLETED）：不改状态机边。
            return Err(RouterError::Conflict("discovery_session_not_undoable"));
        }
        status =
            discovery::transition_session_in(&mut tx, tenant_id, session_id, SESSION_STATUS_PAUSED)
                .await?
                .status;
    }

    sqlx::query("UPDATE ai.conversation SET discovery_session_id = NULL WHERE id = $1")
        .bind(conversation_id)
        .execute(&mut *tx)
        .await?;
    let marker = insert_message_in(
        &mut tx,
        conversation_id,
        "system",
        HANDOFF_UNDONE_NOTICE,
        "text",
        serde_json::json!({
            "handoff_undone": { "discovery_session_id": session_id }
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(UndoOutcome {
        discovery_session_id: session_id,
        session_status: status,
        boundary_message_id: marker.id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_mode_defaults_off_and_misspelling_falls_back() {
        assert_eq!(RouterMode::parse(None), RouterMode::Off);
        assert_eq!(RouterMode::parse(Some("off")), RouterMode::Off);
        assert_eq!(RouterMode::parse(Some("on")), RouterMode::On);
        assert_eq!(RouterMode::parse(Some("onn")), RouterMode::Off);
    }

    #[test]
    fn local_change_intent_rules() {
        assert!(has_local_change_intent("本项目需要手机登录"));
        assert!(has_local_change_intent("We SHOULD support SSO"));
        assert!(!has_local_change_intent("这个项目现在进度怎么样？"));
        assert!(!has_local_change_intent("你好"));
    }

    #[test]
    fn intent_vocabulary_round_trips_and_maps_kinds() {
        for i in Intent::ALL {
            assert_eq!(Intent::parse(i.as_str()), Some(i));
        }
        assert_eq!(Intent::parse("other"), None);
        assert_eq!(
            Intent::RequirementIntake.session_kind(),
            Some("TASK_DISCOVERY")
        );
        assert_eq!(Intent::KbCorrection.session_kind(), Some("UPDATE"));
        assert_eq!(
            Intent::ProjectOnboarding.session_kind(),
            Some("INITIAL_ONBOARDING")
        );
        assert_eq!(Intent::ProjectQuery.session_kind(), None);
        assert_eq!(Intent::ChitChat.session_kind(), None);
    }

    #[test]
    fn handoff_summary_is_a_summary_not_a_copy() {
        let s = handoff_summary(
            "",
            7,
            HandoffTrigger::ImplicitIntent,
            Some(Intent::KbCorrection),
        );
        assert!(s.contains("7"));
        assert!(s.contains("kb_correction"));
        assert!(s.contains("未复制"));
    }
}
