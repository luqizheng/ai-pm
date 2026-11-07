//! 13 项跨服务事件目录（V1.1 §13.3；**目录=单一真源，新增/变更走评审**）。
//!
//! 本清单是 wire 层权威：事件类型常量、event_version、payload schema（字段
//! 级声明）。`pm-application::outbox::CATALOG` 是生产登记面（enabled_by_default
//! 等运行时开关），其 `event_type` 字符串一律引用本清单常量，防两处漂移。
//!
//! 生产规则（沿既有目录语义）：「不假装消费」——目录允许登记暂无消费者的
//! 事件（登记先行），但**未启用前禁止生产**（消费方落地前不生产）。

/// 事件 payload schema 字段声明（字段名 + 类型要点，文档化用）。
pub type PayloadField = (&'static str, &'static str);

/// 一项跨服务事件契约。
#[derive(Debug, Clone, Copy)]
pub struct EventContract {
    /// 事件类型（wire 形态，小写点分）。
    pub event_type: &'static str,
    /// 当前版本（ADR-11：整数递增 + additive-only；13 项全部 v1 起步）。
    pub event_version: u32,
    /// 生产方（信封顶层 producer）。
    pub producer: &'static str,
    /// 消费方。
    pub consumers: &'static [&'static str],
    /// 幂等键业务部分（§13.3 目录列）。
    pub idempotency_key_hint: &'static str,
    /// payload schema（字段级声明；DomainRef 四元组引用 + 摘要/hash，禁止正文）。
    pub payload_fields: &'static [PayloadField],
}

/// 生产方常量（信封 `producer` 取值）。
pub const PRODUCER_PLATFORM: &str = "platform";
pub const PRODUCER_KB: &str = "kb-service";
pub const PRODUCER_PC: &str = "pc-service";
pub const PRODUCER_ANY: &str = "any";

// ── 事件类型常量（单一真源；pm-application CATALOG 引用此处） ──────────────
pub const INTAKE_RECEIVED: &str = "intake.received";
pub const KB_ANALYSIS_REQUESTED: &str = "kb.analysis.requested";
pub const KB_ANALYSIS_COMPLETED: &str = "kb.analysis.completed";
pub const KB_COGNITION_SIGNAL_EMITTED: &str = "kb.cognition_signal.emitted";
pub const KB_MEMORY_APPROVED: &str = "kb.memory.approved";
pub const PC_ANALYZE_REQUESTED: &str = "pc.analyze.requested";
pub const PC_DECISION_SUBMITTED: &str = "pc.decision.submitted";
pub const PC_ANALYSIS_COMPLETED: &str = "pc.analysis.completed";
pub const PC_CHANGE_PROPOSED: &str = "pc.change_proposed";
pub const PC_CONFLICT_DETECTED: &str = "pc.conflict.detected";
pub const PC_COGNITION_VERSION_PUBLISHED: &str = "pc.cognition.version_published";
pub const PC_EVIDENCE_REQUESTED: &str = "pc.evidence_requested";
pub const PROCESSING_FAILED: &str = "processing.failed";

pub const EVENT_VERSION: u32 = 1;

