//! Outbox 写入端口（task state machine spec §20.2 / §20.6）。
//!
//! 业务事务内写入「待发布事件」，让「状态已变、事件却没写」不可表示：`append`
//! 只接受 `&mut Transaction`，事件行随调用方事务一起提交或回滚。投递为至少一次
//! （§20.6.3），故写入按 `idempotency_key` 幂等去重——重复的语义事件在写入时
//! 坍缩为同一行，消费侧 handler 亦须幂等。
//!
//! 端口定义在应用层、由 Postgres 直写适配（[`PgOutboxAppend`]）：应用层不依赖
//! 基础设施层的 outbox 模块，却仍与业务状态变更共用**同一个**事务，不引入第二
//! 套事务（§20.6.1）。

use async_trait::async_trait;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// 一个待写入的 outbox 事件，落在调用方业务事务里。
pub struct OutboxEvent<'a> {
    pub project_id: Uuid,
    pub event_type: &'a str,
    pub aggregate_type: &'a str,
    pub aggregate_id: &'a str,
    pub payload: serde_json::Value,
    /// 语义幂等键：相同键的重复事件在写入时坍缩为同一行。
    pub idempotency_key: &'a str,
}

/// 在同一业务事务内写入 outbox 事件的端口。
#[async_trait]
pub trait OutboxAppend: Send + Sync {
    async fn append<'a>(
        &self,
        tx: &mut Transaction<'a, Postgres>,
        event: OutboxEvent<'_>,
    ) -> Result<Uuid, sqlx::Error>;
}

/// Postgres 直写适配器：在既有事务里 INSERT，冲突键幂等复用既有行。
pub struct PgOutboxAppend;

#[async_trait]
impl OutboxAppend for PgOutboxAppend {
    async fn append<'a>(
        &self,
        tx: &mut Transaction<'a, Postgres>,
        event: OutboxEvent<'_>,
    ) -> Result<Uuid, sqlx::Error> {
        let inserted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO integration.outbox_event \
             (project_id, event_type, aggregate_type, aggregate_id, payload, idempotency_key, tenant_id) \
             VALUES ($1, $2, $3, $4, $5, $6, \
                     (SELECT tenant_id FROM core.project WHERE id = $1)) \
             ON CONFLICT (tenant_id, idempotency_key) DO NOTHING \
             RETURNING id",
        )
        .bind(event.project_id)
        .bind(event.event_type)
        .bind(event.aggregate_type)
        .bind(event.aggregate_id)
        .bind(event.payload)
        .bind(event.idempotency_key)
        .fetch_optional(&mut **tx)
        .await?;
        match inserted {
            Some(id) => Ok(id),
            None => {
                sqlx::query_scalar(
                    "SELECT id FROM integration.outbox_event \
                      WHERE idempotency_key = $1 \
                        AND tenant_id = (SELECT tenant_id FROM core.project WHERE id = $2)",
                )
                .bind(event.idempotency_key)
                .bind(event.project_id)
                .fetch_one(&mut **tx)
                .await
            }
        }
    }
}
/// 事件类型的**单一真源**（T-061 §6）：谁生产、谁消费、默认是否生产。
///
/// 事件类型过去散落在 `event_is_enabled` 的 match、Worker 的 `.with_handler`
/// 调用与各生产点的字符串字面量里，三处不同步就会出「产生了但没人消费」或
/// 「消费了但没人产生」。本目录把三处收敛到一处。
///
/// V1.1 §13.3（KB-PC 服务化）起，跨服务事件的 wire 权威 =
/// `kb-pc-contracts::events::CATALOG`（event_version / payload schema /
/// producer 消费方契约）；本目录是**运行时登记面**（has_handler /
/// enabled_by_default），`event_type` 字符串一律引用契约 crate 常量。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventSpec {
    pub event_type: &'static str,
    /// 事件契约版本（ADR-11 additive-only；与 kb-pc-contracts 声明一致）。
    pub event_version: u32,
    /// 是否存在真实消费 handler。`false` 的类型**禁止生产**——生产了也没人
    /// 消费，只会变成死信污染运维面板（§20.6「不假装消费」）。
    pub has_handler: bool,
    /// 默认是否生产。环境变量只能在 `has_handler = true` 的集合内收窄，
    /// 不能突破 `has_handler` 限制。
    pub enabled_by_default: bool,
    /// 生产者所在位置（文档用）。
    pub producer: &'static str,
}

