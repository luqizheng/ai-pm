# I2 · Finding 人工确认机制设计稿 V1.1（修订定稿）

> **文档性质**：S01 设计稿——Finding 人工确认与可信事实管理  
> **版本**：V1.1  
> **修订日期**：2026-11-03  
> **状态**：设计裁决已完成，待实施前代码核验与实施  
> **修订依据**：I2 V1.0、2026-11-03 技术反馈及用户 R2 裁决  
> **核心原则**：Finding 是待核验信息或已确认事实的承载物。Agent 可以创建、分析和补充 Finding，但不能伪造人类确认。可信 human 直接提交的 `user_confirmed` Finding 创建即为 CONFIRMED。所有人工决定必须经过服务端统一入口，并验证调用人、Tenant、Project、Session 和 Finding 的真实归属。

---

# §0. 本次修订摘要

## 0.1 总体判定

I2 V1.0 的设计方向成立：

- Finding 是项目发现信息的承载物；
- 人工确认是可信事实形成的重要边界；
- `governance.audit_log` 是确认人和确认时间的权威来源；
- `core.project → discovery_finding` 是统一锁序；
- Finding 的确认、拒绝和修改应通过统一服务入口；
- 未确认 Finding 不得被下游当作正式确认事实。

本次技术反馈发现了两个必须修复的高危现状缺陷：

1. Finding 确认操作在归属检查之前已经修改数据库，并且没有验证 Finding 实际所属 Project。
2. Finding 创建操作的 REST 和 MCP 入口均存在 Session 与路径 Project 不匹配的跨项目问题。

这两个问题属于现有代码的 P0 安全缺陷，不能仅作为未来设计建议。

## 0.2 本次用户裁决

| 编号 | 事项 | 最终裁决 |
|---|---|---|
| R1 | F3 原子更新触发条件 | 仅当人工修正同时改变 kind 和 verification_status 时，要求同一 UPDATE 原子完成 |
| R2 | 人类直接提交的 user_confirmed 初态 | 可信 human 直接提交时创建即 CONFIRMED；Agent 转述仍为 PENDING |
| R3 | Session 状态是否作为决定门禁 | 不增加 Session 状态门禁 |
| R4 | 待补充状态的判定 | 从最近一次有效人工决定审计推导，不从 open question 推导 |

## 0.3 本次新增代码事实与安全裁决

| 编号 | 事项 | 最终处理 |
|---|---|---|
| S1 | G9：确认操作跨 Project 越权及先修改后验证 | P0 修复 |
| S2 | G10：创建操作跨 Project 越权 | P0 修复，覆盖 REST + MCP |
| S3 | Project 归档状态门禁 | 使用 `status != archived` 语义 |
| S4 | 真人判定机制 | 服务端检查 `core.principal.kind == human` |
| S5 | 审计列与 helper | 补全审计身份字段，复用现有审计 helper |
| S6 | 幂等 | 优先复用现有 `operation_id` 模式 |
| S7 | NewFinding 定态 | 外部调用方不能自行指定 verification_status |

---

# §1. 目标与范围

## 1.1 设计目标

本设计解决以下问题：

1. Agent 创建的 Finding 如何与人类确认的事实区分？
2. 谁可以确认、拒绝或要求补充 Finding？
3. 如何保证确认人身份可信？
4. 如何避免跨 Tenant、跨 Project、跨 Session 操作 Finding？
5. 如何保证业务修改与审计记录一致？
6. 如何避免并发确认、重复提交和状态覆盖？
7. 如何让下游 Agent 正确区分待确认信息与正式事实？
8. Project 归档后如何阻止 Finding 继续发生业务变更？

## 1.2 设计范围

本设计覆盖：

- Finding 创建；
- Finding 确认；
- Finding 拒绝；
- Finding 要求补充；
- Finding 分类修正；
- `user_confirmed` 特殊创建语义；
- REST 和 MCP 调用边界；
- Tenant / Project / Session / Finding 归属校验；
- 人类身份验证；
- 审计；
- 并发和幂等；
- Agent 上下文读取；
- Project 归档门禁。

## 1.3 不在本设计范围内

本设计不负责：

- Discovery Session 的完整生命周期；
- Session 的暂停、恢复和完成策略；
- 普通 Knowledge 的发布流程；
- Project Goal；
- Git 仓库权限；
- Task 生命周期；
- Project 归档本身；
- 所有 Discovery 类型的业务定义。

Session 是发现过程的容器，Finding 是发现过程中形成的信息。二者的生命周期可以相关，但不要求完全一致。

---

# §2. 现有代码事实

本节严格区分已核验事实、设计裁决和待核验事项。

## 2.1 Finding 现有模型

已核验事实：

1. 系统已有 `discovery_finding`。
2. Finding 关联 Discovery Session。
3. Finding 包含 `kind` 和 `verification_status`。
4. 当前 `discovery_finding` 没有 version/revision 字段。
5. 当前 `discovery_finding` 没有 `decided_by`、`decided_at`、`updated_at` 等人工决定权威字段。
6. 确认人和确认时间的权威落点是 `governance.audit_log`。

因此，本设计不依赖在 Finding 表中新增确认人和确认时间作为第二权威来源。

## 2.2 当前状态展示

前端 `DiscoveryPanel.vue:34` 的 `CHOICE_STATUS` 映射：

| 前端选择 | verification_status |
|---|---|
| confirmed | CONFIRMED |
| rejected | REJECTED |
| unsure | PENDING |

普通确认和拒绝只改变 `verification_status`。

`unsure` 表示尚不能确认，不能因为要求补充信息就擅自修改 Finding 的 `kind`。

## 2.3 已通过其他方式解决

现有业务中存在独立的「已通过其他方式解决」语义：

1. 原 Finding 被标记为 CONFIRMED；
2. 新建一条 `kind='user_confirmed'` 的 Finding；
3. 新 Finding 用于记录人类确认的解决事实。

这不是必须原地更新原 Finding 的场景。

原 Finding 与新 Finding 可以分别保留自己的历史和业务含义。

## 2.4 当前人工确认调用链

已核验：

- 前端已经使用人工确认端点；
- `discovery.ts:135` 存在 `confirmDiscoveryFinding`；
- `record_finding_verification` 是当前相关业务入口；
- `set_finding_verification` 位于 `finding.rs:203`；
- 当前更新条件按 `(tenant_id, finding_id)` 执行。

当前发现的问题：

`record_finding_verification` 在更新提交后才检查 `finding.session_id != session_id`。

因此，虽然最终可能返回归属错误，但数据库修改已经发生。

