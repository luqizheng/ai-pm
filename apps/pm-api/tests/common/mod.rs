//! 集成测试共用夹具。
#![allow(dead_code)]

use std::sync::Once;

use pm_application::knowledge::KnowledgeRoot;
use sqlx::PgPool;
use uuid::Uuid;

static KNOWLEDGE_ENV: Once = Once::new();

/// S02-I2：HTTP 链路测试的知识库根——进程级一次性注入临时目录
/// （REST submit 经 `KnowledgeRoot::from_env()` 读取；默认回退路径在 checkout
/// 内会被 ensure_ready 拒绝）。服务层直调测试用 [`knowledge_root`] 每用例独立根。
pub fn ensure_knowledge_env() {
    KNOWLEDGE_ENV.call_once(|| {
        let dir = std::env::temp_dir()
            .join(format!("ai-pm-knowledge-env-{}", Uuid::new_v4()));
        std::env::set_var("AI_PM_KNOWLEDGE_ROOT", &dir);
    });
}

/// S02-I2：服务层直调用例的独立临时知识库根（ensure_ready 自动 git init）。
pub fn knowledge_root() -> KnowledgeRoot {
    KnowledgeRoot::new(
        std::env::temp_dir().join(format!("ai-pm-knowledge-{}", Uuid::new_v4())),
    )
}

/// T-022：`router()` 的 off 模式注入 `dev-off` principal；`/v1/pm/*` 读端点
/// 按 `core.project_member` 过滤，因此测试种子项目必须把该 principal 加为成员。
pub async fn join_dev(pool: &PgPool, project_id: Uuid) {
    // off 模式只在写请求时物化 dev-off principal；纯 GET 用例需先建行满足外键。
    sqlx::query(
        "INSERT INTO core.principal (id, kind, display_name, is_active) \
         VALUES ($1, 'human', 'dev-off', true) ON CONFLICT (id) DO NOTHING",
    )
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.project_member (tenant_id, project_id, principal_id, role) \
         VALUES ((SELECT tenant_id FROM core.project WHERE id = $1), $1, $2, 'owner')",
    )
    .bind(project_id)
    .bind(pm_api::auth::DEV_OFF_PRINCIPAL)
    .execute(pool)
    .await
    .unwrap();
}
