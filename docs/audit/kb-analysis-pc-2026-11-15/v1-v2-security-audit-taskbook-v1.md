# V-1 / V-2 安全与人工确认门专项代码审计任务书 V1.0

> **状态**：已冻结，执行中（V-1 代码级复核已完成→见 [映射任务书 §9](four-domain-registry-mapping-audit-taskbook-v1.md)；负向测试未执行）
> **编制日期**：2026-11-15（专家提供）· **代码快照**：master 05ce33d8（S1 基线，后续换版须重新锚定）
> **上游基线**：《四域对象注册表与现有数据库映射审计任务书 V1.1》
> **关联架构裁决**：Q3、Q11、Q12
> **审计对象**：Project Cognition 人工确认主体门（V-1）、旧 `ai` 栈租户隔离（V-2）
> **执行模式**：代码级只读审计 + 针对性安全测试
> **执行纪律**：审计期间禁止修改 Schema；未经单独批准，不修改业务代码、权限策略及数据。
> **执行原则**：V-1 验证"谁有权确认"，V-2 验证"谁有权访问"，两者不能互相替代。

---

## 1. 任务背景

《四域对象注册表与现有数据库映射审计任务书 V1.1》确定了两个必须优先核实的问题。

### 1.1 V-1：Project Cognition 人工确认主体门

现有审计发现：
- `pm_cognition.rs:279-310` 存在基于 `AuthedPrincipal` 的确认处理逻辑。
- 确认记录存在 `decided_by` 主体信息。
- 代码注释指出 AI 没有确认入口。
- 但另一份四域审计曾将 Project Cognition 确认主体门标记为 P0 缺陷，认为确认入口没有强制人工主体约束。

两处证据存在冲突。因此，不能仅凭代码中存在 `AuthedPrincipal`、`decided_by` 或人工确认界面，就认定 Project Cognition 已经具备可靠的人工确认门。必须验证：
1. 确认入口是否强制人工主体；
2. 服务主体是否可能冒用人工身份；
3. 是否存在绕过正常入口的其他确认路径；
4. 正式认知状态变更是否受到人工决策约束；
5. 决策主体是否可以通过审计记录追溯。

### 1.2 V-2：旧 `ai` 栈租户隔离

V1.1 已发现：
- 新栈采用 Team / Project 双键过滤。
- 旧 `ai` 栈部分路径仅使用 `project_id`。
- 租户信息通过 Project 反查。
- `knowledge.rs:424-438` 是当前已知检查入口之一。

现有证据不足以证明旧 `ai` 栈存在跨租户访问漏洞，也不足以证明其不存在此类漏洞。必须检查完整访问链：HTTP API / MCP / 后台任务 / Agent 执行 / 数据库查询 / 跨域引用 / 异步任务与事件消费。**如果发现未经授权的跨租户读取、修改、确认或发布可能性，立即升级为 P0 安全问题。**

## 2. 审计目标

### 2.1 V-1 目标

确认 Project Cognition 是否真正具备不可绕过的人工确认机制。最终必须能够回答：
- 谁可以提出认知变更？谁可以审核？谁可以正式确认？
- AI Agent 能否确认自己提出的内容？
- Service Principal 能否冒用 Human Principal？
- 哪些业务状态变更必须经过人工确认？
- 所有正式确认能否追溯到真实人工主体？

### 2.2 V-2 目标

确认旧 `ai` 栈是否具备可靠的 Team / Project 租户隔离。最终必须能够回答：
- 每个访问入口如何确定当前 Team？Project 是否始终属于当前 Team？
- 查询是否同时验证 Team 与 Project 的归属关系？
- 是否可能通过猜测或替换 ID 访问其他租户数据？
- 后台任务是否保留原始租户上下文？
- MCP 和 Agent 是否存在绕过租户校验的路径？
- 跨域引用是否执行目标对象的租户归属校验？

### 2.3 共同目标

形成有代码证据、有测试结果、有风险评级、有修复建议的专项审计报告。**本任务不以代码中存在某个校验函数作为通过依据，而以完整访问链和可验证的拒绝行为作为依据。**

## 3. 审计范围

### 3.1 V-1 审计对象

| 对象 | 审计内容 |
|---|---|
| Project Cognition 确认入口 | 确认 API、Service、Domain 入口 |
| AuthedPrincipal | 主体类型、身份来源、可信度 |
| Principal.kind | Human、Agent、Service 等类型约束 |
| decided_by | 决策主体写入和真实性 |
| cognition_item | 提案状态及正式认知状态转换 |
| project_knowledge | 正式认知对象的创建、更新、退役 |
| governance.approval_request | 审批请求与认知确认之间的关系 |
| audit_log | 决策审计和主体追溯 |
| 内部服务调用 | 是否存在绕过人工确认门的调用路径 |
| 后台任务 | 是否能够直接执行正式确认 |

