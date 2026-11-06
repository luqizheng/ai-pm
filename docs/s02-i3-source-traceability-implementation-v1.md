# S02-I3 · Source & Traceability 实施设计 V1.0

> **切片定位**（S02-04 V1.2 §17.1）：S02-I3「Source & Traceability（来源/发现/决策关联/intake）」，P0，依赖 I1/I2/I6-A（均已交付并通过 S02 发布验收）。
> **触发**：S02 发布验收条件 C2 的 NI-1/B07-04 缺口（requirement_finding 关联未实施）。
> **契约源**：§9（Q-4/R5）、§10（Q-5/Q-15/R6/S02D-08）、§5.4 来源五类（R-13/C-1）、§7.4 职责表、Q-18/Q-20/S02D-15/17/19（均已裁决，可冻结）。

## 1. 范围裁决（本切片 Wave 1，克制原则）

| 项 | 本批 | 依据 |
|---|---|---|
| `delivery.requirement_finding` 表 + 服务 + REST | ✅ | §9.2-2 多对多关联（relation=supports）；C2/NI-1 点名缺口 |
| `source` schema + `source_input` + `requirement_source_input` 表 + 服务 + REST | ✅ | Q-18 选项 A；S02D-19 来源属需求级、修订继承；Q-20 旧列已在 0039 DROP |
| `ai.change_proposal.target_type` 扩 `'requirement'`（Decision 治理关联） | ✅ 零代码（读取面已通用） | 0003:133 无 CHECK；§10.3-1 语义由本稿定义；decisions 读模型已通用 join `change_proposal`（pm_lists.rs:414-437）；提案命令映射 I6-B 已通用 |
| `repository_change_intake` 候选 | ⏸ **延后 I7** | §9.2-4「由 I3 统一裁决」——本切片裁决：Q-I7-01 虽倾向新增，但 I7 消费方不存在，先建空表违反克制原则；I7 开工时随其迁移建表并回填本登记 |
| ADR 关联（CAND-9） | ⏸ 延后 S03 | §10.3-2 明示 |

## 2. 数据模型（迁移 0044；0043 已被 ai_model_policy 线占用）

### 2.1 `source.source_input`（来源材料，输入侧实体）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| id | uuid | PK DEFAULT gen_random_uuid() | |
| tenant_id | uuid | NOT NULL | 租户边界 |
| project_id | uuid | NOT NULL | 来源属项目；复合 FK → core.project(tenant_id, id) |
| input_kind | text | NOT NULL CHECK IN ('human_input','discovery','external_feedback','intake','import') | §5.4 来源五类（R-13）；intake 类随 I7 启用 |
| title | text | NOT NULL | 来源标题 |
| content | text | NULL | 原始材料正文（Markdown） |
| source_ref | text | NULL | 外部引用（URL/commit/工单号） |
| created_by | uuid | NOT NULL | 登记人（真人） |
| created_at | timestamptz | NOT NULL DEFAULT now() | |

索引：`(tenant_id, project_id, created_at DESC)`。

### 2.2 `delivery.requirement_source_input`（来源↔需求，多对多）

`(requirement_id, input_id)` PK；`tenant_id/project_id` 复合 FK 双向（requirement + source_input）；`relation TEXT NOT NULL DEFAULT 'source'`；`created_by/created_at`。**来源属需求级，修订继承**（S02D-19，不挂 revision_id）。

### 2.3 `delivery.requirement_finding`（Finding↔需求，多对多）

`(requirement_id, finding_id)` PK；`relation TEXT NOT NULL DEFAULT 'supports' CHECK (relation='supports')`（§9.2-2 V1 单值域）；复合 FK：requirement（tenant,project,id）+ discovery.discovery_finding（tenant,project,id——0031 补 project_id 后的复合键）；`created_by/created_at`。

### 2.4 不复制原则

Finding 正文/来源正文**不复制**进需求正文（§9.2-2/§10.1）；关联是治理事实，需求投影文档正文不受影响（Markdown 权威协议不感知本切片）。

## 3. 服务层（`pm_application::requirement_traceability`，统一服务风格单点实现）

命令（真人主体 `require_human` + 项目归属校验 + 需求 `lifecycle_status='active'`；治理闸不套用——关联是写入操作非确认类，§7.4 职责表：Human Member/Lead/Approver 皆可，AI-PM 仅建议）：