同时，路径 `project_id` 目前只用于成员检查，没有与 Finding 实际所属 Project 进行比对。

## 2.5 Finding 创建调用链

已核验：

- REST 通过 `add_finding_handler` 创建 Finding；
- MCP 通过 `discovery_submit_findings` 创建 Finding；
- REST 入口检查路径 Project 成员；
- `add_finding → get_session` 只检查租户内 Session 存在；
- 当前没有完整验证 `session.project_id == path.project_id`；
- MCP 创建链路存在相同问题。

因此，REST 和 MCP 都必须修复。

## 2.6 NewFinding 初始状态

已核验：

- `NewFinding.verification_status` 当前是 pub 字段；
- REST 和 MCP 当前都显式传 `None`；
- `DiscoveryFindingInput` 不包含 `verification_status`；
- MCP 的 `tools.rs:1126` 显式传入 `None`；
- `web.rs:128` 当前也以 `None` 构造。

但内部调用方仍然可以直接构造 `NewFinding` 并设置初始状态。

这意味着创建时定态的入口尚未收紧。

## 2.7 Principal 身份

已核验：

- `AuthedPrincipal` 位于 `auth.rs:147`；
- 当前结构不包含 principal kind；
- `resolve_principal` 已拒绝 service 令牌和跨租户令牌，相关逻辑位于 `auth.rs:238-243`；
- `core.principal.kind` 已存在；
- migration `0001_core.sql:33` 定义了 principal kind；
- 已知类型包括 `human`、`dev_agent`、`ai_pm`、`service`。

REST 当前的认证构造路径接近 human-only，但不能把这种隐式路径当作最终安全保证。

本设计要求人工决定入口显式验证 principal kind。

## 2.8 审计体系

已核验：

- `governance.audit_log` 是现有审计权威来源；
- 现有审计记录除 actor 基础字段外，还包含 `actor_client`、`actor_auth_mode`、`claimed_device_id`、`actor_identity_id` 等身份相关信息；
- `pm_write/audit.rs:44,76` 已有可复用的审计 helper；
- helper 已覆盖 action、target_type、target_id、after_data、tenant_id 等参数。

本设计要求复用已有 helper，不另写裸 INSERT。

## 2.9 Project 状态

已核验：

当前 `core.project.status` 现有值包括：

- `planning`
- `initializing`

相关路径：

- `advance.rs:73`
- `lifecycle.rs:127`

当前尚无 `active` 和 `archived`。

I1 尚未实施 Project 归档生命周期。

因此，I2 不得使用 `status == active` 作为人工决定门禁。

## 2.10 待核验事项

| 编号 | 待核验事项 | 目的 |
|---|---|---|
| V1 | `discovery_finding.verification_status` 的完整 DB 枚举及 CHECK 约束 | 避免引入不存在的状态值 |
| V2 | `discovery_finding.kind` 的完整允许值 | 确认分类修正边界 |
| V3 | Finding 所属 Session 的实际关联查询 | 保证 Project 归属验证 |
| V4 | `transition_session` 全部消费者 | 确认不增加 Session 门禁不会破坏既有流程 |
| V5 | 所有 Finding 创建入口 | 避免遗漏 REST/MCP 之外的入口 |
| V6 | 所有 Finding 确认、拒绝和补充入口 | 避免旁路 |
| V7 | 审计 action 的实际命名 | 正确推导最近一次人工决定 |
| V8 | `operation_id` 的现有作用域和幂等语义 | 判断是否可直接复用 |
| V9 | Project 状态字段的约束及 I1 后续迁移兼容性 | 确保归档门禁可落地 |
| V10 | 审计 helper 的完整 actor 参数传递 | 确保人类身份信息完整 |
| V11 | 现有 `NewFinding` 的所有调用方 | 收紧外部定态能力而不破坏内部流程 |
| V12 | `user_confirmed` 创建入口的真实调用身份 | 确保直接确认只适用于可信 human |

---

# §3. Finding 的领域语义

## 3.1 Finding 定义

Finding 是 Discovery 过程中形成的一条信息记录。

它可能来自：

- Agent 分析；
- 人类输入；
- 仓库扫描；
- 代码事实；
- 外部资料；
- 人工核验；
- 其他 Discovery 工作流。

Finding 的存在本身不意味着内容已经成为项目正式事实。

是否可信，应结合：

- 来源；
- `kind`；
- `verification_status`；
- 人工决定审计；
- 必要时的证据材料。

## 3.2 Finding 的两种基本维度

`kind` 和 `verification_status` 是不同维度。

| 维度 | 含义 |
|---|---|
| kind | 这条 Finding 属于什么类型、表达什么性质的信息 |
| verification_status | 这条 Finding 当前处于什么核验状态 |

不能将两者混为一个状态机。

例如：

- speculation 是信息分类；
- observed 是信息分类；
- user_confirmed 是人类直接确认事实的分类；
- PENDING 是尚未确认；
- CONFIRMED 是已经确认；
- REJECTED 是已经拒绝。

上述 kind 示例必须以现有 DB 约束和代码枚举为准，不得直接假定所有示例值都已存在。

## 3.3 普通 Finding 创建规则

普通 Finding 创建时：

- Agent 创建：由服务端初始化为 PENDING；
- 人类提交普通待核验信息：默认 PENDING；
- 不允许外部调用方直接指定 `verification_status`；
- 不允许 Agent 通过构造参数将普通 Finding 设置为 CONFIRMED。

## 3.4 user_confirmed 特殊语义

本设计明确采用用户裁决 B：

**可信 human 直接提交的 `user_confirmed` Finding，创建即 CONFIRMED。**

原因：

`user_confirmed` 表达的是人类当前直接作出的事实确认，而不是 Agent 对用户话语的二次归纳。

如果人类亲自提交一条确认事实，再要求人类进入列表进行第二次确认，没有实际业务价值。

但该语义必须依赖可信身份，而不是只依赖 `kind` 字符串。

规则：

| 创建主体 | kind | 初始 verification_status |
|---|---|---|
| 可信 human 直接提交确认事实 | user_confirmed | CONFIRMED |
| Agent 根据资料或代码分析创建 | 普通分类 | PENDING |
| Agent 转述人类曾经说过的内容 | 即使标注 user_confirmed，也不能自动确认 | PENDING |
| 未能识别可信 human 身份的调用 | 不允许使用 human 直接确认路径 | 按普通创建规则或拒绝 |
| 人类提交普通待核验 Finding | 普通分类 | PENDING |

### 3.4.1 可信 human 的判定