重点代码入口：`pm_cognition.rs:279-310`、`AuthedPrincipal` 定义及构造/转换、`Principal.kind` 定义和校验、`decided_by` 写入位置、正式对象状态转换代码、`governance.approval_request` 与 `audit_log` 处理代码。**以上为初始入口，不代表完整范围——必须从入口向下追踪调用链，搜索所有能够修改正式认知状态的写入路径。**

### 3.2 V-2 审计对象

| 对象 | 审计内容 |
|---|---|
| Project 查询 | Project 是否验证 Team 归属 |
| Project Knowledge | 跨租户读取、写入、更新和删除 |
| Cognition Item | 提案及确认数据隔离 |
| Knowledge 查询 | 查询过滤条件是否完整 |
| MCP 接口 | 调用主体及 Team / Project 绑定 |
| Agent 执行 | 执行上下文中的租户身份 |
| 后台任务 | 异步任务的租户上下文传递 |
| Event / Outbox | 事件生产和消费时的租户校验 |
| 跨域引用 | ObjectReference 目标对象租户校验 |
| 数据库访问 | SQL、Repository、事务及批量操作 |

重点代码入口：`knowledge.rs:424-438`、旧 ai 栈 Project 查询与访问控制、Project↔Team 归属校验、MCP 鉴权与项目绑定、Agent/Worker/后台任务及事件消费者、所有能读写 `ai.project_knowledge`/`ai.cognition_item` 的 Repository/Service。**必须通过代码搜索补全实际入口清单。**

### 3.3 不在范围内

修改 Schema / 增删字段 / 重构 Project Cognition 对象模型 / 统一新旧栈结构 / 重写权限系统 / Git 正文权威迁移 / 清理历史数据 / 修改四域裁决。发现相关问题只登记差异、证据和建议。

## 4. 统一审计方法

### 4.1 代码级追踪方法

按序：找到外部访问入口 → 确认认证主体如何建立 → 追踪主体传入 Service → 追踪权限与归属校验 → 追踪 Repository/SQL 实际查询条件 → 检查写入和状态转换 → 检查后台任务及旁路入口 → 设计正向和负向测试 → 记录证据和结论。

**不能仅凭**：API 返回 403 / 代码包含 tenant_id / 查询使用 project_id / 类型名含 Human / 注释声称 AI 无确认权——就认定安全机制成立。必须证明约束在实际执行路径中不可绕过。

### 4.2 证据要求

每条结论至少含：代码证据（路径:函数:行号）/ 调用链证据 / 数据库证据（查询条件、约束、事务边界）/ 身份证据（Principal 来源及类型）/ 测试证据（输入、过程、结果）/ 审计证据（日志或事件中的实际主体）。每条证据标记：已验证 / 仅代码推断 / 尚未验证 / 与其他证据冲突。

### 4.3 结论分类

六分类（MATCH/PARTIAL/MISMATCH/MISSING/STUB/UNKNOWN）+ A-04 差异四分（Schema/Contract/Implementation/Verification Gap），分别描述符合程度与差异性质，不得相互替代。

# 5. V-1：Project Cognition 人工确认主体门专项审计

## 5.1 V1-01 主体身份真实性

确认操作的主体是否真实来自认证上下文？检查：AuthedPrincipal 定义/创建位置/是否由可信中间件建立/kind 是否服务端决定/用户能否经请求参数、请求体或 Header 自行指定主体类型/内部服务调用能否直接构造 Human Principal/Agent·Service 能否被转换成 Human。

**通过标准**：确认主体必须由可信认证链建立，调用方不能自行声明自己是 Human。

## 5.2 V1-02 人工主体强制约束

所有正式确认入口是否强制 Human Principal？检查：确认入口/Service 层二次校验/部分入口校验/其他正式认知状态写入入口/批量或后台自动确认/经通用审批接口间接确认。特别检查 API、Service 内部方法、Repository 直调、批量处理、后台 Worker、事件消费者、管理员或系统内部接口。

**通过标准**：任何能导致正式认知被确认或发布的路径都受不可绕过的人工主体约束。**仅有前端隐藏按钮或 API 层校验，不构成通过。**

## 5.3 V1-03 AI / Service 主体冒用测试（负向）

| 编号 | 主体 | 操作 | 预期 |
|---|---|---|---|
| V1-T01 | Human Principal | 正常确认 | 通过授权及业务校验后允许 |
| V1-T02 | Agent Principal | 正式确认 | 拒绝 |
| V1-T03 | Service Principal | 正式确认 | 拒绝 |
| V1-T04 | 未认证主体 | 正式确认 | 拒绝 |
| V1-T05 | Agent 改请求字段声明为 Human | 正式确认 | 拒绝 |
| V1-T06 | Service 改请求字段声明为 Human | 正式确认 | 拒绝 |
| V1-T07 | 直接调用内部 Service | 正式确认 | 不能绕过主体门 |
| V1-T08 | 后台任务模拟确认 | 正式确认 | 拒绝（除非有明确且经裁决的人工授权委托机制） |

