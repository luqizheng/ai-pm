# ACP 通信面 V1.2 专家复审意见 · 第 3 块（排期依赖与关键路径）

| 项目 | 内容 |
|---|---|
| 审阅对象 | `smark-pm-s03-acp-slice-design-acp01-02-08-v1-2.md`（V1.2）第 3 块提问：V-05-1 排期 / 迁移号 0048 / ACP-04~07·09 先后 / AI Team V2 解堵条件 |
| 代码基线 | ai-pm master @ 220173f |
| 意见到达 | 2026-11-07（用户转发） |
| 收编状态 | **全部收编**：V1.2（修订记录/§9/§10）+ 排期全景落 `docs/project-ai-team-v2/ai-pm-v05-execution-roadmap-v1.md` |

---

以下为专家意见原文（逐字保留）：

这一块我认为比前两块更重要，因为它决定了后续工作会不会出现先做临时结构、再推翻重建，以及多个切片相互等待的问题。

我的总体结论：

基本赞同你们的排期方向，但建议调整三个地方：
1. V-05-1 必须立即升为关键路径，但最小可用版本不能省略凭据撤销能力。
2. ACP-09 的 Model A 回归不能等到最后，应分为早期针对性回归和后期完整回归。
3. ACP-04 不应简单放到 ACP-06/07 之后，应该根据 ACP-05 的交付状态依赖提前处理或并行设计。
另外，AI Team V2 的 task_repository 生产写入口是独立 P0，不应该等待 ACP 全部收口。

一、总体关键路径裁决

建议的整体实施顺序
1. 第一阶段：活风险正确性修复

   ACP-448 + ACP-450/451。

   同步执行 Model A 定向回归。
   最高优先级
2. 第二阶段：身份基座与恢复链路

   V-05-1 最小安全子集 + ACP-452/453 + ACP-C1～C4 + pm_discover_runs + previous_chain_id。
   生产恢复能力前置
3. 第三阶段：ACP-04 / ACP-05

   完成 Delivery/Consumption 持久化与 Continuation 正式状态机，先闭合两者的数据契约。
4. 第四阶段：ACP-06 / ACP-07

   Checkpoint、Workspace Lock、Deadline 与人工授权。
5. 第五阶段：完整 ACP-09 回归

   Model A、Runner 离线、重启、超时、迟到 ACK、多次介入等真实栈验证。
   发布验收

有一个并行工作建议：

task_repository 生产写入口 P0 可以在第一阶段独立启动，不需要等待 V-05-1 或 ACP 全部收口，但必须保留现有 claim 和 execution authorization 约束。

二、3.1 V-05-1 Client Registry 排期

建议立即插入关键路径
① 应该插队还是与 ACP-04 并行？
我的意见：插队到 ACP-04～07 之前，但可以与 ACP-04 的设计工作并行。

原因不是 V-05-1 本身比 Delivery 更重要，而是它现在已经成为三个领域的共同基础：
- ACP 命令面真实身份。
- R-16 离线恢复。
- AI Team V2 的 agent_client。
如果不先完成身份基座，后续会出现三种不良结果：
1. ACP 只能在受控环境中恢复。
2. AI Team V2 必须重新设计一套 Client 身份。
3. 其他切片继续使用临时 client_name，形成迁移债务。
这意味着 V-05-1 已经不是 ACP 的一个普通增强项，而是平台级 Client Identity Foundation。

我建议将它独立提升为一个平台基础切片，而不是仅作为 ACP 的子任务。
② 最小可启用子集应该是什么？
你们倾向于：
client_name 登记表 + 凭据签发 + lease_client_id 回填，撤销后补。

我不完全赞同。

凭据签发而没有撤销能力，不适合作为生产身份基座。

假设某台 Runner 机器泄露了 Client 凭据：
- 系统能签发凭据。
- 系统能验证凭据。
- 但不能立即撤销。
- 被泄露的 Client 仍然能够发现 Run、续领租约、操作命令。
这会直接影响执行端所有权。

所以我建议最小版本是：
V-05-1 最小生产可用集
必须包含
- Client 登记与稳定 client_id。
- client_name 唯一性及归属约束。
- Client 与认证主体绑定。
- 凭据签发、验证。
- 凭据撤销或 Client 禁用。
- lease_client_id 与历史租约绑定。
- 旧身份兼容和回填策略。
- 正式权限码字典及权限校验。

