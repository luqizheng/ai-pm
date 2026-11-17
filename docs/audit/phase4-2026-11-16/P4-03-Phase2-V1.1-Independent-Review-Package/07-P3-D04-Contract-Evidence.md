# 07 — P3-D04 契约兼容证据（E 节）

**证据基准**：master `e05693afdc0a9ae534735c9b6f67117a6b92de8d`。本包只读取证，未修改任何文件。
**定位**：本文件提供 P4-03 阶段 4 兼容性核查（C5/C6）所需的**当前事实基线**；不预作「兼容性通过」结论——该结论属阶段 4 核查 + 阶段 5 冻结评审强制准入（P4-03 V1.1 §10）。

## 1. P3-D04 当前正式设计及版本（事实）

- **冻结基线**：`docs/audit/ia-2026-11-16/P3-D04-02-第一批目标领域模型设计-v0.6.md`（V0.6，R10~R38 全列修订记录；V0.1~V0.5 为历史版本原样保留）。
- **冻结批准令**：`P3-D04-02-Step12冻结批准令-v1.0.md`（2026-11-17 批准）：「正式批准 P3-D04-02 Step 1 + Step 2 冻结」「冻结范围以 V0.6 为基线」「本批准不包含 Step 3~5」。Step 2 = 跨领域引用模型（V0.6 L253「Part 2（Step 2）：跨领域引用模型（O-02 / O-09）」）→ **引用模型条文（含 Pinned/LatestPublished）已随冻结生效**。
- F-01~F-04 已登记强制闭合项（批准令 §三）：F-01 Decision 五元组物理载体→Schema/API；F-02 plan/adr 收敛→S4；F-03 Evidence verification→后续阶段；F-04 AI 提案物化幂等→Step 3。效力规则：「登记不代表已完成…如发现真实冲突应提交正式影响分析；不得仅因进入后续实施阶段就自动重开 Step 1/2」。
- 两句效力边界：边界一=adr 存量引用仅历史兼容，禁新 adr 域引用；边界二=未来领域流程时间语义由该领域独立裁决，不自动重开本次冻结。
- **Step 3 未启动**（批准令 §六：「尚未启动，待依赖核查」；依赖核查报告 V1.0 已产出但无顺序裁决/启动批准文件）。
- ⚠️ **时效性事实**：P4-03 V1.1 §10-C6 行写「P3-D04-02 引用模型设计稿（**现行 V0.4 定点修订稿待复核**：Pinned/LatestPublished 等）」——该表述已过时：现行基线为 **V0.6（冻结）**。两文件同日落盘，先后顺序无法从仓库证据确立（登记至 11 号文件 §3）。

## 2. P3-D04-02（V0.6 冻结）引用模型正式条文摘录

### 2.1 DomainRef 四字段契约（V0.6 §2.1，「冻结，不扩展第五字段」）

| 字段 | 语义 | 冻结约束 |
|---|---|---|
| target_domain | 被引用对象所属业务域 | 域标识见注册表（product_plan 永久排除【冻结 Q-PC-13】） |
| target_object_type | 被引用对象类型 | 必须在目标域类型注册表内 |
| target_object_id | 被引用对象稳定身份（唯一契约字段）：UUID、全局唯一、创建后不可变 | 指向对象身份不指向 Revision；解析只按此字段定位，业务键不参与解析（R10） |
| **revision_context** | 引用契约的版本选择语义（显式模式声明 §2.2） | 无模式声明=非法契约 |

**命名映射（O-09，开放）**：契约抽象字段 `revision_context` ↔ 存储字段 `target_revision_id`（+`revision_missing_reason`）；台账 R-07 裁决：「字段名称映射…须统一（O-09）」。→ **P4-03 V1.1 §7.3 写「revision_context」与冻结契约一致；代码用 target_revision_id 属存储层命名；两者非冲突，但统一命名是未闭合开放项**。

### 2.2 版本语义（V0.6 §2.2，冻结）