/// 13 项跨服务事件契约（V1.1 §13.3 目录全量）。
pub const CATALOG: &[EventContract] = &[
    EventContract {
        event_type: INTAKE_RECEIVED,
        event_version: EVENT_VERSION,
        producer: PRODUCER_PLATFORM,
        consumers: &["platform"],
        idempotency_key_hint: "intake_id",
        payload_fields: &[
            ("intake_ref", "DomainRef"),
            ("intake_type", "string"),
            ("content_hash", "string"),
        ],
    },
    EventContract {
        event_type: KB_ANALYSIS_REQUESTED,
        event_version: EVENT_VERSION,
        producer: PRODUCER_PLATFORM,
        consumers: &["kb"],
        idempotency_key_hint: "intake_task_id",
        payload_fields: &[
            ("intake_ref", "DomainRef"),
            ("task_ref", "DomainRef"),
            ("source_ref", "jsonb(source_reference)"),
        ],
    },
    EventContract {
        event_type: KB_ANALYSIS_COMPLETED,
        event_version: EVENT_VERSION,
        producer: PRODUCER_KB,
        consumers: &["platform"],
        idempotency_key_hint: "kb_analysis_id",
        payload_fields: &[
            ("kb_analysis_ref", "DomainRef"),
            ("source_ref", "jsonb"),
            ("document_ref", "DomainRef|null"),
            ("memory_draft_refs", "DomainRef[]"),
            ("conclusion", "string"),
        ],
    },
    EventContract {
        event_type: KB_COGNITION_SIGNAL_EMITTED,
        event_version: EVENT_VERSION,
        producer: PRODUCER_KB,
        consumers: &["platform"],
        idempotency_key_hint: "signal_hash(intake_id+内容)",
        payload_fields: &[
            ("relevance", "enum(PRODUCT_CANDIDATE|NO_PRODUCT_RELEVANCE)"),
            ("confidence", "enum(high|medium|low)"),
            ("rationale", "string<=256"),
            ("subject_refs", "DomainRef[]"),
            ("evidence_refs", "DomainRef[]"),
        ],
    },
    EventContract {
        event_type: KB_MEMORY_APPROVED,
        event_version: EVENT_VERSION,
        producer: PRODUCER_KB,
        consumers: &["platform"],
        idempotency_key_hint: "memory_revision_id",
        payload_fields: &[
            ("memory_ref", "DomainRef"),
            ("reviewed_by", "string"),
            ("approval_state", "string"),
        ],
    },
    EventContract {
        event_type: PC_ANALYZE_REQUESTED,
        event_version: EVENT_VERSION,
        producer: PRODUCER_PLATFORM,
        consumers: &["pc"],
        idempotency_key_hint: "intake_task_id",
        payload_fields: &[
            ("intake_ref", "DomainRef"),
            ("task_ref", "DomainRef"),
            ("signal", "jsonb(signal_contract)|null"),
            ("content_refs", "DomainRef[]"),
        ],
    },
    EventContract {
        event_type: PC_DECISION_SUBMITTED,
        event_version: EVENT_VERSION,
        producer: PRODUCER_PLATFORM,
        consumers: &["pc"],
        idempotency_key_hint: "intake_task_id",
        payload_fields: &[
            ("intake_ref", "DomainRef"),
            ("task_ref", "DomainRef"),
            ("decision", "jsonb(摘要)"),
        ],
    },
    EventContract {
        event_type: PC_ANALYSIS_COMPLETED,
        event_version: EVENT_VERSION,
        producer: PRODUCER_PC,
        consumers: &["platform"],
        idempotency_key_hint: "pc_analysis_id",
        payload_fields: &[
            ("pc_analysis_ref", "DomainRef"),
            ("conclusion", "enum(PROPOSED|NO_CHANGE|CONFLICT)"),
            ("refs", "DomainRef[]"),
        ],
    },
    EventContract {
        event_type: PC_CHANGE_PROPOSED,
        event_version: EVENT_VERSION,
        producer: PRODUCER_PC,
        consumers: &["platform"],
        idempotency_key_hint: "proposal_id",
        payload_fields: &[
            ("proposal_ref", "DomainRef"),
            ("governance_level", "enum(L1|L2)"),
            ("members", "jsonb(摘要)"),
            ("content_hash", "string"),
        ],
    },
    EventContract {
        event_type: PC_CONFLICT_DETECTED,
        event_version: EVENT_VERSION,
        producer: PRODUCER_PC,
        consumers: &["platform"],
        idempotency_key_hint: "conflict_id",
        payload_fields: &[
            ("conflict_ref", "DomainRef"),
            ("frozen", "jsonb(摘要)"),
        ],
    },
    EventContract {
        event_type: PC_COGNITION_VERSION_PUBLISHED,
        event_version: EVENT_VERSION,
        producer: PRODUCER_PC,
        consumers: &["platform"],
        idempotency_key_hint: "version_id",
        payload_fields: &[
            ("version_ref", "DomainRef"),
            ("version_no", "int"),
            ("snapshot_hash", "string"),
            ("proposal_ref", "DomainRef"),
        ],
    },
    EventContract {
        event_type: PC_EVIDENCE_REQUESTED,
        event_version: EVENT_VERSION,
        producer: PRODUCER_PC,
        consumers: &["platform"],
        idempotency_key_hint: "request_id",
        payload_fields: &[
            ("pc_analysis_ref", "DomainRef"),
            ("requested_about", "DomainRef[]"),
            ("reason", "string<=256"),
        ],
    },
    EventContract {
        event_type: PROCESSING_FAILED,
        event_version: EVENT_VERSION,
        producer: PRODUCER_ANY,
        consumers: &["platform"],
        idempotency_key_hint: "失败任务键+`:failed`",
        payload_fields: &[
            ("failed_ref", "DomainRef"),
            ("error_code", "string"),
            ("retry_count", "int"),
            ("recoverable", "bool"),
        ],
    },
];

/// 查事件契约；未登记 → None（视为不可生产）。
pub fn contract(event_type: &str) -> Option<&'static EventContract> {
    CATALOG.iter().find(|c| c.event_type == event_type)
}

