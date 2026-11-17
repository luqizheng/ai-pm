# P3-D04-02 Step 3 — PC/ProjC 目标状态机与事务边界设计方案 V1.0

- 日期：2026-11-17
- 任务书：《P3-D04-02 Step 3 — PC/ProjC 目标状态机与事务边界设计任务书 V1.0》
- 性质：**设计方案与差距映射**——非实施授权；所有推荐方案未经人工批准不构成裁决
- 设计输入：V0.6 冻结基线 + 冻结批准令（R36/R37/R38）+ 依赖核查报告 V1.0（方案 C）+ A-1（0028 已闭合）/A-2/A-3 裁决 + U-2/U-3 非阻塞登记
- 术语约定：状态名一律沿用代码与 Schema 现值（本设计不重命名状态值域）；差距编号=GAP-xx（交付物 2）；待裁决编号=Q-xx（交付物 3）；证据编号=E-xx（交付物 4）
- 配套文档：`P3-D04-02-Step3-Schema代码Migration差距映射-v1.0.md`｜`P3-D04-02-Step3-待人工裁决事项清单-v1.0.md`｜`P3-D04-02-Step3-设计证据索引-v1.0.md`

---

## 0. 设计立场与总原则

1. **现状即目标态的验证起点**：依赖核查已证实 PC/ProjC 服务层状态机与事务机制同构且成熟（发布十四步单事务、行锁推号、CAS、幂等账本、审计/Outbox 同事务）。本设计的立场是**定稿现有机制为目标设计**，而非另起炉灶；凡现状已满足冻结契约之处，标注「已满足」并停止设计（不发明变化）。
2. **PC 真源、ProjC 镜像**：projc/publish.rs 头注自证「真源 = pc 同名文件；域内改名」。目标设计确认该模式为正式架构约定：状态机与事务语义的真源在 PC 侧定义，ProjC 同构镜像，差异仅限域内改名与已登记的领域差异（§1.8）。
3. **五概念不压缩**（任务书 D1 红线）：提案状态、正式对象生命周期、Revision 归属、审批决策事实、发布结果是五个正交概念，分别由 `pc/projc.proposal.status`、`cognition_object.lifecycle_status`、`current_revision_id + version_member` 双投影、`approval` 行、`cognition_version.status` 五个载体表达。**不引入统一 status 字段**。
4. 时间语义遵循 R38：决策时间=审批事实的组成部分（`approval.decided_at` / `cognition_version.confirmed_at`）；发布生效时间=决策结果应用时点；流程时间戳=流程元数据不属决策事实。
5. 本设计引用的每项现状结论均有证据编号（E-xx，见交付物 4）；证据不足处显式标注，不写成事实。

---

## D1. PC/ProjC 目标状态机

### D1.1 五概念分层（与 V0.6 五形态的映射）

| 概念 | 载体 | 状态值域 | 目标裁定 |
|---|---|---|---|
| 提案状态 | `proposal.status` | draft / submitted / approved / rejected / withdrawn / expired / applied / failed | 采用现值域为正式状态机（§D1.2）；`expired` 语义未定义→Q-03 |
| 正式对象生命周期 | `cognition_object.lifecycle_status` / `relationship.lifecycle_status` | identity / draft / active / deprecated / superseded | 采用现值域（§D1.4）；deprecated=终态，superseded=终态须带 successor（触发器强制） |
| Revision 归属（无独立状态字段） | `current_revision_id`（草稿候选投影）+ `version_member`（正式化投影） | —（双投影表达，不设 status 列） | **裁定：不引入 revision status 字段**——草稿/候选/正式是归属关系而非状态机，与 V0.6「Revision=对象内容的一次版本」一致；修改正文恒产生新 Revision（G-06） |
| 审批决策事实 | `approval` 行 | pending / approved / rejected | 行=不可变决策事实载体（锚列触发器保护）；**重决=新行追加**，行级 append-only；与 R36 五元组同构（对象=proposal、Revision=内容 hash 锚定的成员集、outcome=status、操作者=decided_by、决策时间=decided_at） |
| 发布结果 | `cognition_version.status` | published / archived | 单向（触发器强制 published→archived）；`confirmed_by+confirmed_at`=发布确认决策锚 |

### D1.2 提案状态机（PC 真源；ProjC 同构）

状态语义：

| 状态 | 业务含义 | 可否离开 | 离开路径 |
|---|---|---|---|
| draft | 已起草未送审；成员集已齐备并锚定（成员在 draft 期可增改，重锚后 hash 失配须重新提交核验） | 可 | submit / withdraw |
| submitted | 待人工决策；已写 pending 审批行（内容锚+基线锚） | 可 | approve / reject / withdraw / publish-CAS失配→failed |
| approved | 已获真人批准（暂记）；等待发布（报告 close 批量或显式补发布） | 可 | publish→applied / CAS失配→failed / reject（审阅流 v2 重决互改）/ withdraw |
| rejected | 已被真人驳回（暂记）；**非终态**——审阅流 v2 允许重决翻转为 approved（报告关闭前） | 可 | approve（重决）/ withdraw |
| withdrawn | 撤回（提案方主动或报告 close 批量） | **终态** | 无（复活=另起新提案 re-propose） |
| expired | 预留：提案过期 | 设计未定义（**无任何代码路径置此态**，仅查询排除引用） | **Q-03 待裁决** |
| applied | 已正式发布进版本 | **终态** | 无 |
| failed | 发布 CAS 失配（基线迁移） | **终态** | 无（重做=re-propose against current version） |