可以后续增强
- Client 能力三分的完整动态管理。
- 多凭据轮换与并存。
- Client 生命周期管理 UI。
- 更复杂的设备状态与健康管理。

这里需要特别区分：

撤销能力是安全底线，复杂的撤销管理功能不是。

V1 可以只有一个简单的 revoked_at 或 disabled_at，不需要一开始就做完整设备管理中心。

另外，凭据撤销后，应立即影响所有相关操作，包括：
- pm_discover_runs
- pm_reacquire_lease
- pm_poll_commands
- pm_report_interaction
不能只禁止续领，而允许已经撤销的身份继续轮询命令。
我的最终裁决
V-05-1：
- 立即插入关键路径。
- 最小子集必须含身份、凭据、验证、撤销、绑定和权限字典。
- 完整能力管理可以后补。
- 与 ACP-04 的设计工作允许并行。
- R-16 生产启用必须等待身份安全能力完成。

三、3.2 迁移号 0048 的占用规划

建议拆成两个迁移号
我的建议是：

0048 = ACP V1.2 收敛迁移；0049 = V-05-1 Client Registry 迁移。

不建议把两类内容全部塞进 0048。

原因：
- 0048 属于已经实施的 ACP 通信面收敛。
- 0049 属于平台身份基础设施。
- 两者可以独立审查、测试、回滚和安排实施。
- 如果 V-05-1 设计需要调整，不应该影响 ACP 收敛迁移。
建议划分如下：
Migration	内容	所属工作
0048	worker_command.reacquired_generation	R-16
0048	task_interaction.previous_chain_id	R-4 延伸
0049	Client Registry 登记表	V-05-1
0049	task_run.lease_client_id	V-05-1
0049	历史 Client 身份回填	V-05-1
0049	权限码字典及关联约束	V-05-1

如果 0048 还需要其他收敛字段，应全部纳入 0048，但不要提前混入 Client Registry。
有一个重要的迁移细节
lease_client_id 的历史回填不能简单通过：
LIKE 'mcp:{client_name}%'

自动认定身份。

建议分三类：
历史记录	回填处理
有可靠认证主体与 Client 对应关系	自动回填
只有 client_name 自报文本	暂不认定真实身份
无法确定来源	标记 legacy/unverified

历史记录可以保留原 lease_owner，但不能因此获得新的续领权限。

迁移原则：历史兼容不等于历史身份可信。

此外，迁移号应由一个实施负责人统一分配。0048、0049 一旦进入共享分支，就不要让并行会话自行创建同号迁移。

四、3.3 ACP-04～07/09 的先后关系

这里我对你们的建议有一处调整。
① 收敛批 ①②先行，③～⑨与 V-05-1 合并
我赞同。

但建议把收敛批拆成两个交付层次：
层次	内容	目的
Correctness Fix	ACP-448、ACP-450/451	消除当前错误状态迁移和错误权限
Recovery Foundation	V-05-1、ACP-452/453、ACP-C1～C4、Discover、Chain 关联	建立可靠恢复基础

不要把所有九项作为一个无法拆分的大 PR。

它们可以属于一个收敛批次，但应保持独立提交和独立验收。
② ACP-05 应在 ACP-06/07 之前吗？
完全赞同。

你提到的理由非常充分：

当前 continuation_start_frozen、continuation_generation、Session 记录仍然挂在 Interaction/Command 域。

如果先做 ACP-06/07，就可能出现：
临时 continuation 结构
       ↓
Checkpoint / Workspace Lock
       ↓
Deadline 授权
       ↓
ACP-05 正式状态机
       ↓
再次迁移和调整

这会造成不必要的重复建设。

我建议 ACP-05 的正式状态机先确定，再设计 ACP-06/07 的扩展。

但要补充一个边界：

ACP-05 不应该顺带吸收 ACP-06/07 的全部职责。

它应该先定义好正式的 Continuation 身份、状态、代次和生命周期，然后让 Checkpoint、Lock、Deadline 通过明确接口接入。
③ ACP-04 应该放在哪里？
这里我不完全赞同把 ACP-04 放到 ACP-06/07 之后。

ACP-04 的 Delivery/Consumption 持久化属于执行结果和交付证据基础设施。

它与 Continuation 可能有交叉，但不应该被 ACP-05 完全吞并。

我的建议：

ACP-04 与 ACP-05 先进行联合设计，再根据真实数据依赖确定实施先后；默认 ACP-04 先落地核心交付持久化，ACP-05 随后建立正式 Continuation 状态机。