服务端必须从可信认证上下文中解析真实 principal，并验证：

`core.principal.kind == 'human'`

不得依赖：

- 客户端提交的 `actor_kind`；
- Finding 正文中的用户身份描述；
- Agent 声称内容来自用户；
- 客户端自行提交的 `confirmed_by`；
- 客户端自行提交的 `verification_status`。

### 3.4.2 创建与审计原子性

人类直接创建 `user_confirmed` Finding 时：

同一事务内必须完成：

1. 验证可信 human 身份；
2. 验证 Tenant、Project、Session 归属；
3. 验证 Project 未归档；
4. 创建 Finding，初始状态为 CONFIRMED；
5. 写入人类直接确认的审计记录；
6. 提交事务。

如果审计失败，Finding 创建必须回滚。

不得出现 Finding 已经是 CONFIRMED，但没有对应人类确认审计记录的情况。

### 3.4.3 Agent 转述

Agent 转述人类输入，不等于人类直接确认。

例如：

用户曾经在聊天中说过：

> 这个问题已经解决了。

Agent 根据聊天记录创建 `user_confirmed` Finding，仍然只能进入 PENDING。

如果需要将其升级为 CONFIRMED，必须由人类通过可信入口明确确认。

---

# §4. 人工决定机制

## 4.1 统一入口

所有人工决定必须经过统一领域入口：

`decide_finding`

这是建议统一的领域服务名称，不代表现有代码已经存在同名端点。

REST、MCP 及其他调用入口不能各自实现不同的确认规则。

它们应调用统一领域服务。

## 4.2 支持的人工决定

建议统一入口承载以下业务行为：

| 决定 | 业务语义 |
|---|---|
| confirm | 确认 Finding |
| reject | 拒绝 Finding |
| request_revision | 要求补充或修正 |

实际命令枚举和数据库状态必须以代码核验结果为准。

不得因为 UI 使用了某个名称，就推断 DB 已经存在同名状态。

## 4.3 普通确认

普通确认只更新：

`verification_status`

不要求修改 `kind`。

确认成功后：

- Finding 进入已确认状态；
- 写入可信人工决定审计；
- 下游可以按已确认事实处理；
- 确认人和确认时间从审计记录获取。

## 4.4 普通拒绝

普通拒绝只更新：

`verification_status`

不要求修改 `kind`。

拒绝成功后：

- Finding 进入拒绝状态；
- 写入可信人工决定审计；
- 下游不得将其作为已确认事实。

## 4.5 要求补充

要求补充不应通过随意新增 verification_status 值实现。

现有 `unsure` 对应 PENDING。

因此：

- 要求补充后，Finding 仍然可以保持 PENDING；
- 记录人工要求补充的审计事件；
- 必要时关联补充说明或现有问题记录；
- UI 根据最近一次有效人工决定展示「待补充」。

## 4.6 F3：kind 与 verification_status 原子更新

修订后的 F3：

> 当人工修正操作同时涉及 Finding 的 `kind` 和 `verification_status` 时，两个字段必须在同一事务、同一条 UPDATE 中原子更新。普通确认和拒绝只更新 `verification_status`，不得无必要改变 `kind`。

例如：

```sql
UPDATE discovery_finding
SET
    kind = $new_kind,
    verification_status = $new_status
WHERE
    id = $finding_id;
```

上面是表达原子性要求的示意 SQL，不代表现有 SQL 完全如此。

普通确认不应为了满足 F3 而强行修改 kind。

## 4.7 已通过其他方式解决

保留既有业务语义：

1. 原 Finding 确认解决；
2. 新建 `kind='user_confirmed'` Finding；
3. 新 Finding 表达新的人工确认事实；
4. 两条 Finding 分别保留；
5. 新 Finding 的创建初态按 §3.4 规则确定。

不强制将这类业务改造成原地更新。

---

# §5. 人工决定的权限与身份

## 5.1 人工决定必须由 human 执行

人工确认、拒绝和要求补充属于人工决策。

服务端必须检查：

`core.principal.kind == 'human'`

不能仅依赖 REST 路由当前只接受某类令牌的隐式行为。

## 5.2 不同调用主体的能力

| 主体 | 创建普通 Finding | 创建 user_confirmed 并直接 CONFIRMED | 人工确认/拒绝 |
|---|---|---|---|
| human | 允许 | 允许 | 允许 |
| dev_agent | 允许 | 不允许 | 不允许 |
| ai_pm | 按既有写入规则 | 不允许 | 不允许 |
| service | 按既有服务规则 | 不允许 | 不允许 |

表中「允许创建普通 Finding」仍须满足 Tenant、Project、Session 归属和调用权限要求。

其他系统服务如果需要代替人类提交确认，不能仅凭 service 身份绕过 human 判定；必须有另行明确的授权机制。本设计不默认赋予这种能力。

## 5.3 身份信息来源

人工决定的身份信息必须来自可信服务端认证上下文。

禁止：

- 由客户端自行指定确认人；
- 由 Agent 填写确认人；
- 将调用者传入的字符串作为真实 principal；
- 只通过 Tenant 成员关系推断其 principal kind。

## 5.4 Tenant 和 Project 权限

即使调用者是 human，也必须验证：

1. 调用者属于当前 Tenant；
2. 调用者有目标 Project 的相应权限；
3. Finding 实际属于目标 Project；
4. Finding 实际属于目标 Session；
5. Project 当前允许进行业务修改。

Tenant 成员身份不等于所有 Project 的操作权限。

---

# §6. Tenant / Project / Session / Finding 归属校验

## 6.1 核心安全原则

路径参数只是调用者声明的上下文，不是资源真实归属的证据。

例如：

```text
/projects/X/sessions/S/findings/F
```

不能因为调用者有 Project X 的权限，就认定 Finding F 属于 X。

服务端必须验证完整关系：

```text
Finding F
    |
    v
实际 Session S
    |
    v
实际 Project Y
    |
    v
实际 Tenant T
```

然后与调用路径和调用者权限进行匹配。

## 6.2 归属校验规则

对 Finding 确认、拒绝、要求补充和创建操作，必须满足：

```text
path.tenant_id == actual_finding.tenant_id

path.project_id == actual_session.project_id

path.session_id == actual_finding.session_id

actual_session.tenant_id == path.tenant_id

actual_finding.tenant_id == path.tenant_id
```

创建操作没有现存 Finding，因此应验证：

```text
path.tenant_id == actual_session.tenant_id

path.project_id == actual_session.project_id
```

具体字段及 JOIN 路径必须以现有数据库结构为准。

