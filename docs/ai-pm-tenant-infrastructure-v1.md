# 多租户基础设施 v1（T-021b α + γ/δ）设计依据

> 状态：**🟡 设计稿，待用户过稿**（未产出任何代码与迁移）
> 日期：2026-11-01
> 范围：T-021b 全部——Tenant 实体、39 张租户业务数据表的 `tenant_id`、Agent 三层、Run 身份字段、Tenant Member Role Management API。
> 上位约束：`docs/ai-pm-adr-0001-tenant-agent-executor-identity.md`（Accepted，D1–D6 不得推翻）；`AGENTS.md` 规则 1（新项目、无存量数据，不写回填/兼容脚本）。
> 关联：`docs/ai-pm-principal-identity-v1.md`（β 已完成）、`todo.md` T-021b / T-022。

---

## 1. 已裁定的十一个问题（逐条记录，实施时不得重开）

| # | 问题 | 裁定 | 关键理由 |
|---|---|---|---|
| Q1 | `core.tenant` 的行从哪来 | **镜像上游 `tid`，自动建即生效**（`is_active=true`） | 上游 `tid` 是强制 claim，DB 只做 1:1 镜像；租户边界由上游签发承担。管理权仍由 `tenant_member.role` 门控并可即时撤销，提权面被挡住；不引入「谁激活租户」的引导路径 |
| Q2 | `core.tenant_member` 行与角色怎么建 | **自动建 `member` 行**；env 对 `AI_PM_FIRST_ADMIN_TENANT` / `AI_PM_FIRST_ADMIN_SUBJECT` 同时授予 principal 激活 + `role='owner'`；此后全部走管理 API | D5 要求库里必须有行才能承载与撤销角色；env 对是现成的冷启动钩子 |
| Q3 | `core.principal_identity` 的列形态 | **只留 `tenant_id`，删 `claims_tenant_key`**；UNIQUE 改 `(provider, tenant_id, subject)` | `claims_tenant_key` 的存在理由只是「`core.tenant` 还不存在」；Q1 定 1:1 镜像后它与 `core.tenant.external_key` 是同一事实，保留即双来源（`AGENTS.md` 规则 1 判据） |
| Q4 | 写入时 `tenant_id` 怎么取 | **父行推导为主 + 根表显式** | 37 张表在 `INSERT` 内用子查询取父行 `tenant_id`，零额外往返，组合 FK 兼底；只有真根表由 `AuthedPrincipal` 给 |
| Q5 | 读路径隔离怎么落地 | **分层：对外入口严格 + 内部流转靠蕴含** | 生产 315 条 SQL 中 177 条已按 `project_id` 过滤（组合 FK 已保证同租户），138 条按主键 UPDATE 需从上游上下文取租户 |
| Q6 | 系统/dev/测试的租户从哪来 | **迁移种一个系统租户** `(provider='system', external_key='default')`；`dev-off` 挂为它的 `owner` | 与 0015 种 `ai-pm-dispatch` service principal 同一做法；19 处测试夹具建 project 时用子查询即可，不必改签名 |
| Q7 | 现有 per-project agent principal 怎么处置 | **`dev_agent` 提租户级，`ai_pm` 保持 per-project** | 与 ADR §4 关系图一致（`tenant_agent` 只映射 `kind='dev_agent'`）；`ai_pm` 本就是项目角色 |
| Q8 | 成员管理 API 形态 | **5 个 REST 端点 + 1 个 Owner 转让端点**（用户补充要求，见 §5） | RV-1/RV-2 要求撤权即时生效，必须提供正式写入路径 |
| Q9 | 租户成员变更的审计写哪里 | **新建 `governance.tenant_audit_log`** | `governance.audit_log.project_id` 是 `NOT NULL`（`0003:190`），租户成员操作不属于任何项目；塞占位项目会污染前端 `/audit` 页 |
| Q10 | 端点路径前缀 | **`/v1/pm/tenants/{tenant_id}/...`** | 全库现有业务路由都在 `/v1/pm` 下；裸 `/tenants` 会成为唯一特例 |
| Q11 | Owner 转让形态 | **独立端点，单事务原子交换** | 满足「不允许通过普通 PATCH 随意实现」；V1 不上双人确认（YAGNI） |

> **Q2 的角色值域已被用户补充要求覆盖**：ADR D5 原写 `tenant_admin`/`member` 两级，现改为 **`owner` / `admin` / `member` 三级**。ADR-0001 需同步修订（§8 列出）。

---

## 2. 迁移序列

| 文件 | 内容 | 对应 |
|---|---|---|
| `0019_tenant.sql` | `core.tenant`、`core.tenant_member`、系统租户种子、`governance.tenant_audit_log`、39 张表 `tenant_id` + 组合外键 + 租户内唯一、`principal_identity` 改造 | α |
| `0020_agent_run_identity.sql` | `agent_definition`、`core.tenant_agent`、`delivery.task_run` 三列、`dev_agent` 租户化改造 | γ + δ |

> 0019 不拆分：ADR §9 明说「迁移必须一次覆盖完整清单，中途态（部分表有列）会被 I1 断言判失败」。单文件单事务天然满足。
> 后续任务迁移号不受影响（T-024 仍 `0021`、T-041 仍 `0022`）。

**dev 库处置**：39 张表加 `tenant_id NOT NULL` 无法在有数据行上直接执行。按 `AGENTS.md` 规则 1 —— **dev 库重建**（`docker compose -f docker-compose.dev.yml down -v && up -d`），迁移按 `NOT NULL` 无默认值写，不写 `DEFAULT` + `UPDATE` + `SET NOT NULL` 的三段式（那是数据搬运）。