/// 全部已登记事件类型。`RunRequested` **刻意不在此列**：建 Run 的动作已由
/// `TaskReady` handler 承担，再发一条是重复语义（`target.md` §P8「不为凑数
/// 造 Handler」）。
pub const CATALOG: &[EventSpec] = &[
    EventSpec {
        event_type: "TaskReady",
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "task_dependency.rs",
    },
    EventSpec {
        event_type: "DeliverySubmitted",
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "delivery/mod.rs",
    },
    EventSpec {
        event_type: "ReviewCompleted",
        event_version: 1,
        has_handler: false,
        enabled_by_default: false,
        producer: "review/decide.rs",
    },
    EventSpec {
        event_type: "KnowledgeBaseWriteRequested",
        event_version: 1,
        has_handler: false,
        enabled_by_default: false,
        producer: "review/decide.rs",
    },
    EventSpec {
        event_type: "RunCompleted",
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "run_executor.rs",
    },
    // ── S02D-22 需求域事件族（S02-04 V1.2 §13.3）：目录登记先行，消费方
    // （总控台/通知/I7 巡检差分）落地前不生产——「不假装消费」。生产点=
    // pm-application::requirement（S02-I1 服务内留位，启用时随命令事务写入）。
    EventSpec {
        event_type: "RequirementCreated",
        event_version: 1,
        has_handler: false,
        enabled_by_default: false,
        producer: "requirement.rs（留位）",
    },
    EventSpec {
        event_type: "RequirementRevisionSubmitted",
        event_version: 1,
        has_handler: false,
        enabled_by_default: false,
        producer: "requirement.rs（留位）",
    },
    EventSpec {
        event_type: "RequirementConfirmed",
        event_version: 1,
        has_handler: false,
        enabled_by_default: false,
        producer: "requirement.rs（留位）",
    },
    EventSpec {
        event_type: "RequirementRevisionRejected",
        event_version: 1,
        has_handler: false,
        enabled_by_default: false,
        producer: "requirement.rs（留位）",
    },
    EventSpec {
        event_type: "RequirementSuperseded",
        event_version: 1,
        has_handler: false,
        enabled_by_default: false,
        producer: "requirement.rs（留位）",
    },
    EventSpec {
        event_type: "RequirementWithdrawn",
        event_version: 1,
        has_handler: false,
        enabled_by_default: false,
        producer: "requirement.rs（留位）",
    },
    EventSpec {
        event_type: "RequirementArchived",
        event_version: 1,
        has_handler: false,
        enabled_by_default: false,
        producer: "requirement.rs（留位）",
    },
    EventSpec {
        event_type: "RequirementBasisChanged",
        event_version: 1,
        has_handler: false,
        enabled_by_default: false,
        producer: "requirement.rs（留位；I4 改绑命令启用）",
    },
    // ── KB-PC 服务化 13 项跨服务事件（V1.1 §13.3；wire 权威=
    // kb-pc-contracts::events::CATALOG，此处登记运行时面）。KP-1 起发布事务
    // 真实生产 pc.cognition.version_published（pc/publish.rs ⑫b），Platform
    // 消费面=状态感知留痕（kb_pc_handlers::PcVersionPublishedHandler）。
    EventSpec {
        event_type: kb_pc_contracts::events::INTAKE_RECEIVED,
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "intake.rs（intake 事务内）",
    },
    EventSpec {
        event_type: kb_pc_contracts::events::KB_ANALYSIS_REQUESTED,
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "intake.rs（intake 路由）",
    },
    EventSpec {
        event_type: kb_pc_contracts::events::KB_ANALYSIS_COMPLETED,
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "kb 桩（outbox_handlers.rs）",
    },
    EventSpec {
        event_type: kb_pc_contracts::events::KB_COGNITION_SIGNAL_EMITTED,
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "kb 桩（outbox_handlers.rs）",
    },
    EventSpec {
        event_type: kb_pc_contracts::events::KB_MEMORY_APPROVED,
        event_version: 1,
        // D-8：consumer 仅 platform（状态感知与待办）；不作为 PC 触发输入。
        has_handler: true,
        enabled_by_default: true,
        producer: "kb 记忆审核（Phase 4 留位）",
    },
    EventSpec {
        event_type: kb_pc_contracts::events::PC_ANALYZE_REQUESTED,
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "intake.rs（RT 路由）",
    },
    EventSpec {
        event_type: kb_pc_contracts::events::PC_DECISION_SUBMITTED,
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "intake.rs（product_decision 受理）",
    },
    EventSpec {
        event_type: kb_pc_contracts::events::PC_ANALYSIS_COMPLETED,
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "pc 桩（outbox_handlers.rs）",
    },
    EventSpec {
        event_type: kb_pc_contracts::events::PC_CHANGE_PROPOSED,
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "pc 桩（outbox_handlers.rs）",
    },
    EventSpec {
        event_type: kb_pc_contracts::events::PC_CONFLICT_DETECTED,
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "pc 桩（outbox_handlers.rs）",
    },
    EventSpec {
        event_type: kb_pc_contracts::events::PC_COGNITION_VERSION_PUBLISHED,
        event_version: 1,
        // KP-1：发布事务（pc/publish.rs ⑫b）生产 + Platform 留痕消费。
        has_handler: true,
        enabled_by_default: true,
        producer: "pc 发布事务（pc/publish.rs）",
    },
    EventSpec {
        event_type: kb_pc_contracts::events::PC_EVIDENCE_REQUESTED,
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "pc 桩（outbox_handlers.rs）",
    },
    EventSpec {
        event_type: kb_pc_contracts::events::PROCESSING_FAILED,
        event_version: 1,
        has_handler: true,
        enabled_by_default: true,
        producer: "任意服务失败上报",
    },
];