## 6.3 G9：确认操作跨项目越权修复

现状：

`set_finding_verification` 当前按 `(tenant_id, finding_id)` 更新。

`record_finding_verification` 在更新提交后才检查：

`finding.session_id != session_id`

并且路径 `project_id` 没有与 Finding 实际所属 Project 比较。

修复要求：

**归属校验必须发生在任何业务修改之前。**

建议处理流程：

```text
1. 从路径取得 tenant_id、project_id、session_id、finding_id
2. 验证调用人 Tenant 身份
3. 验证调用人对路径 Project 的权限
4. 只读查询 Finding 实际归属及 Session 实际归属
5. 验证 Tenant / Project / Session / Finding 关系
6. 按统一锁序锁定 Project
7. 检查 Project 未归档
8. 锁定 Finding
9. 重新核验 Finding 与 Session 的实际归属
10. 核验 Finding 当前状态及允许的决定
11. 执行业务 UPDATE
12. 写入审计
13. 提交事务
```

其中：

- 第 4 步的只读查询仅用于确定归属和进入正确锁序；
- 锁定之后必须重新验证关系；
- 任何校验失败都不得发生业务修改；
- 任何失败都不得产生成功的人工决定审计。

不得保留原来「先 UPDATE，再检查 Session」的实现顺序。

## 6.4 锁序

统一遵循：

`core.project → discovery_finding`

如果操作涉及其他关联记录，继续遵循项目统一锁序。

不能为了增加归属检查，在已有事务中先锁 Finding，再补锁 Project。

## 6.5 G10：创建操作跨项目漏洞修复

REST 和 MCP 创建入口都必须验证：

`session.project_id == path.project_id`

只验证 Session 存在于同一 Tenant 不够。

创建流程建议：

```text
1. 验证调用人 Tenant
2. 验证路径 Project 成员权限
3. 查询 Session 实际归属
4. 验证 Session Tenant 与路径 Tenant 一致
5. 验证 Session Project 与路径 Project 一致
6. 按统一锁序锁定 Project
7. 检查 Project 未归档
8. 重新验证 Session 归属
9. 创建 Finding
10. 必要时写入创建审计
11. 提交事务
```

REST 和 MCP 应复用同一领域服务，不能各自维护一套归属校验。

## 6.6 校验失败的副作用约束

以下任意情况必须拒绝：

- Tenant 不一致；
- Project 不一致；
- Session 不一致；
- Finding 不属于路径 Session；
- Finding 不属于路径 Project；
- Project 已归档；
- 调用人无权操作；
- 调用人身份不符合人工决定要求。

拒绝后：

- Finding 不得被修改；
- 不得新增 Finding；
- 不得新增成功决定审计；
- 不得产生部分业务副作用。

失败安全审计如果由统一安全审计策略要求，可以单独记录，但不能伪装成成功业务决定。

---

# §7. Project 生命周期门禁

## 7.1 门禁规则

I2 必须与 I1 的 Project 归档设计兼容。

当前 Project 状态只有：

- planning
- initializing

I1 尚未实施 archived。

因此，I2 不得要求：

`project.status == active`

而采用黑名单语义：

`project.status != archived`

即：

- planning：允许；
- initializing：允许；
- 未来 active：允许；
- archived：拒绝。

当前数据库尚不存在 archived 值，因此现阶段该黑名单条件不会阻断既有流程。

## 7.2 与 I1 的依赖关系

I2 可以先于 I1 实施。

I1 未来增加 archived 状态后，I2 的归档门禁自动生效。

不要求 I2 等待 I1 完成后才能开发。

## 7.3 门禁检查范围

以下操作必须检查 Project 未归档：

- 创建 Finding；
- 人工确认；
- 人工拒绝；
- 要求补充；
- 修改 Finding kind；
- 创建 user_confirmed Finding；
- 其他会改变 Finding 业务状态的操作。

历史查询和审计查询可以只读访问。

## 7.4 归档竞争

Project 归档与 Finding 人工决定并发时：

- 如果 Finding 决定事务先取得 Project 锁并成功提交，则该决定先于归档生效；
- 如果归档事务先完成状态变更，则后续 Finding 决定必须失败；
- 不允许 Project 已归档后仍成功修改 Finding。

具体 Project 锁和归档事务实现继承 I1。

---

# §8. 待确认、待补充、已确认、已拒绝的展示语义

## 8.1 四种业务展示状态

前端需要区分：

| 展示状态 | 含义 |
|---|---|
| 待确认 | 尚无有效人工决定，Finding 处于 PENDING |
| 待补充 | 最近一次有效人工决定要求补充 |
| 已确认 | Finding 当前处于 CONFIRMED |
| 已拒绝 | Finding 当前处于 REJECTED |

这些是业务展示语义，不要求在数据库增加第四种或第五种 verification_status。

## 8.2 待补充的推导

现有 `discovery_question` 没有作者/方向字段，当前字段包括：

- question
- why_blocking
- status
- finding_id

因此，不能通过：

`PENDING + open discovery_question`

推导「待补充」。

因为 AI 自己提出的澄清问题也可能挂在 PENDING Finding 上。

最终规则：

**待补充状态从最近一次有效人工决定审计推导。**

如果最近一次有效人工决定是 `discovery_finding_revision_requested`，则展示为待补充。

其他确认、拒绝和补充相关 action 的真实名称，实施前必须通过代码核验，不得凭设计自行发明审计 action 名称。

## 8.3 审计推导要求

读取侧应：

1. 找到目标 Finding 的相关人工决定审计；
2. 按可信服务端记录的时间和稳定顺序确定最近一次有效决定；
3. 结合 Finding 当前 verification_status 判断展示状态；
4. 不使用 discovery_question 的存在与否代替人工决定；
5. 对审计缺失或历史记录不完整的数据采用保守展示。

建议展示规则：

| 当前 verification_status | 最近一次有效决定 | 展示 |
|---|---|---|
| PENDING | revision_requested | 待补充 |
| PENDING | 无有效决定 | 待确认 |
| CONFIRMED | confirm | 已确认 |
| REJECTED | reject | 已拒绝 |
| PENDING | 历史记录不完整 | 待确认或未知 |

不能因为某条 Finding 挂了 AI 问题，就显示为待补充。

## 8.4 历史数据兼容

如果历史 Finding 缺少完整决定审计：

- 不得伪造确认人；
- 不得推断它是人类确认；
- 不得仅依据 discovery_question 推导待补充；
- 可以展示为待确认或历史状态未知；
- 后续通过人工重新确认补齐可信审计。