**状态转换表**（PC；ProjC 同构仅域前缀差异）：

| # | 当前态 | 动作 | 执行主体 | 前置条件（全部满足才转换） | 目标态 | 产生的事实与副作用 |
|---|---|---|---|---|---|---|
| T-01 | （无） | create | AI 服务链可达 / 人工（目标态，入口后置） | 成员规范化（每身份恰一 Revision）+ 目标解析（存在/归属/生命周期/类型等级）+ F-05 端点并入 + R7/F-07 等级下限 + 基线取当前版本号 + 闭合预检 | draft | proposal 行（content_hash 锚）+ proposal_member 行 + 审计 `pc_proposal.create` |
| T-02 | draft | submit | AI 服务链可达（无 A3 门=提案面）/ 人工 | proposal FOR UPDATE + draft-only + 调用方 CAS（可选）+ 成员完整性断言 + 库内 hash 重算一致 + L3 未冻结 + 版本基线未迁移 | submitted | status=submitted + submitted_at + **pending approval 行**（uq_pc_approval_pending 兜底并发）+ 审计 `pc_proposal.submit` |
| T-03 | submitted | approve | **真人 + A3** | FOR UPDATE 双行（proposal+pending approval）+ 三方哈希比对（提案行/审批行/成员集重算）+ L3（approve 落点）+ 报告未关闭 | approved | approval 行终态化（decided_by/decided_at/reason）+ proposal.status=approved + 审计 `pc_proposal.approve` |
| T-04 | submitted | reject | **真人 + A3** | 同 T-03 但 **L3 不阻断驳回**；报告未关闭 | rejected | 同上（outcome=rejected）+ 审计 `pc_proposal.reject` |
| T-05 | approved / rejected | re-decide（互改） | **真人 + A3** | 报告未关闭（closed 即冻结）；旧 approval 行已终态不可变→**新插 pending 行再决**（旧行留痕） | approved / rejected（翻转） | 新 approval 行 + proposal.status 翻转 + 审计（每次决策一条事实，append-only） |
| T-06 | approved | publish | **真人 + A3**（close 闭告人或显式补发布） | 发布十四步（§D4.9） | applied | cognition_version 行（confirmed_by/at 锚）+ version_member 集 + 审计 `pc_version.publish` + Outbox `pc.cognition.version_published` + 账本 completed + 对象/关系生命周期推进（T-11~T-14） |
| T-07 | submitted / approved | 发布 CAS 失配 | 系统（发布事务内） | current_version_no ≠ expected_version_no | failed | 同事务内置 failed + 审计 `pc_version.publish_failed` + 账本 failed + COMMIT + 409 |
| T-08 | draft / submitted | withdraw | 提案方（created_by 侧）/ 报告 close 批量 | proposal FOR UPDATE | withdrawn | status=withdrawn + 审计 `pc_proposal.withdraw` |
| T-09 | 同态（approved→approve / rejected→reject） | 重复决策 | 真人 + A3 | 状态已是目标态 | 不变 | **幂等返回既有审批事实**（无新行） |

**显式裁定（含裁决项）**：

- **R1-a（设计裁定，推荐口径）**：`rejected` **不允许 resubmit**（submit 仅接受 draft——现状实现即如此）。驳回后出路=①报告关闭前人工 re-decide 翻转（T-05）；②另起新提案（修订集若同内容则与旧提案并存于审批面，各自独立决策）。理由：驳回是真人决策事实，允许 resubmit 会绕过决策记录的可追溯性。→ 若需变更属 Q-02 裁决项。
- **R1-b（设计裁定，推荐口径）**：`withdrawn` / `applied` / `failed` 为**终态**，无程序化离开路径；`applied` 的回退=通过**新提案**在前版本之上修订（版本链单向），不回改版本。
- **R1-c（Q-03 裁决项）**：`expired` 在 CHECK 值域内但无 setter、无触发条件、无查询语义（仅候选排除引用）。备选：①定义「基线迁移后 N 天未决自动 expired」的巡检/触发策略；②从值域移除。**推荐②**：V1 版本基线由人工提案节奏控制，自动过期引入隐性状态转移面，与「服务层唯一推进面」原则冲突；删除值域属 Schema 阶段微迁移（纯 CHECK 收敛）。

### D1.3 审批决策事实状态机（approval 行）

| 当前 | 动作 | 目标 | 约束 |
|---|---|---|---|
| pending | decide（approve/reject） | approved / rejected | 触发器强制：pending→仅终态；终态须 decided_by+decided_at；**已决行状态永不改变**（forbid_approval_mutation） |
| approved / rejected | （无行级动作） | — | 重决=T-05 新插 pending 行（uq_pc_approval_pending 按 proposal_id 部分唯一放行） |
| 行删除/插入锚定 | — | — | 内容锚列（proposal_content_hash/expected_version_no/created_at）不可变 |

**与 R36 的符合性声明**：PC/ProjC 决策事实=独立可追溯的 append-only 事实（outcome 由 status 显式列表达、决策时间独立列、操作者独立列），**非从状态翻转推导**——`proposal.status` 是决策的结果应用（状态翻转），`approval` 行才是决策事实本体。两概念物理分离（S-04 已满足）。无缺口。

### D1.4 正式对象生命周期状态机