---

## 3. 逐表迁移方案（39 张）

### 3.1 四类划分

| 类 | 张数 | 特征 | `tenant_id` 来源 |
|---|---|---|---|
| **A 根表** | 2 | 无 `project_id`，是租户数据的起点 | 显式（`AuthedPrincipal.tenant_id`） |
| **B 直挂 project** | 33 | 有 `project_id` 列 | 子查询 `(SELECT tenant_id FROM core.project WHERE id = $project_id)` |
| **C 锚定非 project 父行** | 3 | 无 `project_id`，但通过 UUID 单列 FK 指向某租户表 | 子查询取该父行 `tenant_id` |
| **D 伪根表** | 1 | `delivery.policy_scope`——`scope_key` 是文本不是 UUID 引用 | 显式 |

A(2) + B(33) + C(3) + D(1) = **39** ✓

### 3.2 A 类：根表（2 张）

```sql
-- core.customer（0017 已加 code/parent_id）
ALTER TABLE core.customer
    ADD COLUMN tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    ADD CONSTRAINT uq_customer_tenant_id UNIQUE (tenant_id, id),
    DROP CONSTRAINT customer_parent_id_fkey,
    ADD CONSTRAINT fk_customer_parent
        FOREIGN KEY (tenant_id, parent_id) REFERENCES core.customer (tenant_id, id);
DROP INDEX customer_code_uniq;
CREATE UNIQUE INDEX customer_code_uniq ON core.customer (tenant_id, code)
    WHERE code IS NOT NULL;                                    -- I4-2
CREATE INDEX customer_tenant_idx ON core.customer (tenant_id, created_at DESC);

-- core.project
ALTER TABLE core.project
    ADD COLUMN tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    ADD CONSTRAINT uq_project_tenant_id UNIQUE (tenant_id, id),
    DROP CONSTRAINT project_customer_id_fkey,                   -- 0001:20 单列
    ADD CONSTRAINT fk_project_customer
        FOREIGN KEY (tenant_id, customer_id) REFERENCES core.customer (tenant_id, id),
    DROP CONSTRAINT project_code_key;                           -- I4-1
CREATE UNIQUE INDEX project_code_uniq ON core.project (tenant_id, code);
CREATE INDEX project_tenant_idx ON core.project (tenant_id, created_at DESC);
```

> `core.project` 原有的 `UNIQUE (id, customer_id)`（`project_id_customer_id_key`）**保留**——它是子表组合 FK 的目标，与租户化无关（I2）。

### 3.3 B 类：直挂 project（33 张）

**统一模式**（每张表两步）：

```sql
-- 1) 加列
ALTER TABLE <schema>.<table> ADD COLUMN tenant_id UUID NOT NULL;
-- 2) 既有单列 project FK 换成组合 FK（约束名按现名，先 DROP 再 ADD）
ALTER TABLE <schema>.<table>
    DROP CONSTRAINT <table>_project_id_fkey,
    ADD CONSTRAINT fk_<table>_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
```

**33 张清单**（约束名取自 dev 库实测 `pg_constraint`）：

| # | 表 | 现有 project FK 名 |
|---|---|---|
| 1 | `core.project_member` | `project_member_project_id_fkey` |
| 2 | `delivery.requirement` | `requirement_project_id_fkey` |
| 3 | `delivery.requirement_revision` | `requirement_revision_project_id_fkey` |
| 4 | `delivery.milestone` | `milestone_project_id_fkey` |
| 5 | `delivery.task` | `task_project_id_fkey` |
| 6 | `delivery.task_requirement` | `task_requirement_project_id_task_id_fkey`（**已是复合**，见下注） |
| 7 | `delivery.task_run` | `task_run_project_id_task_id_fkey`（**已是复合**） |
| 8 | `delivery.task_supersession` | `task_supersession_project_id_new_task_id_fkey`（**已是复合**） |
| 9 | `delivery.task_waiting` | `task_waiting_project_id_task_id_fkey`（**已是复合**） |
| 10 | `delivery.task_policy_override` | `task_policy_override_project_id_task_id_fkey`（**已是复合**） |
| 11 | `delivery.task_repository` | 无单列 project FK（现有 `(project_id, task_id)` 与 `(project_id, repository_id)` 两条） |
| 12 | `delivery.task_run_repository` | 无单列 project FK（现有 `(project_id, repository_id)`） |
| 13 | `delivery.task_acceptance_criterion` | 无单列 project FK（现有 `(project_id, task_id)`） |
| 14 | `delivery.task_knowledge_ref` | 无单列 project FK（现有 `(project_id, task_id)`、`(project_id, document_id)`） |
| 15 | `integration.inbound_event` | `inbound_event_project_id_fkey` |
| 16 | `integration.job` | `job_project_id_fkey` |
| 17 | `integration.notification` | `notification_project_id_fkey` |
| 18 | `integration.project_ssh_key` | `project_ssh_key_project_id_fkey`（`ON DELETE CASCADE`） |
| 19 | `integration.project_repository` | `project_repository_project_id_fkey`（`ON DELETE CASCADE`） |
| 20 | `integration.repo_import` | `repo_import_project_id_fkey`（`ON DELETE CASCADE`） |
| 21 | `integration.outbox_event` | `outbox_event_project_id_fkey` |
| 22 | `ai.project_ai_config` | `project_ai_config_project_id_fkey` |
| 23 | `ai.project_skill_binding` | 无（现有 `project_id → ai.project_ai_config(project_id)`，见下注） |
| 24 | `ai.analysis_run` | `analysis_run_project_id_fkey`（另有 `(project_id, event_id)` 复合） |
| 25 | `ai.change_proposal` | 无单列（现有 `(project_id, analysis_run_id)`） |
| 26 | `ai.agent_feedback` | `agent_feedback_project_id_fkey` |
| 27 | `ai.project_document` | `project_document_project_id_fkey` |
| 28 | `ai.document_revision` | 无单列（现有 `(project_id, document_id)`） |
| 29 | `ai.document_proposal` | `document_proposal_project_id_fkey` |
| 30 | `ai.document_index_state` | `document_index_state_project_id_fkey` |
| 31 | `ai.knowledge_reference` | `knowledge_reference_project_id_fkey` |
| 32 | `governance.human_intervention` | `human_intervention_project_id_fkey` |
| 33 | `governance.audit_log` | `audit_log_project_id_fkey` |

