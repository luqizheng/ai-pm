# 专家稿（§五~§十二）× 代码现实评审 V1.0

> **评审对象**：专家送达的检索设计第二批（Policy Validator 六项校验 / 多路召回+RRF+Reranker / 五维独立评分 / Evidence 统一结构 / 服务边界五端点 / Outbox 索引管线八要求 / S1~S8+RT-001~015 / 九条非目标 / 冻结原则 / 关系维护权）。
> **评审前提**：**专家不了解本仓代码**。评审方法 = 逐条对照代码现实做差分，区分「已建成 / 冲突需调和 / 真增量 / 遗漏 / 需修正」五类，并核出专家不可能知道的新事实。
> **被评审稿的收编状态**：该批已于 `pc-kb-retrieval-technical-design-baseline-v1.md`（V1.1）整体收编；本评审是对那次收编的独立复核，结论 = **V1.1 主体成立，但有 4 处需勘误**（§四），其中 1 处是本次评审新发现的**语义表归属错误**。
> 证据行号均指 `templates/ai-pm` 仓 `migrations/0001_baseline.sql`（下称 baseline）与 crates。

---

## 一、总评

专家稿**方法论健全**：Policy Validator 前置、五维评分不并单一分数、约束先于精排、Evidence 带关系路径、验收矩阵、九条非目标——全是好纪律，且与我们 fail-closed / 派生不僭越权威的既有文化高度相容。**收编是对的。**

但受"不了解代码"所限，存在三类系统性偏差：

1. **约一半"待建"实际已建成**——索引状态机、outbox、关系模型、权限面、引用四元组都有现成形态；照单全收会重复建设。
2. **个别设计点与既有裁决/基建冲突**——已在 V1.1 调和（§二.B）。
3. **漏掉了真正的关键路径前置**——专家的 S1~S8 全部假设"知识内容已在 PC/KB 里"，而代码现实是**通用文档的内容管线（解析→版本族→确认发布）尚未建成，KB 修订连 `published` 转换通道都不存在**（§三.F1）。检索组件做得再好，没有内容可索引。

**一句话结论：专家稿是合格的"目标架构宪法"，不是可开工的"实施计划"；V1.1 已完成两者的桥接，本次评审再修 4 处勘误。**

---

## 二、五类差分

### A. 已建成，专家当新建（照抄会重复建设）

| 专家提案 | 代码现实（已建成形态） | 评审结论 |
| --- | --- | --- |
| `knowledge_relation` 关系表 | `pc.relationship`（baseline:3293）：跨域四元组端点（`*_ext_domain/type/id/revision_id`，CHECK 与 `*_object_id` 互斥）+ lifecycle 五态 + `relationship_revision` 修订链 | **不新建表**（V1.0 已裁，维持） |
| 关系类型白名单（Validator #4） | `pc.relationship_type` 注册表（baseline:10280~10291）**已有 12 条种子**，且带端点类型数组（如 `specified_by`: `{feature}`→`{requirement.requirement_revision}`） | 白名单 = 注册表 `status='active'` + 端点数组双重校验，**零新机制** |
| Outbox Event → Indexing Pipeline | outbox 基建 + 同事务回写 + 消费者模式全在（`outbox_ops.rs`、`kb_pc_handlers.rs` 的 KbMemoryApprovedHandler 同款）；幂等键先例 `kb_memory_approved:{revision_id}` | 管线=新增 outbox 消费者，**不是新基建** |
| 索引状态机 / 重建 / 增量 / veto 停检 | ai 栈 `db-section` 全套：`ai.document_index_state`（pending/indexed/stale/failed）+ `apply_transition` 单原语 + `rebuild_quietly` + veto 清零不变量（`knowledge_indexer.rs`） | kb/pc 域**复制该模式**，八项要求里 5 项是复刻不是新建 |
| 用户能否访问 PC/KB（Validator #1） | `require_project_visible` + 全局 fail-closed 中间件链 + tenant/project 硬过滤 | 校验器#1=调用既有 authz，不另建 |
| Evidence 的引用结构 | `pc.evidence`（四元组+file_path/symbol/code_range）+ `kb.memory_ref`（evidence/subject+locator）+ `ai.knowledge_reference`（锚点级）三套正式引用 | Evidence 结构=**三套既有引用的统一投影层**，不新造引用权威 |
| Embedding Pipeline 独立供给 | `team_llm` Provider 管理面（kind 含 openai_compatible/gemini、endpoint/credential/status 现成） | 供给复用，只扩 purpose=embedding（D-1） |
| 意图分类器 | `conversation_router_model.rs` LlmIntentClassifier（8s 超时/2000 字符截断先例） | Planner 沿用同纪律 |