| 当前 | 进入条件 | 执行主体 | 目标 | 终态性 |
|---|---|---|---|---|
| identity | create_object_identity / ensure_product_singletons（PC 单例骨架）/（ProjC 无单例） | 服务层 | — | 非终态（可补首修订→draft） |
| draft | 首修订落库（upsert_object_draft） | AI 服务链可达/人工（目标态） | — | 非终态 |
| active | 首次随提案发布入正式版本（发布事务内批量推进） | 发布事务（系统） | — | 非终态（可继续 update_object） |
| deprecated | deprecate_object 成员随发布生效 | 发布事务（系统） | — | **终态**（触发器禁改；复活=新提案→按现实现拒绝「revival requires a new proposal」并指向新建对象） |
| superseded | （预留，当前无服务层写入路径） | — | — | **终态**（触发器强制 superseded_by_id 非空） |

关系（relationship）生命周期同构（identity/draft/active/deprecated/superseded）；关系修订链 V1 仅创建 rev=1（U-2 已确认非阻塞）。

终态后的允许操作：只读解析与历史审计；无任何写路径（触发器 fail-closed 双保险：服务层前置检查 + DB 触发器 enforce_object_lifecycle / forbid_identity_delete）。

### D1.5 发布结果状态机

published（INSERT 即 published，confirmed_by/confirmed_at 锚列不可变）→ archived（唯一合法转移，触发器强制）。archived=终态语义（版本链通过 predecessor_version_id 单向生长，无回退）。每 (tenant,project) 版本号严格递增（发布 CAS 保证 proposal.expected_version_no == 当前最大版本号）。

### D1.6 PC 与 ProjC 的统一抽象与领域差异

| 维度 | 统一抽象（真源=PC 定义，ProjC 镜像） | 领域差异（保留，不抹平） |
|---|---|---|
| 提案 8 态机+转换表 | 完全同构 | origin_kind 值域：PC 5 值（ai_agent/kb_memory/manual_input/requirement_change/conflict_resolution）；ProjC 6 值（+repo_analysis） |
| 对象/关系生命周期 | 完全同构 | PC 有 product_profile/product_vision 单例骨架（uq_pc_object_singleton）；ProjC `object_type_singleton` 恒 false |
| 审批决策事实 | 完全同构 | 无 |
| 发布事务十四步 | 完全同构 | advisory 键名/事件名/账本命令前缀（pc_/projc_） |
| L3 冲突门禁 | 门禁机制同构（三落点+同一状态集） | **数据源不对称（A-3 裁决）**：pc.conflict=Legacy 空集；ProjC 冲突源在役（create/resolve 服务） |
| source/evidence | 同构四字段+幂等索引 | source_kind 值域差异（product_cognition/repo_analysis vs project_cognition） |
| 检索投影 | 设计一致 | projc 侧 v_revision_versioned/FTS 缺位→P3-10（B 类并行，非本任务） |

**结论**：两域状态机**统一抽象成立**（一个真源+镜像模式）；必须保留的领域差异仅上表所列六项，均已有 Schema/代码证据，无新差异。

## D2. 人工确认机制与权限矩阵

### D2.1 主体与授权定义（A-2 落实）

- **主体类型**：`core.principal.kind` ∈ {human, ai_agent, service}（代码口径：`ensure_formal_authorized` 仅接受 `kind='human'`，其余**恒 403 fail-closed**）。
- **A3 授权**：项目 Lead 角色 **或** `core.approver_grant` 生效行（存在性即授权，窗口由行有效期决定）。
- **AI 边界**：AI 可起草修订（upsert_object_draft）、登记来源/证据、起草并提交提案（create+submit，提案面）；**不得执行 approve/reject/publish**（三入口独立执行 ensure_formal_authorized，无继承、无代办）。MCP 不暴露确认工具；discovery_writeback 白名单外命令一律拒绝且 `require_human` 为第二道防线。
- **未授权主体**：HTTP 层未认证→401；跨租户/跨项目资源→404（不泄露存在性）；kind 非 human→403。
- **H-11 已知窗口（登记，非新差距）**：A3 判定在事务外只读执行（approve/record_decision/publish ④），授权行在事务开始前的瞬间变化不在本事务感知范围内——现状设计已接受（§23 R-3），维持。

**红线落实**：正式批准权**不可转移**——三重独立门：①approve/reject/decide 入口各自调用 ensure_formal_authorized；②publish 十四步④再次独立判定（不继承 close 调用方之外的任何声明）；③AI/服务主体在主体类型层被拒绝，与权限无关。服务账户（dispatch）仅用于分析链起草/提交（提案面），永不进入正式面。

### D2.2 权限矩阵（目标态；「明确」逐格落实）

图例：✓=允许；✗=禁止（fail-closed 403）；△=条件允许（条件见注）。

| 主体 | 创建提案 | 修改草稿 | 提交审批 | 正式批准 | 拒绝 | 发布 |
|---|---|---|---|---|---|---|
| AI Agent（ai_agent principal，经白名单服务链） | ✓ 注① | ✓ 注② | ✓ 注③ | ✗ | ✗ | ✗ |
| 普通用户（human，无 A3） | ✓ 注④ | ✓ 注④ | ✓ 注④ | ✗ | ✗ | ✗ |
| A3 授权审批人（human：Lead 或 approver_grant 生效行） | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ 注⑤ |
| 未授权主体（未认证/跨租户/其他 kind） | ✗ 401/404 | ✗ | ✗ | ✗ | ✗ | ✗ |

