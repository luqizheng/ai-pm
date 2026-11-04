# I2 · Finding 人工确认机制设计稿 V1.0（最终定稿）

> 文档性质：实施前设计基线  
> 设计范围：Finding 人工确认、拒绝、要求补充、并发与审计协议  
> 状态：**最终设计定稿，待拆分实施任务**  
> 上位原则：人类负责项目，AI Agent 协助执行  
> 上位约束：I1 项目归档生命周期（归档后禁止项目业务写操作）  
> 日期：2026-11-03

> **编者按**：本稿 §1 已按本轮代码核验补全事实，并修正了原稿中的两处不准确表述——(1) 表名应为 `discovery.discovery_finding`（schema 为 `discovery`，非 `governance`）；(2) 「不得假设存在 pending/confirmed/rejected」一句已被核验推翻——真实枚举**已经存在**，且比设计假设更完整（`verification_status` 含 `CONFLICT`，`kind` 含 `user_confirmed`）。因此 §2.4 改为「按真实枚举做语义映射」而不是「待核验」。另新增 §1.4：现有确认路径（`POST .../findings/{id}/verification` → `record_finding_verification` → `set_finding_verification`）**已经存在且已被前端使用**，它当前无行锁、无事务、无审计、无状态前置校验——本设计不是新建能力，而是**收编并加固**这条既有路径。这直接改变了 I2-B 的实施性质（见 §11）。

---

## 0. 设计目标与范围

### 0.1 背景

Discovery 流程会从 Repository 分析、代码扫描和 AI Agent 工作中产生 Finding。

Finding 可能涉及：

- 架构现状；
- 技术债务；
- 代码质量问题；
- 安全问题；
- 项目约束；
- 依赖关系；
- 实施风险；
- 需要项目负责人决策的事项。

AI 发现的内容不等于已经确认的项目事实。

如果 Agent 可以直接把 Finding 标记为已确认，就会出现以下问题：

1. AI 自己发现问题，又自己确认问题。
2. 未经人工核验的信息进入项目可信知识。
3. 错误 Finding 可能影响后续 Task 规划。
4. 无法准确追溯是谁确认了项目事实。
5. 多个 Agent 并行工作时，可能覆盖人工已经作出的决定。

因此，I2 建立 Finding 人工确认机制。

### 0.2 设计目标

必须满足：

1. AI Agent 可以创建 Finding。
2. AI Agent 可以补充 Finding 的分析材料。
3. AI Agent 不得直接确认 Finding。
4. 人类必须通过服务端授权的操作确认 Finding。
5. 确认操作必须校验 Finding 当前状态。
6. 确认操作必须具备并发安全性。
7. Finding 的业务字段更新必须与审计记录在同一事务提交。
8. 确认人和确认时间以审计日志为权威来源。
9. 确认结果可以被项目其他业务流程可靠读取。
10. 历史 Finding 的确认、拒绝和后续变更均可追溯。

### 0.3 不在本设计范围内

- Discovery 算法本身。
- AI Finding 质量评分算法。
- Finding 自动合并算法。
- Project Goal 的完整生命周期。
- Knowledge 文档发布流程。
- Finding 的全文搜索和向量检索。
- Finding 的人工复核工作台视觉设计。

### 0.4 核心定义

**确认**是**唯一**把 Finding 纳入「项目已确认事实集合」的动作。

未确认的 Finding 无论置信度多高、来源多可靠，都不是项目事实。

---

## 1. 现有代码事实与设计依据

本节区分**已核验事实**与**新增设计**。事实部分均在本仓（ai-pm）代码中复核，位置见附录 A。

### 1.1 Finding 数据结构

**已核验事实：**

`discovery.discovery_finding` 实际列为：

`id, tenant_id, session_id, kind, content, confidence, verification_status, evidence, actor_role, created_at`

表中**不存在**：

- `version`；
- `revision`；
- `decided_by`；
- `decided_at`；
- `updated_at`；
- `project_id`（项目经 `discovery_session.project_id` 间接关联）。

因此不能把乐观锁版本号、确认人和确认时间描述为现有字段。

同时，真实枚举**已经存在且带 CHECK 约束**：