**两个注意点**：

- **已是复合 FK 的表**（#6–#10、#11–#14、#25、#28）：现有 `(project_id, 父id) → 父表(project_id, id)` **保持不动**（I2），只需**新增一条** `(tenant_id, project_id) → core.project(tenant_id, id)` 作为 I1 的租户锚。
- **#23 `ai.project_skill_binding`**：现有 `project_id → ai.project_ai_config(project_id)` 是单列指向租户表的 FK。因 `project_id` 全局唯一 UUID，**保留不动**；新增与 #33 同款的租户锚即可。

**统一收尾**（B 类每张表都要）：

```sql
ALTER TABLE <schema>.<table>
    ADD CONSTRAINT fk_<table>_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
```

> 若该表已有同义组合 FK（如 #11），则复用已有约束名，不重复建。

### 3.4 C 类：锚定非 project 父行（3 张）

```sql
-- 1) governance.approval_request（无 project_id，仅 proposal_id）
ALTER TABLE governance.approval_request
    ADD COLUMN tenant_id UUID NOT NULL,
    ADD CONSTRAINT uq_approval_request_tenant_id UNIQUE (tenant_id, id),
    DROP CONSTRAINT approval_request_proposal_id_fkey,
    ADD CONSTRAINT fk_approval_request_tenant_proposal
        FOREIGN KEY (tenant_id, proposal_id) REFERENCES ai.change_proposal (tenant_id, id);
-- 写入：INSERT ... (SELECT tenant_id FROM ai.change_proposal WHERE id = $proposal_id)

-- 2) delivery.task_run_failure（无 project_id，仅 run_id）
ALTER TABLE delivery.task_run_failure
    ADD COLUMN tenant_id UUID NOT NULL,
    DROP CONSTRAINT task_run_failure_run_id_fkey,
    ADD CONSTRAINT fk_task_run_failure_tenant_run
        FOREIGN KEY (tenant_id, run_id) REFERENCES delivery.task_run (tenant_id, id);
-- 写入：INSERT ... (SELECT tenant_id FROM delivery.task_run WHERE id = $run_id)

-- 3) integration.repo_import_source（无 project_id，import_id + repository_id）
ALTER TABLE integration.repo_import_source
    ADD COLUMN tenant_id UUID NOT NULL,
    DROP CONSTRAINT repo_import_source_import_id_fkey,
    ADD CONSTRAINT fk_ris_tenant_import
        FOREIGN KEY (tenant_id, import_id) REFERENCES integration.repo_import (tenant_id, id) ON DELETE CASCADE,
    DROP CONSTRAINT repo_import_source_repository_id_fkey,
    ADD CONSTRAINT fk_ris_tenant_repository
        FOREIGN KEY (tenant_id, repository_id) REFERENCES integration.project_repository (tenant_id, id) ON DELETE CASCADE;
-- 写入：INSERT ... (SELECT tenant_id FROM integration.repo_import WHERE id = $import_id)
```

### 3.5 D 类：伪根表（1 张，`delivery.policy_scope`）

```sql
ALTER TABLE delivery.policy_scope
    ADD COLUMN tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    DROP CONSTRAINT policy_scope_pkey,
    ADD PRIMARY KEY (tenant_id, scope, scope_key);
```

**这是对 ADR §3.5 的修订点**，必须记录：

- ADR I1 原文「每张租户业务表必须有 `tenant_id`（非空）+ **至少一个组合外键**」——`policy_scope` 的 `scope_key` 是文本（`'*'` / project_id 文本 / 任务类型），没有可锚定的 UUID 父键，**只能有单列 `tenant_id` FK**。
- **I1 修订为**：「每张租户业务表必须有 `tenant_id NOT NULL`；**除 `delivery.policy_scope`（伪根表）外**，必须至少一个指向父行的组合外键。」
- 语义顺带升级：`scope='organization'` + `scope_key='*'` 由「平台唯一组织策略」变为「**本租户的组织策略**」——这正是多租户应有的语义，不需要改列。
- **I4 补充第 4 处**：原 `PRIMARY KEY (scope, scope_key)` 必须改成 `(tenant_id, scope, scope_key)`，否则两个租户不能各有一行 organization 策略。

### 3.6 `UNIQUE (tenant_id, id)` 补齐清单（15 张）

凡是被组合 FK 引用的表，都必须有 `UNIQUE (tenant_id, id)`。其中 **5 张今天没有**（ADR §3.5 未提及，属本次新增发现）：

