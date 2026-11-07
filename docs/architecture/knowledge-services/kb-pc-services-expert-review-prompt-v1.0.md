# KB-PC 服务化架构 — 专家裁决评审提示词 V1.0

> 用途：将本提示词全文发给外部专家（可附项目仓库访问权）。专家按本提示词完成裁决评审，产出结论作为用户正式裁决 ADR 的输入。本提示词本身不含未公开信息，全部事实来自项目文档与代码勘察结论。

---

## 一、角色设定

你是本项目的**独立架构评审专家**，同时具备以下视角并对每一项给出独立判断：

1. DDD 领域建模专家（限界上下文、聚合、领域事件）
2. PostgreSQL 架构师（schema 设计、约束、事务、并发、多租户隔离）
3. 分布式系统专家（服务拆分、异步消息、一致性协议、故障恢复）
4. 安全专家（认证授权、租户隔离、服务间信任）

评审纪律：**通读全部必读文档，不得只看摘要**；每项结论附文档章节/行号级证据；区分〔事实〕〔推导〕〔待验证〕；**每项裁决必须给出唯一结论与理由，不得以「两个方案都可以」收尾**；发现本提示词未列出的问题，一并报告。

## 二、背景速览（事实，已核实）

**项目**：PM-AI（Rust axum+sqlx 后端 + Vue3/TS 前端 + PostgreSQL），Cargo workspace 三进程（pm-api / pm-worker / ai-pm-worker）**共享单一 PostgreSQL**，无消息队列。

**已有基建**（已实现，代码证据在仓库）：

- Transactional Outbox：`integration.outbox_event` 同事务写入 + 幂等键 + SKIP LOCKED 租约消费 + 指数退避 + 死信重放 API；入站事件 `integration.inbound_event` 幂等
- 审计：`governance.audit_log`（before/after + actor 三元组）+ pg_notify→SSE
- 认证：RS256 JWT（外部 lmms token-service 签发）+ 本地 JWKS 验签 fail-closed；授权为 handler 级校验（项目可见性 404 防探测 / approver_grant 审批闸 / 租户角色每请求查库）
- KB 文档域已落地：`ai.project_document` 等 7 表闭环（**正文权威=文件系统独立 Git 仓库**（git CLI 写入，commit sha 同事务入 PG），PG 只存身份/状态/hash；写幂等账本 `ai.knowledge_write_operation` + advisory lock + CAS）
- PageIndex 本体未实现，代位为可替换 provider 的 DB 章节索引
- **服务间认证基建不存在**（当前三进程同仓库同部署，无网络级服务间调用）

**PC 现状**：正式产品认知（Product Cognition）目标实体**零实现**；数据模型设计（PC-01B）已完成并经独立评审修订为 V1.1（待终审）：`pc` schema 15 表、双维度 Version Member、Relationship 端点闭合三层防线、发布事务十四步、Git-PG 三态写账本协议；七项技术裁决 PC-D-01~06/09 已关闭。现有代码中 `ai.cognition_*`/`ai.baseline_*` 属另一领域（S01-04 技术认知/S01-05 项目基线），与 PC 无关。

**评审对象**：《KB-Service / PC-Service 独立服务化总体架构设计 V1.0》（1346 行/24 章）——设计三服务边界（Platform / KB-Service / PC-Service）、Intake 模型、API/事件契约、可靠投递、Git-PG 一致性、部署策略、12 项 ADR。该设计已经一轮内部独立评审，结论**「有条件通过」**：发现 2 项 P0 缺陷与 1 项 P0 实质冲突（见裁决 1），条件满足前不得冻结 Phase 1/2 契约。

## 三、必读文档（按优先级）