系统未实现相关主体类型或测试基础设施时，必须记录为 UNKNOWN 或 Verification Gap，**不得伪造测试通过结果**。

## 5.4 V1-04 确认与审批语义隔离

通用审批完成是否被错误当作 Project Cognition 正式确认？检查 approval_request 业务语义/审批完成后是否自动更新 cognition_item/自动转换路径/ProjCognition 自身决策可识别性/跨域审批记录是否被当确认凭证。

**通过标准**：approval_request 完成状态不能自动证明 ProjCognition 正式对象已完成正确人工确认；跨域审批记录可作证据或关联引用，但不能代替自身确认身份。

## 5.5 V1-05 正式认知状态变更约束

检查 Proposed→Confirmed/Active→Superseded→Retired 等全部转换：哪些属正式确认、哪些允许 AI、哪些必须由人、哪些属系统维护、是否存在跳过确认直接进入正式状态的路径。

**通过标准**：需人工确认的转换都有明确主体约束和可追溯决策记录；人工决策与系统维护必须明确区分。

## 5.6 V1-06 决策审计完整性

检查：决策主体/时间/对象/前后状态/结果/依据/关联 Proposal·Knowledge/审计事件。能回答：谁确认、何时、哪个版本、前后状态；**决策记录是否可被事后篡改**。只有 decided_by 但无法验证真实主体，不能判定通过。

## 5.7 V-1 最终判定

| 结果 | 判定条件 |
|---|---|
| MATCH | 所有正式确认入口强制 Human Principal、不能冒用、状态转换和审计完整 |
| PARTIAL | 主入口有人工约束，但其他入口、状态转换或审计链不完整 |
| MISMATCH | 存在 Agent/Service 绕过人工确认的可行路径 |
| MISSING | 缺少必要的人工确认机制或业务决策身份 |
| STUB | 仅有接口、注释或占位逻辑 |
| UNKNOWN | 关键代码、测试或身份基础设施证据不足 |

**风险升级**：发现 Agent/Service 能未经授权完成正式确认、或能伪造 Human Principal → 立即 P0。

# 6. V-2：旧 ai 栈租户隔离专项审计

## 6.1 V2-01 Team / Project 归属关系

访问 Project 时是否验证其属于当前 Team？检查：归属字段与读取方式/Team 身份来源/Project ID 与 Team ID 是否同时参与授权/是否只凭 Project ID 放行/Project 转移、归档、删除后历史任务是否仍有效。

**通过标准**：每次访问都验证 Team 归属及权限，不依赖调用方提供的 Project ID。

## 6.2 V2-02 旧栈查询过滤

覆盖 ai.project_knowledge / ai.cognition_item / 相关 Proposal·Evidence / Project 关联查询 / 批量读写 / 统计列表 / 历史读取。每查询记录：主体 Team 来源/目标 Project 来源/Team 校验/Project 归属/SQL 条件/写入范围/事务边界（TOCTOU）。

特别关注：只过滤 project_id；反查 Team 但未校验当前主体；列表校验详情不校验；读取校验更新删除不校验；批量只校验第一条；Repository 默认信任调用方传入 Project ID。

**通过标准**：不能通过替换 Project ID / Object ID / 引用 ID 访问其他 Team 的对象。

## 6.3 V2-03 HTTP API 访问隔离（负向测试）

| 编号 | 操作 | 预期 |
|---|---|---|
| V2-T01 | Team A 访问自己 Project | 按权限允许 |
| V2-T02 | Team A 访问 Team B Project | 拒绝 |
| V2-T03 | Team A 用 Team B Project ID 查 Knowledge | 拒绝 |
| V2-T04 | Team A 用 Team B Object ID 查详情 | 拒绝 |
| V2-T05 | Team A 修改 Team B Knowledge | 拒绝 |
| V2-T06 | Team A 删除 Team B 对象 | 拒绝 |
| V2-T07 | 批量请求混入 Team B 对象 | 拒绝越权部分 |
| V2-T08 | Team A 引用 Team B 对象 | 拒绝 |
| V2-T09 | 无权限成员访问受限对象 | 拒绝 |
| V2-T10 | Project 与 Team 归属不匹配 | 拒绝 |

区分五级主体（未认证/无 Team 成员资格/成员无项目权限/项目权限有效/目标属其他 Team）。**不能把所有拒绝都归为 403——必须确认拒绝前没有发生数据读取或写入。**

## 6.4 V2-04 MCP 租户隔离

MCP 是否继承可信 Team/Project 上下文？检查：认证方式/API Key→主体映射/请求中 Project ID 可信性/Device-Project Binding 验证/工具参数能否替换目标 Project/是否复用正常 API 权限校验/MCP 内部有无直连旧栈 Repository 旁路。测试：Team A 凭证访问 A/B 项目、替换工具参数 Project ID、替换对象 ID、无绑定设备访问。

