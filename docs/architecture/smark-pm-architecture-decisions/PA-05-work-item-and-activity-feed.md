# PA-05：统一事项模型与信息流

**决策编号：** PA-05\
**状态：** 已确认\
**基线日期：** 2026-11-20

## 1. 目标

建立统一的事项基础能力，同时保留不同领域对象自身的语义和规则；以统一活动历史作为信息流的数据来源，并明确正式知识、执行日志和用户已读状态之间的边界。

## 2. PA-05-01：统一事项基础模型

### 已确认方案

采用"统一基础模型 + 独立领域对象"。

统一基础模型提供：

-   统一事项 ID；
-   基础描述；
-   创建信息；
-   关联关系；
-   活动历史等通用能力。

以下对象保留独立领域模型、专属属性和业务规则：

-   Requirement；
-   Task；
-   Design；
-   Decision；
-   Issue；
-   Risk；
-   Blocker；
-   其他具有明确领域语义的对象。

普通知识文档、技术文档和讨论不强制转化为 Work Item。

## 3. PA-05-02：状态模型

### 已确认规则

-   统一基础状态语义。
-   每种领域对象保留独立状态机。
-   统一状态语义用于待办展示、Agent 调度、信息流过滤、统计和搜索。
-   不得仅因为两个领域对象使用相同状态名称，就认定其业务含义相同。

### 设计要求

统一的是跨领域可理解的状态语义，而不是强行统一所有领域对象的完整生命周期。

## 4. PA-05-03：事项关联关系

### 已确认规则

事项之间支持多对多关联，并使用明确的关系类型。

V1 首批关系类型：

  关系类型         含义
  ---------------- --------------------
  `implements`     任务实现需求
  `supports`       设计支持需求
  `affects`        决策影响事项
  `blocks`         事项阻塞事项
  `derived_from`   事项来源于另一事项
  `references`     普通引用

关联关系不改变对象归属。

V1 不建设通用关系配置引擎。

## 5. PA-05-04：关联维护

### 已确认规则

Agent 可以自动建立语义明确的普通引用、实现、设计支持和阻塞关系，并记录：

-   关系建立的来源；
-   建立关系的理由。

重要业务归属或正式关系变更，由 Agent 提议、人类确认。

人类可以直接调整普通关联。Agent 不得擅自覆盖人类明确修改的关系。

## 6. PA-05-05：Activity History

### 已确认规则

建立统一 Activity History，自动记录：

-   创建；
-   修改；
-   状态变化；
-   关联变化；
-   Agent 分析与执行；
-   人类决策；
-   建议处理；
-   任务执行；
-   阻塞；
-   交付等活动。

记录应明确区分操作来源：

-   人类操作；
-   Agent 操作。

历史记录不直接覆盖。发生纠错时，追加更正记录。

Activity History 支持按以下维度查看：

-   事项；
-   Product；
-   Project；
-   时间。

Activity History 不是正式知识正文。

## 7. PA-05-06：正式知识与历史分离

### 已确认规则

-   Requirement、Design、Decision 等正式领域对象维护自身内容和版本。
-   Activity History 通过引用关联正式知识及其版本。
-   Agent 可以利用历史恢复上下文，但不能将历史事件直接视为当前有效知识。
-   延续既定知识权威边界：Markdown + Git 管理知识正文；PostgreSQL
    管理身份、状态、版本引用、审批和事件记录；PageIndex
    是可重建的派生索引。

### 权威边界

  内容                         权威来源
  ---------------------------- ------------------------------
  正式知识正文                 Markdown + Git
  身份、状态和结构化业务状态   PostgreSQL
  正式知识版本引用             PostgreSQL 中的版本引用信息
  审批与事件记录               PostgreSQL
  检索索引                     PageIndex 等派生索引，可重建

不得把 Activity History
当作正式知识正文的替代品，也不得把派生索引当作权威数据源。

## 8. PA-05-07：活动记录粒度

采用两层记录。

### 8.1 Activity Timeline

面向人类，记录有业务意义的重要活动：

-   事项创建、状态变化和完成；
-   正式版本发布；
-   重要关联变化；
-   人类决策；
-   Agent 识别的重大风险、阻塞和冲突；
-   任务执行、暂停、恢复和交付；
-   重要验收。

### 8.2 Execution Logs

保存详细执行过程，例如：

-   工具调用；
-   文件读写；
-   命令执行；
-   自动检查；
-   Agent 详细执行步骤；
-   错误信息。

### 8.3 粒度规则