/// 校验 payload 引用纪律（§13.2/ADR-07）：payload JSON 中所有 `*_ref`/
/// `*_refs` 命中的对象若形如引用（含 `target_domain` 键），必须为四元组
/// DomainRef 且**不得含 `service`/`source_service`/`target_service` 键**。
///
/// 这是「事件 payload 只含四元组引用、禁止内嵌 service 字段」红线的机械
/// 校验入口（生产方在写 outbox 前调用；消费者亦可在侧做防御性校验）。
pub fn validate_payload_references(payload: &serde_json::Value) -> Result<(), PayloadViolation> {
    walk(payload)
}

/// payload 引用纪律违规详情。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PayloadViolation {
    #[error("embedded service metadata in reference object: {0}")]
    EmbeddedServiceField(String),
    #[error("reference object missing target_domain/type/id at path {0}")]
    IncompleteRef(String),
    #[error("unknown target_domain `{1}` at path {0}")]
    UnknownDomain(String, String),
}

const SERVICE_FORBIDDEN_KEYS: &[&str] = &["service", "source_service", "target_service"];

fn path_join(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        key.to_string()
    } else {
        format!("{parent}.{key}")
    }
}

fn walk(value: &serde_json::Value) -> Result<(), PayloadViolation> {
    walk_at(value, "")
}

fn walk_at(value: &serde_json::Value, path: &str) -> Result<(), PayloadViolation> {
    match value {
        serde_json::Value::Object(map) => {
            // 形如引用的对象：出现 target_domain 键即按引用对象全检。
            if map.contains_key("target_domain") {
                for key in SERVICE_FORBIDDEN_KEYS {
                    if map.contains_key(*key) {
                        return Err(PayloadViolation::EmbeddedServiceField(path_join(
                            path, key,
                        )));
                    }
                }
                for key in ["target_domain", "target_type", "target_id"] {
                    let ok = map
                        .get(key)
                        .map(|v| v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false))
                        .unwrap_or(false);
                    if !ok {
                        return Err(PayloadViolation::IncompleteRef(path_join(path, key)));
                    }
                }
                let domain = map.get("target_domain").and_then(|v| v.as_str()).unwrap_or("");
                crate::registry::parse_domain(domain).map_err(|_| {
                    PayloadViolation::UnknownDomain(path.to_string(), domain.to_string())
                })?;
                if let Some(rev) = map.get("target_revision_id") {
                    if !rev.is_null() && rev.as_str().is_none() {
                        return Err(PayloadViolation::IncompleteRef(path_join(
                            path,
                            "target_revision_id",
                        )));
                    }
                }
            }
            for (k, v) in map {
                walk_at(v, &path_join(path, k))?;
            }
            Ok(())
        }
        serde_json::Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                walk_at(item, &format!("{path}[{i}]"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_exactly_thirteen_events() {
        assert_eq!(CATALOG.len(), 13);
        for c in CATALOG {
            assert_eq!(c.event_version, 1, "{} must be v1", c.event_type);
            assert!(c.event_type.chars().all(|ch| ch == '.'
                || ch == '_'
                || ch.is_ascii_lowercase()
                || ch.is_ascii_digit()));
        }
    }

    #[test]
    fn contract_lookup() {
        assert_eq!(contract(KB_COGNITION_SIGNAL_EMITTED).unwrap().producer, PRODUCER_KB);
        assert!(contract("nope.event").is_none());
    }

    #[test]
    fn payload_guard_rejects_service_field() {
        let bad = serde_json::json!({
            "subject_refs": [
                {"target_domain": "knowledge_base", "target_type": "memory_revision",
                 "target_id": "m1", "target_revision_id": "r1", "service": "kb-service"}
            ]
        });
        assert!(matches!(
            validate_payload_references(&bad),
            Err(PayloadViolation::EmbeddedServiceField(_))
        ));
    }

    #[test]
    fn payload_guard_accepts_clean_refs() {
        let ok = serde_json::json!({
            "relevance": "PRODUCT_CANDIDATE", "confidence": "medium",
            "rationale": "提取到 2 条业务规则候选",
            "subject_refs": [
                {"target_domain": "knowledge_base", "target_type": "memory_revision",
                 "target_id": "m1", "target_revision_id": "r1"}
            ],
            "evidence_refs": []
        });
        assert_eq!(validate_payload_references(&ok), Ok(()));
    }

    #[test]
    fn payload_guard_rejects_unknown_domain() {
        let bad = serde_json::json!({
            "subject_refs": [{"target_domain": "kb", "target_type": "t", "target_id": "1"}]
        });
        assert!(matches!(
            validate_payload_references(&bad),
            Err(PayloadViolation::UnknownDomain(_, _))
        ));
    }
}