注：

- ① 仅限两条既有服务链（分析链 propose_from_analysis、身份综合链 identity.rs）；直接 REST/MCP 创建提案 V1 无入口（GAP-09，接线归 Schema/API 阶段，非本任务）。
- ② 草稿修订=对 `cognition_object` 的 upsert_object_draft（内容寻址去重）；**提案成员一经锚定（非 draft 态）不可改**（触发器）；draft 期提案成员修改：V1 无修改入口（创建即齐备），若 draft 内成员被外部改动则 submit 时 hash 重算失配 409 拦截（fail-safe 已在位）→ 修改入口本身列 GAP-10（后置）。
- ③ 提交动作当前由服务链内聚执行（起草即提交）；独立「提交」入口同 GAP-09 后置。
- ④ 目标态定义：普通用户可起草自有项目内提案与草稿、提交审批；当前无 HTTP 入口（同 GAP-09 后置接线），设计先冻结权限语义。
- ⑤ 发布两条路径均要求 A3：报告 close 批量发布（闭告人主体逐提案过 ensure_formal_authorized）或显式补发布；**close 动作本身的主体要求**（是否须 A3 才能关闭报告并触发未决提案 withdraw）=Q-05 裁决项（现状：close 无域级 A3 门，publish 段有；withdraw 段属流程性动作）。
- 审批撤销：**无 un-approve 语义**——决策事实不可撤销（R36 append-only）；更正=重决互改（T-05，新审批行留痕）或 withdraw（提案方）。审计全历史保留。

## D3. L3 冲突门禁边界（A-3 落实）

| 维度 | PC | ProjC |
|---|---|---|
| 触发时机 | **三落点**：submit（T-02）→ approve（T-03）→ publish 十四步⑥（兜底）；reject 不检查（驳回不改变正式认知） | 同构三落点 |
| 检查对象与范围 | 提案成员圈定的 object_id/relationship_id 集 × `pc.conflict` 中 status ∈ {detected, under_review, awaiting_human, reopened} 的未决冲突（conflict_object 关联） | 同构（projc.conflict/conflict_object，同状态集） |
| 失败结果 | Forbidden（403 语义），转换不发生，提案状态不变 | 同构 |
| 与审批/发布先后 | submit 先于状态转换；approve 先于审批行终态化；publish 在 CAS 之后、成员构建之前 | 同构 |
| 冲突数据来源 | **pc.conflict=Legacy / Not for New Use（R-06）**：无新建入口、当前恒空集→**门禁三落点恒通过** | 冲突源在役：projc/conflict.rs create_conflict/resolve_conflict（真人 A3 + FOR UPDATE + 合法转换校验；resolved/dismissed 解冻、reopened 重冻结） |
| Legacy 边界（A-3） | **保留门禁代码，不删不改**；pc.conflict 存量行留原位（R-10）；不重新启用、不视图伪装迁移 | 不适用 |
| 「机制有效」与「数据源空置」的区分 | 门禁有效性=代码路径+状态集判定逻辑在位且有测试（conflict_freezes_and_resolves 于 projc 侧验证同构逻辑）；数据源空置=Legacy 结果。**二者独立**——不得因空集推导「门禁无效」 | 两侧均成立 |
| 未来扩展边界 | O-06/S5 决定 PC 冲突源恢复方案；届时**仅恢复数据源写入，门禁代码零改动** | 冲突治理细化归 O-06（B 类并行） |

## D4. 事务边界与一致性设计

**总裁定**：每命令=单数据库事务（pool.begin → 全部写入 → commit）；事务内顺序即锁序（§17.3：advisory → proposal → approval → 审计 → outbox）。审计与 Outbox **同事务**写入。跨事务协调仅一处（close_report，§D4.9），并如实标注为多事务。

| # | 操作 | 事务起点→终点 | 原子性要求 | 写入集合 | 审计时机 | Outbox 时机 | 失败后数据状态 | 跨事务协调 |
|---|---|---|---|---|---|---|---|---|
| 1 | 创建提案 | begin→commit | 提案行+成员+审计原子 | proposal + proposal_member×N + governance.audit_log | 同事务提交前 | 无（产生事件在 propose_from_analysis 链路统一发） | 全回滚（无半行） | 无 |
| 2 | 修改提案草稿 | —（V1 无入口，GAP-10） | 目标：成员改动+重锚 hash 原子 | proposal/proposal_member | 同事务 | 无 | 全回滚 | 无 |
| 3 | 提交审批 | begin→commit | 状态翻转+pending 审批行+审计原子 | proposal(status,submitted_at) + approval(pending) + audit_log | 同事务 | 无 | 全回滚（重复提交→唯一约束 409） | 无 |
| 4 | 人工批准/拒绝 | begin→commit | 审批行终态化+提案翻转+审计原子 | approval(status,decided_*) + proposal(status) + audit_log | 同事务 | 无 | 全回滚；并发裁决=FOR UPDATE+行数断言→409 | 无 |
| 5 | 发布正式认知 | begin→commit（十四步单事务） | 版本+成员+生命周期+账本+审计+事件全原子 | write_operation(pending→completed) + proposal/approval FOR UPDATE 读 + cognition_version + version_member×N + cognition_object/relationship 生命周期批量 UPDATE + audit_log + outbox_event | 同事务（十四步⑫） | 同事务（十四步⑫b，`pc.cognition.version_published`，idempotency_key=`pc_publish:{version_id}`） | CAS 失配=事务内补偿（提案内置 failed+账本 failed+审计）后 COMMIT 返回 409；其余失败全回滚（pending 账本行留痕重放） | 无 |
| 6 | 报告关闭（含批量发布） | **多事务**（见 §D4.9） | 见 §D4.9 | 外层：report(status) + proposal(withdrawn)×N + audit×N；子事务：每提案完整十四步 | 各自事务内 | 各自子事务内 | 外层回滚→关闭失败；子发布失败→计数不阻断 | **是** |
| 7 | 多提案批量操作 | 逐提案独立事务（close 内循环） | 无跨提案原子性（有意设计） | 见 6 | — | — | 部分失败如实计数 | 是 |
| 8 | 失败恢复 | — | — | — | — | — | 账本 pending/failed 行保留→同 operation_id 重放；CAS 失配→re-propose | 幂等键重放 |
| 9 | 重复请求处理 | 同 operation_id→账本 completed 短路→幂等返回既有版本（replayed=true，不发新事件不重复写版本） | — | 仅读 | 无 | 无 | — | — |