| 表 | 今天有的唯一键 | 需新增 |
|---|---|---|
| `core.customer` | 无 | ⭐ `UNIQUE (tenant_id, id)` |
| `core.project` | `UNIQUE (id, customer_id)` | ⭐ `UNIQUE (tenant_id, id)` |
| `delivery.milestone` | `UNIQUE (project_id, id)` | `UNIQUE (tenant_id, id)` |
| `delivery.requirement` | `UNIQUE (project_id, id)` | `UNIQUE (tenant_id, id)` |
| `delivery.requirement_revision` | `UNIQUE (requirement_id, id)` | ⭐ `UNIQUE (tenant_id, id)` |
| `delivery.task` | `UNIQUE (project_id, id)` | `UNIQUE (tenant_id, id)` |
| `delivery.task_run` | 无（仅 `UNIQUE (task_id, attempt_no)`） | ⭐ `UNIQUE (tenant_id, id)` |
| `integration.inbound_event` | `UNIQUE (project_id, id)` | `UNIQUE (tenant_id, id)` |
| `integration.project_repository` | `UNIQUE (project_id, id)` | `UNIQUE (tenant_id, id)` |
| `integration.repo_import` | `UNIQUE (project_id, id)` | `UNIQUE (tenant_id, id)` |
| `ai.analysis_run` | `UNIQUE (project_id, id)` | `UNIQUE (tenant_id, id)` |
| `ai.project_document` | `UNIQUE (project_id, id)` | `UNIQUE (tenant_id, id)` |
| `ai.document_revision` | `UNIQUE (document_id, id)` | ⭐ `UNIQUE (tenant_id, id)` |
| `ai.change_proposal` | 无（仅 `UNIQUE (analysis_run_id, proposal_seq)`） | ⭐ `UNIQUE (tenant_id, id)` |
| `governance.approval_request` | 无（仅 `UNIQUE (proposal_id)`） | ⭐ `UNIQUE (tenant_id, id)` |

> 「⭐」= ADR §3.5 的示例里**没有**、必须补的表。这是本设计稿相对 ADR 的主要增量。

### 3.7 I4：全局唯一约束改租户内唯一（4 处）

| # | 对象 | 现状 | 改为 |
|---|---|---|---|
| I4-1 | `core.project.code` | `project_code_key UNIQUE (code)`（`0001:21`） | `UNIQUE (tenant_id, code)` |
| I4-2 | `core.customer.code` | `customer_code_uniq` 部分索引 `(code) WHERE code IS NOT NULL`（`0017:18`） | `(tenant_id, code) WHERE code IS NOT NULL` |
| I4-3 | `integration.outbox_event.idempotency_key` | `UNIQUE (idempotency_key)`（`0010:138`） | `UNIQUE (tenant_id, idempotency_key)` |
| I4-4 | `delivery.policy_scope` PK | `PRIMARY KEY (scope, scope_key)` | `PRIMARY KEY (tenant_id, scope, scope_key)` ⭐ADR 未列 |

**已核对无需改**的其余唯一约束（均以 `project_id` / `task_id` / `requirement_id` / `document_id` / 纯 UUID 为前缀，UUID 全局唯一，跨租户不会假冲突）：`requirement(project_id, code)`、`requirement(project_id, id)`、`requirement_revision(requirement_id, version)`、`requirement_revision(requirement_id, id)`、`task(project_id, id)`、`task_run(task_id, attempt_no)`、`task_supersession(old_task_id, new_task_id)`、`task_acceptance_criterion(task_id, ordinal)`、`task_acceptance_criterion(project_id, id)`、`milestone(project_id, id)`、`project_document(project_id, doc_key)`、`project_document(project_id, canonical_path)`、`project_document(project_id, id)`、`document_revision(document_id, id)`、`document_revision(document_id, revision_no)`、`document_index_state(document_revision_id, provider)`、`knowledge_reference` 表达式唯一索引、`change_proposal(analysis_run_id, proposal_seq)`、`approval_request(proposal_id)`、`job(event_id, job_type)`、`notification(source_proposal_id)`、`inbound_event(project_id, id)`、`inbound_event(project_id, source, idempotency_key)`、`project_repository(project_id, id)`、`project_repository(project_id, url)`、`repo_import(project_id, id)`、`project_ai_config(project_id)`、`project_skill_binding(project_id, skill_id)`、`project_member(project_id, principal_id)`、`project_ssh_key(project_id)`、`task_repository(task_id, repository_id)`、`task_run_repository(run_id, repository_id)`、`task_run_failure(run_id)`、`task_policy_override(task_id)`、`task_waiting(id)`、`audit_log(id)`、`human_intervention(id)`、`agent_feedback(id)`、`analysis_run(project_id, id)`。

### 3.8 新表：`core.tenant` / `core.tenant_member` / `governance.tenant_audit_log`

```sql
CREATE TABLE core.tenant (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider     TEXT NOT NULL,              -- V1 只写 'lmms' 或 'system'
    external_key TEXT NOT NULL,              -- lmms claims.tid 字符串化
    display_name TEXT NOT NULL,
    is_active    BOOLEAN NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (provider, external_key)
);

CREATE TABLE core.tenant_member (
    tenant_id    UUID NOT NULL REFERENCES core.tenant(id) ON DELETE CASCADE,
    principal_id UUID NOT NULL REFERENCES core.principal(id),
    role         TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
    granted_by   UUID REFERENCES core.principal(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, principal_id)
);
CREATE INDEX tenant_member_principal_idx ON core.tenant_member (principal_id);
-- 「每租户至少一个 owner」由应用层 + 事务锁定保证（§5.3），不加 DB 触发器

CREATE TABLE governance.tenant_audit_log (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID NOT NULL REFERENCES core.tenant(id),
    actor_id            UUID NOT NULL REFERENCES core.principal(id),
    actor_identity_id   UUID REFERENCES core.principal_identity(id),
    actor_client        TEXT,
    actor_auth_mode     TEXT,
    action              TEXT NOT NULL,       -- 'tenant_member.added' | 'tenant_member.role_changed'
                                             -- | 'tenant_member.removed' | 'tenant_member.owner_transferred'
    target_principal_id UUID REFERENCES core.principal(id),
    before_data         JSONB,               -- {"role": "admin"}；新增时 NULL
    after_data          JSONB,               -- {"role": "member"}；删除时 NULL
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX tenant_audit_log_tenant_idx ON governance.tenant_audit_log (tenant_id, created_at DESC);
```

