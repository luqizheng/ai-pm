# P4-ORG / P4-PA 资料重建：冲突与 OPEN 核验台账 V0.1

- 日期：2026-11-21
- 状态：工作台账；未关闭项不得进入实施
- 基准输入：2026-11-20《组织关系与权限模型改造计划 V0.3》及 2026-11-19 PA Product Access 阶段性基线

## 1. 权威优先级

1. 用户明确确认的最新决策及 2026-11-20 V0.3 当前实施基线。
2. 2026-11-19 PA Product Access 阶段性基线与 PA-03/04/05/06 已确认决策。
3. 正式签发的 DR-ORG / DR-PA 当前版本（须核实其是否已吸收上述新决策）。
4. P4-01/P4-02 历史冻结材料：只保留未被后续明确修订的技术约束，不得覆盖新业务模型。
5. P4-09 旧成员权限与敏感区草案：作为历史论证材料，不作为当前 Product/Project 成员模型的直接实施依据。
6. 代码与迁移：用于判断实际落地状态，不因代码现状自动改变业务决策；若代码与基线不一致，登记缺陷。

## 2. 已确认的关键修正

| 主题 | 旧资料方向 | 当前基线 | 处理 |
|---|---|---|---|
| Product Membership | 旧 P4-01/P4-09 无 Product Member | `core.product_member`，Owner/Member 双身份 | 旧方向废止 |
| Product 角色 | 旧模型 Owner/Admin 等 | 仅 Owner/Member，无 Admin | 旧角色清理 |
| Team Member | 自动覆盖 Team 下 Product（旧方案可能有继承） | 不自动成为 Product Member | 不得自动继承 |
| Project 人类角色 | Project Owner/Admin/Member 或三角色推导 | Project 无人类角色体系 | 旧模型废止 |
| Agent 职能 | `project_member.role` 承载 Developer/Tester 等 | Agent Registry/Capability | 0041 与解析链迁移 |
| Product Lifecycle | 旧 P4-01 `active/archived` | `active/abandoned` | 新基线覆盖旧状态域 |
| Project Lifecycle | 与 Product/Team 关系混杂 | `active/closed`，close/reopen 受 OPEN 阻塞 | 生命周期独立 |
| Product-Project 关系 | 旧双向权限隔离/不穿透 | 多对多关联；业务执行按 PA 新模型定义；访问资格仍受 OPEN | 逐条按 PA 新决策核验 |
| Tenant Admin | 旧模型可能有 Product 管理穿透 | 租户级管理，不自动成为 Product Member/Project 业务访问者 | 不得隐式穿透 |

## 3. 文档内冲突（阻断项）

| ID | 冲突 | 证据 | 暂行处理 | 关闭条件 |
|---|---|---|---|---|
| C-01 | Team 不可解散 vs Phase 1 “dissolve 全链删除” | V0.3 模型总图与 Phase 1 交付描述 | 不新增/扩大 dissolve；既有实现不作业务语义推断 | 对照 DR-ORG-01～09、0038 migration、Team service/API 与历史决策，明确 dissolve 是正式能力、历史清理或已废止 |
| C-02 | Project 人类角色废止 vs 统一权限服务的 Project 操作权限推导公式未完全明确 | V0.3 Phase 4 与 PA §8 OPEN-6 | Project 人类角色不得恢复；读取资格实现留空/fail-closed | DR-PA 正式文本明确共享 Project 有效业务访问者及操作映射 |
| C-03 | V0.3 声明 DR-ORG-01～09 已签发，但当前资料库未检索到对应正式文件 | V0.3 引用 + Library 清单/检索 | 不根据记忆伪造正式 DR 正文 | 获取原始签发文件或从确证决策记录重建并标注重建稿 |
| C-04 | V0.3 声明 DR-PA-01～06 修订记录已签发，但资料库仅找到 11/19 PA 阶段性基线与旧 P4-09 PROPOSED 草案 | Library 清单/检索 | 旧 P4-09 状态保持历史，不把 PROPOSED 升级 | 获取正式 DR-PA 记录，逐项核对 V0.3 的修订关系 |
| C-05 | V0.3 将 Product Abandoned 在途 Run 定为“零触碰”，PA-04 对只服务 Abandoned Product 的未完成任务要求停止主动推进 | V0.3 Phase 3 与 PA-04-03 | 不自动取消/终止 Run；“停止主动推进”如何影响排队任务/调度入口需明确区分 | 细化“Run 零触碰”与“Task 停止主动推进”的不同对象和检查点 |