/// 查目录条目。未登记的事件类型**一律视为不可生产**。
pub fn event_spec(event_type: &str) -> Option<&'static EventSpec> {
    CATALOG.iter().find(|spec| spec.event_type == event_type)
}

/// 目录中所有**有消费者**的事件类型（`register_all_handlers` 必须覆盖全集）。
pub fn handled_event_types() -> impl Iterator<Item = &'static str> {
    CATALOG
        .iter()
        .filter(|spec| spec.has_handler)
        .map(|spec| spec.event_type)
}

/// 某事件类型当前是否允许产生（§20.6：对暂不支持的事件用功能开关关闭，不产生
/// 无 handler 可消费的悬空事件）。
///
/// **目录优先**（T-061 §6.1）：
/// 1. 未登记的类型 → `false`；
/// 2. `has_handler = false` 的类型 → `false`，**环境变量不能突破**——否则
///    管理员配一个无 handler 的事件类型即可制造悬空事件，随后被 Worker 死信；
/// 3. `AI_PM_OUTBOX_EVENTS`（逗号分隔）存在时，只能在 `has_handler = true` 的
///    集合内**收窄**选择；
/// 4. 否则按目录的 `enabled_by_default`。
pub fn event_is_enabled(event_type: &str) -> bool {
    let Some(spec) = event_spec(event_type) else {
        return false;
    };
    if !spec.has_handler {
        return false;
    }
    if let Ok(value) = std::env::var("AI_PM_OUTBOX_EVENTS") {
        return value
            .split(',')
            .map(str::trim)
            .any(|item| item == event_type);
    }
    spec.enabled_by_default
}