**通过标准**：MCP 不得因为调用者知道目标 Project ID，就获得访问该项目的能力。

## 6.5 V2-05 Agent 与后台任务租户上下文

检查 Run 创建/恢复/重试/Worker 消费/定时任务/异步队列/事件处理/失败恢复/批量任务。重点：创建时校验执行时不校验；只存 Project ID 不存可信租户上下文；Worker 系统级权限读任意 Project；重试丢失原始主体；事件消费者按对象 ID 直读；已归档或已撤权的 Project 仍被旧任务写入；任务参数可被替换。

**通过标准**：异步执行必须保留并验证可信租户上下文；权限撤销、归属变化、任务恢复时不得出现未授权访问。

## 6.6 V2-06 跨域引用隔离

检查 DomainRef 四元组（domain/object_type/object_id/revision）：引用解析是否验证当前 Team/是否经目标 Project 反查 Team/是否验证当前主体对目标对象的访问权/目标不存在是否安全失败/合法四元组能否引用其他 Team 对象/缓存与异步解析是否绕过校验。

**通过标准**：四元组只解决定位，不自动赋予跨域或跨租户访问权；每次解析都验证租户归属和访问权限。

## 6.7 V2-07 数据库与后台访问路径

检查：SQL 缺租户过滤/Repository 默认信任调用方/事务中校验与写入对象不一致/批量 SQL 遗漏租户条件/仅凭 UUID 访问对象的内部方法/高权限连接绕过应用授权/可跨 Team 查询的后台通用函数。DB 无双键字段不自动=漏洞；但应用层无法保证每条路径都执行可靠归属校验时，必须登记为实际安全缺陷。

## 6.8 V-2 最终判定

| 结果 | 判定条件 |
|---|---|
| MATCH | 所有入口和异步路径均验证归属，无跨租户路径 |
| PARTIAL | 主路径安全，存在未验证接口/任务/引用路径 |
| MISMATCH | 存在可执行的跨租户读取或写入路径 |
| MISSING | 缺少必要的租户授权机制 |
| STUB | 权限接口或占位校验存在但未生效 |
| UNKNOWN | 无法获得完整访问链或完成必要测试 |

**风险升级**：发现任何未经授权的跨租户读取、写入、删除、确认或发布能力 → 立即 P0。**不能以生产数据为空为由降低风险等级。**

# 7. 测试数据与执行安全

**测试环境**：独立环境，准备 Team A/B、A 普通成员、A 项目管理员、B 项目成员、Agent Principal、Service Principal、未认证主体、两 Team 各自 Project 及 Knowledge/Cognition Item。测试数据明确标记，不与生产混用。

**禁止**：对生产数据越权攻击/改生产权限/伪造生产人工确认/改正式认知状态/删真实知识对象/直改生产 DB/未批准 Migration/为通过测试临时绕过授权。无法安全复现时记未验证并提出环境准备要求。

**测试结果记录**：编号/环境/主体/对象/前置权限/请求方式/预期/实际/是否发生数据读取/是否发生写入/日志审计/结论。**拒绝响应本身不能证明没有数据泄露——必须检查拒绝前是否已执行敏感查询或副作用。**

# 8. 风险分级与处理规则

- **P0（立即）**：Agent/Service 未授权完成确认；伪造 Human Principal；Team A 读写 Team B 数据；MCP/Worker 跨租户路径；跨域引用绕租户校验；替换对象 ID 绕过权限。处理：登记→存证→影响范围→通知负责人→单独修复任务→修复复测前不关闭。
- **P1（重要）**：部分确认路径缺主体校验但未证实可利用/后台任务缺租户上下文验证/审计不能完整追溯/跨域引用校验不完整/关键负向测试未覆盖。列入优先修复，相关能力正式交付前关闭。
- **P2（一般）**：主体类型契约不清/审计字段不全/内部接口权限表达不统一/重复实现归属校验。进后续设计，不得仅为统一代码形式重构。
- **UNKNOWN**：证据不足必须保留；不能当 MATCH，不视为已通过。

# 9. 交付物

- **D01** V-1 人工确认主体门代码审计报告（入口清单/身份建立路径/校验位置/状态转换/绕过分析/审计完整性/V1-T01~T08 结果/六分类/风险与建议）
- **D02** V-2 租户隔离代码审计报告（API/MCP/Agent·Worker 入口清单/Repository·SQL 清单/归属验证路径/跨域引用/V2-T01~T10 结果/六分类/风险与建议）
- **D03** 代码访问链索引（入口|身份来源|权限校验|Service|Repository·SQL|最终读写，全环节路径:函数:行号）
- **D04** 安全测试证据包（环境/主体与数据/脚本或请求/预期实际/失败用例/日志审计/覆盖范围）
- **D05** 风险与缺陷清单（编号/专项/等级/影响对象/复现路径/代码证据/实际影响/差异分类/推荐修复/是否需 Schema 变更/是否需重新审计）
- **D06** 签收表（V-1/V-2 最终结论、P0/P1 数量、UNKNOWN 数量、是否允许关闭专项、是否允许进入下一阶段、未关闭风险责任归属）

