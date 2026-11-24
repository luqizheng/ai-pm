# ART-02E Schema 核验（SCHEMA-CHECK）

- 比对基准：`migrations/0020_visualization_artifact.sql`（193 行）
  - 该文件在文档分支 `fca8e279`、master `3a50c7cd`、部署源码 `9a1adb3d` / `a8d00cf7` 处**逐字节一致**（`diff` 验证）。
  - 迁移 0021~0059 全量 grep 无一引用 `viz.*` → **0020 即 viz schema 的完整真源**，无后续增量修订需要纳入比对。
- 核验方式：仅查询 PostgreSQL 系统 catalog（`information_schema.columns`、`pg_constraint`、`pg_indexes`、`pg_trigger`、`pg_proc`、`pg_namespace`），**未读取任何业务行**。
- 执行时间：2026-11-24 06:59–07:01 UTC；数据库身份见 [ART-02E-ENVIRONMENT.md](ART-02E-ENVIRONMENT.md)。

## 1. 总判定

| 核验项 | 实测 | 与 0020 比对 | 判定 |
|---|---|---|---|
| viz schema 存在 | 有，owner=postgres | 0020 `CREATE SCHEMA IF NOT EXISTS viz` | VERIFIED |
| schema 内表集合 | 恰好 4 张：artifact / approval / render_output / slot_config | 与 0020 一致，无多余表 | VERIFIED |
| 列定义（4 表共 57 列） | 名称/顺序/类型/NULL/默认值逐列吻合（artifact 19、approval 13、render_output 12、slot_config 13） | 一致 | VERIFIED |
| 主键（4） | `*_pk` 均在 id uuid | 一致 | VERIFIED |
| CHECK 约束（19） | artifact 8、approval 4、render_output 3、slot_config 4 | 一致（catalog 文本含 PostgreSQL 规范化 `::text` 强转与括号，语义等价） | VERIFIED |
| 外键（2） | approval→artifact、render_output→artifact，均 `convalidated = true` | 一致 | VERIFIED |
| 唯一索引（4） | 含 1 个 partial unique（pending 审批） | 一致 | VERIFIED |
| 普通索引（2） | artifact 两辅助索引 | 一致 | VERIFIED |
| 触发器（2）+ 触发器函数（2） | 守卫触发器均在位、函数可解析 | 一致 | VERIFIED |
| **Schema drift** | — | — | **未发现（INCONSISTENT = 0）** |

## 2. 分表实测（以下清单即库内实时证据原文）

### 2.1 viz.artifact（19 列，与 0020 §① 逐列一致）

约束：

| 约束名 | 类型 | 定义（库内原文） |
|---|---|---|
| viz_artifact_pk | PRIMARY KEY | (id) |
| viz_artifact_slot_key_check | CHECK | length(btrim(slot_key)) > 0 |
| viz_artifact_version_check | CHECK | version_no >= 1 |
| viz_artifact_state_check | CHECK | state ∈ {draft, published, stale, rejected} |
| viz_artifact_nature_check | CHECK | nature ∈ {business, explanatory} |
| viz_artifact_nature_publish_check | CHECK | state <> 'published' OR nature = 'business' |
| viz_artifact_spec_hash_check | CHECK | length(btrim(spec_hash)) > 0 |
| viz_artifact_source_hash_check | CHECK | length(btrim(source_hash)) > 0 |
| viz_artifact_provenance_check | CHECK | jsonb_typeof(provenance) = 'object' |

索引：`viz_artifact_pk`(id)、`uq_viz_artifact_slot_version`(project_id, slot_key, version_no) UNIQUE、`idx_viz_artifact_slot_state`(tenant_id, project_id, slot_key, state, created_at DESC)、`idx_viz_artifact_created`(tenant_id, project_id, created_at DESC)。

触发器：`trg_viz_artifact_guard` BEFORE UPDATE → `viz.guard_artifact_immutability()`（函数存在，VERIFIED）。

### 2.2 viz.approval（13 列，与 0020 §② 逐列一致）

约束：