---

# §9. 审计机制

## 9.1 审计权威来源

`governance.audit_log` 是人工决定的权威记录。

不在 `discovery_finding` 中新增第二套确认人和确认时间权威字段。

确认人和确认时间必须从可信审计记录获得。

## 9.2 审计字段

应完整保留现有审计体系中的身份信息，包括：

- actor 基础身份字段；
- `actor_client`；
- `actor_auth_mode`；
- `claimed_device_id`；
- `actor_identity_id`；
- `action`；
- `target_type`；
- `target_id`；
- `after_data`；
- `tenant_id`。

实际列名和完整结构以当前 migration 和审计 helper 为准。

## 9.3 复用审计 helper

复用：

`pm_write/audit.rs:44,76`

不允许在 I2 中另写裸 INSERT 绕过现有审计 helper。

I2-C 应明确：

- 使用统一审计 helper；
- 正确传递 actor 身份；
- 正确关联 Finding；
- 正确关联 Tenant；
- 保存决定类型和必要的 after_data；
- 确保审计与业务变更同事务。

## 9.4 审计内容

人工决定审计至少应能回答：

- 谁作出了决定？
- 决定了哪个 Finding？
- 属于哪个 Tenant 和 Project？
- 决定是什么？
- 决定发生在什么时候？
- 原状态是什么？
- 新状态是什么？
- 是否涉及 kind 修正？
- 是否要求补充？
- 如果是创建即确认，是否来自可信 human 直接提交？

具体 after_data 格式应复用项目既有审计约定。

## 9.5 事务一致性

Finding 业务更新与审计必须处于同一事务。

```text
BEGIN

校验 Project
锁定 Finding
校验归属
校验状态
更新 Finding
写 audit_log

COMMIT
```

如果审计写入失败：

- Finding 更新回滚；
- 不得出现无审计的人工确认；
- 不得返回业务成功。

## 9.6 user_confirmed 创建审计

人类直接创建 `user_confirmed` Finding 时：

- 创建 Finding；
- 初始状态 CONFIRMED；
- 写入对应的人类直接确认审计；
- 同一事务提交。

Agent 转述创建的 Finding：

- 初始状态 PENDING；
- 不得伪造 human 确认审计；
- 可以按现有规则记录 Agent 创建来源。

## 9.7 审计不等于业务状态

审计记录是人工决定的权威来源，但 Finding 当前业务状态仍以 `discovery_finding.verification_status` 为准。

不能只因历史审计里存在一次 confirm，就忽略之后发生的合法状态变更。

读取侧应结合：

- 当前 Finding 状态；
- 最近一次有效人工决定；
- 必要的历史顺序。

---

# §10. 并发、状态一致性与幂等

## 10.1 Finding 当前没有 revision 字段

已知 `discovery_finding` 没有 version/revision。

本设计不强制为了人工确认增加独立版本字段。

并发控制优先使用：

- Project 行锁；
- Finding 行锁；
- 条件 UPDATE；
- 状态复核；
- 审计事务。

## 10.2 并发确认

两个 human 同时确认同一个 Finding：

- 第一个成功取得必要锁并完成事务；
- 第二个必须重新读取 Finding 当前状态；
- 如果状态已不允许该操作，应返回明确冲突；
- 不得重复写入两条成功决定审计。

## 10.3 确认与拒绝竞争

同一 Finding 同时收到 confirm 和 reject：

- 只能有一个合法决定成功；
- 第二个必须看到更新后的状态；
- 不允许后提交者静默覆盖先提交者；
- 两个操作不能都返回成功。

## 10.4 条件更新

建议业务 UPDATE 带有当前状态条件。

示意：

```sql
UPDATE discovery_finding
SET verification_status = $new_status
WHERE id = $finding_id
  AND tenant_id = $tenant_id
  AND verification_status = $expected_status;
```

实际 SQL 应按现有状态字段、类型和事务结构调整。

如果影响行数为零：

- 不得继续写成功审计；
- 返回状态冲突或资源状态已变化；
- 不得用无条件 UPDATE 重试覆盖。

## 10.5 operation_id 幂等

现有 `knowledge_proposal` / TaskReady 已有 `operation_id` 幂等模式。

`decide_finding` 优先评估复用。

如果调用协议适合，可允许传入 `operation_id`。

建议语义：

- 同一 operation_id、同一业务请求重复提交：返回原操作结果；
- 同一 operation_id、不同业务请求：拒绝；
- 重试不得重复写入成功审计；
- 幂等范围必须包含必要的 Tenant 和操作目标，不能造成跨资源碰撞。

是否作为外部 API 必填字段，实施前核验现有调用规范后决定。

不为了 I2 单独建立另一套幂等框架。

## 10.6 Session 状态不作为决定门禁

最终裁决：

不增加 `discovery_session.status` 作为 Finding 人工决定的强制前置条件。

理由：

- Session 是发现过程帧；
- Finding 是事实帧；
- Session 完成不意味着 Finding 自动失去人工决定价值；
- Finding 是否允许修改，由 Project 生命周期、调用权限和 Finding 自身状态决定。

但 I2-A 必须核验 `transition_session` 的全部消费者，避免遗漏现有流程约束。

---

# §11. NewFinding 创建入口收口

## 11.1 外部调用方不得自行指定 verification_status

`NewFinding.verification_status` 当前是 pub 字段，调用方可以直接设置。

这不符合本设计的可信边界。

必须收紧：

- REST 输入 DTO 不暴露 `verification_status`；
- MCP 输入 DTO 不暴露 `verification_status`；
- Agent 创建入口不允许自行定态；
- 普通创建由服务端初始化为 PENDING；
- 可信 human 的 `user_confirmed` 走专门的服务端可信创建路径。

## 11.2 内部领域服务

不要求将内部领域模型所有状态字段删除。

可以保留内部必要的状态表达，但必须保证：

- 外部输入不能直接控制初始 verification_status；
- 服务端根据可信身份和业务操作确定初始状态；
- 只有可信 human 直接提交的 user_confirmed 能走创建即 CONFIRMED 的路径；
- Agent 不能通过伪造 kind 或状态参数绕过检查。

## 11.3 REST 创建入口

`add_finding_handler` 必须：

1. 验证调用人 Tenant；
2. 验证路径 Project 权限；
3. 获取 Session 实际归属；
4. 验证 Session 属于路径 Project；
5. 验证 Tenant 一致；
6. 检查 Project 未归档；
7. 调用统一创建领域服务；
8. 由服务端确定初始状态。