尤其需要检查：
- Delivery 是否引用 Run 或 Session。
- Consumption 是否需要 continuation_generation。
- Delivery 状态是否参与 Task 完成判定。
- ACP-05 是否会消费 ACP-04 的持久化结果。
- ACP-04 是否依赖 ACP-05 尚未建立的正式 Session 身份。
如果 ACP-04 确实需要 ACP-05 的正式字段，则先确定字段契约，再分阶段落地，不能临时另建一套 Session 身份。

所以建议顺序调整为：
ACP-04 / ACP-05 联合设计
          ↓
ACP-04 核心持久化
          ↓
ACP-05 Continuation 状态机
          ↓
ACP-06 Checkpoint / Workspace Lock
          ↓
ACP-07 Deadline / 人工授权

这里 ACP-04 和 ACP-05 的设计可以并行，但实现上要服从数据依赖，而不是机械地认为两个切片必须严格串行。
④ ACP-09 是否提前？
必须提前一部分
我建议 ACP-09 分为两个阶段。

阶段 A：定向回归，随 Correctness Fix 执行。

特别是：
测试	对应风险
Model A human Run 调用 report	ACP-448
Model A 正常 awaiting_decision	原有人工介入流程
Model A 回复后继续执行	ACP-450/451
无 consumed 证据 Adapter	R-4
重复 ACK	幂等性

这部分必须在第一阶段完成，不能等到最后。

阶段 B：完整 ACP-09 回归，放在 ACP-05～07 完成之后。

包括：
- Runner 存活。
- Runner 离线超租约。
- Runner 重启。
- 同主体续领。
- 旧 generation 迟到 ACK。
- terminate 未确认禁止启动新 Session。
- deadline 到期。
- ACP-07 重新授权。
- 多次人工介入。
- 多 Repo Delivery。
- Model A 完整回归。
因此，ACP-09 不应该只有一个最终验收节点。

它应该是一个贯穿式验证工作包。

五、3.4 AI Team V2 主线解堵

这一部分我认为需要单独建立依赖关系，不应该让 AI Team V2 完全等待 ACP 通信面收口。
① task_repository 生产写入口 P0 是否独立先行？
建议立即启动独立修复
完全赞同你们的判断。

如果 task_repository 全仓零生产 INSERT，而后续执行和交付逻辑依赖它，那么这就是一个实际的生产数据链路空洞。

不应该因为 ACP 还在实施，就让它继续存在。

建议独立小切片：
Task Repository Creation
        ↓
Repository Authorization
        ↓
Task Claim
        ↓
Execution Allows
        ↓
Repository Scope
        ↓
Delivery / Commit

重点不是简单补一个 INSERT。

必须保证：
1. Task 与 Repository 关联只能来自当前 Project 的有效 Repository。
2. 创建关联时校验现有授权。
3. 不允许跨 Project 写入。
4. 重复写入具有幂等性。
5. Claim 和 execution_allows 使用同一关联事实。
6. 不允许通过 MCP 参数伪造 Repository 归属。
这里建议沿用已有 Project Repository 领域对象，不要再创造一套 Task Repository 权限体系。

另外，task_waiting 零写入方已经由 ACP 切片补上，这个问题可以从 P0 空洞清单中移出，但需要保留真实栈回归证据。
② 七项待裁中，哪些是 AI Team V2 硬前置？
你们判断的两项基本正确，但我建议把调度入口也提升为 V2 开工前必须闭合的设计问题。

具体分类如下：
待裁项	V2 启动前是否必须	我的判断
Git 归属表述	否	文本修订，可并行
agent_client 载体	是	V-05-1，身份基础
调度入口	是（设计闭合）	不必全部实施，但必须确定调度边界
模型策略 UNIQUE(tenant,provider)	是（Schema 前）	产品决策可并行，落库前必须闭合
project_role vs member.role	是	权限模型硬前置
执行事件持久化	V2 正式运行前必须	可先做最小方案
Phase 5 双轨收敛	否	可独立规划，不阻塞 V2 启动

我逐项说明。
A. agent_client = V-05-1
这是硬前置。

因为 AI Team V2 和 ACP 不能各自拥有一套 Client 身份体系。

建议统一：
Team
  └── Project
       └── Client
            ├── Identity
            ├── Credential
            ├── Capabilities
            └── Revocation

这里 Client 应该是可被多个 Project 绑定的身份载体，具体授权范围由 Team 和 Project 关系控制。

