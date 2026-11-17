# 03 — 八项定向修订逐项证据矩阵

**证据基准**：master `e05693afdc0a9ae534735c9b6f67117a6b92de8d`；代码行号以该 HEAD 为准。本包只读取证，未修改任何文件。

**使用纪律（承任务书）**：本矩阵是开发侧证据整理；「开发自评」不代替独立审查。文档中新增术语/表格**不自动等于**问题已实质闭合——每项的「实质闭合判定权」在独立审查者与人工裁决。

**共性登记**：
- 「原始问题」栏为 RECONSTRUCTED（依据 V1.1 正文闭合标签与版本记录自述反推）；审查意见原文 `NOT_IN_REPO`（见 04 号文件）。
- 「决议依据」栏：八项的共同依据 = **阶段 2 设计审查意见（2026-11-17，PASS WITH CONDITIONS）**，该文件不在仓库（会话工件）；各项修订当前均为 **PROPOSED / 待 V1.1 定向复审**，无任何一项已被裁决 ACCEPTED。
- P4-03 为阶段 2 设计文档，其修订的「实质闭合」指**设计条款层面的闭合**；不要求也不应有对应代码实现（实施授权不存在）。故「代码证据」栏给出的多为：修订条款所依据/所描述的代码现状是否被准确引用，以及目标态是否被正确标记为 DESIGN_ONLY。

---

## ① DomainRef 三层身份映射（新 §2.4，闭合审查 P0-1）

| 字段 | 内容 |
|---|---|
| 原始问题（RECONSTRUCTED） | V1.0 将「PC-P/PC-J 同属 pc 域」（§2.1）与「注册表新增 PC-P 目标域枚举值」（§9.1-H2）并置，混淆业务领域（L1）/协议域标识（L2）/物理存储（L3）三个层面 |
| V1.0 原位置 | §2.1 与 §9.1-H2（据 V1.1 §2.4 开头自述反推；V1.0 原文不在仓库） |
| V1.1 修改位置 | 新增 §2.4（01 文件 L92-115）；§9.1-H2 行改为指向 §2.4 两案（L432）；§13 DR-02 增列「DomainRef 协议域标识方案（α/β，§2.4）」（L533） |
| 修改内容 | L1/L2/L3 三层定义表；L2 两案：α=新增协议域枚举值（如 `pc_product`，倾向，PROPOSED）/ β=单域值+作用域判别；五项闭合要求（身份唯一性/Resolver 无歧义路由/权限闭环识别目标作用域/注册表登记/钉扎引用双携带防混淆）作为 DR-02 验收判据 |
| 代码证据 | `crates/kb-pc-contracts/src/registry.rs:35-58` TargetDomain 枚举现 9 值，`ProductCognition`→wire `pc`、通道 Open，**无 `pc_product` 值**（α 需 additive 加值）；`crates/kb-pc-contracts/src/domain_ref.rs:19-29` 四字段结构（第 4 字段为 `target_revision_id`）；`crates/pm-application/src/reference_targets.rs:53-77` 现 `pc` 域值经 `domain_target` 路由到 **`pc.cognition_object`（project 作用域表）**——即现协议值与 PC-P 概念并不对应，佐证「L1 相同≠L2 相同」的分离必要性；`migrations/0025_projc_domain_bootstrap.sql:7-10` 头注「DomainRef target_domain 与 relationship_type.owner_domain 分离」先例 |
| 决议依据 | 阶段 2 审查意见（会话工件，不入库）；α/β 择一本身是 DR-02 裁决点，本文不裁决 |
| 未闭合事项 | α/β 未裁（V1.1 明示「均为候选」）；五项闭合要求尚未经独立复核；C5 契约级核查（阶段 4）未执行；β 案涉 H2 冻结契约扩展须过 O-09 映射重议（§9.3-I4，L460） |
| 开发自评 | **部分闭合**——三层分离的概念框架与两案影响分析已在文档实质完成；但协议值方案本身待裁、五项要求待复核，不宜视为「已解决」 |

## ② identity 创建路由分流（§7.5 + I1 重命名，闭合审查 P0-2）