| 字段 | 真实取值（CHECK 约束） | 常量位置 |
|---|---|---|
| `kind` | `observed` / `user_confirmed` / `speculation` / `contradicted` | `crates/pm-application/src/discovery/mod.rs:74` `FINDING_KINDS` |
| `verification_status` | `PENDING` / `CONFIRMED` / `REJECTED` / `CONFLICT` | `crates/pm-application/src/discovery/mod.rs:76` `VERIFICATION_STATUSES` |

> **对原稿的修正**：原稿 §2.4 写「不得直接假设当前已经存在 pending/confirmed/rejected 等完整状态集合」。核验结论是——**状态集合确实存在**，大小写为大写，且比设计假设多一个 `CONFLICT`。因此 I2 无需新造状态枚举，改为按真实枚举做语义映射（见 §2.4）。

**设计裁决：**

1. I2 不依赖新增 `version` 字段实现并发控制。
2. 确认人和确认时间通过 `governance.audit_log` 追溯。
3. 不新增 `decided_by` / `decided_at` 字段。
4. 不新增状态枚举值，不改写既有枚举含义。
5. 如果未来需要高频编辑或版本历史，另行设计 Finding revision 机制，不在 I2 中顺手引入。

### 1.2 审计基础设施

**已核验事实：**

`governance.audit_log` 实际列含：`id, tenant_id, project_id, actor_id, action, target_type, target_id, before_data, after_data, compensation_command, compensated_command_id, source_event_id, analysis_run_id, created_at`。

其中 `tenant_id` 由迁移 `0019_tenant.sql` 追加，`actor_id` 引用 `core.principal(id)`。

**设计裁决：**

1. I2 复用该设施，不新建平行的 Finding 确认历史表。
2. `before_data` / `after_data` 直接承载决定前后的 `kind` 与 `verification_status`（JSONB），满足 F3 的双字段留痕要求。
3. `project_id` 为 `NOT NULL`：写审计前必须先由 `discovery_session` 解析出 `project_id`，不能留空。

### 1.3 Finding 字段更新

**已核验事实：**

`kind` 与 `verification_status` 是两个独立字段。

现有代码中「已通过其他方式解决」路径的做法是：**新建第二条 Finding**（`kind='user_confirmed'`，`verification_status` 落库为默认 `PENDING`），而不是在原 Finding 上原地改 `kind`。

**F3 要求**：确认操作同时改变 `kind` 与 `verification_status`，必须原地**同一条 UPDATE** 原子完成。

**设计裁决：**

不得先更新 `kind`，再在另一个事务中更新 `verification_status`。

不得用「新建第二条 Finding」替代「原地原子更新」——新建行会丢失与原发现的血缘关系，且新行本身处于 `PENDING`，无法表达「已确认」。

### 1.4 现有确认路径与缺口（本轮核验新增）

**已核验事实：**

确认能力**已经存在**并且前端已在使用，不是空白：

```
POST /v1/pm/projects/{project_id}/discovery/sessions/{session_id}/findings/{finding_id}/verification
  → apps/pm-api/src/pm_discovery.rs:291  verification_handler
  → crates/pm-application/src/discovery/web.rs:80  record_finding_verification
  → crates/pm-application/src/discovery/finding.rs:195  set_finding_verification
```

当前实现的缺口（逐条对照本设计要求）：

| 缺口 | 事实 | 违反的设计条款 |
|---|---|---|
| G1 无行锁 | `set_finding_verification` 直接 `UPDATE ... WHERE tenant_id AND id`，无 `FOR UPDATE`，且不在事务内 | §4.2 §4.3 |
| G2 无状态前置校验 | `status` 来自客户端 body，只做枚举合法性校验，**不检查当前状态**，可把 `CONFIRMED` 直接改成 `REJECTED` | §5.4、I2-008、I2-009 |
| G3 无审计 | 全链路无 `governance.audit_log` 写入，确认人/时间无处可查 | §7、I2-012、I2-017、I2-018 |
| G4 非单事务 | `set_finding_verification`、`add_question`、`answer_question`、`add_finding` 各自独立提交，中途失败留下半成品状态 | §4.2 §4.5 |
| G5 无归档门禁 | 不检查 `core.project.status`，归档项目仍可确认 | §3.4、I2-015 |
| G6 无角色区分 | 任何项目成员均可确认，无「Finding 确认」权限粒度 | §3.1 §3.3 |
| G7 无 F3 原子更新 | `kind` 与 `verification_status` 通过「新建一行」实现，不是同一条 UPDATE | §4.4、I2-010 |
| G8 存在裸确认原语 | `set_finding_verification` 是 `pub` 并在 `pm-application::lib.rs:139` 公开导出，内部调用方可无审计、无锁、无状态校验地直接确认 | §2.2 §3.2、I2-003 |