### D4.9 close_report 跨事务协调（专节，如实标注非原子）

```
外层事务 A：report FOR UPDATE → 选出 approved 提案集
  ├─ 子事务 B1：publish_version(提案1)（自带 advisory+账本+十四步）成功/失败→计数
  ├─ 子事务 B2：publish_version(提案2) …（幂等键 pc-decision:{id} / projc-decision:{id}）
  └─ 外层续：未决提案 FOR UPDATE → withdrawn×N + 逐条审计 → COMMIT(A)
```

- **不是一个原子事务**：B_i 各自独立提交/回滚；A 持有报告行锁但不持有提案写锁（B 内自行 FOR UPDATE）。
- 一致性保证：①B 与 A 的 withdraw 行集不相交（approved vs draft/submitted）；②B 失败→计数不阻断、提案留在 approved（可重开处理或修基线后 re-propose）；③B 幂等键稳定（decision:{id}）→close 重试（报告已 closed 会 409，但补发布可用显式入口重放同账本键）；
- **目标裁定（推荐，维持现状）**：保持「失败计数不阻断」——批量关闭的语义是审理流程收口，单个提案的发布失败（如基线迁移）属正常业务分支（内置 failed+审计留痕），阻断整个关闭反而制造卡死面。
- **登记（非本任务实施）**：close 无域级 A3 门（publish 段有）→Q-05。

## D5. 并发控制设计

**机制职责表**（目标设计=现状机制定稿；每项回答「为什么需要它」）：

| 机制 | 职责（不可替代的原因） | 使用点 | 获取/释放 | 冲突业务语义 | 自动重试 |
|---|---|---|---|---|---|
| Advisory Xact Lock（`pg_advisory_xact_lock(hashtext(...))`） | 发布入口项目级串行化：十四步含多表批量写与成员集构建，行锁无法覆盖「当前版本号读取→版本号写入」的计算窗口；事务级键随事务自动释放 | `pc_publish:{project}` / `projc_publish:{project}` / `pc_analysis_proposal:{project}` / `projc_analysis_proposal:{project}` | 事务首语句获取；commit/rollback 释放 | 排队等待（无业务冲突语义） | 天然串行，无需重试 |
| FOR UPDATE 行锁 | 单行状态机的「读-判-写」原子化：proposal（状态迁移竞态）、approval pending（并发裁决者串行，后到者读到非 pending→409）、cognition_object（修订推号——行锁推号 §17.2-1，使 revision_no 计算无竞态）、report（close 串行）、conflict（resolve 合法性判定） | 见 E-15/16/18/21 | 事务内获取释放 | 409 Conflict（业务语义明确返回） | 否——409 由人工/调用方决策 |
| CAS（条件 UPDATE） | ①对象指针推进：`current_revision_id IS NOT DISTINCT FROM $locked`（行锁内双保险）②提案基线：`expected_version_no == 当前最大版本号`（跨行一致性，行锁不覆盖）③审批终态化：`WHERE status='pending'` + rows_affected==1 | object.rs:153-163；publish.rs:264-317；proposal.rs:1047-1062 | 同事务 | 失配→提案内置 failed+409（基线迁移须 re-propose）；审批→409「decided concurrently」 | 否 |
| 唯一约束 | **DB 层最终兜底**（应用层判定之外的第二道）：object_key（含 **0028 补齐的 projc**——并发具名首插窗口由 DB 拒绝，服务层映射 409）、operation_id 账本（幂等键）、approval pending（并发提交/重决）、evidence identity（重复登记）、relationship identity（活跃关系）、vm/pm_target（成员/成员去重） | 见 E-08~E-14 | 随 INSERT/UPDATE | 唯一冲突→409 或幂等短路（账本 DO NOTHING） | 账本冲突=幂等命中（重放既有结果）；其余=409 |
| 幂等键（outbox idempotency_key） | 事件写入去重：`ON CONFLICT (tenant_id, idempotency_key) DO NOTHING`——重复语义事件在写入时吸收 | `pc-analyze:{task_id}`、`pc_publish:{version_id}`、`pc-decision:{proposal_id}` 等 | 同事务 | 静默去重（返回既有 event id） | — |
| write_operation 账本 | 发布段意图幂等：同 operation_id 重放→completed 短路返回既有 version（replayed）；状态机触发器防终态回改 | publish 十四步② | 同事务 | 幂等返回 vs failed 留痕 | 是——重放即恢复 |

