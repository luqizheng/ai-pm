# S02-I6-A · 权限基座与人类正式确认协议 代码事实核验报告 V1.0

> 核验对象：《S02-I6-A 权限基座与人类正式确认协议设计稿 V1.1》§18 待代码核验清单 C-01~C-13 及相关 `[待代码核验]` 项。
> 核验基线：worktree `ai-pm-s01` @ `feat/s01-implementation`，2026-11-04（HEAD 后并行会话已推进至迁移 0033）。
> 核验性质：静态代码事实（本报告不构成运行时验证；标注 `[运行时已证]` 者引自 S02-02 报告）。
> 关联缺陷：DEF-01 / DEF-02（P0）。

---

## §1. C-01~C-13 逐项核验结果

| # | 核验内容 | 结果 | 证据 |
|---|---|---|---|
| C-01 | `tenant_member` 结构及 role 枚举 | **role ∈ ('owner','admin','member')**，含 `granted_by`（授权人）与 `updated_at`；PK `(tenant_id, principal_id)` | `0019_tenant.sql:31-39` |
| C-02 | `project_member` 结构及 role 枚举 | **role ∈ ('owner','member','ai_pm','architect','developer','tester')**——一列两义：真人两档（owner/member）+ Agent 职能四值（bootstrap 独占维护，S01-W13 口径）。**无 `lead` 值** | `0030_project_member_role.sql:8-9` |
| C-03 | Project 创建者与 Lead 的关系 | **创建者自动成为 `project_member(role='owner')`**——bootstrap 直接写真实调用者（不造合成 principal），可见性按成员过滤故创建者必为成员。**Project Lead 的现有表达 = `role='owner'`** | `bootstrap.rs:38,118-119,251` |
| C-04 | membership 授权与撤销端点 | **四端点已存在**：`GET/POST /v1/pm/projects/{id}/members`、`DELETE …/members/{principal_id}`、`POST …/owner-transfer`（`project_members.rs:33-48`）。撤权即时生效=判定层每请求实时查库（无缓存）；S01-W13 X-2：租户撤成员**级联**撤其项目成员行（owner 除外——拒绝移除防无主项目） | `project_members.rs:33-48`；todo S01-W13 完成记录 |
| C-05 | 现有权限授予结构（Explicit Grant 先例） | **不存在任何项目级显式授权机制**。唯一 grant 先例 `integration.git_capability_grant`（0032 建）已被 **0033 撤下**（用户简化裁决「不区分，都可读写」——建了又删的同批先例）；`tenant_member.granted_by` 是全库唯一"授权人"字段；IAM 权限码（`PM_MCP_*`）在 JWT claims、租户/平台级、**非项目级**。→ **Requirement Approver 显式授权=全新机制，须新建表** | `0032:21-40` 建 → `0033:26` `DROP TABLE`；`auth.rs:133` claims 权限码 |
| C-06 | Human 登录身份解析 | jwks 模式：JWT → `resolve_or_provision`（`principal_identity` provider+tid+sub → 稳定 `principal_id` + `identity_id`）→ `AuthedPrincipal`。**可信 Human Subject 来源成立**；T-088 四列（auth_mode/identity_id/client/claimed_device_id）已可入审计。**注意：auth-off 模式的 dev-off 也是 human 主体**（实施回归时用例 A-10/H-11 须注意） | `auth.rs:356-375`；`auth.rs:296-352`（dev-off） |
| C-07 | MCP identity propagation | **P0 风险实证**：MCP 凭据=lmms **真人 key**（T-087 裁决，授权码/设备令牌归 V2）；`require_mcp_permission` 只验 bearer + `PM_MCP_READ` + 工具权限码，**不区分调用方是人还是持人 key 的 Agent**；DeviceClaim（`X-AI-PM-Device-Id`）仅审计不门禁（T-081 C176 `device_attribution_ignored` 运行时实证）→ **设计稿 §9.3「AI 主体只能提交申请」在现凭据模型下服务端无法分辨**（详见 §2 R-I6A-1） | `auth.rs:572-614`；记忆 T-087/T-088/T-081 |
| C-08 | Executor ActorContext 构建 | `ExecutionContext{project_id, actor: AuditActor, actor_kind, source}`；`ActorKind=Human/AiPm/DevAgent/Service` 由**进程内构造**（信任边界=进程），无外部冒充面但也无成员校验（`[运行时已证]` S02-02 V-4.5：夹具无成员行仍成功） | `command_executor` ctx 构造；S02-02 报告 V-4 |
| C-09 | `ApproveRequirementRevision` 全部调用链 | 三条：① REST `pm_writes::approve_revision`（仅租户校验）② 执行器 handler（行锁+补偿，无 membership 查询）③ orchestrator AutoApply 提案→执行器（`[运行时已证]` 策略 AutoApply + AiPm 执行成功=DEF-02）。**三条均为 DEF-02 修复面，收敛目标=统一走 Human Confirmation Service** | S02-01 V1.1 §3；S02-02 V-2/V-6 |
| C-10 | Requirement 写入统一 Application Service | **不存在**——现状=REST 直写（`pm_write`）+ 执行器两套独立实现（`[运行时已证]` V-6 语义分叉=DEF-04）→ I6-B 须新建，I6-A 先定授权协议 | S02-02 V-6 |
| C-11 | Audit Event 结构 | `governance.audit_log` 20 列（含 T-088 身份四列 + `claimed_device_id` + `compensation_command`）。REST 路径 append 无 before/after/补偿 vs 执行器路径带快照+补偿（`[运行时已证]` V-6）→ §12 审计设计落地时按统一词表收敛 | `0003/0004` DDL + T-088 迁移；S02-02 V-6 |
| C-12 | Requirement Revision 现有约束 | CHECK `(draft, approved, superseded)`——**无 `pending_confirmation`/`rejected`**（扩状态须迁移）；`UNIQUE(requirement_id, version)` 已有；`approved_by` 列现存（确认实体落地后**去留须裁决**，建议降为投影或删除）；**确认实体表不存在**（全库 grep `confirmation` 零命中） | `0002_delivery.sql:16-34` |
| C-13 | Migration 编号与现有数据 | **0030/0031/0032/0033 已全部被 S01 并行批占用**（0030 成员角色、0031 发现导入 v2、0032 Git 能力、0033 简化撤表）；0029 空号。→ **S02 迁移自 0034 起**（此前「0032 起」口径全局过期，见 §2 R-I6A-6） | `ls migrations/` |

