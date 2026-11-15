# 交付物 05：P1 真实业务链 E2E 测试方案 V1.1

> 2026-11-16 · 上游：三域裁决 V1.0（六 Phase P1）、四域对象模型 §14、V-1/V-2 专项任务书（S4/S7 负向测试共用本方案基础设施）
> 定位：**测试设计方案**——本方案不被用来证明当前代码已具备完整 E2E 能力；实际能力判定须待基础设施落地、测试执行并产生 D04/D06 证据。
> **版本记录**：V1.0 专家评审=技术路线认可，修改后批准；V1.1 吸收 M1~M4+E1~E3+G1~G4；**2026-11-16 终审：主体技术设计通过，吸收 4 处边界修订（F1~F4，见 §8）后正式冻结为测试设计基线（版本号保持 V1.1）**。冻结=批准测试设计，≠批准 SEC-P0 修复、Schema Migration 或启动真实业务链；实施授权尚未授予。**V1.0 原文结构保留。**

## 1. 载体分工（对齐五层测试策略）

| 层 | 载体 | 承载内容 |
|---|---|---|
| L2（Rust 集成，进程内 router） | `apps/pm-api/tests/*_security.rs`（新模式：`router_with_auth` + 测试 JWKS 签发器） | **负向安全测试**：V1-T01~T12、V2-T01~T10、P0 回归组。理由：需双租户夹具、服务 JWT 签发、cpk 凭据签发、DB 直查断言"拒绝前无写入" |
| L3/L4（playwright p0 栈） | 新 `playwright.chain.config.ts` + `e2e/chain/*.spec.ts`（复用 p0 栈模式：一次性库+JWKS+真 pm-api+真 worker+FakeModel） | **真实业务链** V-3/V-4：仓库分析→报告→KB 确认→PC Proposal→人工审批→发布→Close |

## 2. 测试夹具（一次性基础设施，双向复用）

### 2.1 双租户夹具（L2/L3 共用）

```
tenantA / tenantB（core.tenant 各一）
  userA1（tenantA 交互真人，interactive）  userA2（tenantA 无项目成员资格）
  ownerB（tenantB，持 tenantB 项目）
projectA1 / projectB1（各归其租户，project_member 各自配置）
```
- 种子方式：直接 INSERT（sqlx 夹具/playwright 全局 setup），全部标记 test 前缀。
- **断言库（M4+F3）**——读写分离两种断言：
  - `expect_cross_tenant_denied`（**写操作**）：HTTP 4xx **且** DB 快照前后无变化 **且** 无后台任务/外部副作用；SEC-P0-2/3 额外断言命令执行表与补偿记录无变化；
  - `expect_cross_tenant_read_denied`（**读操作**，如 SEC-P0-5）：HTTP 拒绝 **且** 响应体不含目标租户数据（逐字段断言）**且** 无未授权数据输出审计记录（如有该机制）——DB 前后一致对 GET 无证明力。

### 2.2 主体凭据夹具

| 主体 | 签发方式 |
|---|---|
| Human interactive | 测试 JWKS 签 JWT（claims 含 tid/sub，auth_mode=interactive 路径） |
| Service JWT | 同 JWKS 签 service 类 claims（预期被 resolve_principal :327 拒） |
| Agent/dev_agent principal | core.principal kind='dev_agent' 行 + 其凭据 |
| cpk 凭据 | devenv.device + device_credential 签发（owner=userA1），granted_permissions 三码 |
| 未认证 | 无 Authorization 头 |

### 2.3 FakeModel 确定性规范（E3）

FakeModel 不只固定 LLM 输出，**全部输入同样固定**，测试输入/输出作为 **Fixture 版本化**（失败可复现）：
- 输入仓库 commit（固定夹具仓库+固定 HEAD）；
- 分析任务参数与 config_snapshot；
- 模型输出结构与版本号；
- 时间相关字段（注入固定时钟或断言相对时间）；
- 异步任务重试行为与 Worker 并发顺序（单 worker 串行或显式排序）；
- Proposal 生成顺序、发布批次及排序规则（保证 snapshot_hash 稳定可复算）。

## 3. 测试类别与用例（M1：三分类，M3：T07 拆分）

**三分类口径（M1）**——只有 Security Regression 与 Business E2E 计入安全/业务通过：

| 类别 | 目的 | 计入通过 |
|---|---|---|
| **Characterization（现状特征）** | 记录修复前已知缺陷的真实表现，作为修复基线证据 | **否** |
| **Security Regression（安全回归）** | 修复后验证攻击路径被拒绝 | 是 |
| **Business E2E（业务链）** | 验证合法业务链完整成功 | 是 |