## 11.4 MCP 创建入口

`discovery_submit_findings` 必须执行相同规则。

MCP 不能因为属于 Agent 调用路径，就降低 Project 归属校验要求。

REST 和 MCP 最终调用同一领域服务，避免逻辑分叉。

## 11.5 user_confirmed 专用路径

可信 human 创建即确认的逻辑，应与普通 Agent 创建逻辑区分。

建议内部领域服务根据可信 principal 和明确业务操作确定创建模式。

不能接受客户端提交：

```text
verification_status = CONFIRMED
```

然后仅凭这个值创建确认事实。

---

# §12. 下游消费者规则

## 12.1 基本原则

下游 Agent、Task 和 Knowledge 流程必须区分：

- 待确认 Finding；
- 待补充 Finding；
- 已确认 Finding；
- 已拒绝 Finding。

不能将所有 Finding 统一当成事实。

## 12.2 消费规则

| Finding 状态 | 下游使用规则 |
|---|---|
| PENDING / 待确认 | 可以作为线索，不得作为已确认事实 |
| PENDING / 待补充 | 可以作为待处理问题，不得当作已确认事实 |
| CONFIRMED | 可以作为已确认项目事实 |
| REJECTED | 不得作为有效事实使用 |

## 12.3 user_confirmed

可信 human 直接提交的 `user_confirmed` 创建即 CONFIRMED，因此可以直接进入已确认事实消费范围。

Agent 转述的 `user_confirmed` 仍然 PENDING，不能获得相同权威性。

## 12.4 状态变化后的消费者一致性

如果 Finding 从 CONFIRMED 变为其他状态：

- 下游读取必须尊重当前状态；
- 不得只依赖历史确认审计；
- 缓存或派生索引必须能够反映状态变化；
- 已经生成的文档或结论是否需要重审，遵循对应业务流程。

---

# §13. REST / MCP 安全一致性

## 13.1 统一领域规则

REST 和 MCP 是不同调用入口，不是不同业务规则。

以下规则必须统一：

- Tenant 验证；
- Project 成员权限；
- Session 归属；
- Finding 归属；
- Project 归档门禁；
- 初始状态控制；
- 人工决定身份；
- 审计；
- 幂等；
- 并发控制。

## 13.2 不允许的实现方式

禁止：

- REST 校验 Project，MCP 不校验；
- REST 使用统一服务，MCP 直接 UPDATE；
- MCP 允许 Agent 指定 CONFIRMED；
- 仅检查 Tenant 不检查 Project；
- 先修改 Finding 再验证 Session；
- 依赖客户端传入的 principal kind；
- 使用不同入口维护不同状态转换规则。

## 13.3 建议架构

```text
             REST
              |
              v
       REST Handler
              |
              v
       统一领域服务
              ^
              |
          MCP Handler
              |
              v
        Agent 调用入口


       统一领域服务负责：

       - 身份校验
       - Tenant / Project 归属
       - Session / Finding 归属
       - Project 状态门禁
       - Finding 状态转换
       - 并发控制
       - 审计
       - 幂等
```

统一领域服务是逻辑职责，不要求新增独立微服务。

---

# §14. 异常与失败语义

## 14.1 归属不一致

如果路径 Project、Session 和 Finding 实际归属不一致：

- 拒绝请求；
- 不执行任何业务修改；
- 不产生成功决定审计；
- 返回统一的资源归属错误或安全错误。

不应向无权调用者泄露其他 Project 的敏感信息。

## 14.2 Project 已归档

拒绝所有 Finding 业务写操作。

历史读取不受影响。

## 14.3 人工身份不合法

Agent、service 或其他非 human principal 调用人工决定入口：

- 拒绝；
- 不更新 Finding；
- 不写成功决定审计。

## 14.4 状态冲突

Finding 当前状态已经变化，不允许直接覆盖。

返回明确冲突，由调用方重新读取当前状态。

## 14.5 审计失败

人工决定与审计同事务。

审计失败则业务操作整体回滚。

## 14.6 重复提交

同一 operation_id 重复提交时，按幂等规则返回原结果。

没有 operation_id 的调用，仍须通过条件更新和事务保证不会重复产生成功决定。

## 14.7 历史审计缺失

历史 Finding 没有完整人工决定审计时：

- 不得伪造确认人；
- 不得推断确认时间；
- 不得通过当前内容猜测谁确认过；
- 必要时重新走可信人工确认流程。

---

# §15. 验收矩阵

原有 I2-001～I2-022 编号保留。本次新增 I2-023～I2-033。

## 15.1 原有验收范围 I2-001～I2-022

以下为本次定稿必须保持的原有业务验收范围；实施前须与 V1.0 原矩阵逐项核对，不能因本次修订丢失原有测试。

| 编号 | 验收项目 | 验收标准 | 优先级 |
|---|---|---|---|
| I2-001 | Finding 创建 | 可以通过合法入口创建 Finding | P0 |
| I2-002 | Tenant 隔离 | 不允许跨 Tenant 访问或修改 Finding | P0 |
| I2-003 | Agent 创建初态 | Agent 创建 Finding 初始为 PENDING | P0 |
| I2-004 | 人工确认入口 | 人工决定经过统一服务入口 | P0 |
| I2-005 | 人工身份 | 只有可信 human 可以执行人工决定 | P0 |
| I2-006 | Project 权限 | 必须验证调用人对目标 Project 的权限 | P0 |
| I2-007 | Project 生命周期 | 已归档 Project 不允许业务修改 | P0 |
| I2-008 | 状态合法性 | 只允许合法的 Finding 状态转换 | P0 |
| I2-009 | 普通确认 | 确认只改变 verification_status | P0 |
| I2-010 | 普通拒绝 | 拒绝只改变 verification_status | P0 |
| I2-011 | 分类修正 | kind 与 verification_status 同时变化时原子更新 | P0 |
| I2-012 | 已解决场景 | 原 Finding 确认并允许创建新 user_confirmed Finding | P0 |
| I2-013 | 确认审计 | 人工决定写入 governance.audit_log | P0 |
| I2-014 | 审计原子性 | 业务更新与审计失败整体回滚 | P0 |
| I2-015 | 确认人来源 | 确认人从可信审计记录读取 | P0 |
| I2-016 | 确认时间来源 | 确认时间从可信审计记录读取 | P0 |
| I2-017 | 已确认事实消费 | 下游可区分已确认与未确认 Finding | P0 |
| I2-018 | 已确认内容修改 | 实质性修改已确认 Finding 后不能静默保持原确认语义 | P0 |
| I2-019 | 并发确认 | 并发决定不能互相覆盖 | P0 |
| I2-020 | 条件更新 | 状态变化后旧操作不能无条件覆盖 | P0 |
| I2-021 | REST 一致性 | REST 遵循统一领域规则 | P0 |
| I2-022 | MCP 一致性 | MCP 遵循统一领域规则 | P0 |