**系统租户种子**（幂等，与 0015 的 `ai-pm-dispatch` 同做法）：

```sql
INSERT INTO core.tenant (id, provider, external_key, display_name)
VALUES ('00000000-0000-0000-0000-000000000010', 'system', 'default', 'system')
ON CONFLICT (id) DO NOTHING;
```

### 3.9 `core.principal_identity` 改造

```sql
-- Q3：删上游原文列，加内部租户外键。无回填（新项目，dev 库重建）
DROP TABLE core.principal_identity;   -- 表内无业务数据，审计列引用一并重建
```

实际写法（保留表、改列结构，避免审计列悬空）：

```sql
-- 1) 先删引用它的审计三列（0018 加的）
ALTER TABLE governance.audit_log DROP COLUMN actor_identity_id;
-- 2) 重建表
ALTER TABLE core.principal_identity
    DROP CONSTRAINT principal_identity_provider_claims_tenant_key_subject_key,
    DROP COLUMN claims_tenant_key,
    ADD COLUMN tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    ADD CONSTRAINT uq_principal_identity UNIQUE (provider, tenant_id, subject);
-- 3) 审计三列按新 FK 加回
ALTER TABLE governance.audit_log
    ADD COLUMN actor_identity_id UUID REFERENCES core.principal_identity(id),
    ADD COLUMN actor_client      TEXT,
    ADD COLUMN actor_auth_mode   TEXT;
```

> 因 `tenant_id NOT NULL` 在空表上可执行；`actor_identity_id` 先删再加是为了避免「表重建时 FK 指向临时状态」。dev 库重建后本段等价于干净建表。

---

## 4. 应用层改造

### 4.1 auth 链（`apps/pm-api/src/auth.rs`）

| 位置 | 现状 | 改为 |
|---|---|---|
| `AuthenticatedIdentity` | `claims_tenant_key: String` | 保留（**token claims 就是原文**，不落库） |
| `AuthedPrincipal` | `claims_tenant_key` + `is_admin` | `tenant_id: Uuid` + `tenant_role: TenantRole` + `is_tenant_admin: bool` |
| `resolve_or_provision` | 按三元组查 `principal_identity` | 先按 `(provider, claims_tenant_key)` upsert `core.tenant`（Q1 镜像），再按 `(provider, tenant_id, subject)` 查 identity |
| `provision_principal` | 建 principal + identity | 追加：建 `tenant_member(role='member')`；env 对命中时 `role='owner'` + `is_active=true`（Q2） |
| `is_admin` 计算 | `identity.roles` 含 `tenant_admin` | **查库** `tenant_member.role IN ('owner','admin')`（D5、用户补充 §2.3）；token roles 不再参与授权 |
| `inject_dev_principal` | 无租户 | 挂系统租户（Q6）+ `tenant_member(role='owner')` |

**每次请求的库往返**：`core.tenant` upsert（1 次，仅新租户）+ identity 查询（1 次）+ `tenant_member` 角色查询（1 次）。三者可在同一事务内合并为 **2 条 SQL**（identity JOIN member）。

**撤权即时生效**的机制：`is_tenant_admin` 每请求查库 → 降级后下一次请求即为 `false`，不等 token 过期（用户补充 §2.4/§2.5）。

### 4.2 写路径（Q4 父行推导）

**根表显式**（3 处写点）：

```rust
// core.customer / core.project：从调用者取
INSERT INTO core.customer (id, tenant_id, name, ...) VALUES ($1, $authed.tenant_id, ...)
```

**B 类推导**（33 张表的 63 处 `INSERT`）：

```rust
INSERT INTO delivery.task (id, project_id, tenant_id, ...)
VALUES ($1, $2, (SELECT tenant_id FROM core.project WHERE id = $2), ...)
```

> 该子查询与组合 FK 重复校验，但**保留**：它让「漏传 project_id」在 INSERT 阶段就报错（而非延迟到 FK 校验），且 SQL 自解释。

**内部/系统路径**（worker、`ai-pm-dispatch`、bootstrap）：父行推导同样适用，不需要额外上下文。

### 4.3 读路径分层（Q5）

生产代码 315 条 SQL 分三类处置：

| 类 | 条数 | 处置 |
|---|---|---|
| 已按 `project_id` 过滤 | 177 | **不改**——组合 FK 已保证 `project_id` 属于同租户，多一层 `tenant_id` 谓词是冗余 |
| 按主键/`task_id` 等直查，但**调用方已持有已验租户的资源** | 约 100 | 不改，但调用链上游必须是「已按租户校验过的入口」（下表列明入口） |
| **对外入口**（HTTP handler / MCP 工具） | 见下 | **必须加 `tenant_id` 谓词**，跨租户返回 404（不泄露存在性） |