- **Pinned**：`{mode: pinned, revision: <明确 Revision 标识>}`——解析到钉扎的具体 Revision；不可达→REV_MISSING。
- **LatestPublished**：`{mode: latest_published}`——**动态选择器**：每次解析返回目标域正式化机制下的当前有效 Revision（认知域=当前 Publication 成员）；具体 Revision 可随目标演进变化，选择器语义不变。
- LatestPublished 必须显式声明、不得隐式推断；**只允许用于非正式依据**的展示/聚合/统计；**不得成为确认、审批、发布或正式认知的依据**；依据类引用（evidence/basis）强制 Pinned。
- 三层规则（R12）：第 1 层引用类别→允许的版本模式（evidence/basis/association 仅 Pinned；display/aggregation 可 Pinned 或 LatestPublished）；第 2 层版本选择模式；第 3 层解析操作意图（READ/NAVIGATE/ITERATE）——intent 不改变版本选择，mode 不改变权限判定。
- 引用稳定性原则（§2.9，R9+R24，随 Step 2 冻结）：正式引用一经确认/发布，目标身份与版本语义必须稳定；版本更新/替代/废弃/权限变化/删除**不得静默改变原引用含义**；任何重定向须引用所属领域显式变更+确认。
- 四分支分流（R37）：权限失败+不可见→UNREACHABLE(NOT_FOUND)；权限失败+可披露→DENIED；权限通过+Revision 缺失→REV_MISSING；权限通过+对象不存在→NOT_FOUND。
- 三层可见性（R34）：引用锚（永久保留）/目标身份（NOT_FOUND 时 target 全 null）/目标内容（不满足读取条件不返回）。
- 逐边授权（R23）：每条引用边、每个目标对象独立权限闭环，不继承上一跳。跨域操作须同时满足：调用方授权+目标领域授权+目标对象当前状态约束（红线二：引用关系本身不构成授权凭证）。

## 3. 当前代码事实（对应 §2 条文的实现态）

| V0.6 条文 | 代码现状 | 差距定性 |
|---|---|---|
| 四字段契约 | `domain_ref.rs:19-29` 已实现（第 4 字段=存储名 target_revision_id，Option，wire 冻结） | 实现（命名映射 O-09 未统一） |
| Pinned/LatestPublished 模式声明 | **无实现**（契约/代码无 mode 概念；registry.rs:27 仅注明 ReadOnly 通道不钉 Revision） | DESIGN_ONLY |
| REV_MISSING/DENIED/NOT_FOUND 四分支 | 无实现（现有错误=NotFound/Invalid/Forbidden 直返） | DESIGN_ONLY |
| 三层可见性/逐边授权 | 无 resolver 级实现；projc resolve_endpoint 做通道+存在性+归属校验（无权限闭环） | Partial（O-01） |
| 引用类别体系（evidence/basis/…） | pc/projc.source 有 source_kind/evidence_type 词表（不同概念体系） | 未对齐（待 C5 核查） |
| DomainRef「唯一持久化引用结构」 | 事实偏差：pc.relationship/projc.relationship 的 **ext_* 四元组列**与 pc.source 等是并列的持久化引用承载（domain_ref.rs 结构仅用于 outbox payload）；V0.6 契约与现役多载体关系的收敛未裁决 | ⚠️ 登记（11 号文件 §5） |
| product_plan 永久排除 | registry.rs PermanentlyExcluded ✅ | 一致 |
| adr 存量（边界一） | registry.rs `adr` 域值仍 Open——**现役代码可继续创建 adr 域引用**，与边界一「不得据此创建新的 adr 域引用」存在待收敛差距（S4 收敛前现状对照条款） | ⚠️ 登记 |

其他代码事实：O-01 resolver=Partial（reference_targets.probe in-scope/anywhere；resolve_reference 通道裁决；完整跨 schema resolver 归 P3-10，R-04④=Partial 冻结令明文）；O-02 引用完整性=**仅写入时点校验**（SEC-P2-2 存在性探测、kb_document 强制钉修订、组合键即时校验；存续期失效对账无实现，代码注释「另批」）；O-09 命名未统一。

## 4. 逐问回答（任务书 E 节八问）

判定标记：`CONTRACT`=冻结契约有明文；`IMPL`=代码已实现；`NOT_IMPL`=无实现；`UNKNOWN`=无证据。