| 命令 | 语义 | 错误面 |
|---|---|---|
| register_source_input | 登记来源材料（human_input 等） | 校验 input_kind 枚举 |
| link_source_input | 需求关联来源 | 重复关联 → Conflict；需求/来源任一不可见 → NotFound |
| unlink_source_input | 解除来源关联 | 未关联 → NotFound |
| list_requirement_sources | 需求的来源清单 | — |
| link_finding | Finding 支撑需求 | **跨项目 finding → NotFound**（negative_4 同语义：不泄露存在性）；重复关联 → Conflict；仅 CONFIRMED finding 可关联？→ **V1 不限状态**（§9 无此约束，PENDING finding 亦可作支撑记录；verification_status 随 discovery 域独立演化） |
| unlink_finding | 解除支撑关联 | 未关联 → NotFound |
| list_requirement_findings | 需求的支撑 Finding 清单 | — |

审计词表（S02D-23 点号式）：`source.registered` / `requirement.source_linked` / `requirement.source_unlinked` / `requirement.finding_linked` / `requirement.finding_unlinked`；actor=认证主体（audit_actor），AI 主体调用登记/关联命令 → 403（require_human；AI 建议走提案池既有路径）。

## 4. REST 面（pm_requirements.rs 薄适配扩展）

| 端点 | 语义 |
|---|---|
| `POST /v1/pm/projects/{project_id}/source-inputs` | 登记来源（201） |
| `GET /v1/pm/requirements/{rid}/source-inputs` | 需求来源清单 |
| `POST /v1/pm/requirements/{rid}/source-inputs` | 关联 `{input_id}`（201） |
| `DELETE /v1/pm/requirements/{rid}/source-inputs/{input_id}` | 解除（204） |
| `GET /v1/pm/requirements/{rid}/findings` | 支撑 Finding 清单 |
| `POST /v1/pm/requirements/{rid}/findings` | 关联 `{finding_id}`（201） |
| `DELETE /v1/pm/requirements/{rid}/findings/{finding_id}` | 解除（204） |

错误映射沿用 ApiError 惯例（NotFound 不泄露存在性 / Conflict 409 / Forbidden 403）。

## 5. 测试与验收

- 集成测试 `apps/pm-api/tests/pm_traceability.rs`：正常关联链（登记→关联→清单→解除）；非成员 404；跨项目 finding 404；重复关联 409；AI 主体 403；审计断言；lifecycle=archived/withdrawn 需求拒绝关联。
- QA 回填：`test-case/s02-qa-test-cases-v1.md` B07-04（requirement_finding）由 NOT IMPLEMENTED → 实测结果；NI-1 闭合；C2 中 I3 部分解除。
- 完成标准（§17.3）：本稿 §9/§10/§5.4/§7.4 涉及裁决全部落库落码 + 集成测试全绿 + QA B07-04 回填。

## 6. 风险与边界

1. **迁移号**：0044（0043 被并行线占用）；并入迁移登记表。
2. **discovery_finding 复合 FK**：0031 后 finding 表有 UNIQUE(tenant,project,id)，FK 挂复合键保持租户/项目双边界。
3. **intake 枚举占位**：input_kind 含 'intake' 但 I7 未实施——枚举先占位（CHECK 值域预留），无写入路径即无风险。
4. **UI 面**：S02-UI 页面仍为 fixtures（NI-4），本切片不动 UI；前端接入时随 S02-UI 切片暴露关联能力（C2 的 UI 暴露禁令随 NI-4 一并解除评估）。
## 7. 实施结果（2026-11-06）

| 项 | 结果 |
|---|---|
| 迁移 | `0044_s02_i3_source_traceability.sql`（source schema + 3 表 + 复合 FK/索引） |
| 服务层 | `pm_application::requirement_traceability`（7 命令 + 审计 5 词表；`requirement::require_human` pub 化复用） |
| REST 面 | 7 端点（source-inputs ×4 / findings ×3，薄适配） |
| 集成测试 | `apps/pm-api/tests/pm_traceability.rs` **6/6 全绿**（来源全链+Finding 链+跨项目 404+AI 主体 ai_pm 403+非成员 404+archived 拒绝+审计断言） |
| 回归 | S02 核心四套件 **11/11 全绿**（pm_requirement 5/5、projection 3/3、approvers 2/2、tenant_isolation 1/1）——I3 改动零破坏 |
| 实施修正 | 夹具三处编译修正（sqlx::test pool 按值、FromRow derive、AI 主体 kind='ai_pm' 对齐 principal_kind_check 值域与 pm_requirement 既有惯例） |
| QA 回填 | B07-04 NOT IMPLEMENTED → **PASS**；NI-1 闭合；C2 的 I3 部分解除（I4/S03 部分维持） |