## 4. PA §8 七项 OPEN（必须原样保留语义）

> 下表按 V0.3 语义整理为检索/核验标签，不代替正式 P4-PA 修订记录的原编号和原文。

| 标签 | OPEN 内容 | 当前禁止事项 |
|---|---|---|
| O-1 | Product Owner 离开 Team 后，普通 Product Membership 是否保留 | 不自动移除或保留普通 Member 关系作为已冻结规则 |
| O-2 | 多 Owner 生命周期决策是否任意一位单独执行，是否收紧 | 不增加多人会签；不扩张单人权限超出当前 API 基线 |
| O-3 | Product Member 是否可任命/降级 Owner | 不实现 |
| O-4 | Product Membership 可添加对象范围 | 第一轮限本 Tenant principal；跨 Tenant 不实现 |
| O-5 | Product Abandoned 时在途决策请求、通知、Agent Run 如何收尾 | Run 按 V0.3 零触碰；其他事项不自动撤回/关闭/重派 |
| O-6 | 普通知识读取的精确业务资格边界，尤其共享 Project 多 Product | 读取资格实现留空/fail-closed，不推导 Product Member 全量继承 |
| O-7 | Product 成员/Owner 变更的审计事件与通知要求 | 保留 V0.3 最低事件，不自行增加通知/审批要求 |

## 5. 需要核对的资料

### 当前可见且可用

- `smark-pm-pa-product-access-decisions-2026-11-19.md`
- `P4-09-decision-records-draft-v0.1.md`（历史草案，全部 PROPOSED）
- `P4-09-成员权限与敏感区补充设计-v0.1.md`（历史补充设计）
- `P4-成员权限与敏感区-核对与影响分析报告-v1.0.md`
- `P4-成员权限与敏感区-冻结影响分析报告-v1.0.md`
- `P4-00_Current_Architecture_Ownership_Audit_V1.0.md`
- `PA-03-product-information-routing.md`
- `PA-04-shared-project-collaboration.md`
- `PA-05-work-item-and-activity-feed.md`
- `PA-06-agent-human-collaboration.md`
- `PA-07-developer-implementation-specification-v1.0.md`

### 当前检索未找到（不能断言不存在）

- `P4-ORG-组织关系与权限模型基线-v0.1.md`
- `P4-ORG-修订决策记录-v1.0.md`
- `P4-PA-修订决策记录-v1.0.md`
- `P4-PA-基线对P4-ORG-影响分析-v0.1.md`
- V0.3 本身的正式独立文件副本（用户当前消息提供了全文，但 Library 未发现同名文件）

## 6. 代码/迁移核验清单

- [ ] Migration 0038 全文：Team/Product/Link 的真实字段、PK/FK、CHECK、索引、触发器。
- [ ] Migration 0040 全文：`core.product_member`、Product status、role CHECK、tenant 一致性约束。
- [ ] Migration 0041 全文：`ai.project_agent_member` 与旧 `project_member.role` 收敛。
- [ ] Team dissolve 全链删除实现与当前“不解散”描述的语义冲突。
- [ ] Team Owner leave/继任/末位保护并发测试。
- [ ] Product 创建 + 首位 Owner 同事务。
- [ ] Product Member 普通成员添加/移除守卫。
- [ ] Owner 继任、末位保护、恢复性任命并发测试。
- [ ] Product Gate 全写入口覆盖，历史读不阻断。
- [ ] Project Gate 全写入口覆盖，close/reopen API 不开放。
- [ ] Agent Registry 任务承接解析无旧映射回退。
- [ ] Issue 验收入口排除 Agent principal。
- [ ] 搜索、导出、下载、MCP、后台 Worker 权限检查一致。

## 7. 交付顺序

1. 完成正式 DR-ORG/DR-PA 文件补齐或重建，并先解决 C-01。
2. 完成数据库/服务/API 实际代码核验，修订正式基线中的“逻辑契约”。
3. 完成 OPEN 对照和 Project 读取资格边界，未关闭的继续阻塞相关实现。
4. 输出实施差异清单（已实现 / 部分实现 / 缺失 / 不符合基线）。
5. 再发布最终实施计划和验收矩阵；不得将“文件已整理”误报为“代码已验收”。