-   普通连续字段修改可以合并为有意义的活动。
-   合并不得丢失重要业务变化。
-   详细执行日志不应全部混入 Activity Timeline。

## 9. PA-05-08：信息流

### 已确认方案

统一 Activity 数据源提供三个信息流视角，不另建独立消息系统：

#### My Feed

展示当前用户需要关注的：

-   决策；
-   重要事项；
-   Agent 请求；
-   Agent 执行结果。

My Feed 根据 Product Membership 和事项路由生成。同一事件涉及多个 Product
时，应合并去重。

#### Product Feed

展示对应 Product 的业务信息。

#### Project Feed

展示 Project 研发和技术活动，不因 Product Membership
而切割普通研发知识。

### 信息流呈现规则

-   普通进展进入 Feed。
-   决策请求和重要事项应突出显示。
-   活动可以跳转到对应领域对象。
-   Feed 是统一 Activity 数据的不同视图，不是彼此独立的消息记录系统。

## 10. PA-05-09：已读状态

### 已确认规则

-   已读状态是用户个人状态，每个用户独立维护。
-   一个用户的阅读行为不影响其他用户。
-   决策请求已读，不等于已处理或已批准。
-   已完成决策的通知仍可查看，但不再作为待处理请求。
-   普通活动支持批量标记已读。
-   不设强制阅读确认、阅读回执或全员已读机制。

### 必须区分的四种概念

  概念        含义
  ----------- ----------------------
  Read        用户已阅读该信息
  Notified    系统已向用户发送通知
  Processed   用户已处理对应事项
  Approved    用户已作出批准决定

四者互不等同，不得用一个状态字段混合表达。

## 11. 实施约束汇总

1.  不以统一事项模型抹平领域对象差异。
2.  不把所有文档和讨论强制转化为 Work Item。
3.  不把 Activity History 当作正式知识。
4.  不把详细执行日志全部展示在 Timeline。
5.  不把 Feed 实现为三套重复消息系统。
6.  不把已读、通知、处理和批准合并成同一状态。
7.  不引入全员已读或强制阅读回执。

## 12. 落地记录（2026-11-21，批次 A+B）

用户裁决：PA-05-01 采用**注册表形态**（新建 `core.work_item` 注册表，领域对象保留独立表与状态机；映射式不建表方案否决）。

已实施（migration 0050 + `pm-application/src/work_item.rs`）：

- **PA-05-01/02**：`core.work_item`（统一事项 ID=主键、tenant/target_type/target_id 唯一、title、统一状态 8 值、project 归属）；Task/Requirement/ADR 三类显式注册；状态语义映射在服务层逐域显式声明（task 13 值 / requirement 3 值 / adr 6 值），同名状态不默认同义。
- **PA-05-03/04**：`core.work_item_relation`（V1 六关系类型 + origin human|agent + reason；唯一 (source,target,type)；self 拒绝）；解除通道仅服务人类操作面（PA-05-04 Agent 不得覆盖人类修改）。
- **PA-05-05/06/07**：`core.work_item_activity` 追加式 Timeline（无 UPDATE 路径，纠错=追加 correction；human|agent 来源区分；事项/Project/Product/时间四维索引）。Execution Logs 不入 Timeline（归 `delivery.task_run_event`）；正式知识正文权威=Markdown+Git，Timeline 仅存 revision_ref（ADR 首版已挂）。
- **PA-05-09**：`core.work_item_read_state` per-user（read_at/notified_at 两列；Processed/Approved 留在领域对象，四概念不分混）。

接线（批次 B，五个写入点）：create_task（创建即注册+created 活动）→ `apply_task_transition` 单点（自愈注册+状态同步+语义变化记 status_changed，§8.3 合并规则=语义未变不记）→ create_requirement / create_adr（创建即注册，ADR 挂 revision_ref）→ link_task_requirement（钉版定稿需求 → implements 关联，human 来源+理由）。接线与既有审计同事务（同一致性级别）。

验收：`pm-application/tests/work_item.rs` 6/6 绿（注册幂等不覆盖状态/迁移语义变化/钉版 implements+幂等/四维查询/关系校验与解除/创建接线）；受影响面回归 48 例绿（command_executor 10 + task_dependency 9 + requirement_basis 8 + design_s03 4 + state_transition_guard 17）。

后续批次（未实施）：批次 C = 三 Feed API + 前端页 + 已读状态读写面；Issue/Risk/Blocker 实体（PA-05-01 遗留领域对象）随后续批次显式加入；产品维度活动（product_id 列已预留，接线随 Task-Product 链路补）。