| # | 文档 | 说明 |
|---|---|---|
| 1 | `docs/architecture/knowledge-services/kb-pc-independent-services-architecture-v1.0.md` | 主评审对象（1346 行，通读；重点 §5/§7/§9~§16/§18/§19/§21/§22） |
| 2 | `docs/architecture/product-cognition/pc-phase1-current-review-and-plan-v1.1.md` | PC-01A V1.1 领域基线（重点 §6 目标模型、§6.6 四元组、§7 阶段计划） |
| 3 | `docs/architecture/product-cognition/pc-01b/pc-01b-00-overview-and-decisions-v1.1.md` | PC-01B 总纲与 PC-D 裁决（重点 §11.2 PC-D-02、§24） |
| 4 | `docs/architecture/product-cognition/pc-01b/pc-01b-05-storage-cross-domain-v1.1.md` | PC-01B Git-PG 协议与跨域引用 |
| 5 | `docs/architecture/product-cognition/pc-01b/pc-01b-06-constraints-and-implementation-v1.1.md` | PC-01B 隔离矩阵/发布事务十四步/测试方案 |
| 6 | `docs/architecture/product-cognition/pc-01b/pc-01b-02-core-tables-object-relationship-v1.1.md`、`...03-version-member-and-closure-v1.1.md`、`...04-governance-traceability-v1.1.md` | PC-01B 表结构细节（按需查阅） |
| 7 | `docs/product-cognition-knowledge-base/product-cognition-knowledge-base-definition-architecture-v1.md` | KB 产品定义（§5 资料三层模型/记忆体系，按需） |

（无仓库访问权时：本提示词「二、背景速览」已含裁决所需全部关键事实；但强烈建议申请文档 1~3 全文。）

## 四、不可违反的既有裁决（红线，评审结论不得推翻）

1. **Q-PC-01~13 产品裁决**：PC 与 S01-04 按产品/技术认知划分唯一权威；Requirement 关联必钉 Revision；Product Vision 与 Project Goal 分层；L0~L3 变更权限矩阵（P0~P3 优先级、PC-G17 风险等级不得混淆）；V1 审批人 = Lead 与授权审批人同权；业务规则仅由 PC 正式确认；**KB 审核通过 ≠ PC 正式确认**（KB 知识经 PC 提案通道输入，不自动转正）；**Product Plan 不属于 PC**（PC 仅做影响分析）；Roadmap 边界未定义，不裁决。
2. **领域权威**：KB=原始资料/解析/记忆/来源证据/浅层记忆审核；PC=正式产品认知/版本/关系/治理/冲突；Platform=Team/Project/流程协调；Requirements 统一服务是需求正式确认唯一责任方（PC 不审批 Requirement）；Design/ADR 正文归 Design 域（尚未建设，PC 不得以普通文档冒充）。
3. **三层存储权威**：Markdown+Git = 正文唯一权威；PostgreSQL = 身份/状态/审批/版本引用/审计；PageIndex = 可重建派生索引。禁止 JSONB 正文副本、禁止多权威双写。
4. **治理红线**：正式认知变更必须人工确认、版本化、审计；AI 不得绕过审批（产出止步于草稿/Proposal）；L3 冲突冻结相关对象确认入口；**M3 里程碑前禁开正式认知写入**。
5. **零迁移**：新项目无存量数据负担——不设计历史数据迁移/兼容/回填/双写；改契约直接改。Git-PG 跨存储一致性协议**不属于**双写禁令（既有裁决已作术语澄清）。
6. **PC-01B 已关闭的技术裁决**（pc schema 15 表、PC-D-01 PC 专属提案审批结构、PC-D-03 完整快照、PC-D-04 双注册表、PC-D-05 Source/Evidence 分离、PC-D-09 三态写账本）：服务化不得推翻；唯一例外=本次裁决 1 涉及的 PC-D-02（正文承载位置），且改动仅限「正文表迁址」这类机械适配，须登记差异复核。
7. **现状裁决「不引入消息队列」**：DB Outbox 轮询投递；仅物理拆分（Phase 7）时可再评估。

## 五、待裁决问题

### 第一批：边界与所有权（裁决 1~5）

#### 裁决 1（P0，最高优先）：ai.project_document 多域复用冲突（评审编号 D-1/C-1）