| # | 问题 | 回答（事实） |
|---|---|---|
| 1 | PC-P 和 PC-J 是否可以被稳定区分？ | **当前不能。** 代码/DB 中不存在 PC-P/PC-J 概念；pc 域全部对象 project 作用域（J1/F-3）；协议域 `pc`（ProductCognition）现路由到 project 作用域表 pc.cognition_object（reference_targets.rs:53-77）。区分能力是 P4-03 §2.4 L2 两案（α/β）+五项闭合要求的裁决对象，均未裁决未实现。`NOT_IMPL` |
| 2 | 引用是否指向不可变版本？ | **契约**：依据类引用强制 Pinned（CONTRACT，V0.6 §2.2）。**代码**：被指对象侧不可变性 pc=触发器完备（trg_pc_revision_immutable/vc_immutable 等）；**projc.cognition_revision 无不可变触发器**（J5）→projc 侧「不可变版本」的物理保障不完整。引用值本身（target_revision_id）可携带修订标识（IMPL，字段级）。综合：契约成立、pc 侧成立、projc 侧部分成立（依赖服务层纪律而非触发器） |
| 3 | LatestPublished 是动态解析，还是保存解析后的版本？ | **契约**：动态解析（动态选择器，每次解析返回当前有效 Revision；CONTRACT）。**代码**：无 LatestPublished 实现；唯一近似=任务知识锚 COALESCE(pinned_revision_no, latest)（run_context.rs:466-482）——读取时动态解析且不持久化解析结果，但属 task_knowledge 通道非 DomainRef。→ 契约=动态；实现态 `NOT_IMPL`（DomainRef 通道） |
| 4 | 下游能否依赖 LatestPublished？ | **契约**：不得作为确认/审批/发布/正式认知依据；仅限非正式依据的展示/聚合/统计（CONTRACT）。**代码**：无该限制的实现（无 mode 校验）→ 依赖约束当前仅存在于契约文本，无运行时强制。`NOT_IMPL` |
| 5 | 新版本发布是否会改变已有消费关系的状态？ | **契约**：不得静默改变原引用含义（引用稳定性原则 CONTRACT）；Pinned 引用不受影响，LatestPublished 的解析结果会变（定义使然）。**代码**：无 update-pending/感知机制（0 命中）；pc 发布 outbox 事件存在（PC_COGNITION_VERSION_PUBLISHED）但无消费引用方。→ 「已有消费关系」当前不存在（零负载），行为无从实证：`UNKNOWN`（运行时行为无案例）+契约有明文 |
| 6 | 跨域引用是否自动获得读取权限？ | **否（双源一致）**。契约：红线二+权限闭环三要素+逐边授权（CONTRACT）。代码：projc resolve_endpoint 仅通道/存在性/归属校验、无授权授予；0026 implements 种子注释明文「不构成跨域授权」；registry ProjectCognition=ReadOnly（红线二）。`IMPL`（无自动授权=现状成立） |
| 7 | 被引用对象不存在时，是否保留历史记录？ | **引用记录侧**：pc/projc 认知对象禁删（trg_pc_object_delete 等触发器）→「对象消失」主要形态为 deprecated/superseded（保留行）；source/relationship 行亦无删除路径（pc 侧）。**契约**：引用锚永久保留（R34 三层可见性 CONTRACT）。**缺口**：引用目标存在性的存续期对账无实现（写入时点校验 only，注释「另批」）。→ 保留=成立（结构上无删除路径）；对账/失效检测 `NOT_IMPL` |
| 8 | 历史引用是否可以还原当时的身份和版本？ | **身份**：可以——target_id 为不可变 UUID（契约 R10+IMPL）。**版本**：引用值携带 target_revision_id（IMPL）；被指版本内容可还原性 pc=成立（revision 不可变触发器+content_hash）；projc=部分（无触发器，依赖纪律）。**警告**：kb 侧 memory_ref 的 XOR 语义（有 revision 或有 missing_reason）意味着「无修订引用」仅存原因不存版本——该类历史引用无法还原版本（契约允许、语义为显式缺失）。综合：`IMPL`（pc 侧完整；projc 侧结构弱化；缺失型引用按定义不可还原） |

## 5. P3-D04 归口事项与 P4-03 的接口（现状，非结论）

| 事项 | 状态（现状事实） | 与 P4-03 的接口 |
|---|---|---|
| D4 REPLACE（CognitionPage→projc、ai.project_knowledge REMOVE） | 方向已裁决保留；五项完成条件未满足；收口归 Step 5/O-05；代码零执行（旧通道在役） | C1：正交，P4-03 DR 不预设 REPLACE 细节 |
| D6-⑴ 项目知识并入 ProjC | 登记待裁决（归 O-05 内闭合） | C2：正交 |
| D6-⑶ AI 上下文注入源切换 | 登记待裁决（=REPLACE 完成条件 2）；现状注入源=ai.project_knowledge（ai_context_items.rs:1159-1170） | C3：P4-03 落地后增量加入 PC-P 授权面内容 |
| O-01 resolver | Partial（P3-10 归口） | C5①②③ 依赖其结论 |
| O-02 引用生命周期语义 | Part 2 已冻结（V0.6）；**存续期对账实现未落地** | C5④⑤ 的核查基准=V0.6 冻结文本+实现差距 |
| O-09 命名映射 | 未统一（revision_context↔target_revision_id） | C5/DR-02 起草时须显式处理 |
| O-06 Conflict 生产登记入口 | 未闭合（R-06 三层区分：模型 IMPLEMENTED/入口未闭合） | P4-03 DR-06 需与其协调（projc conflict 生产入口不存在，pc.conflict Legacy 无写入者） |