**对外入口清单（必须严格）**：

| 入口 | 类型 | 加谓词的查询 |
|---|---|---|
| `apps/pm-api/src/pm_lists.rs` | 项目/任务/需求列表 | `WHERE tenant_id = $authed.tenant_id` |
| `apps/pm-api/src/customers.rs` | 客户列表/详情/写 | 同上 |
| `apps/pm-api/src/pm_runs.rs` | Run 列表 | `WHERE tenant_id = $authed.tenant_id` |
| `apps/pm-api/src/pm_writes.rs` | 5 个写 handler | 写前校验项目属于本租户 |
| `apps/pm-api/src/documents.rs` / `pm_artifacts.rs` / `pm_repositories.rs` / `pm_imports.rs` | 文档/产物/仓库/导入 | 同上 |
| `apps/pm-api/src/principals.rs` | `/v1/pm/me`、`/principals/{id}` | `/me` 不含租户数据；`PATCH` 限本租户成员 |
| **新增** `apps/pm-api/src/tenant_members.rs` | 成员管理 6 端点 | 见 §5 |
| `crates/pm-mcp/src/tools.rs`（11 工具） | MCP 工具 | `pm_get_task_context` 等按 `project_id + tenant_id` 校验 |

> MCP 工具的租户从「`project_id` → `core.project.tenant_id`」查得，不接受客户端传入（`AuthedPrincipal` 不在此链上）。

### 4.4 bootstrap 的 `dev_agent` 租户化（Q7）

| 现状（`bootstrap.rs:172-189`） | 改为 |
|---|---|
| 每建项目建 **3** 个 `dev_agent` principal（architect/developer/tester）+ **1** 个 `ai_pm` + **1** 个 owner human | 租户下**首次**建项目时：建 3 个租户级 `dev_agent` principal + 3 行 `core.tenant_agent`；后续项目只写 `project_member` 授权行 |
| `ai_pm` per-project | **不变**（Q7） |
| 5 行 `project_member` | 仍 5 行（外键指向租户级或项目级 principal） |

**受影响的既有断言**：`bootstrap` 相关测试里「每个项目 5 个 principal」的断言要改成「每租户 3 个 dev_agent + 每项目 1 个 ai_pm + 1 个 human」。这是预期变更，不是回归。

---

## 5. Tenant Member Role Management API

### 5.1 端点契约

全部在 `/v1/pm` 下（Q10），`{tenant_id}` 必须等于调用者的 `AuthedPrincipal.tenant_id`（否则 404，不泄露存在性）。

| 方法 | 路径 | 权限 | 语义 |
|---|---|---|---|
| `GET` | `/v1/pm/tenants/{tenant_id}/members` | `owner` / `admin` | 列表（含 `principal_id`、`display_name`、`role`、`is_active`、`created_at`） |
| `GET` | `/v1/pm/tenants/{tenant_id}/members/{principal_id}` | `owner` / `admin` | 单条详情 |
| `POST` | `/v1/pm/tenants/{tenant_id}/members` | `owner` / `admin` | body `{principal_id, role}`；role 只能 `member`（见 5.2） |
| `PATCH` | `/v1/pm/tenants/{tenant_id}/members/{principal_id}/role` | `owner` / `admin` | body `{role}`；普通角色变更 |
| `DELETE` | `/v1/pm/tenants/{tenant_id}/members/{principal_id}` | `owner` / `admin` | 移除成员 |
| `POST` | `/v1/pm/tenants/{tenant_id}/owner-transfer` | **仅 `owner`** | body `{new_owner_principal_id}`；单事务原子交换（Q11） |

**`{principal_id}` = `core.principal.id`**（本系统无独立 user 表）。用户补充要求里的 `{user_id}` 即此值。

### 5.2 角色规则（用户补充 §3）

| 规则 | 实现 |
|---|---|
| 值域 | `owner` / `admin` / `member`，DB CHECK 约束 + Rust 枚举 `TenantRole` |
| 与项目角色分离 | `tenant_member.role` **不出现** `developer` / `tester` / `reviewer` / `ai_pm` / `owner`(项目级)；项目角色仍只在 `project_member.role` |
| 最后一个 Owner 保护 | 降级/删除会使其成为 0 个 owner → `409 OWNER_LAST` |
| Owner 转让独立 | `PATCH` 不接受 `role='owner'`（返回 `409 USE_OWNER_TRANSFER`）；只有 `owner-transfer` 端点能产生新 owner |
| 不得越权提升 | `admin` 只能设 `member`；`owner` 可设 `admin`（含降级他人 admin → member）。两者都不能通过 `POST`/`PATCH` 产生第二个 `owner` |
| 不能自我提权 | `admin` 对自己 `PATCH` 到 `admin` 之外的值一律 403；`owner` 对自己只能通过 `owner-transfer` 降为 `admin` |

**能力矩阵**（`✓` 允许，`✗` 403，`→owner-transfer` 走独立端点）：

| 操作者 \ 目标值 | `member` | `admin` | `owner` | 移除 |
|---|---|---|---|---|
| `owner` | ✓ | ✓ | →owner-transfer | ✓（非最后 owner） |
| `admin` | ✓ | ✗（含自己） | ✗ | ✓（仅 `member` 目标） |
| `member` | ✗ | ✗ | ✗ | ✗ |

### 5.3 事务模板（用户补充 §4）

所有写操作走同一个函数 `apply_member_change(tx, op)`，**单事务**：