**设计裁决：**

1. I2 是**收编并加固既有路径**，不是新建一条平行路径。
2. 既有 HTTP 路由保持不变（前端不必改造），但其实现必须改为调用 §4.1 的统一决定服务。
3. `set_finding_verification` 必须下沉为**私有**实现细节，或改为必须传入事务句柄且强制带状态前置条件；`pub` 裸导出必须收口，否则 G8 长期存在。
4. 上表 G1–G8 全部进入 §10 验收矩阵。

### 1.5 现有权限与主体事实

**已核验事实：**

- REST 侧项目可见性由 `apps/pm-api/src/support.rs:126` `project_visible` 判定：`core.project` 与 `core.project_member` 联结，要求同租户且**调用人是项目成员**。
- `apps/pm-api/src/pm_discovery.rs:104` 的 `project_in_tenant` 复用该判定，跨租户与非成员统一按 `ProjectNotFound`（404）处理，不泄露存在性。
- 令牌内已有权限码通道：`apps/pm-api/src/auth.rs:93` `permissions: Vec<String>`，`has_permission(code)` 按精确字符串匹配；MCP 侧已有先例——`PM_MCP_READ` 为读工具基线，`MCP_TOOL_PERMISSIONS` 为写工具逐工具映射（`auth.rs:559`）。
- MCP 工具 `pm_discovery_submit_findings` 的入参 `DiscoveryFindingInput`（`crates/pm-mcp/src/tools.rs:868`）**不含** `verification_status`，因此当前 Agent 无法经 MCP 直接写确认态。
- MCP 工具 `pm_discovery_submit_answers`（`tools.rs:1137`）会写 `discovery_answer`，但 `actor_id` 是项目的 `ai_pm` 执行主体——这是**Agent 写下的答复**，不得被视为人工确认。

**设计裁决：**

1. 复用 `core.project_member` 成员边界 + 令牌权限码两条既有通道，不新建用户/角色体系。
2. Finding 确认应引入独立权限码（命名沿用 `PM_*` 前缀，与 `PM_MCP_*` 同族），具体码值在 I2-B 中与权限模型文档对齐。
3. Agent 主体（`ai_pm` 执行主体）在决定服务入口处**必须**被拒绝，不得因具备项目成员身份而放行。

### 1.6 锁顺序与项目解析

**已核验事实：**

`discovery_finding` 无 `project_id`，项目路径为：

`discovery.discovery_finding.session_id → discovery.discovery_session.project_id → core.project`

**设计裁决：**

1. 确认事务必须先解析 `session → project_id`，再按 I1 锁顺序加锁。
2. 锁顺序：`core.project → discovery_finding`（与 I1 一致），中间不得出现反向取锁。

---

## 2. 核心领域语义

### 2.1 Finding 的性质

Finding 是 AI 或其他 Discovery 来源提出的**待核验信息**。

它不是天然可信的项目事实。

只有经过规定的确认流程，才可以被下游业务作为已确认信息使用。

### 2.2 两类主体

| 主体 | 允许操作 |
|---|---|
| AI Agent | 创建 Finding、补充分析、提交证据、提出建议 |
| 有权限的人类成员 | 查看、确认、拒绝、要求补充、执行允许的人工修正 |

AI Agent 不得通过 MCP、内部服务调用、数据库写入封装或其他间接路径绕过人工确认。

### 2.3 确认与拒绝

确认和拒绝是不同的业务决定。

- **确认**：人类认可 Finding 的内容及其分类、核验结论。
- **拒绝**：人类认为该 Finding 不成立、不适用或不应纳入项目事实。
- **要求补充**：当前证据不足，要求 Agent 或其他人员补充材料。

要求补充不等于确认，也不等于拒绝。

### 2.4 状态语义映射（按真实枚举）

真实枚举已存在（§1.1），I2 按下表映射，**不新增状态值**：