# 10. 完成标准

**V-1**（须同时满足）：全部确认入口已识别 / Human Principal 可信来源已验证 / Agent·Service 无法冒用 / 状态转换均有主体约束 / 通用审批不能替代自身确认 / 决策主体与版本可完整追溯 / 必需负向测试已执行 / 无未解释旁路。

**V-2**（须同时满足）：HTTP/MCP/Agent·Worker/Repository·SQL 入口覆盖 / 归属验证确认 / 跨域引用执行权限校验 / 负向测试证明跨租户被拒 / 无未解释跨租户路径。

**整体**：完成≠缺陷全部修复，但关键入口全识别、测试全有结果、问题全有等级、P0 已升级、UNKNOWN 有责任人及计划、修复与审计分离、无未批准 Schema 变更。**只有 V-1 与 V-2 均达 MATCH 且无未关闭 P0/P1 才能签收通过**；存在未证实可利用的 P1 须负责人明确接受风险并记录例外。

# 11. 执行顺序

| 阶段 | 内容 | 产出 |
|---|---|---|
| S1 | 锁定代码快照与证据基线 | 提交号、审计环境 |
| S2 | 识别 V-1 全部确认入口 | 入口清单 |
| S3 | 追踪 V-1 身份与状态转换 | 确认调用链 |
| S4 | V-1 负向测试 | 测试结果 |
| S5 | 识别 V-2 全部访问入口 | 入口清单 |
| S6 | 追踪 API/MCP/Worker/SQL | 隔离调用链 |
| S7 | V-2 负向测试 | 测试结果 |
| S8 | 汇总风险与差异 | 缺陷清单 |
| S9 | 专项审计报告 | D01~D06 |

V-1/V-2 可并行，但须同一代码快照、同一证据格式与风险标准。

# 12. 与整体四域项目的关系

本专项是 V1.1 的优先验证工作，不改六阶段安排。结果影响：ProjCognition 决策身份设计 / Review 契约 / 旧栈租户权限模型 / 新旧栈引用与访问边界 / P1 E2E 可信度 / Migration 与安全修复优先级。

特别明确：V-1 不得靠加 decided_by 字段宣告完成；V-2 不得靠加 tenant_id 字段宣告完成；结构符合≠权限执行可靠；API 拒绝≠底层无读取；存在校验≠所有路径都校验；发现问题不授权直接改 Schema。

# 13. 最终审计结论模板

**V-1**：结论分类/差异类型/确认入口总数/已审计数/Human Principal 校验/冒用测试/状态转换约束/决策审计完整性/P0·P1·P2 数量/UNKNOWN 数量/是否通过。

**V-2**：结论分类/差异类型/API·MCP·Worker·SQL·跨域引用覆盖/跨租户负向测试/P0·P1·P2 数量/UNKNOWN 数量/是否通过。

**总裁决**：V-1 通过？V-2 通过？存在 P0？存在未接受 P1？未关闭关键 UNKNOWN？允许关闭 V-1/V-2？允许进入 P1 E2E？允许进入 P2 Analysis 详细设计？允许启动 Schema Migration？

**默认规则**：专项通过只表示安全边界与人工确认门已充分验证，不代表四域架构完成，也不代表允许直接实施 Migration。

---

**文档结论**：本任务书是 V-1/V-2 专项审计执行基线。以指定代码快照为依据，完整追踪入口、身份、权限、查询、写入和审计链路。任何未验证的安全约束不得标记为已实现；任何实际跨租户访问或确认门绕过能力必须优先登记 P0。本任务书不修改 Q1~Q12 裁决、不改变四域边界、不授权 Schema 变更。

---

## 附：本仓执行状态（2026-11-16）

- **S1 代码快照**：master 05ce33d8（与 9 分册审计同基线）。
- **V-1 代码级复核（S2/S3 等价）已完成** → 映射任务书 §9：结论 **PARTIAL**。要点：AuthedPrincipal 无 kind 字段、decide_item 仅 tenant+成员校验；service JWT 被拒（auth.rs:327）但 cpk_ 通道以 owner 真人 principal 直接放行（auth.rs:318,240-284）；MCP 无决策工具、状态变更单点（FOR UPDATE 仅 proposed）；审计含 client/auth_mode；拒绝测试缺失（7 测试全 happy-path）。R1=P1 非 P0（需真实凭据+成员身份）、R2=决策端点对齐 pc ensure_formal_authorized 口径（pc/mod.rs:157-162）或改收 HumanActor 强类型、R3=补三类拒绝测试。
- **待执行**：S4 负向测试 V1-T01~T08（需测试主体基础设施：Agent/Service Principal 凭据）；S5~S7 V-2（代码级追踪进行中，负向测试待环境）；S8~S9 汇总与 D01~D06。
- **V1-04 已有部分证据**：governance.approval_request 用于导入链审批（advance.rs:99），与 cognition_item 确认（decide_item）无代码关联——语义隔离初步成立，待负向测试确认。