**事实**：`ai.project_document` / `ai.document_revision`（文档域七表）当前承载：① KB 文档（七表主体）；② Project Goal（doc_key='project-goal' 零表实现，归项目管理域）。服务化设计声明「七表归 KB-Service 独占所有权，其他服务只读」。但 PC-01B PC-D-02 裁决 **PC 正文承载复用这两张表**（doc_type 枚举扩值 'pc_object'，`pc.cognition_revision`/`pc.source`/`pc.evidence` 三处 FK 指向它们），即 PC 需要**写入** KB 所有权下的表——直接冲突。另：kb 计划迁独立 `kb.*` schema，届时三处 FK 与 doc_type ALTER 还要机械适配。

**候选**：
- (a) PC 经 KB 服务端口代写正文：Git-PG 写协议变成跨服务协作，PC-01B §12「账本与 Git 同服务事务」前提被打破，发布事务被牵入跨服务协调；
- (b) PC 正文独立承载：新建 `pc.document` / `pc.document_revision`（同构：身份+git_commit_sha+content_hash，无正文列）+ PC 独立 Git 根（服务化设计已裁决 PCGIT 与 KB GIT 物理分离，天然配套）；PC-01B 三处 FK 改指新表 + doc_type 扩值不再需要，属登记制机械适配；
- (c) 维持 PC 写 ai 表并接受「共享正文表」：违反服务数据所有权与自治原则。

**项目内推荐**：(b)。请裁定，并指出 (b) 的隐藏成本/风险（如：文档模板复用、KB→PC 追溯四元组 `knowledge_base` 通道语义是否受影响、PCGIT 双 Git 根运维成本）。

#### 裁决 2：ADR-01 — KB / PC 是否为独立一级服务

**候选**：A 独立一级服务（领域独立=硬要求；部署独立分阶段）；B Platform 内部模块；C 立即物理独立部署。
**推荐**：A。请确认或反对，并给出你的部署形态判断依据。

#### 裁决 3（含 P0 缺陷 D-2）：ADR-02 — 服务协作模式与 KB 触发权

**背景**：V1.0 原设计「KB 审核通过后自行判断并直接调用 PC 发起影响分析（KB→PC 点对点，触发判断权归 KB）」被用户质疑且内部评审确认两个问题：① PC 触发规则隐藏在 KB 内部、跨服务流程决策不可观测；② KB 发起的 PC 分析在 Intake 台账无任务行，整体状态提前 COMPLETED（台账缺口 D-2）。

**修正方案（待确认）**：三段职责——**KB 产出「认知候选信号」**（结构化：相关性结论+confidence+引用，仅建议）→ **Platform 依路由策略决策**（阈值/开关/频控/去重，创建 pc_analyze 任务行并入 Intake 台账，修复 D-2）→ **PC 判断是否构成有效认知变更**（Proposal/Conflict/NO_CHANGE）。多上游（用户决策直达、工程回传、外部系统）全部经 Platform 路由，KB 不是 PC 唯一上游。

**请裁定**：① 修正方案是否成立；② 「认知候选信号」的契约应包含哪些最小字段；③ 触发阈值/策略的归属（Platform 配置 vs 跨服务契约评审）。

#### 裁决 4：ADR-04 — Intake 权威归属

统一接收壳（intake_record + intake_task，Platform 拥有）vs 各服务自建接收。**推荐**：Platform 统一（五层状态分离：Intake 接收/单服务任务/整体流程/人工审批/正式知识认知状态）。请确认，并审查「Platform 聚合状态与服务权威状态漂移」的防护是否充分（设计为：权威状态只在 KB/PC，Platform 仅存事件摘要与引用 + 超时状态校正协议）。

#### 裁决 5：ADR-06 拆分 — 数据所有权与 schema 边界

**06a（目标归属，请本次裁定）**：① PC=独立 `pc.*` 15 表；② KB 目标 schema=`kb.*`（现状 ai.* 七表混布，属边界瑕疵）；③ Platform=core/delivery/integration/governance/discovery/devenv/source；④ **Project Goal 行的归属**（现与 KB 同表，裁决 1 采用 (b) 后 Goal 是否仍留在 KB 表/是否随 kb.* 迁移/是否项目管理域独立建表——请给唯一推荐）；⑤ 物理隔离目标形态=三服务各自 schema + DB role GRANT（Phase 6），非立即三库。
**06b（迁移工程，单独审批，本次只确认框架）**：数据处置（随迁/丢弃/cut-over）、窗口与顺序（先于 PC 0045 Migration 以免 FK 改两次）、引用点全集、失败处置等六问。请确认 06a/06b 拆分是否成立，并补充 06b 必答清单的遗漏项。