```rust
// 1) 序列化该租户的全部成员变更（Q：并发安全的关键）
let _ = sqlx::query("SELECT id FROM core.tenant WHERE id = $1 FOR UPDATE")
    .bind(tenant_id).fetch_one(&mut *tx).await?;

// 2) 校验调用者当前权限（读库，不读 token）
let actor_role: Option<String> = sqlx::query_scalar(
    "SELECT role FROM core.tenant_member WHERE tenant_id = $1 AND principal_id = $2")
    .bind(tenant_id).bind(actor_principal_id).fetch_optional(&mut *tx).await?;

// 3) 锁定目标成员行（若存在）
let target: Option<(String,)> = sqlx::query_as(
    "SELECT role FROM core.tenant_member \
      WHERE tenant_id = $1 AND principal_id = $2 FOR UPDATE")
    .bind(tenant_id).bind(target_principal_id).fetch_optional(&mut *tx).await?;

// 4) 校验角色转换合法性（§5.2 能力矩阵）+ 最后 owner 保护
let owner_count: i64 = sqlx::query_scalar(
    "SELECT count(*) FROM core.tenant_member WHERE tenant_id = $1 AND role = 'owner'")
    .bind(tenant_id).fetch_one(&mut *tx).await?;

// 5) 更新 tenant_member
// 6) 写 governance.tenant_audit_log（同一 tx，失败即全部回滚）
tx.commit().await?;
```

**为什么锁 `core.tenant` 行而不是锁 owner 行**：锁 owner 行时，两个事务可能各自锁不同的 owner 行，在 `READ COMMITTED` 下都能读到「owner 数 = 2」然后各降一个 → 剩 0。锁 `core.tenant` 行把同一租户的**所有**成员变更串行化，彻底消除该竞争。成员变更是低频操作，串行化代价可接受。

**审计与业务同事务**（用户补充 §4 末条、测试 8）：`tenant_audit_log` 的 INSERT 在同一个 `tx` 内；任何一步失败（含审计 INSERT）→ 整体回滚，不会出现「角色改了但没审计」或「审计了但角色没改」。

### 5.4 审计载荷

`action` 取值（操作标识）：`tenant_member.added`、`tenant_member.role_changed`、`tenant_member.removed`、`tenant_member.owner_transferred`。

| 字段 | 内容 |
|---|---|
| `tenant_id` | 目标租户 |
| `actor_id` | 操作者 principal |
| `actor_identity_id` / `actor_client` / `actor_auth_mode` | 从 `AuthedPrincipal::audit_actor()` 取，与 0018 三列同口径 |
| `target_principal_id` | 被操作用户 |
| `before_data` | `{"role":"admin"}`；新增时 `NULL` |
| `after_data` | `{"role":"member"}`；删除时 `NULL` |
| `created_at` | 时间 |

`owner-transferred` 的 payload 额外带 `{"from_role":"owner","to_role":"admin"}`（旧 owner）与目标行的 `{"from_role":?, "to_role":"owner"}`，写**两条**审计行（一条旧 owner 降级、一条新 owner 晋升），共享同一 `action`。

---

## 6. 验收矩阵

### 6.1 ADR-0001 §10 已定的（不得弱化）

| 编号 | 场景 | 断言 |
|---|---|---|
| XT-1 | 租户 A 的 token 读租户 B 的 project / task / run | 404，审计不留成功行 |
| XT-2 | 租户 A 的 token 写租户 B 的资源 | 404；库内无新增行 |
| XT-3 | `information_schema` 断言 | 39 张表全部有 `tenant_id NOT NULL`；除 `policy_scope` 外至少一个 `(tenant_id, *)` 组合外键 |
| XT-4 | 两租户各建同 `code` 的 project / customer | 两者都成功（I4-1/I4-2） |
| FK-1 | 插入 `tenant_id=B` 而 `project_id` 属于 A | 违反外键被拒（DB 层） |
| FK-2 | `core.project` 的 `customer_id` 跨租户 | 违反外键被拒 |
| AG-1 | MCP 写路径 | `audit_log.actor_id` = `dev_agent`；`actor_identity_id` = 真人；`task_run.agent_id` 与 `actor_id` 同一 id |
| AG-2 | 未启用的 `tenant_agent` 调 `pm_claim_task` | 403；无 run 产生 |
| EX-1 | 写 `executor_id = NULL` 与非 NULL | 均可落库（无 FK） |

### 6.2 成员角色管理（用户补充 §5 九条 + ADR RV-1/RV-2，重新编号）

| 编号 | 场景 | 断言 | 来源 |
|---|---|---|---|
| RV-1 | `owner` 将 `admin` 降为 `member` | 200；`tenant_member.role='member'`；审计一行 `role_changed`，before/after 正确 | 用户 1 |
| RV-2 | 被降级者**继续用旧 JWT** 调 `GET .../members` | **立即 403**（≤1 个请求周期），不等 token 过期 | 用户 2 / ADR RV-1 |
| RV-3 | 成员被 `DELETE` 后继续用旧 JWT 调管理员 API | **立即 403** | 用户 3 |
| RV-4 | `member` 调任一成员管理端点 | 403 | 用户 4 |
| RV-5 | `admin` 试图把任何人为 `owner` | 409 `USE_OWNER_TRANSFER`；库内无变化 | 用户 5 |
| RV-6 | 降级或删除**最后一个** `owner` | 409 `OWNER_LAST`；库内无变化 | 用户 6 |
| RV-7 | 跨租户成员操作（token 租户 A，路径 `{tenant_id}` = B） | 404；库内无变化 | 用户 7 |
| RV-8 | 审计 INSERT 失败（用 `raw_sql` 注入非法 `action` 或临时约束） | 角色变更一并回滚；`tenant_member` 保持原值 | 用户 8 |
| RV-9 | **并发**两个请求同时降级同一租户的两个不同 `owner` | 一个成功、一个 409 `OWNER_LAST`；最终 owner 数 ≥ 1（`tokio::join!`，`FOR UPDATE` 串行化验证） | 用户 9 |
| RV-10 | token `roles` 含 `tenant_admin` 但库内无 `tenant_member` 行 | 403（D5：token 不作为事实源） | ADR RV-2 |