| 字段 | 内容 |
|---|---|
| 原始问题（RECONSTRUCTED） | V1.0 §9.3-I1 将 identity 链变更表述为单点「改锚」，未区分 D-4 产品身份起草路由（→PC-P）与 PC-J 项目特有/派生/差异对象的通用草稿通道（保留），路由边界不清 |
| V1.0 原位置 | §9.3-I1（据 V1.1 §7.5 自述「§9.3-I1 的准确表述=『identity 创建路由的作用域分流与迁移』，非单点改锚」反推） |
| V1.1 修改位置 | §7.5 路由表+D-4 拆分答案+P3-11 约束保护段（L288-300）；§9.3-I1 重写（L457）；§11.1 identity 链行（L490）；§13 DR-01/DR-02 职责划归（L532-533） |
| 修改内容 | 五行路由表（product_profile/vision→PC-P 提案链；PC-P 候选→PC-P 提案链；project_original→现有项目草稿通道原样保留；derived/variant→现有通道+创建时 provenance 强制校验守卫；局部差异→variant/divergence）；D-4 链产出限定→整链改路由 PC-P，PC-J 剩余创建不走 identity 链 |
| 代码证据 | D-4 现状：`crates/pm-application/src/identity.rs:140-207` `propose_pc_object`（profile/vision 走 `pc::upsert_object_draft`→`create_proposal`→`submit_proposal`，origin_kind=`ai_agent`）；`identity.rs:211-339` `apply_identity_drafts`（审计 `identity_synthesis.applied`）；`crates/pm-ai/src/identity_synthesis.rs:53`（开关 `AI_PM_IDENTITY_SYNTHESIS`，off→501）；通用草稿通道：`crates/pm-application/src/pc/object.rs:60-65` `upsert_object_draft`（与 identity 链自始同口、独立存在）；单例骨架：`bootstrap.rs:323-333`→`pc/object.rs:322-357`——V1.1 §8 引用「bootstrap.rs:325 现状」**行号准确**；PC-P 提案链：**不存在（DESIGN_ONLY）** |
| 决议依据 | 阶段 2 审查意见（会话工件）；分流切换属 I1 目标态登记，实施=独立任务包（§9.3-I1） |
| 未闭合事项 | 路由分流为实施期变更（须 PC-P 链先就绪，§11.1）；与 P3-D04 O-05（identity 收口）/O-06 协调未做（§9.3-I1 明示）；provenance 校验守卫无实现 |
| 开发自评 | **部分闭合**——目标态路由表齐备且对现状代码的引用（同口通道、bootstrap 行号）经核属实；切换本体属实施期，符合「阶段 2 不实施」定位 |

## ③ 消费授权双语义分离 + 授权契约六问（§7.8，闭合审查 P1-3）

| 字段 | 内容 |
|---|---|
| 原始问题（RECONSTRUCTED） | V1.0 存在把 P4-02 Link 写操作权限（双侧交集）与「授予项目成员读取 PC-P」混同的风险——有权建立 Link ≠ 已授权认知读取 |
| V1.0 原位置 | §7.8 授权形态段（据 V1.1 L342 自述「V1.1 闭合审查 P1-3」及「G1（候选，重新定位）」字样反推） |
| V1.1 修改位置 | §7.8：双语义分离段（L342）、G1 重新定位/G2 两案（L344-348）、**授权契约六问表**（L349-358）；§13 DR-08 增「G1/G2 择一；授权契约六问；AI 四条边界」（L539） |
| 修改内容 | 显式声明两语义独立、消费授权必须是可审计显式决定、不得由 Link 存在隐含推导；六问推荐答案（授权主体=产品侧 PO∪TA；V1 不强制双侧确认；范围=已发布内容整域；撤销不需 Project 确认；Link ended 必然撤销；授权⊆Link 存续——允许「Link active 但授权已撤销」态） |
| 代码证据 | **消费授权零实现**：`consumption_grant` 全仓 0 命中；`product_cognition.consumption_granted/.revoked` 审计事件 0 命中（DESIGN_ONLY）。现权限底座（被引用为基座的部分均属实）：`crates/pm-domain/src/tenant.rs:19-58`（Owner/Admin/Member）；`crates/pm-application/src/approver.rs:46-101`（Lead=`project_member.role='owner'`，`ensure_governance_authorized`=Lead OR `core.approver_grant`）；正式写门 `pc/mod.rs:151-176`（真人+A3）；**Product Owner 角色不存在**（全仓 0 命中，P4-01 DR-06 仅设计冻结） |
| 决议依据 | 阶段 2 审查意见（会话工件）；G1/G2 择一=DR-08 裁决点 |
| 未闭合事项 | G1/G2 未裁；六问推荐答案全部 PROPOSED；授权对象（Link/授权记录）无任何实现；P4-02 Link 本身零代码（见 09 号文件）——G1「挂接 Link 生命周期」的目标宿主尚不存在 |
| 开发自评 | **部分闭合**——文档层面的语义分离与契约六问实质完成；其闭合质量依赖 DR-08 裁决与后续实现，当前不应视为已解决 |