### 第二批：机制（裁决 6~12）

#### 裁决 6：ADR-07 — 跨服务引用契约：四元组 vs 五元组

**背景**：PC-01A/01B 已裁决持久化引用 = `target_domain + target_type + target_id + target_revision_id` 四元组。服务化 V1.0 曾提出加 `service` 字段的五元组，被质疑与 target_domain 语义重复、且已渗入持久化（intake_task.result_ref / outbox payload）。

**修正方案（待确认）**：**四元组 = 唯一持久化引用本体**（与 PC-01B 单一同构，零修改）；`service` 仅作传输层路由元数据（事件信封 producer 字段、intake_task.target_service 列、HTTP 寻址），删除 result_ref 内嵌 service；Phase 7 后服务寻址由路由配置承载。请裁定，并评估边界情形：脱离信封的裸引用（审计存档）如何解析归属。

#### 裁决 7：ADR-03 — 异步机制

不引入 MQ；Transactional Outbox + DB 轮询（现状基建复用）；事件目录单一真源 + 评审准入。**推荐**：维持。请确认，并评估该决策在「三服务同库」与「Phase 7 拆库」两种形态下的失效模式。

#### 裁决 8（含 P1 缺陷 D-5）：ADR-08 — 服务间身份与权限传递 + 权限事实来源

**已定方向**：用户 JWT 端到端透传（KB/PC 自行 JWKS 验签）+ 服务身份凭据双通道；KB/PC 每请求执行四步自校验（服务身份→JWT→项目归属→动作级审批权）。
**待裁 subs 问题（D-5）**：KB/PC 执行「项目归属/角色」校验需要的权限事实（core.tenant_member / core.project_member / core.approver_grant）从哪来？候选：(a) V1 同库直读，Phase 6 DB role 隔离后给 kb/pc role 保留 core.* 只读 GRANT；(b) 经 Platform 鉴权 API 查询（每请求 +1 跳，符合自治原则但引入可用性耦合）；(c) Platform 在调用时附权限断言、KB/PC 抽验。**请给唯一推荐**（考虑：自治原则要求知识服务不依赖 Platform 业务库；但鉴权数据的实时性与一致性要求）。

#### 裁决 9：ADR-09 — Git-PG 一致性协议

统一「写幂等账本（pending→git_committed→completed/failed/abandoned 四态收敛）+ commit trailer 反查 + 后台 reconcile + hash 对账」协议；PC 侧以 PC-01B §12 为唯一基线零修改，KB 侧现状实现同构、补状态机与 reconcile。**推荐**：采纳。请确认，并审查 KB/PC 各自独立 Git 根 + 独立账本在裁决 1=(b) 后是否有遗漏的跨存储场景。

#### 裁决 10：ADR-11 — 事件版本兼容

event_version 整数递增 + additive-only + 消费者容忍未知 version + 新旧并存窗口（建议 ≥2 个发布周期）+ 事件目录单一真源评审准入。**推荐**：采纳。请确认。

#### 裁决 11：ADR-12 — 失败重试与人工恢复

投递层指数退避（2s 起/5min 封顶/8 次上限）→ 死信 + 人工 replay；服务内 Git 写账本独立计数（5 次上限，沿 PC-01B）；intake 人工重试端点；Platform 超时状态校正（不臆测终态）。**推荐**：采纳。请确认参数合理性（内部系统、无 SLA 压力）。

#### 裁决 12：ADR-05（建议从第三批提前）— 原始文件持久化与访问

