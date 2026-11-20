# P4-PA 修订决策记录 V1.0（Product Membership / Project 角色废止 / Product 生命周期）

**版本：** V1.0（已签发生效）
**日期：** 2026-11-20
**基线：** [smark-pm-pa-product-access-decisions-2026-11-19](smark-pm-pa-product-access-decisions-2026-11-19.md)（PA 阶段性基线，人工确认）
**冲突依据：** [P4-PA-基线对P4-ORG-影响分析-v0.1](../../audit/P4-PA-基线对P4-ORG-影响分析-v0.1.md)
**被修订对象：** [P4-ORG-修订决策记录-v1.0](P4-ORG-修订决策记录-v1.0.md)（DR-ORG-01~09，2026-11-19 签发）

**纪律声明：** 本记录不改动被修订条款历史原文；效力以本记录为准。PA 基线 §8 七项待确认事项一律登记为待产品决策（OPEN），不得实现（PA 基线 §9.5）。DR-ORG-01/02/08/09 不受影响，继续有效。

---

## DR-PA-01 Product Membership 恢复（修订 DR-ORG-03）

**Status: ACCEPTED（2026-11-20，PA 基线 §2/§3/§4 已确认部分）**

- **修订关系：** DR-ORG-03①②（「不建立 Product 成员表」「Team Member 自动获得全部 Products 访问权」）→ **效力废止**；DR-ORG-03③（其他 Team 不自动获得）保留。
- **决定：**
  1. Product Membership 表示参与某 Product 的业务协作与决策；成员由 Product 内部管理；Team Member 不自动成为所属 Team 下所有 Product 的成员（PA §2）。
  2. 新建 `core.product_member (product_id, principal_id, tenant_id, role ∈ {owner, member})`，PK=(product_id, principal_id)；多 Owner 并存。
  3. Product 角色仅 Product Owner / Product Member 两值；不设 Product Admin、不设 Reporter/Developer/Tester 等细分（PA §3）。
  4. **Product Owner**：生命周期决策（abandon / restart）；不垄断日常管理与决策。**Product Member**：全部日常管理与决策权限（产品/需求管理、项目协调、技术决策、知识管理、AI 监督与验收、Issue 处理、添加/移除普通成员）（PA §3）。
  5. 创建 Product 时由所属 Team Owner 指定首位 Product Owner（PA §2）；创建者与 Team Owner 均不因组织身份自动成为 Product Owner。
  6. Team Owner 仍负责管理其 Team 下的所有 Products（Team 面组织管理权，≠ 自动 Product Membership/Owner 身份）（PA §2）。
- **实现要点（Phase 3）：** product_member DDL；成员添加/移除 API（所有 Product Member 可添加/移除**普通成员**，PA §3/§4）；首位 Owner 指定入参。
- **OPEN（PA §8）：** 成员可添加的对象范围（第一轮按本 Tenant 内 principal 最小实现）；Member 是否可任命/降级 Owner（**不实现**，第一轮 Owner 变更仅「Team Owner 恢复性任命」+ 继任制变更）；Owner 离开 Team 后普通 Membership 去留（撤销语义留接口，不自动撤销）。

## DR-PA-02 Product Owner 保护与继任（新增，对齐 Team 面管线）

**Status: ACCEPTED（PA §4 已确认部分）**

- **决定：**
  1. 任意有效 Product Owner 可执行 Product 生命周期决策（PA §4；§8-2 若后续要求共同确认再修）。
  2. 不可直接移除最后一位 Product Owner（OWNER_LAST）；更换最后一位 Owner 必须先指定继任者再完成变更（继任指定→原子变更，同一事务）。
  3. Product Owner 离开所属 Team **不自动**撤销其 Product Owner 身份；最后一位 Owner 离开前必须指定继任者。
  4. 特殊情况导致 Product 无 Owner 时，由所属 Team Owner 恢复性任命（唯一显式 Owner 授予通道，第一轮实现）。
  5. 保护管线复用 Team 面锁序：锁 core.product 行 → 锁调用者/目标 product_member 行 → owner 计数 → 规则裁决 → DML + 审计。
- **OPEN：** Member 间 Owner 任命/降级（PA §8-3）。

## DR-PA-03 Product 生命周期 {active, abandoned}（新增）

**Status: ACCEPTED（PA §5 已确认）**

- **决定：**
  1. product.status ∈ {active, abandoned}，CHECK 约束；restart 是 abandoned→active 的转换，不设持久化 reactivated 态。
  2. abandoned（由 Product Owner 决定）：不再接收新 Product 需求；不产生新 Product 业务决策任务；AI 不再主动开展该 Product 业务工作；保留历史资料与读取；Owner 可重启。
  3. 拦截由后端统一执行（复用 project_gate 模式建 product_gate：需求写入、业务决策任务创建、AI 主动业务入口）；历史读取不拦。
  4. **Product/Project 生命周期独立**：abandoned 不自动关闭关联 Projects、不停止在飞 Agent Run；共享 Project 服务其他 active Products；仅服务已放弃 Product 的 Project 由人类独立处理；重启恢复协作（PA §5）。
- **OPEN（PA §8-5）：** abandoned 时在途决策请求/通知/Agent Run 收尾策略（不得自行设计为绕过拦截的例外；fail-closed 底线）。

## DR-PA-04 Project 人类角色整体废止（修订 DR-ORG-05①②）