**0028 在 ProjC 并发身份创建中的作用（任务书 D5 指定核查项）**：`projc/object.rs:60-90` 具名分支为「SELECT（key 查 existing）→ 无则 INSERT identity 行」。两并发同 key 首插：SELECT 均空→双 INSERT——0028 前产生重复身份行（S-06 缺口）；0028 后第二个 INSERT 被 DB 拒绝，服务层收到 unique violation→映射 Conflict 409（与 pc 侧行为对齐）。匿名分支（NULL key）不受影响（NULLS DISTINCT）。【已确认，代码+E-24 活库验证】

**revision_no 推号**：对象侧=object 行 FOR UPDATE 后取 max+1（行锁消除竞态）；DB 层 (object_id, revision_no) 唯一约束缺位=C-1 后置兜底。关系侧 V1 仅 rev=1（U-2）。目标设计维持行锁推号方案（不引入序列/计数器列——避免新状态面）。

## D6. F-04：AI 提案全链路幂等设计（核心闭合项）

### D6.0 现状幂等机制矩阵（先如实盘点）

| 链路段 | PC 现状机制 | ProjC 现状 | 缺口 |
|---|---|---|---|
| 产生（分析→候选判定） | advisory(`pc_analysis_proposal:{project}`) 串行化 + 候选排除（已入最新版本 ∨ 已在开放提案的修订） | 同构 | 跨任务/跨链路的重复提案 unprotected（见 6.1） |
| 产生（事件写入） | outbox idempotency_key=`analysis_outcome_key(task_id)`=**一任务一结论** | 同构 | 无 |
| 物化（草稿对象/修订） | object_key 唯一约束（0028 后两域齐备）+ content_hash 内容寻址去重 | 同构（0028 后） | 无 |
| 物化（来源/证据） | uq_pc_evidence_identity 幂等唯一索引 | 同构 uq_projc_evidence_identity | 无 |
| 物化（提案行） | **无幂等锚**——create_proposal 无 operation_id，proposal_content_hash 无唯一约束 | 同构 | **GAP-01** |
| 提交 | pending approval 唯一约束兜底并发提交；重复 submit（非 draft）→409 | 同构 | 重复提交语义=409 非幂等返回→Q-04 |
| 重投递（事件消费） | outbox lease/at-least-once（attempts≤5→dead_letter）+ 写侧幂等键去重 + 任务终态短路 | 同构 | 消费者自愈依赖候选排除（非账本） |
| 审批 | 同态重放幂等返回既有事实 | 同构 | 无 |
| 发布 | write_operation 账本（completed 短路 replayed）+ advisory + 幂等键 `pc-decision:{id}` | 同构（`projc-decision:{id}`） | 无 |
| **身份综合链（第二 AI 提案源）** | **仅靠 object_key 草稿去重**——无 advisory、无候选排除、无账本、无事件幂等键约束（三提案各自独立，部分失败如实报错） | 不适用（PC 独有链路） | **GAP-02：同轮重跑→同修订集重复提案行** |

### D6.1 产生

- **稳定身份**：一次逻辑分析提案的身份锚=`(task_id, source_id)`——`analysis_outcome_key(task_id)` 作为该次分析的**结论幂等键**（一任务一结论：Proposed 与 NoChange 共用键，事件层防重复结论）。
- **重复执行识别**：三层——①advisory 串行化（同项目分析起草互斥）；②候选排除 SQL（修订已入最新版本 ∨ 已在非 failed/expired 提案中→跳过）；③outbox 幂等键写入去重。
- **不同提案不合并**：成员集参与 `proposal_content_hash` 规范化哈希——不同修订集必然不同 hash，提案行互不合并（但**可重复**，见缺口）。
- **失败/超时/重试**：任务级失败→`processing.failed` 事件+重试（outbox at-least-once）；重试重入 propose_from_analysis 时由①②③短路。
- **目标裁定（GAP-01 之补强，推荐方案 A 的一部分）**：候选排除的「开放提案」判定已能防**同源重复**；跨源（分析链 vs 身份链）重复由成员修订唯一性天然部分隔离（同修订同提案内容）。**残余风险=低**，但按 F-04「真正闭合」要求，升级为账本式（§6.6）。

### D6.2 物化

- **幂等锚点**：领域草稿层两级既有锚——①具名对象=object_key DB 唯一（0028 后两域齐备）；②内容=content_hash 去重（同 hash 复用既有 Revision，`revision_created=false`）；证据=uq evidence identity。
- **提案行物化锚（目标设计新增建议）**：在 `write_operation` 账本扩展命令（`pc_propose` / `pc_submit` / `projc_propose` / `projc_submit`——wire 名实施阶段定，Q-06），operation_id 规范=`{domain}-{chain}:{业务锚}`：分析链=`pc-analyze:{task_id}`；身份链=`pc-identity:{round_id}`（身份轮次 id 需在调用方生成并透传，现状 identity.rs 无轮次 id——**GAP-02 闭合的前置**）。
- **中途失败恢复**：账本行 pending 留痕→重放走「非终态继续」路径（publish 段已有先例：pending/failed 重放继续执行，completed 短路）；草稿段天然可重入（content_hash 去重保证幂等）。
- **重复物化返回既有结果**：账本命中→返回既有 proposal_id/version_id（publish 段 replayed=true 模式推广）。
- **避免重复草稿/Revision**：既有 content_hash 去重 + object_key 唯一已闭环（无新增设计）。