## 附二：V-2 复核结果（2026-11-16，代码级，关键证据已人工二次验证）

**结论：V-2 = MISMATCH——存在真实跨租户访问路径，按 §6.8/§8.1 升级 P0（5 项）**。生产模式（AI_PM_AUTH_MODE=jwks 默认 fail-closed）下 routes.rs:396 router_with_auth 对同一路由表挂 require_bearer，故任意**已认证用户**（任意租户）可触达以下 legacy 端点——它们只验 JWT、不验项目成员/租户归属：

| 编号 | 端点（routes.rs） | 已验证证据 | 影响 |
|---|---|---|---|
| **P0-1** | POST /api/v1/events（:232→handler :532） | handler 签名仅 State+Json，无 `Extension<AuthedPrincipal>`（已亲验）；submit_event（ingestion.rs:70-95）只查 project 存在 | 任意租户登录用户可对任意 project_id 注入事件+ai_analysis job（触发受害项目 AI 分析、actor_id 由客户端声明伪造审计） |
| **P0-2** | POST /api/v1/supervision/approvals/{id}/decision（:235→handler :615） | handler 无 principal（已亲验）；decide_approval（supervision/approval.rs:24-35）仅按 approval_id 定位 | 批准其他租户红线审批 → AI 命令在受害项目**真实执行（跨租户写）** |
| **P0-3** | POST /api/v1/supervision/veto（:233→handler :574） | veto.rs:20-26 仅 `WHERE id=$1 AND project_id=$2`；`AuditActor::principal(req.actor_id)` 主体取自 body（已亲验） | 回滚其他租户已执行的 AI 命令（executor.compensate） |
| **P0-4** | POST /api/v1/supervision/freeze\|unfreeze（:238-239→handler :668/:691） | project_id/actor_id 全由 body 提供 | 翻转其他租户项目 automation_mode |
| **P0-5** | GET /api/v1/projects/{id}/notifications（:241→handler :712） | feed.rs:13-18 `WHERE project_id=$1` 无 tenant（已亲验） | 跨租户读 notification payload |

**其余检查项结论**：
- V2-01/后台任务：PASS——job_queue.rs:34-73 claim JOIN ai.project_ai_config、tenant 一律由 core.project 反查派生（outbox_handlers.rs:371）；但事件入队口被 P0-1 击穿后后台会忠实执行任意租户事件。
- V2-04 MCP：PASS——caller.rs:36-56 只认鉴权层 HumanActor、authz.rs:33 成员+双 tenant 绑定、pm_submit_result 仅 run_id+lease_token 无伪造面、cpk_ 凭据仅限 /mcp 面（auth.rs:224-227）。
- V2-02 旧栈 SQL：PASS（有外围守卫时）——knowledge.rs:358/658 等 WHERE project_id 查询的 project_id 均来自已过 project_visible 守卫的 handler；例外即上表 5 点。
- V2-06 跨域引用：PARTIAL——kb.memory_ref 非 KB 域引用仅结构校验不验目标归属（memory.rs:175-227，P2 悬空指针）；pc.source target_* 无存在性/同租户校验（evidence.rs:17-63），analysis_report.rs:228-250 mark_analyzed 全局 LIMIT 1 可能错配；pc.evidence 主体与 document_section 已校验、其他 evidence_type 的 target_* 不校验。

**修复方向（单独安全修复任务，本审计不执行）**：5 个 legacy 路由统一接入 `require_project_visible`（support.rs:199）并从 AuthedPrincipal 取 actor、禁止 body 传 actor_id；对照正面样板 kb_memory_api.rs:73-89 / knowledge_services.rs:141-147。

**对整体计划的影响**：按任务书 §13.3 默认规则——P0 未关闭前**不允许进入 P1 E2E**（E2E 会真实驱动这些端点）；P2 Analysis 详细设计可并行。§10.3 签收：V-1=PARTIAL（R1~R3 待修）、V-2=MISMATCH（P0×5）、专项不通过、禁止启动 Migration。

## 附三：专家终审意见落位 + R1 专项验证记录（2026-11-16）

