use super::*;

/// 读取单一作用域层的策略声明；缺行返回 `None`（该层不表态）。
pub(crate) async fn load_scope_layer(
    tx: &mut Transaction<'_, Postgres>,
    scope: &str,
    scope_key: &str,
) -> Result<Option<PolicyLayer>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT auto_accept, auto_merge, auto_publish_knowledge, \
                require_human_approval, additional_checks \
           FROM delivery.policy_scope \
          WHERE scope = $1 AND scope_key = $2",
    )
    .bind(scope)
    .bind(scope_key)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| {
        Ok(PolicyLayer {
            auto_accept: row.get("auto_accept"),
            auto_merge: row.get("auto_merge"),
            auto_publish_knowledge: row.get("auto_publish_knowledge"),
            require_human_approval: row.get("require_human_approval"),
            additional_checks: decode_checks(row.get("additional_checks"))?,
        })
    })
    .transpose()
}


/// 任务级覆盖；缺行返回 `None`，`NULL` 列由 `PolicyLayer` 逐字段表达。
pub(crate) async fn load_task_layer(
    tx: &mut Transaction<'_, Postgres>,
    task_id: Uuid,
) -> Result<Option<PolicyLayer>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT auto_accept, auto_merge, auto_publish_knowledge, \
                require_human_approval, additional_checks \
           FROM delivery.task_policy_override \
          WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| {
        Ok(PolicyLayer {
            auto_accept: row.get("auto_accept"),
            auto_merge: row.get("auto_merge"),
            auto_publish_knowledge: row.get("auto_publish_knowledge"),
            require_human_approval: row.get("require_human_approval"),
            additional_checks: decode_checks(row.get("additional_checks"))?,
        })
    })
    .transpose()
}


/// 四层策略读取（§15.4）：organization → project → task_type → task。
pub(crate) async fn load_policy_layers(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    task_type: Option<&str>,
) -> Result<PolicyLayers, sqlx::Error> {
    let organization = load_scope_layer(tx, "organization", "*").await?;
    let project = load_scope_layer(tx, "project", &project_id.to_string()).await?;
    let task_type_layer = match task_type {
        Some(kind) => load_scope_layer(tx, "task_type", kind).await?,
        None => None,
    };
    let task = load_task_layer(tx, task_id).await?;
    Ok(PolicyLayers {
        organization,
        project,
        task_type: task_type_layer,
        task,
    })
}


/// `additional_checks` 必须是数组；脏数据上抛为策略读取失败，**不得静默忽略**。
pub(crate) fn decode_checks(value: serde_json::Value) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    serde_json::from_value(value).map_err(|error| sqlx::Error::Decode(Box::new(error)))
}


/// 有效审批评估（R5）：`required` 已由四层合成给出，因此任务级 `false`
/// 无法放宽高层 `true`。需要审批时必须存在 `approved` 证据；未决（pending）
/// 或缺证据（unknown）都**不得**默认放行。
pub(crate) async fn assess_approval(
    tx: &mut Transaction<'_, Postgres>,
    task_id: Uuid,
    required: bool,
) -> Result<(&'static str, bool), sqlx::Error> {
    if !required {
        return Ok(("approved", true));
    }
    let approved: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.change_proposal cp \
           JOIN governance.approval_request ar ON ar.proposal_id = cp.id \
          WHERE cp.target_id = $1 AND ar.status = 'approved'",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;
    if approved > 0 {
        return Ok(("approved", true));
    }
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai.change_proposal cp \
           JOIN governance.approval_request ar ON ar.proposal_id = cp.id \
          WHERE cp.target_id = $1 AND ar.status = 'pending'",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;
    if pending > 0 {
        return Ok(("pending", false));
    }
    Ok(("unknown", false))
}

        /// 可领取性判定的结果：`deny` 非空表示拒绝；策略字段供 `policy_snapshot` 记录。
pub struct ClaimAssessment {
    pub deny: Option<&'static str>,
    pub approval_required: bool,
    pub approval_status: &'static str,
    /// 策略门 + 审批证据的最终可执行判定；**不是**字面量，也不等同于 `auto_accept`
    /// （后者只决定验收是否自动化，不决定本次能否启动）。
    pub execution_enabled: bool,
    pub policy_version: String,
    pub policy_hash: String,
    pub effective_policy: EffectivePolicy,
}


/// 可领取性判定（§5.6 清单，全部通过才领取）。
pub(crate) async fn claimability_assessment(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    task_id: Uuid,
    retry_exhausted: bool,
    task_type: Option<&str>,
) -> Result<ClaimAssessment, sqlx::Error> {
    let layers = load_policy_layers(tx, project_id, task_id, task_type).await?;
    let effective = EffectivePolicy::compose(&layers);
    let (approval_status, execution_enabled) =
        assess_approval(tx, task_id, effective.require_human_approval).await?;
    let policy_hash = effective.content_hash();
    let policy_version = policy_hash[..12].to_string();

    let mut assessment = ClaimAssessment {
        deny: None,
        approval_required: effective.require_human_approval,
        approval_status,
        execution_enabled,
        policy_version,
        policy_hash,
        effective_policy: effective,
    };
    if retry_exhausted {
        assessment.deny = Some("retry_exhausted");
        return Ok(assessment);
    }
    if !assessment.execution_enabled {
        assessment.deny = Some("policy_requires_approval");
        return Ok(assessment);
    }
    // 无活跃等待
    let waiting: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM delivery.task_waiting WHERE task_id = $1 AND ended_at IS NULL LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(&mut **tx)
    .await?;
    if waiting.is_some() {
        assessment.deny = Some("task_waiting");
        return Ok(assessment);
    }
    // 判定 4：已指派且执行端为 dev_agent。
    let assignee_is_executor: Option<bool> = sqlx::query_scalar(
        "SELECT (p.kind = 'dev_agent') FROM delivery.task t \
           JOIN core.principal p ON p.id = t.assignee_id \
          WHERE t.id = $1",
    )
    .bind(task_id)
    .fetch_optional(&mut **tx)
    .await?;
    if assignee_is_executor != Some(true) {
        assessment.deny = Some("no_executor_assignee");
        return Ok(assessment);
    }
    // 至少一个可写仓库
    let writable: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_repository WHERE task_id = $1 AND write_access",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;
    if writable == 0 {
        assessment.deny = Some("no_repository_scope");
        return Ok(assessment);
    }
    // 全部声明仓库已达 verified
    let unverified: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_repository tr \
           JOIN integration.project_repository pr \
             ON pr.id = tr.repository_id AND pr.project_id = tr.project_id \
          WHERE tr.task_id = $1 AND pr.status <> 'verified'",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;
    if unverified > 0 {
        assessment.deny = Some("repository_not_verified");
        return Ok(assessment);
    }
    // 基线可解析
    let unbounded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM delivery.task_repository tr \
           JOIN integration.project_repository pr \
             ON pr.id = tr.repository_id AND pr.project_id = tr.project_id \
          WHERE tr.task_id = $1 AND coalesce(tr.base_commit, pr.head_commit_sha) IS NULL",
    )
    .bind(task_id)
    .fetch_one(&mut **tx)
    .await?;
    if unbounded > 0 {
        assessment.deny = Some("baseline_unavailable");
        return Ok(assessment);
    }
    Ok(assessment)
}