### B. 冲突已调和（V1.1 处理方式复核，均维持）

| 专家设计 | 代码现实/既有裁决 | V1.1 调和 | 复核 |
| --- | --- | --- | --- |
| "旧版本索引保留或归档，支持历史检索" | 版本链裁决=active 索引只含现行 published | 历史走**版本钉死直读+现场分节**，不建归档索引（修订不可变+解析确定性→按需解析零风险） | ✅ 优于归档（省一倍索引存储与一致性负担），RT-007/008 双过 |
| 五个 REST 端点（query/plan/execute/expand/evidence） | ADR-10 方案 A=进程内端口；且 expand 暴露为公开端点有**绕过计划校验直接触发遍历**的风险 | V1 只实现 `POST /v1/retrieval/query`；其余四能力为 `RetrievalPort` 内部边界 | ✅ 安全面更稳 |
| S5 跨域遍历（专家：必进 V1） | 我 V1.0 原排 R3 后置 | 改为 S5 必交付；**渐进的是关系数据丰度不是引擎** | ✅ 维持专家裁决 |
| PC/KB-Service / Retrieval Service / Reranker / Embedding Pipeline 各自独立服务 | 全部为 pm-application 进程内模块（pc/、kb_service/） | 逻辑服务边界成立，部署单元不拆（注册表分工表原样收编） | ✅ |
| `authority_status: "approved"` 词表 | PC lifecycle=identity/draft/active/deprecated/superseded；kb=draft/published/rejected | 规范五值词表：current_published / historical_published / draft_candidate / superseded / speculative | ✅ |
| RabbitMQ 未来扩展 | PG outbox 够用 | 不引入，登记演进项 | ✅ |

### C. 真增量（我们原来没有、专家稿带来的——照单保留）

1. **Policy Validator 作为独立关卡**：六项校验+校验结论进审计——即便六项都映射到既有机制，"计划先过确定性校验器再执行、结论留痕"这个**编排纪律**是新的。
2. **五维独立评分 + 禁止单一分数**：改变了 V1.0 组装器设计（原把权威性折进剪枝排序）。
3. **约束前置序**：权限/版本/权威性在通道级 WHERE 硬过滤，精排只对过门候选——索引友好且可解释。
4. **Evidence 统一结构 + relationship_path 一等公民 + 六问**（含 gaps/conflict 联动）。
5. **RT-001~015 验收矩阵**：直接可固化为测试集（RT-014 环检测是 V1.0 漏的）。
6. **九条非目标**：其中"不把 Embedding 相似度当权威性"（#5）与"检索结果不覆盖人工批准"（#9）值得进红线。
7. **关系维护权分工**：防止 Retrieval Service 成为第三知识权威。

### D. 专家稿遗漏（代码现实必须补——V1.1 已补，复核维持）

1. **Token Budget 纪律**：专家的 Context Assembly 通篇未提上下文窗口管理。已按 DK-0 口径补（总量硬预算+分节 CAP+**单条上限**+DROP 序，`discovery_context.rs:58-110` 先例）。
2. **降级链**：vector 不可用→BM25+PageIndex 兜底；路由失败→pc_kb 双域直查——fail-closed 文化在检索面的延伸。
3. **域边界红线**：索引表不碰 `ai.document*`、不碰 `guard_ai_container_users`（双栈并存现实）。
4. **MCP 四工具签名冻结**。
5. **迁移号纪律**（新迁移自 0003 起）。
6. **CJK 分词**（中文 bigram，llm_wiki 先例）。

### E. 需修正/降级的专家设计

1. **`/retrieval/expand` 公开端点** → 降为内部（见 B 表，安全理由）。
2. **Reranker 独立服务** → 降为进程内 trait（listwise 起步，D-2）；Cross-Encoder 是目标形态非 V1 前置。
3. **性能指标后置**（专家自己也声明了）——同意，基准测试后定。
4. **专家语义表的"Requirement"归属** → **修正**（本次评审新发现，见 F2 与 §四勘误 1）。

---

## 三、本次评审新核出的事实（专家不可能知道的）

### F1 · KB 修订的 `published` 转换通道不存在 —— S8 的上游是断的

全仓检索：**没有任何代码把 `kb.document_revision` 从 draft 推到 published**。现状 = 分析草稿落库（`kb_pc_analysis.rs`：draft 落库"等待人工确认"）+ K1/K3/K6 端点 501。对照 PC 侧：`pc.cognition.version_published` 事件已有生产者（`pc/publish.rs` ⑫b，`kb_pc_handlers.rs:18`）。

