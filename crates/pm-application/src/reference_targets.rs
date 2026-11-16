//! 跨域引用目标的物理表映射与作用域探测（SEC-P2-1 / SEC-P2-2）。
//!
//! 域枚举与通道状态沿用 `kb_pc_contracts::registry::TargetDomain`；物理表映射
//! 属于应用服务层（contracts 不感知 schema）。所有表名都是本模块内的编译期
//! 常量，不接受外部输入，因此 SQL 中直接拼接表名不存在注入面。
//!
//! 两类目标语义（实施前验证结论）：
//! - **对象型引用**（`strict = true`）：`target_id` 必须指向本租户 / 本项目内
//!   真实存在的对象；不存在或跨租户一律拒绝。
//! - **逻辑检索锚**（`strict = false`）：`kb.memory_ref` 的 `requirement/task`
//!   subject 是检索锚（定义稿 §5.1），Platform 侧 `discussion_memory::resolve_refs`
//!   已做同项目探测；服务层只兜底「该 id 若已登记为对象，不得属于别的租户」，
//!   表中不存在则沿用现状放行——否则会破坏「同主题 id 跨项目不得串」的检索
//!   隔离语义（`discussion_memory_injection` 既有夹具）。

use kb_pc_contracts::registry::TargetDomain;
use sqlx::{Postgres, Row};
use uuid::Uuid;

/// 目标探测结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetProbe {
    /// 目标存在于指定作用域内。
    pub in_scope: bool,
    /// 目标在表中存在（不限作用域）——用于区分「跨租户」与「纯逻辑锚」。
    pub anywhere: bool,
}

/// 域目标：物理表 + 是否要求存在性。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainTarget {
    pub table: &'static str,
    pub strict: bool,
}

impl DomainTarget {
    /// 裁决探测结果：`true` = 允许引用。
    pub fn allows(self, probe: TargetProbe) -> bool {
        if probe.in_scope {
            return true;
        }
        // 已登记但不在作用域内 → 跨租户 / 跨项目引用，一律拒绝。
        if probe.anywhere {
            return false;
        }
        // 表中不存在：对象型引用要求存在性；逻辑检索锚沿用现状放行。
        !self.strict
    }
}

/// `target_domain`（+ `target_type`）→ 物理表。
/// 映射口径：registry.rs 逐域通道表 + SEC-P2-1 任务书域→表清单。
pub fn domain_target(domain: TargetDomain, target_type: &str) -> Option<DomainTarget> {
    use TargetDomain::*;
    let (table, strict) = match domain {
        // task 是 V1 逻辑检索锚；requirement 等其余类型是实体引用。
        Requirement => match target_type {
            "task" => ("delivery.task", false),
            _ => ("delivery.requirement", true),
        },
        ProjectGoal => ("ai.project_document", true),
        // 旧 ai 栈 cognition_item 与新 pc 栈 cognition_object 均属该域（registry.rs
        // 注释：S01-04 技术项目认知，ai.cognition_*，V1 只读）。
        ProjectCognition => match target_type {
            "cognition_item" => ("ai.cognition_item", true),
            _ => ("pc.cognition_object", true),
        },
        ProductCognition => ("pc.cognition_object", true),
        Design => ("design.design", true),
        Adr => ("design.adr", true),
        // knowledge_base 由 kb_service::memory::check_kb_refs_tx 的既有
        // `kb.source → kb.source_project` 通道裁决，此处不重复判定；
        // 另两个域已被 resolve_reference 的通道状态机拒绝。
        KnowledgeBase | ProductReality | ProductPlan => return None,
    };
    Some(DomainTarget { table, strict })
}

/// `pc.source.source_kind` → 物理表（白名单见 0001_baseline `source_kind_check`）。
/// 外部 / 自由文本来源（`manual_input` / `external_reference`）的 `target_id`
/// 是外部标识，不做对象存在性探测，保留既有通道状态机裁决。
pub fn source_kind_target(kind: &str, target_type: &str) -> Option<&'static str> {
    match kind {
        // revision 表无 project_id 列；实体表有，revision 归属在 register_source
        // 内以 kb.document_revision JOIN kb.document 单独校验。
        "kb_document" => Some("kb.document"),
        "kb_memory" => Some("kb.memory"),
        "requirement" => Some("delivery.requirement"),
        "requirement_revision" => Some("delivery.requirement_revision"),
        "discovery_finding" => Some("discovery.discovery_finding"),
        "project_cognition" => Some(match target_type {
            "cognition_item" => "ai.cognition_item",
            _ => "pc.cognition_object",
        }),
        // P3-11：projc.source 来源登记的两类新 kind——product_cognition 指向
        // pc.cognition_object（PC 对象可作为 ProjC 认知的来源被探测）；repo_analysis
        // 的 target_id 是仓库分析自述标识，不做对象探测（external 同口径）。
        "product_cognition" => Some("pc.cognition_object"),
        "repo_analysis" => None,
        "manual_input" | "external_reference" => None,
        _ => None,
    }
}

/// 探测目标归属。`project_id = None` 时只按租户裁决（SEC-P2-1 同租户口径）；
/// `Some` 时同时按项目裁决（SEC-P2-2 同 project 口径）。
pub async fn probe<'e, E>(
    executor: E,
    table: &str,
    target_id: Uuid,
    tenant_id: Uuid,
    project_id: Option<Uuid>,
) -> Result<TargetProbe, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let project_clause = if project_id.is_some() {
        " AND project_id = $3"
    } else {
        ""
    };
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM {table} \
                        WHERE id = $1 AND tenant_id = $2{project_clause}) AS in_scope, \
                EXISTS (SELECT 1 FROM {table} WHERE id = $1) AS anywhere"
    );
    let mut query = sqlx::query(&sql).bind(target_id).bind(tenant_id);
    if let Some(project_id) = project_id {
        query = query.bind(project_id);
    }
    let row = query.fetch_one(executor).await?;
    Ok(TargetProbe {
        in_scope: row.try_get("in_scope")?,
        anywhere: row.try_get("anywhere")?,
    })
}