## ④ 证据快照授权边界（§7.8-E，闭合审查 P1-4）

| 字段 | 内容 |
|---|---|
| 原始问题（RECONSTRUCTED） | R5 反馈流中，产品侧若凭 evidence DomainRef 实时读取项目对象，将形成产品侧对项目认知的未授权读取通道（违反 §2.3 铁律 2 / H3 双向不穿透） |
| V1.0 原位置 | §7.6/§7.8 证据相关段落（据 V1.1 L360「H3 在数据读取层的落实」反推） |
| V1.1 修改位置 | §7.8-E 证据形式四行表（L360-369）+推荐形态段；§13 DR-07 增「证据快照契约（§7.8-E）」裁决点（L538） |
| 修改内容 | 四类证据的产品侧可读性分级：提案正文（进链后可读）；**证据快照**（提交时固化内容副本+来源钉扎+内容哈希，不可变，提交行为=授权固化）；原始 DomainRef（仅溯源元数据展示，**不开通实时读取**）；未授权项目内部对象（不可读，无任何通道） |
| 代码证据 | 现状证据面：`pc.evidence`/`projc.evidence` 表（0001:3182、0025:268）+ `register_source`/`register_evidence`（`pc/evidence.rs:27-135/166-266`；`evidence_hash` 必填、`verification_status`、`confidence`；`revision_missing_reason` XOR CHECK 0001:3410）——**快照物化机制不存在**：无「提交时内容副本固化」表/逻辑（DESIGN_ONLY）；`evidence.rs:210-231` 注释明示「写入时点；存续期对账另批」——即现存证据校验为写入时点，快照级不可变副本是新增概念；检索侧元数据契约 `kb-pc-contracts/src/retrieval.rs:541-570`（RetrievalEvidence/AuthorityStatus）可作读面元数据先例 |
| 决议依据 | 阶段 2 审查意见（会话工件）；快照契约=DR-07 裁决点 |
| 未闭合事项 | 快照的物理载体/不可变性保障无设计与实现细节（仅概念定义）；与 F-03（Evidence verification，P3-D04-02 Step12 批准令强制闭合项）的关系未显式对齐 |
| 开发自评 | **部分闭合**——授权边界规则在文档层实质成立；快照机制本体留 DR-07/实施 |

## ⑤ 类型/作用域/来源关系三维度分离（§8 重写，闭合审查问题 5）

| 字段 | 内容 |
|---|---|
| 原始问题（RECONSTRUCTED） | V1.0 将「类型级默认作用域」与「original/derived/variant 来源分类」并入一套字典语义，两概念未正交分离 |
| V1.0 原位置 | §8（据 §8 开头自述「V1.0 曾把…并入一套字典语义，修正如下」反推） |
| V1.1 修改位置 | §8 全章重写（L398-421）；§13 DR-05 增「三维度正交性」、DR-02 增「字典 allowed_scopes/default_scope」（L536/L533） |
| 修改内容 | 三维正交表：A=对象类型（现有 14 类不变，归 O-03）；B=作用域 `product`/`project`（每类型 allowed_scopes+default_scope，创建时定死、不可静默迁移）；C=来源关系 provenance（PC-J 必标 project_original/derived/variant，PC-P 固定 product_authoritative）；分配表（⭐双作用域类型 6 组）；单例骨架迁移登记（bootstrap 变更=实施项） |
| 代码证据 | 字典现状：`pc.object_type` 18 行种子（0001:10255-10272：14 active + 4 snapshot 已 0027 废弃），**列结构无 scope/provenance/allowed_scopes 任何载体**（0001:3220）；`pc.cognition_object` 无 `product_id` 列（migrations 全量 grep `product_id` 零命中）——三维 B/C 均为目标态增量（DESIGN_ONLY）；运行库实证：`pc.object_type` active=14、deprecated=4（`aipm_mig_full` 只读查询）；字典治理归 O-03（P3-D04 开放项）与 V1.1 §8「字典治理归 O-03」表述一致 |
| 决议依据 | 阶段 2 审查意见（会话工件）；分配表=DR-02 裁决点 |
| 未闭合事项 | 分配表未裁；作用域/来源元数据的 DB 载体不存在（F1/F2 未裁）；「显式迁移=新对象+deprecated」的迁移通道无实现 |
| 开发自评 | **部分闭合**——正交概念框架与分配表实质完成；载体与裁决待后 |