**专家终审**（审阅截至 V-2 完成前版本）：V-1 PARTIAL 合理；cpk_ 通道身份代理是最值得深挖的问题——R1 暂定 P1 待验证，若非人工执行者可借此完成确认则升 P0；补 V1-T09~T12；R2 不照抄 PC 实现（先比身份契约，保持领域独立，PC 审批记录不得替代 ProjCognition 决策身份）；R3 测试分三组（身份拒绝/身份伪造/授权失效）且必须查 DB 写入而非只看 HTTP 状态；V-2 以全部数据访问入口为中心建清单（project_id 过滤≠Team 隔离）。

**R1 专项验证结论（代码级已证伪，原 P1 判定撤销）**：
| 专家四属性 | 实证 |
|---|---|
| 凭据签发主体 | device.owner_principal_id（devenv.device，Q-C 0049 Client Registry） |
| 凭据实际持有者 | device（凭据=key_hash 绑定设备，非身份冒名） |
| 凭据授权范围 | granted_permissions 三码白名单（pm_devices.rs:305-331，签发后可收紧）+ **路径门 auth.rs:225-227（仅 /mcp，业务端点 IAM_AUTH_1007 401）** |
| 撤销与失效 | credential_revoked_at 即时生效（device_credential.rs:196，下一请求 401，无缓存）+ 四项链每请求直查（:273-308：key_hash/status=active/owner 同租户且 active/未撤销） |

**语义裁定（回答专家核心问题）**：Owner 在 cpk 通道是**授权人**而非每次操作的决策人——权限被白名单收窄且 /mcp 工具面无确认工具。**"AI 借 Owner 人工身份完成正式确认"在当前代码不可达**。

**残余动作**（并入 S4）：
- V1-T09 Owner 真人凭据确认=允许（已有 happy-path 覆盖，需显式断言 decided_by=owner）
- V1-T10 Agent/Worker 持 cpk 打业务决策端点 → 断言 401 IAM_AUTH_1007（回归路径门）
- V1-T11 Service 持 cpk 同上
- V1-T12 Owner 撤销成员资格后旧 cpk 凭据 → 断言 /mcp 401（四项链 owner_active 校验）
- R2/R3 维持原判（显式 kind 校验缺失、拒绝测试缺失），R2 按专家意见先比身份契约再定方案，不照抄 PC。

**V-2 状态更正**：专家审阅基于 V-2 完成前的版本。**V-2 已完成并升 P0×5**（见附二，关键证据已人工二次验证：events/approvals/veto/freeze|unfreeze/notifications 五个 legacy 端点仅验 JWT 不验归属）。专家 V-2 优先级清单已被代理覆盖（第 1~4 优先级均扫过）。执行序更新：S4 负向测试（含 T09~T12）→ P0 修复（单独任务）→ 复测 → D01~D06 签收。

## 附四：阶段 A1~A3 执行记录（2026-11-16，DEC-SEC-P0-01）

**裁决**：DEC-SEC-P0-01——批准 SEC-P0-1~5 统一修复（require_project_visible + AuthedPrincipal 主体来源 + 对象归属校验），不授权 Schema Migration，不代表 V-1/V-2 专项通过。

**A1 修复**（commit `ee4cd046`，新代码快照基线）：
- routes.rs 新增 `require_legacy_project_access`（统一守卫：off 模式豁免，jwks 生产走 require_project_visible 不可见即 404）；五 handler 全部接 `Extension<AuthedPrincipal>`；
- decision（P0-2）：`approval_head(tenant, approval_id)` 从审批对象反查 project（guard_query 仓储，tenant 锚定）→ 404 不泄露存在性 → 再验成员可见性；
- veto（P0-3）：caller 对 req.project_id 守卫 + 服务层 `WHERE id AND project_id` 配对 ⇒ 合取"caller 是 proposal 所属 project 成员"（A-03 链式成立）；
- actor：全部改取 caller（`audit_actor()` 完整身份 / `principal_id`），body actor_id 仅保留反序列化兼容不再采信（A-02）；
- 零 Schema 变更；SQL 门禁 PASS（support.rs 豁免内=1）。

**A2 人工复核（对照 D03 C7~C11）**：
- C7 events：guard→ingestion，拒绝时无 event/job 写入 ✓
- C8 decision：approval_head→404→guard→decide_approval，命令执行仅在双重校验后 ✓
- C9 veto：guard 先于 veto()，补偿不会在授权失败后触发 ✓
- C10 freeze/unfreeze：guard 先于 set_automation_mode ✓
- C11 notifications：guard 先于 feed 查询（拒绝路径零读取）✓

**A3 G1 复测**（`security_p0_regression.rs`，5/5 绿，auth-on 真 JWT）：
- 每缺陷：攻击路径 404 + DB 零副作用（approval 仍 pending/proposal 不变/无补偿/automation_mode 不变/payload 不泄露，A-04/A-06）；
- 合法路径双向断言：userA 对 projectA1 仍 202/200/rejected/rolled_back/frozen（A-05）；decided_by=认证主体落库（A-02）。

