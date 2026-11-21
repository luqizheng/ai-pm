# smark-pm 组织关系与权限模型改造计划 V0.3

**日期：** 2026-11-20
**用途：** 开发实施计划（取代 V0.2）
**基线：**
- [P4-ORG-组织关系与权限模型基线-v0.1](P4-ORG-组织关系与权限模型基线-v0.1.md)（Team 面，附录 A = D-1~D-5 裁决）——**继续有效**
- [smark-pm-pa-product-access-decisions-2026-11-19](smark-pm-pa-product-access-decisions-2026-11-19.md)（PA 阶段性基线）——**Product/Project 域方向修正**
**修订记录：** [P4-ORG-修订决策记录-v1.0](P4-ORG-修订决策记录-v1.0.md)（DR-ORG-01~09）+ [P4-PA-修订决策记录-v1.0](P4-PA-修订决策记录-v1.0.md)（DR-PA-01~06，修订 DR-ORG-03/04/05/06/07）
**影响分析：** [P4-PA-基线对P4-ORG-影响分析-v0.1](../../audit/P4-PA-基线对P4-ORG-影响分析-v0.1.md)

## 〇、相对 V0.2 的修正

1. **Product Membership 恢复**（DR-PA-01）：V0.2 Phase 3 的「Product 无成员名单、Team Member 自动访问」废止——改为 Product 内部管理成员 + 双身份 Owner/Member。
2. **Project 人类角色整体废止**（DR-PA-04）：V0.2 Phase 4 的「三角色 + Owner 推导」废止——Project 不设人类角色体系；Phase 4 收缩为 Agent Registry 迁移。
3. **Product 生命周期新增**（DR-PA-03）：{active, abandoned}，与 Project Closed 独立。
4. **Phase 2（Team 面）继续有效**：core.team/team_member、邀请链、leave、多 Owner、末位保护、dissolve 删除——PA 基线 §2 确认 Team Membership=组织归属、Team Owner 管理 Products。**Phase 2 已实施并验证，状态不变。**
5. PA §8 七项待确认事项全部 OPEN 不得实现（详见 P4-PA 修订记录 §三台账）。
6. 验收目标不变：数据库、权限服务、API、Agent、UI 统一新模型；无迁移/兼容层（AGENTS.md 规则 1）。

## 一、当前实施状态（截至 2026-11-20）

| 阶段 | 状态 | 交付 |
|---|---|---|
| Phase 0 文档基线 | ✅ 完成 | 基线入库 + DR-ORG-01~09 签发 + P4-09 甲部分撤回 |
| Phase 1 组织数据模型 | ✅ 完成 | 0038（core.team/team_member/product/product_project_link）+ dissolve 全链删除 + 约束测试 |
| Phase 2 Team 成员管理 | ✅ 实施完成（验证收尾中） | Team 面全量切 core.team_member；leave；Owner 管理管线；前端全量切换 |
| Phase 3 Product 域 | ✅ 后端实施完成（2026-11-20，UI 下一批） | 0040 + product.rs/products.rs + product_domain 8 用例 |
| Phase 4 Project 角色域 | ✅ 4a 实施完成（2026-11-20） | 0041 ai.project_agent_member + 解析链换源 + role CHECK 收敛 + legacy 映射删除 |
| Phase 5 生命周期 | ✅ 实施（2026-11-21） | 0042 + close/reopen（守卫=关联 Products Product Owner）+ gate 语义 closed |
| Phase 6 人工处理中心 | 🔜 依赖 Phase 3 | 验收主体=Product Member |
| Phase 7 清理回归 | 🔜 最后 | — |

## 二、模型总图（PA 基线后）

```text
Tenant（公司边界；Tenant Admin 仅租户级，D-1）
  └── Team（长期组织单元，不可解散）
        ├── Team Owner(s) / Team Member(s)          ← core.team_member（Phase 2 已交付）
        └── Product(s)（归属唯一 Team）
              ├── Product Owner(s) / Member(s)      ← core.product_member（Phase 3）
              │     · Owner：生命周期决策（abandon/restart）
              │     · Member：全部日常管理/决策（平权）
              └── Product ↔ Project 多对多           ← core.product_project_link（已建）

Project（AI 员工执行上下文；不设人类角色）
  ├── 人类操作权 = 经关联 Product 的 Product Member 日常权限
  ├── 生命周期 {active, closed}（closed 守卫 OPEN，拦截机制先行）
  └── Agent Registry / Capability                     ← ai_pm/architect/developer/tester（Phase 4）
```