不要将 Client 简化为某一个 Project 内的 Runner 名称。
B. project_role vs member.role
这是硬前置，而且优先级不低于 Client Registry。

原因是：

member.role 是什么范围的角色？

如果它是 Team Membership 角色，那么它不应该直接承担 Project 内的管理授权。

建议明确：
Team Membership
    └── Team Role

Project Membership / Project Grant
    └── Project Role

两者职责不同。

例如：

一个 Team Admin 可以管理团队级 LLM Provider，但不一定天然拥有所有 Project 的业务管理权限。

一个 Project Admin 可以管理本 Project 的 Git、Runner、Project AI 配置，但不能绕过 Team 的模型授权范围。

这也与你们此前已经确定的 Team LLM Provider 授权原则一致。

因此，Project Role 模型必须在 V2 权限代码和 API 落地前确定。
C. 调度入口
建议 V2 开工前先明确设计，不要求先完成全部调度实现。

至少明确：
- 谁创建 Agent 执行请求。
- 谁选择 AI Role。
- 谁选择模型。
- 谁决定执行 Project。
- 谁将任务分发给 Client。
- 调度失败后由谁负责重试和终结。
不要让 AI Team、ACP Runner、Project Agent 各自发展出不同的调度入口。
D. 模型策略 UNIQUE(tenant,provider)
我认为它属于 Schema 硬前置，而不是 V2 产品启动硬前置。

你们之前已经确定：
- Team 拥有 Provider 凭证。
- Team Admin 管理模型授权范围。
- Project Admin 在授权范围内选择模型。
- 默认模型按 AI Role 分配
因此，如果 UNIQUE(tenant,provider) 会把模型配置限制为一个 Team 只能有一份 Provider 配置，就需要重新核对它是否符合实际多配置、多模型的产品语义。

建议先确定：
Provider Connection
    ↓
Team Model Authorization
    ↓
Project AI Role Model Selection

在这个关系明确前，不要冻结相关数据库唯一约束。
E. 执行事件持久化
不是 V2 启动的硬前置，但应该是正式运行的硬前置。

我建议先定义最小事件持久化：
- event_id
- team_id
- project_id
- task_id
- run_id
- client_id
- event_type
- generation
- occurred_at
- received_at
并保证事件可以幂等写入和审计查询。

不要为了执行事件持久化，又将所有执行端日志同步成平台数据库中的大型事件溯源系统。
F. Phase 5 双轨收敛
可以独立规划，不阻塞 V2 基础能力启动。

但必须明确最终收敛方向，避免双轨长期并存成为事实架构。

六、AI Team V2 与 ACP 是否必须串行？

这里我想进一步强调：ACP 通信面与 AI Team V2 应该共享基础设施，但不应该变成相互等待的两个大项目。

建议采用双主线、共享基础设施的方式。
双主线实施关系
共享基础设施
- V-05-1 Client Registry
- Team / Project 权限边界
- 凭据与授权机制
- 执行事件基础

ACP 主线
- 通信面收敛
- R-16 离线恢复
- ACP-04/05
- ACP-06/07
- ACP-09

AI Team V2 主线
- Team Domain
- Team Membership
- Project Role
- Team LLM
- Agent 调度

共享身份和授权基础设施，但保持各自业务状态机与交付边界独立。
这里有一个重要的解堵判断：

AI Team V2 的 Team 创建、成员管理、角色权限、LLM 配置等基础领域工作，不需要等待 ACP-05、06、07 全部完成。

但涉及真实 Agent Client 注册、执行调度和跨 Project 执行授权时，必须使用 V-05-1 的正式身份及权限模型。

也就是说：

V2 可以提前开始，但不能提前创造第二套 Client Identity。

七、最终关键路径与并行安排

我把上面意见整理成一张执行依赖表。
工作包	优先级	依赖	可并行性
ACP-448	P0	已有 ACP 通信面	立即
ACP-450/451	P0	已有 Interaction/Command	立即
Model A 定向回归	P0	上述修复	与修复同步
Task Repository INSERT	P0	现有 Project Repository、授权机制	独立并行
0048 ACP 收敛迁移	P1	收敛设计冻结	与 V-05-1 设计并行
V-05-1	P0	身份架构裁决已完成	立即启动
0049 Registry 迁移	P1	V-05-1 实施设计	紧随 V-05-1
ACP-452/453	P1	V-05-1 身份安全	依赖 V-05-1
ACP-05	P1	Continuation 数据契约	与 ACP-04 联合设计
ACP-04	P1	Delivery 数据契约	可与 ACP-05 设计并行
ACP-06	P1	ACP-05	串行
ACP-07	P1	ACP-05，Deadline 契约	与 ACP-06 设计并行
ACP-09 完整回归	P1	ACP-04～07 及恢复链路	最终验收
AI Team V2	独立主线	Client、Project Role、调度设计	基础工作可提前

