# P3-D04-02 Step 3 — Proposal Expiration 触发主体与事务契约核验 V1.0

- 日期：2026-11-17｜任务书条目：N-03
- 目的：区分已冻结规则/待项目负责人裁决事项/可实施阶段落实的技术细节；不自选权限模型

## 1. 已冻结规则（Q-03 裁决——不再讨论）

expired 为终态；不允许 expired→submitted/approved；继续推进=新 Proposal（新逻辑操作身份）；原 Proposal 保留**过期时间、过期原因**和审计记录；submitted→expired 为合法转换（T-10）。

## 2. 现状事实

- **CODE FACT**：`expired` 无任何 setter（全库仅查询排除引用 pc/proposal.rs:1438）。
- **SCHEMA FACT**：proposal 表无过期时间/过期原因列（0001:3240-3262 仅 status CHECK 含 'expired'；grep expire=12 处均为值域/查询引用）。
- **TEST FACT**：无 expired 相关测试用例。
- 结论：expired 终态化=**纯目标语义，零现状实现**（GAP-36 全部为待实施）。

## 3. 三种触发模式分析（不得混为一谈）

### 模式 A：人工管理/运维触发

| 项 | 分析 | 分类 |
|---|---|---|
| 触发主体 | 项目管理者/平台运维（真人 principal） | **HUMAN DECISION REQUIRED**（主体与权限模型待裁决=Q-12） |
| 角色与权限 | 候选：项目 Lead / A3 授权人 / 平台管理员——三种模型影响面不同 | HUMAN DECISION REQUIRED |
| 是否真人+A3 | 建议=是（expire 使 submitted 提案永久终态，属治理动作；与 approve/reject 同级） | DESIGN REQUIREMENT（待 Q-12 确认） |
| 原因是否必填 | 建议=必填（终态理由进入决策/治理记录，与 reject 的 reason 可空不同——expire 非纠纷性动作，理由是唯一追溯线索） | DESIGN REQUIREMENT（推荐，实施细节可调） |
| 同事务要求 | 是：status=expired + expired_at + expired_reason + 审计（`pc_proposal.expire`）同一事务 | DESIGN REQUIREMENT（DESIGN 已定义，实施=GAP-36） |

### 模式 B：系统自动到期

| 项 | 分析 | 分类 |
|---|---|---|
| 是否属于当前设计范围 | **否**——无确定性到期规则存在（无 due 时点字段/无 N 天策略裁决/无巡检任务），V1.0.2 曾以「隐性转移面」为由不推荐 | HUMAN DECISION REQUIRED（是否引入+规则=Q-12 附项） |
| 确定性到期条件 | 若引入：候选规则=「基线迁移后 N 天未决」或「submitted 后 N 天未决」——均需裁决时点起点与 N 值 | HUMAN DECISION REQUIRED |
| 独立服务身份 | 需独立巡检 worker（服务主体 dispatch 类）——其触发属**系统治理动作**，须与人工审批/withdraw/publish 并发安全 | DESIGN REQUIREMENT（若引入） |
| 防重复处理 | 状态守卫（仅 submitted 可 expire）+FOR UPDATE 行锁→先到先得；巡检天然幂等（已 expired 跳过） | DESIGN REQUIREMENT（若引入） |
| 与人工动作并发 | expire 与 approve/reject/withdraw/publish 均从 submitted 出发的互斥转换→proposal FOR UPDATE 串行+状态守卫→后到者 409 | 已有机制覆盖（D5 机制表） |
| 同事务 | 状态+expired_at+reason+审计同事务（与人工模式一致） | DESIGN REQUIREMENT |

### 模式 C：AI 行为

- AI 可**识别到期风险并产生建议**（待办/通知/报告标注），但其建议**不构成过期动作**——expire 属正式治理动作，AI 触发=违反「AI 不拥有治理权限」冻结边界（A-2/Q2-C）。
- 不得将 AI 建议伪装成已授权过期动作（建议与动作分离：建议=Analysis/待办对象；动作=人工或裁决授权的系统任务）。
- 确认度：DESIGN REQUIREMENT（边界重申，无新设计）。

## 4. GAP-36 完整范围（任务书 §5.4）

| 组成 | 内容 | 阶段 |
|---|---|---|
| ① 转换入口 | expire 命令（submitted→expired 单向；FOR UPDATE+状态守卫+行数断言） | Schema/API 实施批 |
| ② 时间/原因承载 | proposal 表新增 expired_at timestamptz NULL + expired_reason text NULL（或等效承载）——**Q-03「保留过期时间、过期原因」的物理落点** | Schema/API 实施批 |
| ③ 权限门 | 触发主体权限模型——**待 Q-12 裁决**（候选：A3/平台管理员） | 随 Q-12 |
| ④ 审计 | `pc_proposal.expire` 审计动作（同事务，四身份列；reason 入 after_data） | Schema/API 实施批 |
| ⑤ 幂等/并发 | 终态一次（状态守卫）；与 approve/reject/withdraw/publish 行锁互斥 | 机制已在位（D5） |
| ⑥ 测试 | expire 终态化/权限 fail-closed/并发互斥用例 | 实施批 |
| ⑦ 自动到期（可选） | 若 Q-12 引入：规则+巡检 worker | 随 Q-12 |

**所需 Schema/API 变更清单（供实施批，本轮零实施）**：2 列新增+1 命令入口+1 审计动作+测试；无 Migration 本轮执行。

## 5. 待项目负责人裁决事项（HUMAN DECISION REQUIRED）

| # | 问题 | 备选 | 本设计倾向 |
|---|---|---|---|
| Q-12 主问 | submitted→expired 的触发主体与权限 | A=人工管理/运维触发（真人，权限模型待选：Lead/A3/平台管理员）｜B=引入系统自动到期（需定规则）｜C=组合（人工为主+到期告警） | **A（人工为主；V1 不引入自动到期——与 Q-03 阶段裁定一致，隐性转移面最小）** |
| Q-12 附 1 | 人工触发是否要求 A3 | 是/否（或平台管理员单独模型） | 建议是（治理动作） |
| Q-12 附 2 | expired_reason 是否必填 | 必填/可选 | 建议必填（终态唯一追溯线索） |
| Q-12 附 3 | 是否引入自动到期 | 引入（需定规则）/不引入 | 建议不引入（V1） |

**均不自行选定；Q-12 列为 A 级阻塞项**（expired 转换是状态机组成部分，触发机制未定=状态机无法定稿）。

## 6. 与既有机制的关系

- 与 approve/reject/withdraw/publish 的并发互斥由既有 proposal FOR UPDATE+状态守卫覆盖（无新增锁需求）；
- 审计/Outbox：expire 无 Outbox 事件需求（V1 目录无对应事件；如需通知面→后置评估，不阻塞）；
- R-04 状态：**部分收窄**——「管理/运维动作」方向已入设计（模式 A），主体/权限细节=Q-12 待裁；R-04 未升级为裁决，亦未无声关闭。
