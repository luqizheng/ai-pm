//! 可见性预取与内存判定器（应用层）。
//!
//! 依据 docs/visualization/ai-pm-viz-14-v0-3-projection-consistency-design-v1.md §7：
//! 可见性判定**统一汇到项目成员口径**（`core.project_member` + 项目未删除）——
//! 不为图查询另建一套可见性规则（否则两套规则必然漂移）。
//!
//! ## 为什么要「预取 + 内存判定」两段式
//!
//! 判定器 trait 是**同步**的（校验器与投影层因此保持纯函数，见 12 号设计 §4.2），
//! 但可见性要落到库上。解法是把异步环节收敛到本模块：
//!
//! 1. [`probe_visible_objects`]：一次性把 Spec 涉及的引用批量查库，算出可见集合；
//! 2. [`InMemoryObjectResolver`]：纯内存查表，实现同步 trait。
//!
//! 顺带避免 N+1：逐节点查库会随图规模线性放大往返次数。
//!
//! ## 口径要点
//!
//! - 跨对象可见性：对象 → 所属项目 → 项目成员判定（先例：
//!   `apps/pm-api/src/pm_designs.rs:54-95` 的 `require_adr_visible`）；
//! - **无权一律按"不存在"处理**——对外不区分"不存在"与"无权"（404 同形惯例）；
//! - 端点**双端隔离**：关系两端都要通过可见性判定，跨项目脏引用不得渗入
//!   （先例：`migrations/0007_relationship_owner_domain_seeds.sql:5-8`）。

use std::collections::HashSet;

use sqlx::PgPool;
use uuid::Uuid;

use pm_domain::visualization::ObjectResolver;

/// 调用者上下文（预取时绑定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallerScope {
    /// 调用者租户。
    pub tenant_id: Uuid,
    /// 调用者主体。
    pub principal_id: Uuid,
}

impl CallerScope {
    pub fn new(tenant_id: Uuid, principal_id: Uuid) -> Self {
        Self {
            tenant_id,
            principal_id,
        }
    }
}

/// 一次引用：`domain` + `type` + `id`（与 `source_ref` 同构）。
pub type SourceKey = (String, String, String);

/// 内存判定器：基于预取好的可见集合做查表。
#[derive(Debug, Clone, Default)]
pub struct InMemoryObjectResolver {
    visible: HashSet<SourceKey>,
    visible_ids: HashSet<String>,
}

impl InMemoryObjectResolver {
    /// 由预取结果构建。
    pub fn new(visible: HashSet<SourceKey>) -> Self {
        let visible_ids = visible
            .iter()
            .map(|(_, _, id)| id.clone())
            .collect::<HashSet<String>>();
        Self {
            visible,
            visible_ids,
        }
    }

    /// 可见对象数（审计用）。
    pub fn visible_count(&self) -> usize {
        self.visible.len()
    }
}