### 3.1 V-1 组

| 编号 | 类别 | 步骤 | 断言 |
|---|---|---|---|
| V1-T01 | Regression | userA1 正常 confirm projectA1 cognition_item | 200；decided_by=userA1；knowledge 落行 |
| V1-T02/T03 | Regression | dev_agent / service JWT 调 decision | 401/403；cognition_item 状态不变 |
| V1-T04 | Regression | 未认证 | 401；无写入 |
| V1-T05/T06 | Regression | 请求体/header 伪造 kind=human 声明 | 同 T02（服务端不读声明字段） |
| **V1-T07-A**（M3） | Regression | **服务层授权边界**：以非 Human Actor（Agent/Service CallerContext）调用正式决策**服务** | 拒绝；DB 无状态变化。验收依赖 SEC-P1-1 身份契约落地（HumanActor 强类型或 kind 校验），落地前为 Characterization |
| **V1-T07-B**（M3） | Characterization | **Repository 层内部调用边界**：登记 repo decide_item 的可达调用方清单与实际保护机制，**不要求 Repository 自行识别 JWT/Human 身份**（身份授权职责在 Application/Service 层） | 产出调用边界登记表，作为 SEC-P1-1 设计输入 |
| V1-T08 | Characterization | worker 上下文模拟 confirm | 无路径可达（代码断言+运行时无入口） |
| V1-T09 | Regression | owner cpk 凭据 → 决策端点 | 401 IAM_AUTH_1007（路径门回归）；/mcp 面正常 |
| V1-T10/T11 | Regression | Agent/Service 持 cpk → 决策端点 | 同 T09 |
| V1-T12 | Regression | 撤销 userA1 project 成员资格后，旧 cpk 调 /mcp 工具 | 401/404（四项链 owner_active+成员校验）；无写入 |
| R3 补充 | Regression | 凭据撤销（credential_revoked_at 置位）后任意调用 | 401（即时撤销回归） |

**R3 测试三组（专家口径）**：身份拒绝（T02~T04）/身份伪造（T05~T07）/授权失效（T12+凭据撤销）；全部断言 DB 无写入，不只看 HTTP 状态。

### 3.2 V-2 组

| 编号 | 类别 | 步骤 | 断言 |
|---|---|---|---|
| V2-T02~T04 | Regression | userA1 访问 projectB1 / 用 B 的 project/object id 查 knowledge | 404（不可见即 404）；B 侧无读写副作用 |
| V2-T05/T06 | Regression | userA1 改/删 tenantB knowledge 对象 | 404；无写入 |
| V2-T07 | Regression | 批量请求混入 B 对象 | 越权子项拒绝，合法子项正常 |
| **V2-T08**（M1 拆分） | **T08-C：Characterization** | 记录 kb.memory_ref 非 KB 目标 / pc.source.target_* 当前可达性（SEC-P2-1/2 现状基线） | 固化现状证据，**不计入通过**；SEC-P2 修复属后续阶段 |
| | **T08-R：Regression** | SEC-P2 修复落地后：跨租户引用被拒 | 届时启用，计入通过 |
| V2-T09 | Regression | userA2（无成员资格）访问 projectA1 | 404 |
| V2-T10 | Regression | project_id 与 tenant 不匹配构造 | 404 |

### 3.3 P0 逐项回归映射（M4）

每个端点执行**双向断言**：攻击路径（userA1→projectB1）拒绝且零副作用；**合法路径（ownerB→projectB1）仍成功**（防误伤）。

| 缺陷 | 攻击入口 | 攻击行为 | 修复后预期（Regression） | DB 副作用断言 |
|---|---|---|---|---|
| SEC-P0-1 | POST /api/v1/events | userA1 向 B 项目注入事件 | 拒绝 | 无 inbound_event/job 写入 |
| SEC-P0-2 | POST /api/v1/supervision/approvals/{id}/decision | userA1 批准 B 项目审批 | 拒绝 | **审批状态不变、命令未执行**（无 audit/revision 变化） |
| SEC-P0-3 | POST /api/v1/supervision/veto | userA1 回滚 B 项目命令 | 拒绝 | **命令状态不变、无补偿记录** |
| SEC-P0-4 | freeze/unfreeze | userA1 修改 B 项目 automation_mode | 拒绝 | 配置不变 |
| SEC-P0-5 | GET /api/v1/projects/{id}/notifications | userA1 读 B 项目通知 | 拒绝 | 不返回 payload |