| 业务语义 | `verification_status` | 伴随 `kind` 处理 | 说明 |
|---|---|---|---|
| 待确认 | `PENDING` | 保持原值 | 初始态，默认值 |
| 已确认 | `CONFIRMED` | 见 §4.4 F3 | 进入已确认事实集合 |
| 已拒绝 | `REJECTED` | 保持原值 | 保留历史，不删除行 |
| 要求补充 | `PENDING` | 保持原值 | **状态不变**，用补充请求承载（见 §5.3） |
| 事实冲突 | `CONFLICT` | 保持原值 | 沿用既有语义：与其他来源矛盾/失效，本设计不改写 |

**要求补充不新增状态**的理由：`discovery_question.status` 已有 `open/answered/deferred` 三值，配合 `discovery_answer` 已足以表达「已提出补充要求 / 已答复 / 暂缓」。

**映射裁决：**

1. 「要求补充」= `verification_status` 保持 `PENDING` + 生成/复用一条 `open` 状态的 `discovery_question`（带 `finding_id` 锚定）+ 记录补充要求正文。
2. `CONFLICT` 保留给「事实冲突」语义，不用于表达「要求补充」，也不被 I2 的人工决定写入。
3. 实施前仍须核验 `discovery_question.status` 与 `deferred` 的全部消费者，确认复用不会产生语义歧义（I2-A）。

---

## 3. 人工确认权限

### 3.1 权限原则

确认 Finding 必须满足：

1. 调用人是已认证的**真人**（不是 Agent 执行主体）。
2. 调用人属于 Finding 所属 Project 对应的 Tenant。
3. 调用人是该 Project 的成员（现有 `project_visible` 判定）。
4. 调用人具备该 Project 的 Finding 确认权限（权限码）。
5. Project 未归档。
6. Finding 当前状态允许执行该操作。

以上条件必须由服务端检查。

### 3.2 Agent 权限

Agent 可以：

- 创建 Finding；
- 更新允许由 Agent 更新的分析材料；
- 附加证据；
- 提交确认建议。

Agent 不可以：

- 确认 Finding；
- 拒绝 Finding；
- 代替人类修改确认结果；
- 伪造确认人；
- 伪造确认时间；
- 通过内部服务绕过人工授权；
- 通过 `pm_discovery_submit_answers` 写入的答复被计为人工确认（现有实现中该答复的 `actor_id` 是 `ai_pm` 执行主体）。

### 3.3 权限复用

复用现有 Tenant 成员授权体系（`core.project_member` + 令牌 `permissions` 权限码，见 §1.5）。

不在 I2 中另建一套独立用户、角色或权限系统。

具体角色到操作权限的映射，应以现有项目授权模型为准（见 `docs/ui/smark-pm-menu-roles-permission-model-v1.md`）。

如果现有权限模型没有 Finding 确认能力，应在 I2 实施范围中补充明确的权限定义及授权入口。

### 3.4 归档项目限制

I1 归档后，项目业务写操作被禁止。因此：

- archived Project 不允许确认 Finding；
- archived Project 不允许拒绝 Finding；
- archived Project 不允许通过 Agent 补充业务 Finding；
- archived Project 不允许要求补充；
- 历史 Finding 仍允许读取。

---

## 4. 确认操作协议

### 4.1 服务端统一入口

所有 Finding 人工决定必须进入统一 Application Service。

逻辑入口建议：

`decide_finding`

操作参数至少包括：

- finding_id；
- decision；
- decision_reason（可选）；
- correction（仅在允许人工修正时使用）。

调用人身份必须从可信服务端上下文取得，不能由客户端传入一个任意 `user_id` 作为确认人。

既有 HTTP 端点 `POST .../findings/{finding_id}/verification`（§1.4）必须改为调用本入口，其路由与请求体形状保持兼容。

### 4.2 确认事务

一次确认操作必须在一个数据库事务中完成：

1. 解析可信调用人身份。
2. 解析 Finding → Session → Project。
3. 检查调用人身份为真人（非 Agent 执行主体）。
4. 检查调用人权限（成员边界 + 权限码）。
5. 锁定 Project（`SELECT ... FROM core.project ... FOR UPDATE`）。
6. 检查 Project 未归档。
7. 锁定 Finding（`SELECT ... FROM discovery.discovery_finding ... FOR UPDATE`）。
8. 检查 Finding 当前状态。
9. 检查本次决定是否允许。
10. 执行 Finding 字段更新。
11. 写入 `governance.audit_log`。
12. 提交事务。