权限三事分立（DR-ORG-04③ 保留）：访问资格 / 操作权限 / 对象状态限制，统一推导服务承载。

## 三、Phase 3：Product Membership 与生命周期（DR-PA-01/02/03）

任务：

1. 0040 迁移：`core.product_member (product_id, principal_id, tenant_id, role ∈ {owner, member})`，PK=(product_id, principal_id)；`core.product.status ∈ {active, abandoned}` CHECK（默认 active）。
2. Product 创建 API：Team Owner 独占；入参含首位 Product Owner 指定；同事务落 product 行 + product_member owner 行。
3. 成员管理 API（复用 Team 面管线模式）：所有 Product Member 可添加/移除**普通成员**；Member 范围第一轮限本 Tenant principal（OPEN-4）。
4. Owner 保护：OWNER_LAST（不可直接移除末位 Owner）；继任制变更（先指定继任者再原子变更）；Owner 离开 Team 不自动撤销；无 Owner 时 Team Owner 恢复性任命（唯一显式授予通道，第一轮）。
5. 生命周期：abandon/restart API（任意有效 Product Owner，OPEN-2 可收紧）；product_gate 统一拦截（新需求/新业务决策任务/AI 主动业务入口 fail-closed）；历史读取不拦；对关联 Project 与在飞 Run 零触碰。
6. 审计：product_member.added/removed/role_changed + product.lifecycle_changed（沿用 tenant_audit_log 管线）。
7. 约束与权限测试（仿 org_model_phase1 / Team 面测试模式）。

不做（OPEN 闸门）：Member 任命/降级 Owner；abandoned 自动收尾；跨 Tenant 成员；知识库 ACL 化。

## 四、Phase 4：Agent Registry 迁移与 Project 人类角色废止（DR-PA-04）

任务：

1. Agent Registry / Capability 建模：ai_pm/architect/developer/tester 迁出 `core.project_member.role`；bootstrap 播种的 dev_agent 行迁移；任务承接解析（resolve_role_assignee / ROLE_EXECUTOR_MAP 下线）改走 Registry 能力匹配。
2. project_member 人类角色域废止：bootstrap 建项目 owner 行写入删除；is_project_manager 的 Team Owner 短路去留随 Project 权限裁决（暂不扩散）；人类 project_member 行不再产生。
3. 统一权限服务：三事分立接口落定（资格/操作/状态）；读取资格实现留空（OPEN-6）。
4. 旧角色清理检查（Reporter 全仓 0 命中已确认；Developer/Tester 仅存在于 Agent 职能域）。

## 五、Phase 5：Project Closed 收尾（守卫 OPEN）

- Closed 全局写入拦截机制（project_gate）已在 Phase 1/2 确认可复用；状态域 {active, closed} CHECK 化。
- **守卫裁决（2026-11-21）**：close/reopen 授权主体 = 关联 Products 的任意有效 Product Owner（P4-PA-05② OPEN 关闭）；`POST /v1/pm/projects/{id}/close|reopen` 已开放，0042 状态域 CHECK 收敛，在飞 Run/Import 收口复用 archive 管线。
- 前端仅入口展示，后端唯一强制点。

## 六、Phase 6：人工处理中心与 Issue 验收（依赖 Phase 3）

- 统一入口（interaction 链聚合）+ 信息路由按 Product Membership（PA §6 架构方向）。
- 验收守卫：人类 principal + 排除 Agent principal 先行；Product Membership 资格项随 Phase 3 落地补齐（DR-PA-06）。
- Issue 状态机与唯一受守卫迁移入口保留。

## 七、Phase 7：清理与整体回归（不变）

- 全量权限回归；架构文档与代码一致性检查；PA §8 OPEN 台账核对（无擅自实现）。

## 八、验收标准增补（PA 域）

22. Product Membership 由 Product 内部管理；Team Member 不自动成为 Product 成员。
23. Product 仅 Owner/Member 双身份；无 Admin 与细分人类角色。
24. Product Owner 多人并存；末位 Owner 保护 + 继任制；Team Owner 恢复性任命可用。
25. Product {active, abandoned}；abandoned 后端拦截生效且不触碰关联 Project/在飞 Run。
26. Project 无人类角色体系；project_member.role 不承载人类值。
27. Agent 职能仅在 Agent Registry；不获得人类验收权。
28. PA §8 七项 OPEN 均未实现、均有台账登记。