## ⑥ PC-J 机制表述收紧（§2.1/§7.2/§11.1）

| 字段 | 内容 |
|---|---|
| 原始问题（RECONSTRUCTED） | V1.0「PC-J 保留现有机制」的表述可能被读作「零结构/零契约变化」，掩盖 provenance/作用域/守卫/路由分流等增量的契约影响 |
| V1.0 原位置 | §2.1 PC-J 行、§7.2 第 2 项、§11.1「现有 pc.*」行 |
| V1.1 修改位置 | 三处统一加限定：「**保留项目级身份/版本/提案/人工确认/发布的基本机制**，增加来源关系、作用域标识、消费/派生边界约束与 identity 路由分流等增量…——『基本机制保留』≠『零结构与契约变化』」（L64/L250/L489）；§13 DR-02/DR-05 相应承接 |
| 修改内容 | 表述收紧：明确增量为「独立实施任务包设计」的对象，非零变更 |
| 代码证据 | 现状机制面（被「保留」的对象）核实属实：18 表（0001:3021-3453+0007/0009）、8 态提案 CHECK（0001:3258）、十四步发布事务（pc/publish.rs:68-75）、不可变触发器组（0001:6966-7078）、A3 门（pc/mod.rs:151-176）；增量载体（provenance/scope 列、守卫）现状**零存在**——与「增量=新增」表述一致 |
| 决议依据 | 阶段 2 审查意见（会话工件） |
| 未闭合事项 | 增量清单本身明示留实施任务包细化（符合阶段定位，非遗漏）；但审查者应确认「语义收窄对既有 pc 契约（如 close 发布范围）的影响」已在 I2 登记而非散落 |
| 开发自评 | **已闭合（表述层面）**——三处限定一致且与代码现状无矛盾；实质增量设计本就归属后续阶段 |

## ⑦ 方案 B 缺陷描述精确化（§3/§4/§5/§6.1）

| 字段 | 内容 |
|---|---|
| 原始问题（RECONSTRUCTED） | V1.0 对方案 B 的缺陷刻画过度（「项目差异完全无法表达」），未承认反馈提案/交付工件两条间接表达通道，论证精度不足 |
| V1.0 原位置 | §3 方案 B、§4 矩阵、§5 场景 B 列、§6.1 论证二 |
| V1.1 修改位置 | 四处统一精确化为：B 并非使项目差异**完全无法表达**（反馈提案与交付工件仍可间接表达），而是使其**无法在产品认知域内以项目为主体建立独立、持续演进的正式认知对象**——表达即升级，项目侧不留存、不沉淀、不可追溯演进（L137/L161/L181/L184/L188/L216） |
| 修改内容 | 论证精度修正；方案比较结构未动（符合「不重写方案比较」约束） |
| 代码证据 | 方案比较为设计论证，无直接代码对应；关联事实核实：S2 评估中「交付域完整」依赖的现状（projc 独立 schema 与服务层，0025/0026+projc 模块 7 文件 3963 行）属实；「入 ProjC=违反 R-02/R-03 域界」引用的裁决原文见 08 号文件 §b |
| 决议依据 | 阶段 2 审查意见（会话工件） |
| 未闭合事项 | 无已知未闭合项；A/B/C 比较本身待阶段 3 DR-01 裁决（非本修订范围） |
| 开发自评 | **已闭合（文本层面，待独立复审确认）** |

## ⑧ 生命周期状态机对齐（§7.9，闭合审查问题 8）