## 15.2 本次新增验收 I2-023～I2-033

| 编号 | 验收项目 | 验收标准 | 优先级 |
|---|---|---|---|
| I2-023 | user_confirmed 创建即确认 | 可信 human 直接提交时创建即 CONFIRMED，且创建和审计同事务 | P0 |
| I2-024 | Agent 转述隔离 | Agent 创建或转述 user_confirmed 时初始仍为 PENDING | P0 |
| I2-025 | Principal kind 检查 | 人工决定入口服务端检查 `core.principal.kind == human` | P0 |
| I2-026 | NewFinding 定态收口 | 外部 REST/MCP 不能自行指定 verification_status | P0 |
| I2-027 | Session 门禁 | 不增加 Session 状态决定门禁，且 transition_session 消费者已核验 | P1 |
| I2-028 | 确认 Project 归属 | 路径 Project 与 Finding 实际 Project 不一致时拒绝且零变更 | P0 |
| I2-029 | 确认 Session 归属 | 路径 Session 与 Finding 实际 Session 不一致时拒绝且零变更 | P0 |
| I2-030 | 确认失败零副作用 | 归属检查失败时不更新 Finding、不写成功决定审计 | P0 |
| I2-031 | REST 创建 Project 归属 | REST 路径 Project 与 Session 实际 Project 不一致时拒绝 | P0 |
| I2-032 | MCP 创建 Project 归属 | MCP 路径 Project 与 Session 实际 Project 不一致时拒绝 | P0 |
| I2-033 | 创建失败零副作用 | Project/Session 归属不一致时不创建 Finding、不产生部分业务副作用 | P0 |

## 15.4 V1.0 遗留验收的承接（I2-034～I2-038）

V1.0（`smark-pm-s01-i2-finding-human-confirmation-v1.md` §10）的 **I2-023～I2-027** 已被本稿
§15.2 用作新编号，导致那五条旧验收失去编号位。本次裁决（2026-11-03）：**保留本稿
I2-001～I2-033 一律不动**，以 **I2-034～I2-038** 承接 V1.0 那五条，一条不丢。

| 编号 | 来源 | 验收项目 | 验收标准 | 优先级 |
|---|---|---|---|---|
| I2-034 | V1.0 I2-023 | 既有 verification 端点行为 | 路由与请求体保持兼容，实现改为统一决定服务 | P1 |
| I2-035 | V1.0 I2-024 | `set_finding_verification` 裸导出 | 不再作为可绕过审计/锁的公开原语存在 | P0 |
| I2-036 | V1.0 I2-025 | 确认操作的事务原子性 | Finding 更新 + question/answer + 审计单事务提交 | P0 |
| I2-037 | V1.0 I2-026 | 跨租户/非成员调用 | 404（不泄露存在性），与既有 `project_visible` 语义一致 | P0 |
| I2-038 | V1.0 I2-027 | 确认后读取 `kind`/`verification_status` | 与 `after_data` 一致（F3 原子性可观测） | P1 |

**编号占用登记**：I2-034～I2-038 由本节占用。后续新增验收从 **I2-039** 起，不得回填
I2-001～I2-038。

---

## 15.3 补充并发与安全专项测试

以下是 I2-019、I2-020、I2-028～I2-033 的必要测试场景：

1. 同一 Finding 两个人同时确认；
2. 同一 Finding 同时确认和拒绝；
3. Project 归档与 Finding 确认竞争；
4. Project X 成员通过 X 路径操作 Y 的 Finding；
5. 同 Tenant 下跨 Project 创建 Finding；
6. 同 Project 下跨 Session 操作 Finding；
7. MCP Agent 尝试把 Finding 创建为 CONFIRMED；
8. Agent 把人类历史发言转述为 user_confirmed；
9. Human 直接创建 user_confirmed；
10. 审计写入失败后确认事务回滚；
11. 相同 operation_id 重试；
12. 相同 operation_id 携带不同业务内容；
13. 两个 Agent 并发创建相似 Finding；
14. 历史 Finding 缺少人工决定审计；
15. PENDING Finding 同时挂有 AI open question 和人工 revision_requested。

---

# §16. 实施任务拆分

## I2-A：现状核验与安全边界确认

目标：完成 Finding 现有模型和全部调用入口的代码核验。

工作内容：

1. 核验 verification_status 完整枚举；
2. 核验 kind 完整枚举；
3. 核验 discovery_finding 与 discovery_session 关联；
4. 核验 transition_session 的全部消费者；
5. 搜索全部 Finding 创建入口；
6. 搜索全部 Finding 修改入口；
7. 搜索全部确认、拒绝和补充入口；
8. 核验 REST 当前先更新后验证的问题；
9. 核验 MCP 创建链路；
10. 核验 `core.principal.kind` 查询方式；
11. 核验审计 helper 参数；
12. 核验 operation_id 先例及作用域；
13. 核验 Project 状态约束；
14. 核验 NewFinding 所有调用方。

交付物：

`docs/s01-全局主菜单与信息架构设计基线/docs/s01-i2-code-facts.md`

必须输出：

- 已证实代码事实；
- 当前漏洞及复现路径；
- 待核验事项；
- 拟修改文件清单；
- 受影响 REST/MCP 入口。

## I2-B：Finding 领域规则

目标：落实 Finding 的状态与创建语义。

工作内容：

1. 修订 F3 原子更新语义；
2. 实现普通 Finding 创建初始 PENDING；
3. 实现可信 human 直接创建 user_confirmed 初始 CONFIRMED；
4. 保证 Agent 转述仍为 PENDING；
5. 收紧 NewFinding 对外部调用方的定态能力；
6. 明确普通确认、拒绝和要求补充规则；
7. 保留已通过其他方式解决的独立新建 Finding 语义；
8. 明确已确认 Finding 实质性修改后的重新确认规则。

交付物：

Finding 领域规则及对应测试。

## I2-C：人工决定入口与审计

目标：实现可信人工决定闭环。

工作内容：