**推论**：专家稿索引管线第一要求"知识版本变化触发索引更新"在 KB 侧**没有触发器可挂**。S8 的真实前置是 KB 确认/发布通道（K3 语义）——这属于内容管线，不在专家的 S1~S8 里。**必须补为 S2 的前置依赖或单列 S0**（§四勘误 2）。

### F2 · 关系语义表大半不是新对象；"Requirement"应锚 S02 需求域而非 KB

`pc.object_type` 已有 **18 条种子**（baseline:10255~10272）：专家语义表里的对象几乎全部已有对应物——business_capability、business_process、business_rule、domain_object（≈Business Object）、feature_module（≈Product Module）、scenario（≈Product Scenario）、product_decision 全在，且分层齐（L2_structure/L3_usage/L4_semantic/L5_decision）。

`pc.relationship_type` 已有 **12 条种子**，其中 3 条的 ext 端点已指向 **`requirement.requirement_revision`**（= `delivery.requirement_revision`，S02 正式需求域，baseline:1620）——`specified_by`（功能规格于需求）、`constrains`（规则约束需求）、`informs`（决策告知需求）。

**推论**：
- 专家九条关系里 PC 侧对象**几乎零新增**，真实增量 = 若干指向 KB 文档端点的新 relationship_type 种子（如 design-constrained-by-adr）；
- **需求不在 KB**：正式需求权威 = S02 需求域（delivery.requirement_revision），且既有种子已用 `requirement.requirement_revision` 这个 ext 词表指过它。专家表把 Requirement 放在"KB 对象"列、我 V1.1 §6.4 沿着映射到 `doc_type=requirement` ——**两处都错**，需勘误为 S02 需求域四元组目标（这同时意味着跨域遍历实际是 **PC↔KB↔S02 需求域三域**，不是专家假设的两域）。

### F3 · 注册表端点数组可直接当白名单判据

`relationship_type` 种子自带 source_types/target_types 数组（如 `{feature}`→`{requirement.requirement_revision}`）——Policy Validator #4 的白名单校验 = 类型 active + **端点数组命中** 双重判据，设计稿已按此落，此处补证据。

---

## 四、对 V1.1 的勘误清单（4 处）

| # | 勘误 | 位置 | 内容 |
| --- | --- | --- | --- |
| **1** | **Requirement 归属修正** | §6.4 语义表 | "Requirement（doc_type=requirement）" → **S02 需求域（delivery.requirement_revision）**，沿用既有 ext 词表 `requirement.requirement_revision`；capability-realized-by-requirement 等种子端点据此改锚；**跨域范围表述从"PC↔KB"修为"PC↔KB↔S02 需求域"** |
| 2 | **S 序补内容管线前置** | §12.1 | S2（BM25+pgvector）与 S8（Indexing）加前置依赖：KB 确认/发布通道（K3 语义，F1）+ 六格式解析层（选型稿）——没有它们，通道无内容可索引、管线无事件可消费；专家 S 序不改动，只补依赖标注 |
| 3 | **RT-002 门控标注** | §12.2 | RT-002（自然语言→语义召回）标注：向量半边受 D-1（embedding provider）门控；D-1 未决时以 BM25+CJK bigram 验收并记降级路径 |
| 4 | **种子增量澄清** | §6.4 / §三 | 注明：PC 对象类型零新增（18 条种子已覆盖）；真实增量 = 指向 KB 端点的新 relationship_type 种子若干条 + owner_domain 属性扩展 |

## 五、结论

1. **V1.1 主体成立**：专家稿 §五~§十二 收编无误，调和决定（历史直读/单端点/遍历进 V1/进程内服务）复核全部维持。
2. **执行 4 处勘误**（§四）——其中 #1 是语义正确性问题（需求域锚错会误导种子迁移与遍历算法的目标解析），优先级最高。
3. **排期真风险不在检索组件**：S1~S8 的检索件约一半有现成资产可复刻；**真正的关键路径是内容管线前置**（F1 的 KB 发布通道 + 六格式解析 + 版本族归并）——它同时是任务书四项之外建议追加的第 5 项（与选型稿的解析层设计合并闭合）。
4. 专家稿价值定性：**目标架构宪法**（纪律与验收面极有价值）× 代码桥接（V1.1 已做）× 内容管线前置（本次评审补出）——三层合一才是可开工的实施基线。