Project 锁顺序遵循 I1：

`core.project → discovery_finding`

如果实际 Finding 表与 Project 之间存在其他需要锁定的关联记录，应遵循统一锁顺序，不能反向申请 Project 锁。

### 4.3 为什么需要锁 Finding

当前 Finding 没有 version/revision 字段。

因此不采用依赖版本号的乐观锁方案。

通过事务行锁和当前状态条件实现并发保护：

- 先锁定 Finding；
- 再读取当前状态；
- 根据当前状态判断是否允许；
- 执行条件更新；
- 写审计；
- 提交。

并发操作必须串行化。

不能使用「先 SELECT 状态，再无条件 UPDATE」的方式——这正是现有 `set_finding_verification` 的写法（G1/G2）。

### 4.4 F3 原子更新

当决定涉及 `kind` 与 `verification_status` 时，必须在同一个 UPDATE 中更新。

逻辑示意：

```sql
UPDATE discovery.discovery_finding
SET
    kind = $new_kind,
    verification_status = $new_verification_status
WHERE tenant_id = $finding_tenant
  AND id = $finding_id
  AND verification_status = $expected_current_status;
```

实际 WHERE 条件必须包含适当的当前状态约束。

更新行数必须检查。

如果没有更新到目标行，应返回明确的冲突或状态错误，不得继续写入成功审计。

> 说明：现有实现用「新建一条 `kind='user_confirmed'` 且 `verification_status='PENDING'` 的新行」表达同一语义（G7），既不满足 F3，也丢失血缘。I2 必须改为原地原子更新。

### 4.5 审计事务

Finding 更新和审计日志必须处于同一事务。

如果审计写入失败：

- Finding 更新回滚；
- 不允许出现业务状态已改变但审计缺失。

现有 `record_finding_verification` 的四段独立提交（G4）必须合并为单事务。

---

## 5. 决定类型与状态转换

### 5.1 确认

人类确认 Finding 时：

- 检查当前状态允许确认；
- 更新对应业务字段；
- 必要时同时更新 `kind` 和 `verification_status`（同一 UPDATE）；
- 写入确认审计；
- 返回确认结果。

确认后的 Finding 才能被下游业务作为已确认 Finding 使用。

### 5.2 拒绝

人类拒绝 Finding 时：

- 检查当前状态允许拒绝；
- 更新真实存在的状态字段 `verification_status = 'REJECTED'`；
- 写入拒绝原因（写入审计 `after_data`，并视现有模型能力决定是否落到 `discovery_answer`）；
- 写入审计；
- **不删除 Finding**。

拒绝必须保留历史记录，以便追溯 AI 为什么提出了该 Finding，以及人类为什么否决。

### 5.3 要求补充

要求补充时：

- Finding **不得**进入确认状态（`verification_status` 保持 `PENDING`）；
- 保留原始发现内容；
- 记录补充要求（`discovery_question` + `discovery_answer` 或审计 `after_data`）；
- 将任务交回允许补充的 Agent 或人工流程。

按 §2.4 的映射，本决定**不新增状态枚举**，复用现有 `discovery_question.status='open'`（及既有的 `deferred` 暂缓）承载。

如果现有数据模型不能表达补充请求，应先确认是否能复用现有状态或备注字段，不得未经设计直接新增一套工作流状态。

### 5.4 重复决定

重复提交相同决定：

- 不重复产生状态转换；
- 不重复产生确认副作用；
- 返回当前权威状态。

如果此前已经确认，而本次提交的是拒绝，必须按冲突处理，不允许静默覆盖。

> 现状 G2：`set_finding_verification` 直接用客户端给的 `status` 覆盖，无条件更新，可把 `CONFIRMED` 改写为 `REJECTED`。I2 必须以「条件更新 + 行数检查 + 状态机判定」取代。

---

## 6. 人工修正

### 6.1 修正与确认的区别

人工修正是人类对 Finding 内容作出修改。

确认是人类认可某个 Finding 作为可信信息。

两者可以在同一事务内完成，但必须在审计中明确区分（审计 `action` 或 `after_data` 中区分「决定」与「修正内容」）。

### 6.2 修正范围

I2 允许确认操作携带必要的修正信息，但不得默认允许任意字段修改。

实施时必须明确：