impl ObjectResolver for InMemoryObjectResolver {
    fn exists_and_visible(&self, domain: &str, r#type: &str, id: &str) -> bool {
        // 优先按完整键匹配（domain+type+id）；派生路径上的中间对象只带 id，
        // 按 id 匹配（其可见性在预取时已按实际来源域判定）。
        if self
            .visible
            .contains(&(domain.to_string(), r#type.to_string(), id.to_string()))
        {
            return true;
        }
        self.visible_ids.contains(id)
    }

    fn relation_exists(&self, from: &str, to: &str, _relation: &str) -> bool {
        // 关系存在 ⇔ 两端都可见（不可见端不得通过"关系是否存在"被探测到）
        self.visible_ids.contains(from) && self.visible_ids.contains(to)
    }
}

/// 批量预取：算出 Spec 涉及的引用中，**调用者可见**的那些。
///
/// 返回可见引用集合（供 [`InMemoryObjectResolver`] 使用）。
///
/// 说明：`domain`/`type` 取值与 `source_ref` 同构（`delivery`/`requirement`、
/// `design`/`design_revision` …），与库内表名一一对应。
pub async fn probe_visible_objects(
    pool: &PgPool,
    scope: CallerScope,
    refs: &[SourceKey],
) -> HashSet<SourceKey> {
    let mut visible: HashSet<SourceKey> = HashSet::new();
    if refs.is_empty() {
        return visible;
    }

    // 收集 uuid 形态的 id（业务自然键 id 无法按 uuid 查——它们按 id 直接命中）
    let mut uuid_ids: Vec<Uuid> = Vec::new();
    for (_, _, id) in refs {
        if let Ok(parsed) = Uuid::parse_str(id) {
            if !uuid_ids.contains(&parsed) {
                uuid_ids.push(parsed);
            }
        }
    }
    if uuid_ids.is_empty() {
        return visible;
    }

    // 一次查询取回所有候选对象的 (id, domain, type, project_id)
    let detailed: Vec<(Uuid, String, String, Uuid)> = sqlx::query_as(
        "SELECT id, 'delivery', 'requirement', project_id FROM delivery.requirement \
          WHERE id = ANY($1) AND tenant_id = $2 \
         UNION ALL \
         SELECT r.id, 'delivery', 'requirement_revision', r.project_id \
           FROM delivery.requirement_revision r \
          WHERE r.id = ANY($1) AND r.tenant_id = $2 \
         UNION ALL \
         SELECT id, 'delivery', 'task', project_id FROM delivery.task \
          WHERE id = ANY($1) AND tenant_id = $2 \
         UNION ALL \
         SELECT id, 'design', 'design', project_id FROM design.design \
          WHERE id = ANY($1) AND tenant_id = $2 \
         UNION ALL \
         SELECT r.id, 'design', 'design_revision', d.project_id \
           FROM design.design_revision r JOIN design.design d ON d.id = r.design_id \
          WHERE r.id = ANY($1) AND r.tenant_id = $2 \
         UNION ALL \
         SELECT id, 'design', 'adr', project_id FROM design.adr \
          WHERE id = ANY($1) AND tenant_id = $2 \
         UNION ALL \
         SELECT r.id, 'design', 'adr_revision', a.project_id \
           FROM design.adr_revision r JOIN design.adr a ON a.id = r.adr_id \
          WHERE r.id = ANY($1) AND r.tenant_id = $2 \
         UNION ALL \
         SELECT id, 'plan', 'task_plan', project_id FROM plan.task_plan \
          WHERE id = ANY($1) AND tenant_id = $2 \
         UNION ALL \
         SELECT id, 'pc', 'cognition_object', project_id FROM pc.cognition_object \
          WHERE id = ANY($1) AND tenant_id = $2 \
         UNION ALL \
         SELECT id, 'kb', 'document', project_id FROM kb.document \
          WHERE id = ANY($1) AND tenant_id = $2 \
         UNION ALL \
         SELECT id, 'kb', 'memory', project_id FROM kb.memory \
          WHERE id = ANY($1) AND tenant_id = $2 \
         UNION ALL \
         SELECT id, 'kb', 'source', project_id FROM kb.source \
          WHERE id = ANY($1) AND tenant_id = $2 \
         UNION ALL \
         SELECT id, 'ai', 'cognition_item', project_id FROM ai.cognition_item \
          WHERE id = ANY($1) AND tenant_id = $2",
    )
    .bind(&uuid_ids)
    .bind(scope.tenant_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if detailed.is_empty() {
        return visible;
    }

    // 批量判定项目可见性（唯一口径：core.project_member + 项目未删除）
    let mut project_ids: Vec<Uuid> = detailed.iter().map(|(_, _, _, pid)| *pid).collect();
    project_ids.sort();
    project_ids.dedup();

    let visible_projects: HashSet<Uuid> = sqlx::query_scalar::<_, Uuid>(
        "SELECT p.id FROM core.project p \
           JOIN core.project_member m ON m.project_id = p.id \
          WHERE p.id = ANY($1) AND p.tenant_id = $2 AND m.principal_id = $3'",
    )
    .bind(&project_ids)
    .bind(scope.tenant_id)
    .bind(scope.principal_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    for (id, domain, r#type, project_id) in detailed {
        if visible_projects.contains(&project_id) {
            visible.insert((domain, r#type, id.to_string()));
        }
    }

    visible
}