修复前各端点先跑一次 **Characterization**（保留攻击成功证据存 D04），修复后 Regression 必须执行；SEC-P0-2/3 必须验证 DB 副作用（可能触发真实命令执行/补偿），不得只看 HTTP 状态。

**Characterization 环境安全约束（F2）**——防止为证明漏洞而真实攻击其他项目：
- 仅用一次性测试数据库与测试租户；
- **禁止连接真实外部执行器、生产 MCP、真实通知服务及一切生产级外部副作用通道**；
- SEC-P0-2/3 使用 **Fake Executor / 隔离副作用接收器**：保留命令执行路径与记录证据，不执行真实外部操作；
- SEC-P0-1 产生的任务只由受控测试 Worker 消费，或隔离环境明确禁止自动执行；
- 复现数据、请求、响应、DB 变化及执行器记录一并归档 D04。

## 4. 真实业务链用例（V-3/V-4）

### 4.0 阶段边界声明（M2）

**本阶段 E2E 按各领域当前有效权威模型执行。** 尚未进入实施阶段的能力（KB Git 正文权威=P3、PCGIT=P4、完整 Conflict 生命周期=P4、审阅中心全量接入=P4）**不作为 P1 主链通过前提**；相关断言以"当前实现行为验证"或 STUB 证据记录，**不得通过提前迁移 Schema 或双写实现来凑绿**。

### 4.1 主链（验收=裁决 P1 完成标准"至少一条真实完整业务链成功"）

```
场景：tenantA/projectA1 绑定真实 Git 仓库（fast-import 夹具或真远端）
S1  仓库 verify/analyze → repo_analysis_report(analyzing→analyzed)
      断言：报告门 409 生效（重复分析被拒且带 report_id）；commit_sha/since_commit 记录
S2  KB：kb.document_revision(draft, analysis_mode∈{derived,llm}) 生成
      断言：AI 直写仅 draft；【当前行为验证】正文权威=PG content（P3 目标不作断言）
S3  PC：derive_pc_cognition → cognition_object(draft)+proposal(ai_agent)+approval(pending)
      断言：lifecycle 恒 draft；AI 无 active 写入；pc.source/evidence 钉扎 KB 修订
S4  人工：confirm_revision（KB draft→published）+ decision（P5，approval→approved）
      断言：真人门（dev_agent 403）；触发器白名单；【当前行为验证】FTS 仅 published 后可检索
      （核实当前检索实现后按实际行为断言，不预设新架构迁移结果）
S5  发布/Close：close_report → approved 提案批量 publish_version
      断言：lifecycle→active；cognition_version/version_member 快照+snapshot_hash
      （【当前能力】publish 事务现状即支持，非 P4 迁移产物）；未决提案→withdrawn；
      KB 残稿→rejected；last_analyzed_commit_sha 固化
S6  幂等/恢复：见 §4.3（E2 扩充）
```

### 4.2 对象身份关联表（E1，对齐 A-01~A-04）

主链每步产出后必须登记以下关联（作为 S1~S6 的统一断言附件）：

| 对象 | 必须保留的关联 |
|---|---|
| Analysis Run | 本次分析执行唯一身份 |
| Report | 所属 Analysis 业务结果身份 |
| Report Revision（目标态，P2 落地前=现状登记） | 对应 Report、Revision 身份和正文版本引用；**不得与 KB Document Revision 混同**（现状借住关系登记为 Compatibility Ref）。**P1 现状口径（非阻断澄清）**：当前 Report 身份=repo_analysis_report 行；正文由 kb.document_revision 持有；kb_revision_id=历史兼容引用（非正式业务关联）；**P2 完成前 E2E 用 report_id（+intake_id）作为一次分析与报告的追踪关联键**——防止误把 kb_revision_id 当成已形成的领域关系 |
| KB Document / Revision | KB 正式文档身份 / 本次分析产生的草稿版本 |
| PC Proposal | 钉扎具体 **KB Revision**（非仅 KB Document）+ 目标 Cognition Object |
| PC Approval/Decision | 被批准的 Proposal + **实际 Human Actor 身份**（非仅 approval.status=approved） |
| Cognition Version | 发布时形成的正式版本快照 |
| Close | 本次发布采用的**准确版本集合**（version_member 明细） |

### 4.3 Close 部分失败与恢复场景（E2）

> **契约状态（F4）**：E2-1/3/4/5 的"断言"目前是**候选设计，非已裁决契约**——Close 的部分成功语义、重复返回语义、幂等键作用域、一致性窗口均待业务契约确认后方可成为强制验收断言；E2-2 的"不重复发布"按幂等性目标处理，需核实持久状态机制。测试方案只登记场景，**不得由测试设计反向决定业务模型**。