---

## §2. 评审意见（核验中发现的设计问题）

**R-I6A-1 · P0 设计缺口：MCP 真人 key 冒充面（C-07）**
T-087 裁决下 AI Agent 持**真人 lmms key** 调 MCP——服务端身份解析结果=human，设计稿以 actor-kind 为防线（§9.3「AI 主体只能提交确认申请」）在该凭据模型下**失效**：Agent 可替 key 主人完成 ConfirmRevision 的全部检查（身份✓成员✓Lead/Approver✓版本✓）。候选处置：
- **(a) V1 确认操作仅 REST/UI 提供，MCP 不出 ConfirmRevision 工具**（工具面裁剪，I6-B 落实）——最简，推荐；
- (b) MCP 确认要求 step-up（重新认证/专用确认凭据）——T-087 已定授权码/设备令牌归 V2，等于推迟；
- (c) DeviceClaim 门禁——与 T-087 定稿「Device ID 仅不可信声明、不作为授权依据」冲突，不可行。
**需用户裁决**；无论选哪个，§9.3 须补「确认入口白名单」一段。

**R-I6A-2 · Project Lead 映射裁决（C-02/C-03）**
现表达=`project_member.role='owner'`（两档，无 lead 值）。建议**复用 owner=Project Lead**（别名映射），不新增枚举值——0033 简化裁决（「不区分，都可读写」）显示用户偏好少枚举；C-03 已证创建者自动 owner。

**R-I6A-3 · Approver 授权表为全新机制（C-05）**
无先例可复用（唯一 grant 先例已撤）。建议照设计稿 §11.2 逻辑模型新建（单权限 `requirement.confirm`、单项目、绑定 Human），并吸取 0033 教训：**最小化**——不做能力数组/依赖闭包/多权限复用。迁移落 0034+。

**R-I6A-4 · `approved_by` 列去留（C-12）**
确认实体落地后该列冗余（同一事实两处）。建议随扩状态迁移一并裁决：降为派生投影或删除（AGENTS 规则 1 无存量→可直接删）。挂 S02-I1。

**R-I6A-5 · 人类起草路径仍未定义**
§7.3 确认流程从「AI-PM 创建草稿」起——人类直接起草（S02-03 §4.1）时谁执行 Markdown 文件写入仍未定义（我方两次提出）。建议 §16 依赖表补一行：人类起草入口的写入路径由 S02-I2 定义、I6-A 仅约束其授权。

**R-I6A-6 · 迁移号口径全局更正（C-13）**
「S02 自 0032 起」已在三处过期：启动基线 §6.3、我方 R-17、todo §10.9。**正确口径：S02 迁移自 0034 起（0029 空号，0030-0033 已用）**。

**R-I6A-7 · 小项**
① auth-off 的 dev-off 是 human 主体——验收用例 A-10/H-11 的实施回归须用真 jwks 身份或显式注明 dev-off 语义；② 确认实体字段建议补 `identity_id` 与 `claimed_device_id`（对齐 T-088 审计四列，供归因与 R-I6A-1 的后续治理）；③ §17 验收表质量高，建议补一条 H-15「MCP 面 ConfirmRevision 工具不存在/被拒」（取决于 R-I6A-1 裁决）。

---

## §3. 设计稿可定稿项（核验支持）

设计稿以下内容与代码事实相容、无需改动：授权判断顺序（§3.3 十一步）、Actor 分类与禁止自报身份（§4，与 auth.rs 可信解析一致）、Human Confirmation 三对象分离与不可变规则（§6/§8，C-12 证新表无冲突）、Q-A4 替代式修正（§8.3 与 Q-17/修订史相容）、错误语义表（§14）、DEF-01/02 验收用例 A-01~A-10 / H-01~H-14（§17）。

**结论**：设计稿方向成立；关闭 §20 完成条件中「membership 与角色模型已核验」一项（本报告 C-01~C-04）；R-I6A-1（MCP 冒充面）为**唯一 P0 待裁**，R-I6A-2/3/4 需用户确认映射与表设计取向后即可进实施设计。