| 字段 | 内容 |
|---|---|
| 原始问题（RECONSTRUCTED） | V1.0 生命周期矩阵存在「为冻结引入新状态值/混用 failed 与 withdrawn」等与既有状态机不一致的风险 |
| V1.0 原位置 | §7.9 |
| V1.1 修改位置 | §7.9 矩阵（L382-392）+「状态机对齐声明（V1.1，闭合审查问题 8）」（L394）；§13 DR-09 增「状态机合法转移集」（L540） |
| 修改内容 | 声明不引入任何新状态值；withdraw 转移集=（draft/submitted/**approved**→withdrawn）自称已被 close_report 批量撤回先例覆盖；failed（CAS 失败终态）与 withdrawn（撤回终态）严格区分不混用；其余情形走「状态保留原样+主体态守卫冻结」；系统级撤回授权=实施登记项 |
| 代码证据 | 8 态 CHECK：0001:3258（`draft/submitted/approved/rejected/withdrawn/expired/applied/failed`）。**现状转移全集（实证）**：draft→submitted（proposal.rs:810-811）；submitted↔approved↔rejected 暂记互改（proposal.rs:1063-1066）；**draft/submitted→withdrawn**（proposal.rs:1181-1183；close 批量撤回 analysis_report.rs:615-635，WHERE 子句同为 `status IN ('draft','submitted')`）；**approved→applied**（publish.rs:597-600）；**submitted/approved→failed**（publish.rs:277-281 CAS 补偿）；**approved→withdrawn 转移在全部代码中不存在**；expired 无生产写入路径（仅 CHECK 与过滤排除） |
| 决议依据 | 阶段 2 审查意见（会话工件） |
| 未闭合事项 | **⚠️ 发现一处 V1.1 现状陈述与代码事实不符（须复审处置）**：V1.1 L390 称 close_report 批量撤回先例「已覆盖含 approved 的未决态撤回」——代码实证：close 先例仅撤 `draft/submitted`（analysis_report.rs:615-635），approved 提案在 close 被**发布为 applied**（analysis_report.rs:588-607），approved→withdrawn 无任何代码先例。该陈述属「把目标态写成已覆盖的现状先例」。另：系统级撤回通道/清扫主体登记为实施项属实（`DISPATCH_SERVICE_PRINCIPAL` 定义于 `run_dispatch/lease.rs:214`，现用作发布 CAS 失败审计的系统身份（pc/publish.rs:303、projc/publish.rs:304），**dissolve_team 未使用它**（team_onboarding.rs:1232 起以 owner 主体落痕且不清扫认知数据）——「P4-02 C-05 先例」作为清扫身份先例成立（设计文档条文），作为代码先例尚无 dissolve 路径 |
| 开发自评 | **部分闭合**——四分原则、状态保留原样+守卫冻结、failed/withdrawn 区分均与代码现状一致且实质收紧；但其中一处先例引用与代码不符，须由复审裁决：要么修正措辞（approved 在 close 先例中以「发布」终结、withdrawn 先例仅覆盖 draft/submitted；dissolve 场景的 approved 处置为新设计），要么为 dissolve 显式登记 approved→withdrawn 为新增合法转移（状态机扩展须走 DR-09 明示裁决）。本包不代为裁决，仅登记事实 |

---

## 附：版本记录所列三项「另按审查意见增补」

| # | 增补 | V1.1 落点 | 实质状态 | 开发自评 |
|---|---|---|---|---|
| 增 A | S9 派生跨版本冲突场景 | §5 表 S9 行（L188）+ §5.1 七问闭合（L192-204，DR-04/05/06 联合验证场景） | 七问全部有目标态答案（update-pending 覆盖派生引用；再基线不自动产生候选；来源钉扎按修订携带；双钉扎可比；AI 仅起草；双侧各走各门；拒绝再基线 V1 默认不通知产品侧） | 已闭合（设计层）；全部 PROPOSED |
| 增 B | DR 职责边界重划+五要素 | §13 表（L526-540）：九 DR 各含「直接裁决问题/明确不裁决/上游冻结约束/依赖 P3-D04/影响的其他 DR/输入与推荐」 | 五要素齐备；职责重叠消除（如 DR-01 不再含授权执行模型→DR-08） | 已闭合（结构层）；DR 正文=阶段 3 交付 |
| 增 C | C5/C6 契约级核查=阶段 5 强制准入 | §10（L475-478）+§0 第 5 条+§14 第 3 条 | C5 五项清单+核查协议明文；**注意：C6 行写「现行 V0.4 定点修订稿待复核」，与仓库现状不符**——P3-D04-02 基线已为 **V0.6 且 Step1+2 已冻结**（Step12 批准令 v1.0，2026-11-17）；Pinned/LatestPublished 已是冻结契约条文，四问可对冻结文本作答（见 07 号文件） | 增补本体已闭合；C6 引用对象已过时（登记至 11 号文件 §3） |