现状事实：pm-api 无文件上传路由；前端经外部基座 lmms file-service 上传对象存储（RustFS，预签名下载，**实现不在本仓库，可靠性不可验证**）；文本类输入走 DB TEXT。设计：文件=对象存储 + intake 引用（file_ref+sha256）+ KB 凭服务身份拉取后重算 hash；文本内联。**待裁**：① 采纳该模式还是自建文件服务；② 原始文件保留策略（Team 级配置，默认永久+手动清理？）；③ lmms 依赖是否需要 Platform 侧副本兜底。

### 第三批：部署（裁决 13）

#### 裁决 13：ADR-10 — 部署独立策略

**推荐方案 A**：先领域独立（crate 边界+schema 归属+契约先行，V1 进程内端口调用、契约不变），物理拆分（Phase 7）由触发条件驱动：① 独立发布节奏成为需求；② KB 解析负载引发资源竞争且纵向扩容失效；③ 合规/部署形态要求物理隔离；④ 团队规模瓶颈。**请确认或反对，并审查触发条件阈值是否可操作；若反对请给出你推荐的目标部署形态与迁移路径。**

### 原则与红线确认（裁决 14~15）

#### 裁决 14：「知识服务自治原则」立法 + 两处精确化

拟新增原则：**KB-Service 与 PC-Service 必须拥有各自领域内完整自治能力。Platform 可接收请求、调度任务、聚合状态、展示结果，但不得成为知识服务运行所依赖的内容权威或内部业务协调中心；知识服务不得直接读写 Platform 或其他服务的业务数据库完成领域操作。**
两处精确化（待确认）：① 共享基建例外——Outbox/Inbound Event 表（integration.*）三服务经契约端口写入，Phase 6 DB role 为该例外开白名单；② 与裁决 8 联动——权限事实来源若选「Platform 鉴权 API」则属允许的控制面依赖（非业务数据依赖）。请确认原则文本与两处精确化，或给出你的版本。

#### 裁决 15：服务安全红线

拟新增：**任何网络边界上的 KB/PC 服务调用，在用户身份认证、服务身份验证、Team/Project 权限校验未全部通过之前，不允许进入生产环境**；服务间认证缺失（R-1）定级为「阻塞 Phase 6/7，不阻塞 Phase 0-5 领域独立与契约设计」。请确认。

### 修订共识打包批准（裁决 16）

以下 7 项为内部评审已达成共识的修订（V1.0→V1.1），请整体确认或指出反对项：
① ADR-02 改三分法（裁决 3）；② ADR-06 拆 06a/06b（裁决 5）；③ ADR-07 改四元组本体（裁决 6）；④ 安全红线入 §16.3（裁决 15）；⑤ 自治原则入 §5.2（裁决 14）；⑥ P1 表述修订：P2 接口响应改纯受理语义（删除「同步等待 AI 生成提案」矛盾）、未登记事件 `pc.decision.submitted` 补入目录或改用已登记事件、DB role 对 outbox/inbound_event 白名单、§19「零修改/无调整」口径改 additive 修订登记制、架构图清理无效节点、§14.3 重试时长勘误；⑦ ADR-05 提前至第二批（依赖倒挂：其 file_ref/source_ref 设计是 Intake DDL 组成部分）。

## 六、输出格式要求

```
# 专家裁决评审报告
## 一、逐项裁决（裁决 1~16）
每项：【结论：采纳推荐 / 反对+替代方案】→ 理由 → 对既有裁决的影响 → 实施成本 → 风险与缓解
## 二、五类结论清单
1. 可直接批准的架构原则
2. 需用户正式裁决的 ADR（附你的推荐票）
3. 必须修订的设计缺陷（P0/P1 分级 + 落点）
4. 仅需补充说明的文档问题
5. 当前无法验证的实现事实
## 三、新发现问题（提示词未覆盖的）
## 四、总裁决建议（通过 / 有条件通过 + 条件清单 / 不通过）
```

## 七、背景补充：项目内 ADR 批次计划（供参照，非结论）

第一批 ADR-01/02/04/06 → 第二批 ADR-03/05/07/08/09/11/12 → 第三批 ADR-10。你的裁决票将按此批次向用户提交。