### D6.3 提交

- **幂等身份**：pending approval 唯一约束（DB 层）；提案状态机 draft-only（非 draft 重复 submit=409）。
- **重复提交语义**：现状=409 Conflict；**Q-04 裁决项**：备选 A=同态幂等（submitted+同 hash+同基线→返回既有 receipt，对齐 decide 的同态重放）；备选 B=维持 409。**推荐 A**（调用方超时重试是常态，409 语义应保留给真冲突）。
- **草稿修改后再提交**：成员锚定触发器使非 draft 态成员不可变；draft 期修改（无入口，GAP-10）→submit 时库内重算 hash 失配 409「members changed after drafting」=fail-safe 已在位。目标语义：修改必须产生新锚定（重算+更新 proposal_content_hash）——实现归后置。
- **同 Revision 重复提交 vs 不同 Revision**：同 Revision（同 content_hash 锚）重复提案=独立提案行（各自审批），不自动合并（裁决项 Q-07 可选「同内容提案折叠提示」，**推荐 V1 不做**，保持审批面简单）；不同 Revision=天然不同成员集。

### D6.4 重投递

- **客户端超时但服务端已成功**：发布段=账本 completed 短路幂等返回（replayed=true）；提交段=Q-04 裁决 A 后同态返回；决策段=同态重放已实现。
- **Outbox 重复投递**：lease 派发（locked_until/lease_owner，at-least-once）→消费者必须自幂等——PC/ProjC 消费面（Platform 任务终态推进）以任务终态短路+写侧幂等键吸收。
- **消费者重复执行**：同上；分析链消费者（kb_pc_handlers→derive_projc_cognition→propose_from_analysis）再入由 D6.1 三层防护。
- **消费失败重试**：attempts≤5、next_attempt_at 退避、超限→dead_letter（人工介入面）。
- **长时间未完成 operation**：write_operation pending/git_committed/failed 为非终态→同 operation_id 重放继续（发布段已实现）；**待裁决 Q-08**：非终态账本行的超时巡检（如 pending 超 N 小时告警/abandoned）——推荐后置（Schema/API 阶段配合运维面）。
- **账本与领域对象不一致自愈**：发布事务保证账本与版本同事务写（不可能半边）；**跨库对账巡检（账本 completed 但 version 缺失等）**=Q-09 裁决项（推荐后置，Schema/API 阶段加只读巡检任务）。

### D6.5 审批与发布

- **审批幂等**：同态重放（approved→再 approve）→返回既有 approval 事实，无新行；异态互改=T-05 新行（决策历史 append-only）。
- **同一决策重复提交**：同上；**不同决策竞争**（一人 approve 同时另一人 reject）：pending approval 行 FOR UPDATE 串行→后到者读到非 pending→409「decided concurrently」。
- **发布幂等身份**：operation_id（close 链=`pc-decision:{proposal_id}`；显式补发布=调用方给定）；`write_operation` UNIQUE(tenant,project,operation_id) + completed 短路 + replayed 标记。
- **已发布但调用方未收到响应**：重放同 operation_id→幂等返回既有 version_id/version_no/snapshot_hash——**恢复语义闭环已实现**。
- **账本与上游 operation_id 关联**：operation_id 本身即关联键（规范格式内嵌业务锚：`pc-decision:{proposal_id}` 直接可反查提案）；账本行另存 version_id 回填。

### D6.6 统一幂等模型（端到端）

**两级幂等锚架构（推荐方案 A）**：

| 层 | 锚 | 载体 | 作用域 | 状态 |
|---|---|---|---|---|
| L1 事实幂等（内容级） | content_hash / evidence identity / object_key | 业务表唯一约束 | 同对象内 | **已满足**（0028 后闭环） |
| L2 意图幂等（账本级） | operation_id | write_operation 账本（扩展命令集） | 每次逻辑操作 | **发布段已实现；起草/提交段=目标设计新增（GAP-01/02 闭合方案）** |

**operation_id 规范**：`{domain}-{chain}:{business_anchor}`
- 分析链：`{pc|projc}-analyze:{task_id}`（覆盖产生+物化+提交三段合一——一段逻辑操作一次账本登记）
- 身份链：`pc-identity:{round_id}`（round_id=身份综合轮次标识，调用方生成——GAP-02 前置）
- 决策发布：`{pc|projc}-decision:{proposal_id}`（现状保持）
- 显式补发布：调用方给定（现状保持）

**生成方与生命周期**：调用发起方生成→账本 INSERT（ON CONFLICT 幂等）→pending→completed（回填产物 id）/failed（留 last_error）/abandoned（人工终态）；重放规则=非终态继续、completed 短路返回；清理与保留策略=**Q-07 裁决项**（推荐：completed 保留≥审计窗口、随系统级数据治理政策，不自动清理）。

**审计追踪**：账本行+operation_id 入审计 after_data（publish 已实现）→每笔幂等命中/重放可经 operation_id 全链反查。

**备选方案取舍**（推荐≠已批准，Q-01 裁决）：

