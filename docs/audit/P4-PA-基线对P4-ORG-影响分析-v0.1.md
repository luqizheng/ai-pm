# P4-PA 基线对 P4-ORG 修订决策记录（DR-ORG-01~09）的影响分析

**版本：** V0.1
**日期：** 2026-11-20
**新基线：** [smark-pm-pa-product-access-decisions-2026-11-19](../architecture/smark-pm-pa-product-access-decisions-2026-11-19.md)（阶段性决策基线；PA 整体尚未冻结；文中明确区分已确认/建议/待确认）
**受审对象：** [P4-ORG-修订决策记录-v1.0](../architecture/phase4/P4-ORG-修订决策记录-v1.0.md)（DR-ORG-01~09，2026-11-19 签发）及其执行层（P4-ORG-改造计划-v0.2、Phase 1/2 已实施代码）

**纪律声明：** 本报告只做只读盘点；修订以《P4-PA-修订决策记录-v1.0》为准。PA 基线 §8 七项待确认事项一律不得实现为已确认规则（PA 基线 §9.5）。

---

## 一、结论速览

| DR-ORG | 判定 | 说明 |
|---|---|---|
| DR-ORG-01 Team 不可解散 | **不受影响** | PA 基线未触及 Team 生命周期 |
| DR-ORG-02 Team 角色收敛/多 Owner/D-1 TA 不穿透 | **不受影响** | PA 基线 §2 确认 Team Owner 仍管理其 Team 下所有 Products；Team Membership=组织归属 |
| DR-ORG-03 Product 无成员名单、Team Member 自动访问 | **核心条款废止** | PA 基线 §2：恢复 Product Membership（Product 内部管理）；Team Member 不自动成为 Product 成员 |
| DR-ORG-04 正向传播/统一权限服务/D-4 直改免审批 | **部分废止** | 传播链废止（§7「不应将 Team Member 自动传播为所有关联 Product 的 Product Member」）；三事分立与统一服务思想保留；D-4 精神保留（主体改 Product Member） |
| DR-ORG-05 Project 三角色/Owner 推导/D-2 Agent 分离 | **前两项废止** | §7「不再按原方案设计 Project Owner / Admin / Member」「PA-01 实时推导不再适用」；**D-2 Agent Registry 分离不受影响且更必要** |
| DR-ORG-06 Project Active/Closed 仅 Owner 关开 | **守卫条款失效** | Closed 概念保留（§5「Project Closed 与 Product Abandoned 是不同生命周期概念」）；但 Project Owner 不存在 → close/reopen 守卫归属**待裁决**；全局写拦截机制保留 |
| DR-ORG-07 Issue 验收权=Member | **主体改写** | 验收权归 Product Member（§3「AI Agent 工作监督、结果验证与验收」）；Agent 止步提交不变 |
| DR-ORG-08 人工处理中心 | **不受影响** | §3 Product Member 职责含「Issue 处理、人工决策和日常协作」 |
| DR-ORG-09 无迁移纪律 | **不受影响** | — |

**净效果：** Team 面（Phase 1/2 已交付：core.team/team_member、邀请链、leave、末位 Owner 保护、dissolve 删除）**全部继续有效**；Product 域方向反转（从「无成员名单」改为「Membership 内部管理 + 多 Owner + 生命周期」）；Project 人类角色域**整体取消**。

---

## 二、PA 基线已确认项 → 实现含义