| 编号 | 场景 | 候选断言（待契约确认） | 契约状态 |
|---|---|---|
| E2-1 | 一批 Proposal 部分发布成功、部分失败 | Close 仍成功收敛；publish_failed_proposals 可查询、可恢复的持久状态（当前字段核实） | **待确认**：部分成功后 Close 整体成功/部分成功/失败 |
| E2-2 | 部分成功 → Worker 中断 → 重启恢复 → 重复 Close | 最终状态收敛：已成功者不重复发布（幂等键生效）、失败者可重试或明确落败 | 幂等性目标；持久状态机制待核实 |
| E2-3 | 重复 Close | 返回原结果（CloseOutcome），不重新执行整个发布流程 | **待确认**：重复返回语义 |
| E2-4 | pc-decision:{id} 幂等键 | 唯一性范围、并发请求行为（恰一成功）、重复请求返回语义 | **待确认**：作用域与并发语义 |
| E2-5 | Close 成功 vs Proposal 发布状态 | 允许的暂时不一致窗口与最终一致性条件 | **待确认**：一致性窗口 |

## 5. 分层验收门（M4/E：替代原"全部通过"）

| 门 | 通过条件 | 允许的后续动作 |
|---|---|---|
| **G1：P0 安全修复** | SEC-P0-1~5 修复并通过逐项复测（含合法路径不误伤） | 允许启动安全负向测试 |
| **G2：V-1/V-2 专项** | 规定负向测试（Regression 类）通过；无未关闭 P0/P1；关键 UNKNOWN 关闭 | 允许关闭 V-1/V-2 |
| **G3：P1 业务链** | S1~S6 至少一条完整链成功；人工门无绕过；恢复和幂等（E2-1~E5）验证通过 | 允许认定 P1 E2E 完成 |
| **G4：证据签收** | D04/D06 证据完整、可复现（含失败日志与 DB 前后状态、环境快照） | 允许专家签收对应交付物 |

**概念区分**：G2 安全专项通过 ≠ G3 业务链通过；G3 通过也不能替代 G2 签收。Characterization 用例不进任何门的通过统计。

## 6. 实施顺序（专家定序 A~E，依赖严格化）

| 阶段 | 内容 | 前置 |
|---|---|---|
| A | SEC-P0 修复（单独批准）+ 人工复核 + 最小负向回归 | 用户批准修复任务 |
| B | L2 安全测试基础设施（双租户/Human-Agent-Service-cpk 凭据/DB 副作用断言；Characterization 与 Regression 分离） | A 完成 |
| C | V-1/V-2 安全测试（T01~T12、T01~T10、P0 回归组） | B 完成 |
| D | L3/L4 真实业务链 S1~S6（只验证 P1 已批准的领域契约） | **G1 通过**；可与 C 部分并行（夹具共享） |
| E | 证据归档 D04/D06 + 专家签收 | C+D 完成 |

并行规则：P2 Analysis 详细设计可随时并行（纯设计），**但不得把 P2 架构目标提前变成 P1 E2E 的硬性验收条件**。

## 7. 不批准事项（沿用专家终审）

- SEC-P0×5 修复并复测前启动真实业务链；
- 关键 UNKNOWN 未关闭前签收 V-1/V-2；
- 未经单独批准提前启动 Schema Migration。

## 8. 修改记录（V1.0 → V1.1）与待实施清单

**M1** 测试三分类（Characterization/Security Regression/Business E2E），V2-T08 拆 T08-C/T08-R，Characterization 不计入通过——§3 口径+§3.2。
**M2** 阶段边界声明——§4.0；S2/S4/S5 越界断言改为"当前行为验证"，Conflict 留 STUB 证据。
**M3** T07 拆分——T07-A 服务层授权边界（Regression，依赖 SEC-P1-1 落地）/T07-B Repository 内部调用边界登记（Characterization，不要求 Repo 识别身份）。
**M4** P0×5 逐项回归映射 + 双向断言（攻击拒 + 合法通）+ DB 副作用强制——§3.3；验收改 G1~G4 分层门——§5。
**E1** 对象身份关联表——§4.2。
**E2** Close 部分失败/恢复五场景——§4.3。
**E3** FakeModel 确定性规范 + Fixture 版本化——§2.3。

**待实施事项（批准后执行）**：① 阶段 A 修复任务批准；② L2 夹具 PR；③ L2/L3 spec 编写；④ 执行与 D04/D06 回填。
