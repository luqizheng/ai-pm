# P4-ORG / P4-PA 资料与代码证据补充核验记录 V0.1

**日期：** 2026-11-21  
**状态：** 资料库交叉核验补充记录；不新增业务决策，不替代正式 DR  
**归档建议路径：** `/smark-pm/architecture/organization-access-v1/`

## 1. 本轮核验范围

本轮重新读取并交叉核对：

- `P4-ORG-修订决策记录-v1.0.md`
- `P4-PA-修订决策记录-v1.0.md`
- `P4-ORG-改造计划-v0.3.md`
- `P4-09-成员权限与敏感区补充设计-v0.1.md`
- `P4-09-decision-records-draft-v0.1.md`
- `P4-成员权限与敏感区-核对与影响分析报告-v1.0.md`
- 本资料包 V0.2 实施契约、V0.2 冲突与 OPEN 台账、V1.0 决策效力总表。

本记录只依据上述资料库文件；没有读取当前代码仓库、实际 Migration SQL 或运行数据库。

## 2. 已确认的正式决策关系

### 2.1 P4-ORG 与 P4-PA 的效力顺序

1. P4-ORG DR-ORG-01~09 于 2026-11-19 签发。
2. P4-PA DR-PA-01~06 于 2026-11-20 签发，对指定 DR-ORG 条款作后续修订。
3. DR-ORG-01、02、08、09 不受 P4-PA 修订，继续有效。
4. DR-ORG-03、04、05、06、07 不可整体视为仍按原文生效；必须按 DR-PA 指定条款逐项合并。
5. V0.3 是实施计划状态报告，不得反向覆盖正式 DR；其“已实施”陈述必须由代码、迁移、测试证据独立验证。

### 2.2 现行模型的关键边界

- Team 不可解散；删除的是旧 Team dissolve API、服务和级联清扫能力，不是执行 Team 解散。
- Team 仅 Owner/Member，可多 Owner；末位 Owner 保护。
- Product 唯一归属 Team；Product Membership 独立，角色仅 Owner/Member；Team Member 不自动成为 Product Member。
- Product Owner 负责 abandon/restart；任意有效 Owner 当前可执行，OPEN-2 可能收紧，不得预先增加会签。
- Product abandoned 与 Project Closed 是独立生命周期；abandoned 不自动关闭 Project，也不自动停止共享 Project 的在途 Run。
- Project 不设人类 Owner/Admin/Member 角色体系；Agent Registry/Capability 与人类权限分离。
- Issue 验收必须是 human principal，排除 Agent principal；业务资格映射仍受 OPEN-6 限制。
- 权限判断必须分离 Eligibility、Operation、State Gate；统一后端服务覆盖 API、MCP、Agent、Worker、内部服务等路径。
- DR-ORG-09 禁止本轮存量迁移、双写、兼容分支和过渡层。

## 3. P4-09 历史方案状态澄清

P4-09 的配套 Decision Records Draft V0.1 明确声明全部 DR-01~11 为 PROPOSED，而非 ACCEPTED/FROZEN。

- DR-01/02/03 是对 P4-01/P4-02 冻结条文的修订请求；P4-ORG DR-ORG-03 的 D-3 已明确撤回这三项旧修订请求。不得把其多来源成员传播、Product Member 对关联 Project 完整只读等内容带入现行实现。
- DR-04~11 的敏感区设计方向被 P4-PA 记录为“乙部分不受影响，继续有效”，但 P4-09 Decision Records Draft 本身仍声明 PROPOSED。现有资料不足以将每一项都提升为 ACCEPTED/FROZEN 实施契约。
- P4-ORG DR-ORG-04 明确保留 DR-04 的显式可见性限制例外框架，但具体策略仍 OPEN；P4-ORG 签发后待办要求修订 P4-09 授权主体称谓，并补 Closed 态交互定义。
- 因此敏感区应保持 fail-closed 和既有安全边界；在找到后续逐项签发记录前，不扩大授权主体或读取范围。

## 4. V0.3 实施状态与当前证据等级

V0.3 声称：

| 项目 | V0.3 报告状态 | 本轮独立证据 |
|---|---|---|
| 0038 Team/Product/Link 数据模型及 dissolve 能力删除 | Phase 1 完成 | 未取得 0038 SQL、实际 Schema 或测试结果 |
| Team Membership 与 Owner 管理 | Phase 2 实施完成、验证收尾 | 未取得服务/API/测试源码 |
| 0040 Product Membership 与生命周期 | Phase 3 后端实施完成 | 未取得 0040 SQL、服务/API/测试源码 |
| 0041 Agent Registry 迁移 | Phase 4a 完成 | 未取得 0041 SQL、Registry 解析链和回归测试源码 |
| Product Gate / Project Gate | 分阶段实施/拆分 | 未取得完整入口覆盖与测试证据 |
| 人工处理中心、Issue 验收 | Phase 6 待实施 | 未取得当前代码证据 |

**结论：以上实施状态目前只能标记为 Plan-reported，不能标记为 Code-verified。**

本轮资料库列表中未找到 0038、0040、0041 的 Migration SQL 原文件，也未找到组织权限改造对应的完整源代码快照。资料库内的 P4-03 Cognition 审计与 Migration Evidence 属于认知域（PC/ProjC）审计，不能替代组织权限 Migration 证据。

此前 GitHub 连接器检索 `smark-pm` 仓库未返回匹配仓库。因此当前代码级审计的阻塞点是可访问的目标仓库/源码证据缺失，不是架构决策资料缺失。

## 5. 下一轮代码核验所需的最小证据

取得目标仓库访问后，按以下顺序直接核验，不要求用户重复提供已经归档的架构决策文档：

1. `migrations/0038*`、`0040*`、`0041*` 完整 SQL。
2. Team、Product、Product Membership、Product-Project Link 的 Domain / Application / API 实现。
3. Agent Registry 建模与任务分配解析调用链；全仓搜索旧 `project_member.role` 人类角色残留。
4. Product 创建与首位 Owner 同事务实现；Owner 继任、末位保护、恢复性任命及并发测试。
5. Product Gate / Project Gate 的全部写入入口清单，覆盖 REST、MCP、Agent、Worker、内部服务。
6. Issue 验收服务层 human-principal 守卫及 Agent principal 拒绝测试。
7. Team dissolve API、服务、UI、清扫链删除的全仓搜索与负向测试。
8. 当前数据库 Schema / 约束核验结果（只读），确认与 Migration 目标一致。

## 6. 验收纪律

- Decision-confirmed：仅表示正式 DR 已签发。
- Plan-reported：仅表示计划/报告声称完成。
- Code-verified：必须附当前源码路径、Migration 文件、测试名称/结果或数据库只读证据。
- OPEN：不得实现。
- Superseded：被后续正式 DR 明确取代。

本记录不批准任何 OPEN，不修改任何正式 DR，也不把计划状态升级为代码验收结论。