| 约束名 | 类型 | 定义（库内原文） |
|---|---|---|
| viz_approval_pk | PRIMARY KEY | (id) |
| viz_approval_fk_artifact | FOREIGN KEY | (artifact_id) REFERENCES viz.artifact(id)，convalidated=true |
| viz_approval_status_check | CHECK | status ∈ {pending, approved, rejected, cancelled} |
| viz_approval_target_hash_check | CHECK | length(btrim(target_hash)) > 0 |
| viz_approval_reject_reason_check | CHECK | status <> 'rejected' OR (decision_reason 非空且非空白) |
| viz_approval_decided_check | CHECK | status = 'pending' OR decided_by IS NOT NULL |

索引：`viz_approval_pk`(id)、`uq_viz_approval_pending`(project_id, slot_key) **WHERE status='pending'**（partial unique，与 0020 注释「每槽位至多一条 pending」一致）。

触发器：`trg_viz_approval_guard` BEFORE UPDATE → `viz.guard_approval_decision()`（函数存在，VERIFIED）。

### 2.3 viz.render_output（12 列，与 0020 §③ 逐列一致）

约束：

| 约束名 | 类型 | 定义（库内原文） |
|---|---|---|
| viz_render_output_pk | PRIMARY KEY | (id) |
| viz_render_output_fk_artifact | FOREIGN KEY | (artifact_id) REFERENCES viz.artifact(id)，convalidated=true |
| viz_render_output_format_check | CHECK | format ∈ {html, svg, png} |
| viz_render_output_content_check | CHECK | (html/svg ⇒ content 非空且 content_bytes 为空) OR (png ⇒ content 为空且 content_bytes 非空) |
| viz_render_output_hash_check | CHECK | length(btrim(content_hash)) > 0 |

索引：`viz_render_output_pk`(id)、`uq_viz_render_output_artifact_format`(artifact_id, format) UNIQUE。

### 2.4 viz.slot_config（13 列，与 0020 §④ 逐列一致）

约束：

| 约束名 | 类型 | 定义（库内原文） |
|---|---|---|
| viz_slot_config_pk | PRIMARY KEY | (id) |
| viz_slot_config_slot_key_check | CHECK | length(btrim(slot_key)) > 0 |
| viz_slot_config_nature_check | CHECK | allowed_nature ∈ {business, explanatory, both} |
| viz_slot_config_max_versions_check | CHECK | max_versions >= 1 |
| viz_slot_config_retention_check | CHECK | retention_days >= 0 |

索引：`viz_slot_config_pk`(id)、`uq_viz_slot_config_slot`(project_id, slot_key) UNIQUE。

## 3. 任务指定重点核验项

| # | 核验点 | 结果 | 判定 |
|---|---|---|---|
| 1 | approval 是否关联 artifact | `viz_approval_fk_artifact`：FK (artifact_id) → viz.artifact(id)，已验证 | VERIFIED（存在） |
| 2 | render_output 是否关联 artifact | `viz_render_output_fk_artifact`：FK (artifact_id) → viz.artifact(id)，已验证 | VERIFIED（存在） |
| 3 | tenant/project 与父 artifact 的复合约束 | 两个 FK 均为**单列** artifact_id，无 (artifact_id, tenant_id, project_id) 复合约束 | **NOT FOUND**（与 0020 一致——0020 设计口径 #8「跨聚合无 FK，同聚合有 FK」，tenant/project 引用走应用层校验，属设计决策而非漂移） |
| 4 | artifact / slot_config 是否有 project/tenant 外键 | 两表均无任何外键（仅 PK + CHECK） | **NOT FOUND**（与 0020 一致，同上设计口径） |
| 5 | render_output 的 (artifact_id, format) 唯一约束 | `uq_viz_render_output_artifact_format` 在位 | VERIFIED（存在） |
| 6 | slot_config 的 (project_id, slot_key) 唯一约束 | `uq_viz_slot_config_slot` 在位 | VERIFIED（存在） |

## 4. 结论

1. 库内 viz schema 实时结构与 `migrations/0020_visualization_artifact.sql` **全项一致，未发现任何 schema drift**（INCONSISTENT = 0）。
2. 设计口径提示（非漂移）：跨聚合引用（tenant/project）无 DB 级 FK、approval/render_output 与父 artifact 亦无 tenant/project 复合绑定 —— 与 0020 注释声明的设计一致；独立 Artifact Service 若要求跨域租户一致性硬约束，需在 ART-03+ 设计中显式补强，而非依赖现有结构。
3. 两个守卫触发器与函数在位，发布版不可变（published→stale 终态机）与审批锚点不可变语义由 DB 强制，与 ART-02 文档描述相符。