- 哪些字段允许人工修正；
- 哪些字段由 Discovery 系统管理；
- 哪些字段只允许 Agent 更新；
- 哪些字段修改后必须重新确认。

按真实表结构（§1.1），候选字段为：`content`、`kind`、`confidence`、`evidence`、`actor_role`。

### 6.3 已确认 Finding 再修改

已确认 Finding 如果发生实质性内容变更，不应继续沿用旧确认结果。

必须重新进入待确认流程，或者明确证明该修改属于不影响确认结论的非实质性元数据更新。

不得出现：

内容已经改变，但旧的人工确认仍然被当成对新内容的确认。

---

## 7. 审计设计

### 7.1 存储

复用：

`governance.audit_log`

不新增 `decided_by`、`decided_at` 字段作为第二权威来源。

### 7.2 审计事件

建议使用明确的 action：

- `discovery_finding_confirmed`
- `discovery_finding_rejected`
- `discovery_finding_revision_requested`

最终 action 命名应与现有审计规范统一（现有 `governance.audit_log.action` 为自由文本，各域自行约定）。

### 7.3 审计内容

至少记录（按真实列映射）：

| 设计要求 | `audit_log` 列 |
|---|---|
| Tenant | `tenant_id` |
| Project | `project_id`（由 session 解析，必填） |
| Finding | `target_type='discovery_finding'` + `target_id` |
| 操作者 | `actor_id`（真人 principal） |
| 操作时间 | `created_at` |
| 操作类型 | `action` |
| 操作前状态 | `before_data` |
| 操作后状态 | `after_data` |
| 变更字段 | `before_data` / `after_data` 键集差异 |
| 决定原因（如有） | `after_data`（如有承载位） |

对于 F3，同时记录：

- 原 `kind` 与新 `kind`；
- 原 `verification_status` 与新 `verification_status`。

### 7.4 权威查询

确认人和确认时间必须从对应审计记录取得。

如果页面需要显示：

- 谁确认的；
- 何时确认的；
- 为什么确认；

应通过审计记录或明确的审计查询接口提供。

不得从客户端提交参数推断确认人和确认时间。

---

## 8. 下游消费约束

### 8.1 未确认 Finding

未确认 Finding 可以：

- 显示在 Discovery 工作台；
- 提供给 Agent 进一步分析；
- 作为待确认事项展示；
- 用于生成后续建议。

但不能作为已经确认的项目事实。

### 8.2 已确认 Finding

下游业务可以读取已确认 Finding，但必须使用服务端的权威状态判断。

不能仅根据 UI 显示标签判断。

### 8.3 被拒绝 Finding

被拒绝的 Finding：

- 保留历史；
- 不进入已确认事实集合；
- 不应被后续 Agent 误认为待确认的新发现；
- 可以作为历史否决信息供后续分析参考。

### 8.4 Agent 上下文

Agent 获取 Finding 时，必须能够区分：

- 待确认（`PENDING`）；
- 已确认（`CONFIRMED`）；
- 已拒绝（`REJECTED`）；
- 待补充（`PENDING` + 存在未答复的补充问题）；
- 事实冲突（`CONFLICT`）。

不能把不同状态混成一个无差别的 Finding 列表。

Agent 不得因为某条 Finding 出现在上下文中，就将其理解为已确认事实。

---

## 9. 并发与异常处理

### 9.1 两个人同时确认

两个用户同时对同一 Finding 作出决定：

- 通过 Finding 行锁串行化；
- 第一个成功提交的决定生效；
- 第二个用户读取到新状态后，根据状态规则返回成功幂等结果或冲突；
- 不得覆盖第一个人的决定。

### 9.2 Agent 更新与人工确认并发

Agent 更新 Finding 内容时，也必须遵守 Finding 的行锁与字段所有权规则。

如果 Agent 的内容更新发生在人工确认之前，应由人工看到最终待确认内容。

如果人工确认先完成，后续 Agent 不得静默修改已确认内容。

实质性修改必须重新进入确认流程。

### 9.3 Project 归档与 Finding 确认并发

遵循 I1 的 Project 行锁规则：

- 人工确认先获得 Project 锁并提交，随后归档；
- 归档先提交，后续确认必须拒绝。

不能出现 Project 已归档后，Finding 仍然成功确认。

### 9.4 审计写入失败

审计失败时，业务操作必须回滚。

### 9.5 重复请求