**Status: ACCEPTED（PA §1/§7 已确认）**

- **修订关系：** DR-ORG-05①（Project 三角色 Member/Admin/Owner）、②（Owner=Team Owner 推导）、Admin 任命条款 → **效力废止**；DR-ORG-05③（D-2 Agent Registry 分离）**继续有效且升级为该 DR 剩余主体**。
- **决定：**
  1. Project 不设人类 Owner/Admin/Member 角色，不建立独立人类 Project 权限体系（PA §1.3）；不建 Project-only Member 授权机制（PA §7）。
  2. Project Owner/Admin 推导方案（原 PA-01）废止（PA §7）。
  3. `core.project_member.role` 的人类值（owner|member）废止：Phase 2 适配保留的 is_project_manager Team Owner 短路去留随 Project 权限新模型裁决（暂不扩散）；bootstrap 建项目写调用者 owner 行删除。
  4. **Agent 职能迁移（D-2）照旧执行**：ai_pm/architect/developer/tester 迁出 project_member.role 落独立 Agent Registry / Capability 模型；Agent 职能不得成为人类角色、不获得人类验收权。迁移完成后 project_member 人类角色域清空，该表仅余 Agent 职能历史承载（随 Registry 落地一并处置）。
  5. Project 上的人类操作权按 PA 基线归 **Product Member 日常权限**（经 Product↔Project 关联的业务参与），不是 Project 内角色。
- **OPEN（PA §8-6）：** 共享 Project「有效业务访问资格」的精确边界（读取资格接口留空，不实现）。

## DR-PA-05 Project Closed 与知识开放（修订 DR-ORG-06/DR-ORG-04④）

**Status: ACCEPTED（PA §5/§6 已确认部分；守卫归属 OPEN）**

- **修订关系：** DR-ORG-06②（仅 Project Owner 关开）→ **守卫条款失效**（Project Owner 不存在）；DR-ORG-06①③④⑤（Closed 全局只读拦截/AI 可读历史/fail-closed/审计）→ **机制保留**；DR-ORG-04④（D-4 人类直改免审批）→ 保留，主体改 Product Member。
- **决定：**
  1. Project Closed 与 Product Abandoned 是不同生命周期概念（PA §5）；Project Active/Closed 两态与全局写入拦截（project_gate）机制继续有效。
  2. **OPEN（待产品裁决）：** close/reopen 的授权主体（Project Owner 已废止；候选=Product Owner（关联 Products）∪ Team Owner，待裁）。close/reopen API 在裁决前**不开放入口**（fail-closed），拦截机制先行为所有写入口就绪。
  3. 知识开放（PA §6）：对同一 Project，具有有效业务访问资格的相关人类可读取完整普通研发知识（需求/设计/技术认知/任务/决策记录/历史）；知识读取能力与正式决策权分离；敏感/未发布/撤回/非正式内容遵循各自状态规则不因开放原则绕过；Product Membership 不作为知识库 ACL（PA §9.2）。
  4. 人类 Product Member 直接修改正式需求与项目知识不经 AI 审批链；K4 等审阅链仅约束 AI 生成/回写（DR-ORG-04④ 主体更新）。
- **OPEN：** §8-6 资格边界；close/reopen 守卫。

## DR-PA-06 Issue 验收权主体更新（修订 DR-ORG-07）

**Status: ACCEPTED（PA §3）**

- **修订关系：** DR-ORG-07①「Member 验收」→ 主体改 **Product Member**；其余（Agent 止步 Submitted、守卫收紧排除 Agent principal、Admin/Owner 争议裁定→随 Project 角色废止改「管理性争议按 Product Owner 生命周期外的日常裁定权归 Product Member 平权体」）。
- **决定：** AI Agent 工作监督、结果验证与验收、Issue 处理与人工决策归 Product Member（PA §3）；Agent 不得代替验收；验收守卫=「人类 principal + 目标 Project 有效业务访问资格 + Product Membership」（第一轮先实现人类 principal + 排除 Agent，资格项随 DR-PA-04⑤ 落地补齐）。

## 三、待确认事项台账（PA §8 七项，全部 OPEN 不得实现）

| # | 事项 | 实现闸门 |
|---|---|---|
| 1 | Owner 离开 Team 后普通 Membership 去留 | product_member 撤销语义留接口（当前=不自动撤销，已确认 §4） |
| 2 | 多 Owner 生命周期决策规则 | abandon/restart 按「任意有效 Owner」实现，标记可收紧 |
| 3 | Member 任命/降级 Owner | 不实现；仅继任制变更 + Team Owner 恢复性任命 |
| 4 | Membership 可添加对象范围 | 第一轮限本 Tenant principal |
| 5 | Abandoned 在途收尾 | fail-closed；不做自动收尾 |
| 6 | 共享 Project 资格边界 | 读取资格接口留空 |
| 7 | 审计/通知要求 | 基础审计事件先落；通知归信息路由 |

## 四、签发后待办

1. 改造计划升 v0.3（Phase 3/4/5 重定义）。
2. Phase 2 收尾（Team 面测试/前端验证）不受本记录影响，继续。
3. Phase 3 实施：product_member + Product Owner 管理 + product 生命周期。
4. Phase 4 收缩为：Agent Registry 迁移 + project_member 人类角色废止。
5. P4-09 乙部分（敏感区 DR-04~11）不受影响，继续有效（PA §6 确认敏感内容遵循各自规则）。