| PA 条款 | 实现含义 | 对已实施代码的影响 |
|---|---|---|
| §2 Product Membership 内部管理 | 新建 `core.product_member`（role ∈ {owner, member}） | 0038 建的 core.product 表保留；「无成员名单」约束解除 |
| §2 Team Owner 指定首位 Product Owner | Product 创建 API 入参含首位 Owner（Team Owner 独占调用） | Phase 3 新建，无存量 |
| §2 Team Owner 不因组织身份自动成为 Product Owner | Product Owner 是显式授予身份 | — |
| §3 Product 双身份（Owner/Member）、无 Admin | product_member.role 两值；Member 权限=全部日常管理/决策（含添加/移除普通成员） | — |
| §4 多 Product Owner + 末位保护 + 继任制 | 末位 Owner 不可直接移除；变更需先指定继任者；无 Owner 时 Team Owner 恢复性任命 | 仿 Team 面 OWNER_LAST 管线（锁 team/product 行 + owner 计数） |
| §4 Owner 离开 Team 不自动撤销 Product Owner 身份 | Product Member 与 Team Membership 解耦（撤销仅显式） | — |
| §5 Product 生命周期 {active, abandoned}、restart 为转换 | product.status CHECK 两值；abandoned 后端拦截（新需求/新业务决策任务/AI 主动业务工作） | Phase 3 新建 |
| §5 Product/Project 生命周期独立 | Product abandoned 不触碰关联 Project 与在飞 Agent Run | project_gate 仅管 Project 域 |
| §6 知识开放原则（有效业务访问资格可读完整普通研发知识） | 读取资格推导=待确认 §8-6；Product Membership 不作知识库 ACL | 统一权限服务保留「资格」接口位，实现留待裁决 |
| §6 信息路由按 Product Membership | 通知/决策请求路由按成员与归属，非广播 | 架构方向，随人工处理中心实施 |

## 三、PA 基线废止项 → 已实施/在途资产处置

| 废止项 | 资产 | 处置 |
|---|---|---|
| Project 人类三角色（DR-ORG-05①） | P4-ORG-修订决策记录 DR-ORG-05、改造计划 v0.2 §4.2 | DR-PA 修订记录废止；计划升 v0.3 |
| Project Owner 推导（DR-ORG-05②） | 同上；project_member.rs is_project_manager 的 Team Owner 短路（Phase 2 适配保留） | 推导方案废止；Team Owner 短路去留随 Project 权限新模型裁决（暂保留现状，不扩散） |
| Team→Product→Project 正向传播（DR-ORG-04①③） | DR-ORG-04、审查报告 R4 | 废止；Phase 4 原计划内容作废 |
| P4-09 撤回的甲部分（DR-01/02/03） | 撤回公告 | **撤回维持有效**——PA 基线恢复的 Product Membership 是「Product 内部管理」，与旧 DR-01/02/03 的「Project→Product 反向继承」机制不同，旧方案不复活 |
| s01-02 §3.3（建项目 Owner 默认当前用户）等旧条文 | 冻结文档 | 随 DR-PA 修订一并标注（不动原文） |
| bootstrap 建项目写调用者为 project owner 行 | bootstrap.rs:119-135 | **仍需删除**（Project 人类角色取消后无 owner 行可写）——归 Phase 4 |

## 四、待确认事项（PA §8 七项 → 实现闸门）

1. Owner 离开 Team 后是否保留普通 Product Membership → **撤销语义留接口**
2. 多 Owner 生命周期决策规则（单人/共同）→ abandon/restart 守卫先按「任意有效 Product Owner」（§4 已确认「任意有效 Product Owner 都可以执行 Product 生命周期决策」），若 §8-2 后续收紧再修
3. Member 是否可任命/降级 Owner → **不实现**；第一轮仅 Team Owner 恢复性任命 + Member 添加/移除普通成员（§3/§4 已确认部分）
4. Product Membership 可添加对象范围 → 第一轮按「本 Tenant 内 principal」最小实现，范围扩展待裁决
5. Abandoned 时在途决策/通知/Agent Run 收尾 → **不实现自动收尾**；fail-closed 拦截新写入，在途项留待裁决
6. 共享 Project 的有效业务访问资格边界 → **不实现**；读取资格接口留空
7. 审计事件与通知要求 → 沿用 tenant_audit_log 管线先落基础事件（added/removed/role_changed/lifecycle_changed），通知归信息路由设计

## 五、执行层修订清单

| 文件 | 修订 |
|---|---|
| P4-ORG-修订决策记录-v1.0.md | 不改原文；由 P4-PA-修订决策记录 逐条登记效力变化 |
| P4-ORG-改造计划-v0.2.md | 升 v0.3：Phase 3/4/5 重定义（见该文件修订记录） |
| P4-09-decision-records-draft-v0.1.md | 撤回公告补一句：PA-2026-11-19 基线恢复 Product Membership 但旧 DR-01/02/03 机制（反向继承）仍废止 |
| Phase 3 实施包 | product_member 表 + Product Owner 管理 + product 生命周期 |
| Phase 4 实施包 | Agent Registry 迁移 + project_member 人类角色废止 + Project Owner 推导代码不建 |