客户端超时重试时，应通过当前状态和幂等语义防止重复决定。

如果现有系统已经有统一请求幂等设施，优先复用。

如果没有，不为 I2 随意新增一套通用幂等框架。

---

## 10. 验收矩阵

| 编号 | 验收项 | 预期结果 | 对应现状缺口 |
|---|---|---|---|
| I2-001 | Agent 创建 Finding | 允许 | — |
| I2-002 | Agent 直接确认 Finding | 拒绝 | — |
| I2-003 | Agent 通过 MCP 间接确认 | 拒绝 | G8（`pm_discovery_submit_answers` 答复不得计为人工确认） |
| I2-004 | 无权限成员确认 | 拒绝 | G6 |
| I2-005 | 有权限成员确认 | 成功 | — |
| I2-006 | 有权限成员拒绝 | 成功 | — |
| I2-007 | 要求补充 | 不进入确认状态（保持 `PENDING`） | — |
| I2-008 | 确认状态不允许重复决定 | 返回幂等结果或冲突 | G2 |
| I2-009 | 已确认 Finding 被拒绝请求覆盖 | 拒绝 | G2 |
| I2-010 | F3 更新 kind 和 verification_status | 同一事务原子更新 | G7 |
| I2-011 | F3 第二字段更新失败 | 整体回滚 | — |
| I2-012 | 审计写入失败 | Finding 更新回滚 | G3 |
| I2-013 | 两个人同时确认 | 只有一个有效决定 | G1 |
| I2-014 | 确认与 Agent 内容更新并发 | 不覆盖已确认内容 | — |
| I2-015 | Project 归档后确认 | 拒绝 | G5 |
| I2-016 | 确认与归档并发 | 遵循 Project 锁顺序 | — |
| I2-017 | 确认人查询 | 从 audit_log 获取 | G3 |
| I2-018 | 确认时间查询 | 从 audit_log 获取 | G3 |
| I2-019 | 已拒绝 Finding | 保留历史 | — |
| I2-020 | 下游读取未确认 Finding | 不作为已确认事实 | — |
| I2-021 | 已确认 Finding 实质性修改 | 必须重新确认 | — |
| I2-022 | 重复请求 | 不产生重复业务副作用 | — |
| I2-023 | 既有 verification 端点行为 | 路由与请求体保持兼容，实现改为统一决定服务 | §1.4 |
| I2-024 | `set_finding_verification` 裸导出 | 不再作为可绕过审计/锁的公开原语存在 | G8 |
| I2-025 | 确认操作的事务原子性 | Finding 更新 + question/answer + 审计单事务提交 | G4 |
| I2-026 | 跨租户/非成员调用 | 404（不泄露存在性），与既有 `project_visible` 语义一致 | §1.5 |
| I2-027 | 确认后读取 `kind`/`verification_status` | 与 `after_data` 一致（F3 原子性可观测） | §4.4 |

---

## 11. 实施任务拆分建议

### I2-A：Finding 状态与消费者核验

范围：

- 核验真实状态枚举（已初步完成：见 §1.1，实施前复核 CHECK 与常量一致性）；
- 核验 `discovery_question.status` 全部消费者，确认复用 `open/deferred` 表达「要求补充」无歧义；
- 核验所有 Finding 写入口（REST、MCP、内部服务）；
- 核验所有 Finding 状态消费者；
- 确定确认、拒绝、补充的现有状态映射（已初步给出：§2.4）。

交付：

Finding 状态与消费者矩阵。

### I2-B：人工确认服务端入口

范围：

- 可信调用人上下文（含真人/Agent 主体判定）；
- 权限检查（成员边界 + Finding 确认权限码）；
- Project 活动门禁（复用 I1）；
- Finding 行锁与条件更新；
- 统一决定服务 `decide_finding`；
- 收口 `set_finding_verification` 公开导出（G8）。

交付：

统一的 Finding 人工决定服务。

### I2-C：原子更新与审计

范围：

- 确认；
- 拒绝；
- 要求补充；
- F3 双字段原子更新（改为原地 UPDATE）；
- `audit_log` 同事务写入（含 `project_id` 解析）；
- 合并现有四段独立提交（G4）。

交付：

可追溯的 Finding 决定记录。

### I2-D：Agent 写入边界

范围：