### 6.3 新增（本设计稿要求）

| 编号 | 场景 | 断言 |
|---|---|---|
| TN-1 | 首次见未知 `tid` 的 token | 自动建 `core.tenant`（`is_active=true`）+ `principal_identity(tenant_id)` + `tenant_member(role='member')`；principal 默认 `is_active=false` |
| TN-2 | env 对命中（`AI_PM_FIRST_ADMIN_TENANT` + `SUBJECT`） | 首次 provision 即 `is_active=true` + `role='owner'` |
| TN-3 | 同一 `tid` 第二次请求 | 不重复建 tenant（`UNIQUE(provider, external_key)`）；`last_seen_at` 节流生效 |
| TN-4 | `owner-transfer` | 单事务：旧 owner→`admin`、目标→`owner`；两条审计行；中途失败全回滚 |
| TN-5 | `owner-transfer` 用非 owner 调用 | 403；`admin` 即使角色够也拒绝 |
| TN-6 | `dev-off` 模式写请求 | principal 与 `tenant_member(role='owner')` 一起物化到系统租户；GET 仍返回零总数 |
| TN-7 | `policy_scope` 两租户各建 organization 策略 | 两行共存（I4-4） |
| TN-8 | `outbox_event` 两租户同 `idempotency_key` | 两行共存（I4-3） |

---

## 7. 实施顺序

| 步 | 内容 | 验证 |
|---|---|---|
| S0 | dev 库重建（`down -v && up -d`）；确认全部 18 个迁移可重放 | `cargo test --workspace` 现状基线（401 passed） |
| S1 | 迁移 `0019_tenant.sql`（§3 全部） | `cargo check --workspace` 编译失败数量 = 切入点清单 |
| S2 | 补 `UNIQUE(tenant_id, id)` 与 4 处 I4 | TN-7/8、XT-4 |
| S3 | auth 链改造（§4.1）+ `tenant_member` 角色读库 | RV-2/3/10 的机制前提 |
| S4 | 写路径父行推导（63 处）+ 根表显式（3 处） | FK-1/2、XT-2 |
| S5 | 读路径对外入口加谓词（§4.3 清单） | XT-1、XT-2 |
| S6 | `tenant_members.rs` 6 端点 + `apply_member_change` 事务 | RV-1..RV-9、TN-4/5 |
| S7 | 迁移 `0020_agent_run_identity.sql` + bootstrap 租户化（§4.4） | AG-1/2、EX-1 |
| S8 | 全量回归 | `cargo test --workspace` + `cargo check --all-targets` |

**每步单独可编译**；S1 之后会有大量编译错误，这是预期（契约变更的直接结果），不写兼容分支。

---

## 8. 对 ADR-0001 的修订（需同步提交）

本设计稿产出后，ADR-0001 有 5 处必须改：

| # | ADR 位置 | 现状 | 改为 |
|---|---|---|---|
| 1 | D5 与 §3.3 | `role IN ('tenant_admin','member')` | `role IN ('owner','admin','member')`（用户补充 §3） |
| 2 | §3.5 I1 | 「必须至少一个组合外键」 | 「除 `delivery.policy_scope`（伪根表）外，必须至少一个组合外键」（§3.5） |
| 3 | §3.5 I4 | 列了 3 处 | 补第 4 处 `delivery.policy_scope` PRIMARY KEY（§3.7） |
| 4 | §3.5 | 示例未覆盖「作组合 FK 目标需 `UNIQUE(tenant_id, id)`」 | 补 §3.6 的 15 张清单（5 张为新增发现） |
| 5 | §7.1 | 「`principal_identity.tenant_id` … 由 `(provider, claims_tenant_key)` 解析得到」+ 保留 `claims_tenant_key` | Q3 已裁：**删 `claims_tenant_key`**，只留 `tenant_id` |

---

## 9. 遗留与不做的

| 项 | 处置 |
|---|---|
| `tenant_member` 的「至少一个 owner」DB 触发器 | 不做（应用层 + 事务锁定已足够；DB 触发器会让测试难写且逻辑藏起来） |
| Owner 转让双人确认 | V1 不做（Q11）；出现误操作证据再上 |
| `tenant` 的 `is_active=false` 路径 | 表里有列但 Q1 定「自动建即生效」，即 V1 恒为 `true`；禁用租户是运维动作，不在本期 |
| 费用核算 / 配额聚合（ADR R4） | 不做；`task_run.tenant_id` 作为载体在 0019 落地 |
| Executor Registry（ADR D6） | 不做（T-030 / D3）；`executor_id` 无 FK |
| Git 凭据层级（ADR R3） | 不动（项目级 `project_ssh_key`） |
| `tenant_admin` 跨项目可见性（ADR R6） | 与 T-022 一起裁；默认**不可见** |
| RBAC 细粒度权限码 | 不做（V1 只有三个租户角色） |
| MCP 侧审计三列（T-059） | 仍受 rmcp 3.8 transport 限制，不在本期 |