这里的 P0/P1 是建议的实施优先级，并非你们已有的正式任务等级。
推荐实际启动方式
不要让所有事情排成一条长队。建议立即开四个工作包：
WP-A · ACP Correctness
立即启动

ACP-448、ACP-450/451、Model A 定向回归。
WP-B · Client Identity
立即启动

V-05-1 实施设计、0049 迁移设计、身份安全。
WP-C · Repository Integrity
立即启动

Task Repository 生产写入口 P0 修复。
WP-D · ACP-04/05 Architecture
并行设计

Delivery 与 Continuation 的正式数据契约联合设计。
这样可以让四条工作线同时取得进展，而不让相互依赖的代码提前落地。

八、最终专家裁决：第 3 块

以下是我建议正式收编的裁决。
编号	裁决
3.1 V-05-1 排期	立即插入关键路径，可与 ACP-04 设计并行
3.1 最小子集	身份 + 凭据签发 + 验证 + 撤销 + Client 绑定 + 权限字典
3.2 Migration	0048 ACP 收敛，0049 Client Registry
3.3 收敛批	ACP-448、450/451 优先
3.3 恢复链路	V-05-1 完成后实施 ACP-452/453
3.3 ACP-05	先于 ACP-06/07
3.3 ACP-04	与 ACP-05 联合设计，按数据依赖实施，不能简单推迟到最后
3.3 ACP-09	定向回归提前，完整回归最后
3.4 task_repository	独立 P0，立即启动
3.4 agent_client	V-05-1，V2 硬前置
3.4 project_role	V2 权限实现硬前置
3.4 调度入口	V2 开工前完成架构设计
3.4 模型策略 UNIQUE	Schema 冻结前完成裁决
3.4 执行事件持久化	V2 正式运行前完成最小实现
3.4 Phase 5	不阻塞 V2 启动

九、一个需要防止的项目管理错误

我不建议把这轮工作简单命名为：
ACP V1.2 收敛完成后，再启动 AI Team V2。

这个顺序会让 AI Team V2 不必要地等待 ACP-04～09。

更准确的项目划分应该是：
- ACP V1.2 收敛：解决现存通信面正确性和恢复可靠性。
- Client Identity Foundation：解决平台统一执行端身份。
- AI Team V2：建设团队、成员、角色、模型和调度领域。
- ACP-04～09：继续完成交付、Continuation、Checkpoint、授权和回归。
它们共享底层能力，但不是同一个交付物。

我的最终建议：先完成 ACP 活风险修复，同时立即启动 V-05-1 和 Task Repository P0。不要等待整个 ACP 通信面全部完成才启动 AI Team V2。

这应该是目前整体返工风险最低、并行效率最高的排期方案。

---

## 收编对照（我方，2026-11-07）

| 专家裁决 | 落位 |
|---|---|
| 五阶段关键路径 + WP-A~D 立即启动 | `docs/project-ai-team-v2/ai-pm-v05-execution-roadmap-v1.md` §3/§4 |
| V-05-1 = 平台级 Client Identity Foundation、最小集含撤销、撤销影响全部工具面 | roadmap §6；V1.2 §3.6 生产启用门一致 |
| 0048/0049 拆分 + 回填三分类 + 迁移号统一分配 | roadmap §7；V1.2 §10.4 |
| 收敛批两层（Correctness Fix / Recovery Foundation） | V1.2 §10.5 |
| ACP-04/05 联合设计五检查 + 顺序链 | roadmap §10 |
| ACP-09 两阶段贯穿式 | V1.2 §10.5 层1③ + roadmap §3 阶段5 |
| task_repository P0 独立切片六约束 | roadmap §8 |
| V2 七待裁分类（调度入口升硬前置/UNIQUE=Schema 前/project_role 硬前置/Team Role≠Project Role） | roadmap §9 |
| 双主线共享基础设施 + 四交付物划分 | roadmap §1/§2 |

**文档结束**