| 方案 | 内容 | 优点 | 缺点 | 判定 |
|---|---|---|---|---|
| **A（推荐）** | 扩展 write_operation 账本至起草/提交段（新命令+operation_id 规范） | 复用已验证机制（账本表/触发器/重放语义全套在位）；与发布段统一模型；审计/重放/自愈语义现成 | 需 Schema 阶段扩 command CHECK+账本写入点改造（4 个调用点） | **推荐** |
| B | 维持现状（advisory+候选排除+事件幂等键） | 零改动 | 身份链无防护（GAP-02 开放）；「同一逻辑操作重放」无统一语义；不满足 F-04「真正闭合」 | 否 |
| C | proposal 表加 idempotency_key 唯一列 | 提案行级直锚 | 新列=Schema 变更面更大；账本语义（状态机/重放/自愈）仍缺，等于两套机制 | 否 |

**端到端时序图**（分析链为例；ProjC 同构）：

正常流：
```
分析任务 → collect_facts → register_source/evidence(L1幂等)
  → advisory(pc_analysis_proposal) → 候选排除
  → [方案A] 账本登记 pc-analyze:{task_id}(pending)
  → upsert_object_draft(L1: object_key+hash) → create+submit proposal
  → outbox(pc.change_proposed, key=analysis_outcome_key) → 审计
  → COMMIT → 账本 completed
人工审批（真人A3） → approval 终态化（append-only）
报告 close（真人A3） → publish_version(operation_id=pc-decision:{id})
  → advisory(pc_publish) → 账本(pending) → CAS → 十四步 → 账本 completed
  → outbox(pc.cognition.version_published) → COMMIT
```

重复请求：同 operation_id 重入 → 账本 completed 命中 → 幂等返回既有产物（replayed=true），不产生第二提案/版本/事件。

部分失败：起草段失败 → 事务回滚+账本 pending/failed 留痕 → 重放继续；发布 CAS 失配 → 事务内提案/账本内置 failed + 审计 → COMMIT → 409 → 人工 re-propose。

超时重投递：调用方超时但服务端已提交 → 重投递命中账本/同态重放 → 幂等返回；outbox 重复投递 → 消费侧任务终态短路 + 写侧幂等键去重；5 次失败 → dead_letter 人工介入。

## D7. 审计、事件与 Outbox

**权威性裁定**：领域事实权威=**业务表**（proposal/approval/cognition_version/write_operation…）；技术审计=`governance.audit_log`（同事务追加）；Outbox 事件=**投递通知，非权威**（`integration.outbox_event` 行可 dead_letter/清理，不得作为事实来源）。三者事务关系：领域写入+审计+事件**同一事务**；事件消费失败不回滚领域事实。

| # | 事实 | 领域权威记录 | 审计记录（同事务） | Outbox 事件 | 消费者幂等责任 |
|---|---|---|---|---|---|
| 1 | 提案产生 | pc/projc.proposal+member 行 | `pc_proposal.create`（成员/hash/等级） | 无（统一在起草链尾部发 `pc.change_proposed`） | — |
| 2 | 草稿物化 | cognition_object/revision/document_revision | （审计经 created_by/updated_by 留痕；无独立 audit 动作——登记 GAP-11，推荐后置补 `pc_object.draft_upsert` 审计） | 无 | — |
| 3 | 草稿修改 | 新 Revision（G-06） | 同上 | 无 | — |
| 4 | 提交审批 | proposal.status=submitted+approval(pending) | `pc_proposal.submit` | 无 | — |
| 5 | 人工决策 | approval 终态行（append-only） | `pc_proposal.approve/reject`（重决每次一行审计） | 无 | — |
| 6 | 正式发布 | cognition_version+version_member+对象生命周期 | `pc_version.publish`（operation_id 入 after_data） | `pc.cognition.version_published`（key=`pc_publish:{version_id}`） | 任务终态推进幂等（终态短路） |
| 7 | 失败与恢复 | 账本 failed 行+提案 failed | `pc_version.publish_failed` | `processing.failed`（平台侧） | 失败任务幂等键=`失败任务键+:failed` |
| 8 | 重复请求/幂等命中 | 账本行（重放不新写领域对象） | 无新审计（重放返回既有事实；命中可查账本） | 无新事件 | — |
| 9 | 补偿操作 | 提案内置 failed（CAS 失配，事务内补偿） | publish_failed 审计 | 无 | — |

**一致性要求**：
- 审计四身份列全写（actor_identity_id/actor_client/actor_auth_mode/claimed_device_id）——静态清点测试守卫生产写入点清单；
- Outbox 投递：lease at-least-once（attempts≤5→dead_letter）；事件重复=消费者幂等（终态短路/键去重）；乱序=事件含 aggregate id 与版本号，消费方按业务键幂等，不依赖顺序；延迟=可接受（通知/终态推进语义）；失败=退避重试→dead_letter 人工介入；
- 事件 payload 纪律：仅 DomainRef 四元组引用+摘要/hash，禁止内嵌 service 字段（validate_payload_references 机械校验，生产方写 outbox 前调用）；
- 事件目录：16 项跨服务目录（kb-pc-contracts/events.rs，单一真源）+「未启用禁止生产」（不假装消费）。

**与 R38 的符合性**：审计行时间戳=技术记录时间；决策事实时间=decided_at/confirmed_at（R36 组成部分）；两者分离，无混用。

---

**设计冻结前置**：本方案中标注 Q-01~Q-09 的 9 项裁决未决前，Step 3 设计不得冻结（见交付物 3）。本稿不自行宣告冻结。