1. 统一 `decide_finding` 领域入口；
2. 检查 `core.principal.kind == human`；
3. 验证 Tenant 和 Project 权限；
4. 验证 Finding 与 Session、Project 的真实归属；
5. 遵循 `core.project → discovery_finding` 锁序；
6. 检查 Project `status != archived`；
7. 实现条件 UPDATE；
8. 复用 `pm_write/audit.rs` helper；
9. 保证业务更新和审计同事务；
10. 处理并发决定；
11. 评估 operation_id 幂等复用；
12. 从审计推导待补充状态。

交付物：

统一人工决定服务、审计集成和测试。

## I2-D：Finding 创建入口与 P0 安全修复

目标：修复 REST 和 MCP 的跨 Project 漏洞。

工作内容：

1. 修复 REST `add_finding_handler`；
2. 修复 MCP `discovery_submit_findings`；
3. 验证 Session 实际所属 Project；
4. 验证 Tenant 一致性；
5. 验证 Project 未归档；
6. 创建前执行全部归属检查；
7. 与统一 Finding 创建领域服务集成；
8. 禁止外部传入 verification_status；
9. 保证 human user_confirmed 创建路径可信；
10. 保证 Agent 转述不能创建即确认；
11. 增加跨项目拒绝测试；
12. 增加失败零副作用测试。

交付物：

REST/MCP 创建入口修复及 P0 安全测试。

## I2-E：并发、审计推导与全量验收

目标：完成一致性验证。

工作内容：

1. 并发确认测试；
2. 确认与拒绝竞争测试；
3. Project 归档竞争测试；
4. operation_id 幂等测试；
5. 待补充审计推导测试；
6. 历史审计缺失测试；
7. REST/MCP 一致性测试；
8. 下游 Finding 消费规则测试；
9. I2-001～I2-033 验收矩阵全部执行；
10. 输出实施事实与遗留问题。

交付物：

`docs/s01-全局主菜单与信息架构设计基线/docs/s01-i2-implementation-acceptance.md`

---

# §17. 实施顺序与依赖

建议顺序：

```text
I2-A 现状核验
  |
  v
I2-B Finding 领域规则
  |
  +----------------+
  |                |
  v                v
I2-C 人工决定    I2-D 创建入口
  |                |
  +----------------+
          |
          v
     I2-E 全量验收
```

I2-C 和 I2-D 可以在共享领域规则明确后并行开发，但不能各自实现不同的身份、归属和审计规则。

## 17.1 与 I1 的关系

I2 不依赖 I1 先完成。

I2 采用：

`project.status != archived`

语义。

I1 未来增加 archived 后，I2 自动生效。

## 17.2 与 I3 的关系

本项目已裁决取消原复杂 Git 权限设计。

I2 不依赖独立 Git 授权系统。

Finding 的人工确认权限与 Git 仓库读写权限是不同问题，不应混为一谈。

## 17.3 与 Knowledge 的关系

Finding 可以作为 Knowledge 的事实来源，但 Finding 确认不等于自动发布 Knowledge。

下游是否将 Finding 提取、整理并发布到正式 Knowledge，仍然遵循现有 Knowledge 发布管线。

不能将 Discovery Finding 的确认状态与 `ai.document_proposal` 或 `discovery.discovery_proposal` 的状态混淆。

---

# §18. 最终裁决

I2 V1.1 最终确定：

1. Finding 是 Discovery 信息的承载物，不因创建而天然成为已确认事实。
2. 普通 Agent 创建 Finding 初始状态为 PENDING。
3. 人类直接提交的 `user_confirmed` Finding 创建即 CONFIRMED。
4. Agent 转述人类发言仍然是 PENDING，不能伪造人类确认。
5. `verification_status` 由服务端控制，外部 REST/MCP 调用方不得自行定态。
6. 普通人工确认和拒绝只更新 `verification_status`。
7. 只有当人工修正同时改变 `kind` 和 `verification_status` 时，才要求同一 UPDATE 原子完成。
8. 已通过其他方式解决的业务保留原 Finding 确认、新建 `user_confirmed` Finding 的语义。
9. 人工决定必须经过统一领域入口。
10. 人工决定入口必须检查 `core.principal.kind == human`。
11. 所有创建和决定操作必须验证 Tenant、Project、Session、Finding 的真实归属。
12. 归属校验必须发生在业务修改之前，并在事务内防止竞态。
13. REST 和 MCP 必须复用统一领域服务，修复 G9/G10。
14. Project 门禁采用 `status != archived`，不要求 `active`。
15. Session 状态不作为 Finding 决定门禁，但必须核验其全部消费者。
16. 待补充由最近一次有效人工决定审计推导，不能由 open question 推导。
17. 确认人和确认时间以 `governance.audit_log` 为权威来源。
18. 审计必须复用现有 helper，业务更新和审计保持同事务。
19. 幂等优先复用现有 `operation_id` 模式。
20. 并发决定必须通过 Project 锁、Finding 锁、条件更新和事务保证一致性。
21. 归档 Project 的 Finding 业务写入必须失败。
22. 所有安全校验失败必须保证零业务副作用。

**最终设计定位：I2 不是简单地给 Finding 增加一个确认按钮，而是建立一条可信事实形成链路：Agent 可以发现和提交信息，人类可以作出真实决定，服务端负责验证资源归属和身份，审计负责证明决定来源，下游 Agent 只能按照真实确认状态使用事实。**

---

# §19. 设计定稿后的实施前检查

进入 Codex 实施前，必须确认以下事项：

- [ ] I2-A 已核验完整状态枚举和 DB 约束。
- [ ] I2-A 已核验所有 Finding 创建、修改和确认入口。
- [ ] I2-A 已核验 `transition_session` 消费者。
- [ ] I2-A 已核验 REST 和 MCP 的跨项目漏洞。
- [ ] I2-A 已核验 `core.principal.kind` 的读取方式。
- [ ] I2-A 已核验审计 helper 的完整调用参数。
- [ ] I2-A 已核验 operation_id 复用条件。
- [ ] I2-B 已实现 user_confirmed 的可信 human 创建即确认。
- [ ] I2-B 已确保 Agent 转述不能创建即确认。
- [ ] I2-C 已修复先更新后验证问题。
- [ ] I2-C 已实现统一人工决定入口。
- [ ] I2-D 已修复 REST 和 MCP 创建跨 Project 漏洞。
- [ ] I2-D 已收紧 NewFinding.verification_status。
- [ ] I2-E 已通过跨 Project、跨 Session、归档竞争和并发测试。
- [ ] I2-001～I2-033 已完成验收。
- [ ] 实施报告区分已实现、未实现、待核验和遗留风险。

**本设计稿定稿不代表上述代码已经修复。只有完成实施与验收后，才能将 I2 标记为已完成。**