- Agent 创建和更新权限；
- 禁止 Agent 直接确认（含 `pm_discovery_submit_answers` 的答复不算确认）；
- 已确认 Finding 的内容保护；
- 实质性修改重新确认。

交付：

Agent 与人类的 Finding 字段所有权边界。

### I2-E：下游读取与回归

范围：

- Finding 状态读取；
- 确认信息查询（从 audit_log）；
- Agent 上下文状态区分（§8.4 五态）；
- 并发和归档测试。

交付：

Finding 确认机制全链路回归测试。

---

## 12. 最终设计裁决

1. Finding 是待核验信息，不天然等于项目事实。
2. AI Agent 可以提出 Finding，但不得确认 Finding。
3. Finding 人工决定必须由服务端统一入口执行。
4. 必须校验真人身份、Tenant、Project 成员边界、Finding 确认权限和 Project 生命周期。
5. Finding 当前没有 version/revision，不依赖新增版本字段实现并发控制。
6. 通过 Project 行锁、Finding 行锁和条件更新保证并发安全。
7. `kind` 与 `verification_status` 必须在同一事务原子更新（且必须原地更新，不得新建行代替）。
8. Finding 业务更新与 `governance.audit_log` 必须同事务提交。
9. 确认人和确认时间的权威来源是 audit_log。
10. 不新增 `decided_by`、`decided_at` 作为第二权威来源。
11. 已确认 Finding 的实质性内容修改必须重新确认。
12. 已归档 Project 不允许确认 Finding。
13. 下游必须区分待确认、已确认、已拒绝、待补充（及事实冲突）Finding。
14. 未确认 Finding 不得作为已确认项目事实。
15. 真实状态枚举已存在且不新增值：`PENDING/CONFIRMED/REJECTED/CONFLICT` + `observed/user_confirmed/speculation/contradicted`。
16. 「要求补充」不新增状态，用 `PENDING` + `discovery_question` 承载。
17. I2 是收编并加固既有确认路径（`POST .../findings/{id}/verification`），路由保持兼容，实现改为统一决定服务。
18. 既有实现的八项缺口 G1–G8 全部纳入验收矩阵，逐条关闭。

**最终结论：I2 的业务原则、权限边界、并发协议、原子更新要求、审计权威来源和验收标准至此定稿。实施前仍须完成 `discovery_question.status` 消费者核验与 Finding 状态消费者全量核验，不得擅自补造现有状态或字段。**

---

## 附录 A：本轮核验涉及的代码位置

| 位置 | 内容 |
|---|---|
| `migrations/0025_discovery.sql` | `discovery.discovery_finding` 建表与 CHECK 约束；`discovery_question.status`；`discovery_answer` |
| `crates/pm-application/src/discovery/mod.rs:74,76` | `FINDING_KINDS`、`VERIFICATION_STATUSES` 常量 |
| `crates/pm-application/src/discovery/finding.rs:82` | `add_finding`（`verification_status` 可由调用方指定，创建即定态的入口） |
| `crates/pm-application/src/discovery/finding.rs:195` | `set_finding_verification`（无锁、无事务、无审计、公开导出 —— G1/G2/G3/G8） |
| `crates/pm-application/src/discovery/web.rs:80` | `record_finding_verification`（多段独立提交；新建 `user_confirmed` 行 —— G4/G7） |
| `crates/pm-application/src/lib.rs:139` | `set_finding_verification` 公开导出（G8 收口点） |
| `apps/pm-api/src/pm_discovery.rs:44` | 既有 verification 路由 |
| `apps/pm-api/src/pm_discovery.rs:291` | `verification_handler` |
| `apps/pm-api/src/pm_discovery.rs:104` | `project_in_tenant`（成员边界） |
| `apps/pm-api/src/support.rs:126` | `project_visible`（`core.project_member` 联结） |
| `apps/pm-api/src/auth.rs:93,133,559` | 令牌权限码通道与 MCP 工具权限映射先例 |
| `crates/pm-mcp/src/tools.rs:868` | `DiscoveryFindingInput`（不含 `verification_status`） |
| `crates/pm-mcp/src/tools.rs:1137` | `pm_discovery_submit_answers`（答复 actor 为 `ai_pm` 执行主体） |
| `migrations/0003_platform.sql:188`、`migrations/0019_tenant.sql:134` | `governance.audit_log` 建表与 `tenant_id` 追加 |