**连带测试对齐（守卫语义暴露的既有夹具错位）**：
- auth.rs `valid_token_reaches_the_real_route`：tid=1 identity 原映射到随机新租户（守卫前无成员校验故绿），改标 SYSTEM_TENANT 后恢复绿；
- mcp_permission `non_mcp_routes_ignore_the_permission_claim`：无成员主体按可见性口径改断 404（仍证明业务路由无权限码门控）；
- **存量失败登记（非本修复引入，干净 master 已复现）**：pc_proposals p3_expands/p5_approve 两用例环境敏感 flaky（首次全量曾绿），另行排查。

**全量 pm-api 套件**：除上述存量 flaky 外全绿。**阶段 A 完成，G1 通过 → 解锁 B~E**（B 测试基础设施、C 负向测试、D 业务链、E 签收）。

## 附五：阶段 B+C 执行记录（2026-11-16）· D04 证据包

> 代码快照 master `8b25177f`（含 ee4cd046）。只新增/重构测试代码，零业务代码、零 Schema、零 Migration。
> 完整证据（环境/主体/逐用例预期实际/DB 前后状态/失败与 UNKNOWN）：
> [`v1-v2-d04-security-test-evidence.md`](v1-v2-d04-security-test-evidence.md)。

**B1 测试基础设施**（`apps/pm-api/tests/l2_security/mod.rs`）：
- 双租户夹具：A=`SYSTEM_TENANT`（lmms:1 identity 映射）、B=独立 tenant（lmms:2，bootstrap）；
- 凭据签发器：Human interactive JWT / Service JWT / Agent(dev_agent) JWT / cpk（devenv.device + device_credential 三码）/ 未认证；
- 断言库：`expect_cross_tenant_denied`（HTTP 4xx + 12 表项目快照逐字不变）与
  `expect_cross_tenant_read_denied`（HTTP 拒 + 响应逐标记不含目标租户数据）；
- FakeModel Fixture：`fake_model::FIXTURE_VERSION = "fake-model-v1"` 接口预留（阶段 D 用）。

**B2 夹具复用**：`security_p0_regression.rs` 重构为共享模块；P0 复跑 **5/5 绿**。

**C 负向测试结果**（`cargo nextest`，auth-on 真 JWT，sqlx::test 每用例独立库）：
| 套件 | 结果 |
|---|---|
| P0 回归组 | 5/5 PASS |
| V1-T01~T12 + V1-EXT-01 + R3 | 14/14 PASS |
| V2-T01~T10 | 9/9 PASS |
| 合计 | **28/28 PASS** |

要点：
- **P0×5 关闭**：攻击路径 404 + DB 零副作用 + 合法路径双向成功；
- **V1**：T01 真人确认落 decided_by/confirmed_by；T03/T04/T06 拒绝；T09~T12 + R3 路径门/四项链/撤销即时全部拒绝；
  **T02/T05 为 Characterization（缺陷现状）**：Agent(dev_agent) 可确认 cognition item；
  **V1-EXT-01 新增发现**：Baseline 逐项决策入口（`pm_baseline.rs:135-172`）同样无 Human 门——并入 SEC-P1-1；
  T07-A 登记 UNKNOWN（无独立决策应用服务层）；T07-B 登记 Repository 调用边界；T08 以 `tools/list` 证明无 cognition 决策工具；
- **V2**：T01~T05/T07/T09/T10 全拒绝且零副作用/零泄漏；**T06 UNKNOWN**（无 DELETE 端点）；
  **T08-C Characterization**：`pc.source` 与 `kb.memory_ref` 接受跨租户目标（SEC-P2-1/2 基线），T08-R 待修复；
- 存量 flaky（pc_proposals p3/p5）本轮未复现，不在本任务范围。

**结论流转（含 SEC-P1-1 修复）**：
- SEC-P1-2（拒绝测试缺失）**关闭**；
- **SEC-P1-1 已按报批方案修复**（`support.rs::require_human_decision_context` 接入
  cognition + baseline 两个逐项决策入口；零 Schema）；T02/T05/EXT-01 由
  Characterization 转为 Regression，另增 API-Key 拒绝用例；
- 复测：L2 三套件 **29/29 PASS**；`pm_cognition` + `pm_baseline` 既有 happy-path 4/4 PASS；
- V-1 = **MATCH**；V-2 = **PARTIAL**（仅 SEC-P2 引用链，非 P0/P1）；
- **G2 门条件全部满足** → 允许进入 G3（P1 真实业务链）。

**交付物**：
- [`v1-v2-d04-security-test-evidence.md`](v1-v2-d04-security-test-evidence.md)（含 §9 修复复测记录）
- [`v1-v2-d05-d06.md`](v1-v2-d05-d06.md)（D05 状态流转 + D06 G2 核对表）
- [`proj-cognition-human-gate-fix-taskbook-v1.md`](proj-cognition-human-gate-fix-taskbook-v1.md)（已执行）


