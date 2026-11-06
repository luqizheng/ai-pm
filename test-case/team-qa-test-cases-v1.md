# Team 功能 QA · 测试用例明细与执行记录 V1.0（执行完毕回填版）

> **记录约定**（`test-case/README.md`）：本文件即执行记录；未执行不得标 PASS；BLOCKED 写明原因。
> **判定口径**：API 响应 + DB 终态（+ UI 状态）多面证据，HTTP 200 不作唯一证据。计划与环境见 `team-qa-test-plan-v1.md`。
> **执行**：2026-11-06，master @ `3680138`；`run-team-qa.sh --shim --ignored`（最终一轮 `run-20261106-221736.log`）+ 默认模式后端层（`run-20261106-221117.log`，取 DEF-01 证据）。
> **前提注记**：除 B00 外，L1/L2 结果均在「0044_s02_i3 顺移为 0045 的仓外副本」上取得（DEF-01 未修复时仓库本身无法迁移）。
>
> **汇总**：88 条 = **73 PASS / 7 FAIL / 4 PENDING-DECISION / 3 NOT IMPLEMENTED / 1 环境记录**（B00-02）。7 FAIL 对应 6 个缺陷（DEF-04 覆盖 2 条用例）。
> - L1 后端：既有回归 43/43 PASS（team_onboarding 16、tenant_members 12、project_members 15）；新增 `team_onboarding_qa` 33 = 28 PASS / 2 FAIL / 3 待裁决
> - L2 UI：20 = 15 PASS / 4 FAIL / 1 待裁决

用例 ID ↔ 自动化：`TEAM-QA-Bxx-yy` ↔ Rust 测试名前缀 `qa_bxx_yy_*`（`apps/pm-api/tests/team_onboarding_qa.rs`）或 Playwright 标题 `Bxx-yy`（`ai-frontend/e2e/team-onboarding.spec.ts`）；「既有」= 引用已存在的集成测试。

## B00 — 前置

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| TEAM-QA-B00-01 | 迁移版本唯一 | `migrations/` 无重复版本，干净库可迁移 | **0044 重复**；干净库 `duplicate key … (version)=(44) already exists`；默认模式 project_members 0/15，所有 `sqlx::test` 同因失败 | **FAIL → DEF-01（P0）** |
| TEAM-QA-B00-02 | 本地栈健康 | 不作为本 QA 依赖 | `infrastructure-pm-api-1` 循环重启 `VersionMissing(40)`（镜像过旧）；共享 `template1` 被污染 | 记 ENV-1/ENV-2，非产品缺陷 |

## B01 — 建团 / 认领

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| TEAM-QA-B01-01 | 首登无团队（既有 `fresh_principal_is_onboarding_required`） | `onboarding_required=true`、`is_active=false`、memberships `[]` | 一致 | PASS |
| TEAM-QA-B01-02 | 认领建团，创建者=owner + 审计（既有 `create_team_claims_…`） | 201；/me owner；`team.created` 1 行 | 一致 | PASS |
| TEAM-QA-B01-03 | 已治理租户再认领（既有） | 409 `team_already_exists` | 一致 | PASS |
| TEAM-QA-B01-04 | slug 跨租户唯一（既有） | 409 `slug_already_taken` | 一致 | PASS |
| TEAM-QA-B01-05 | 非法 slug / 空名（既有） | 400 | 一致 | PASS |
| TEAM-QA-B01-06 | 档案修改 + 审计四列（既有 `team_profile_update_and_audit_rows`） | 200；`team.updated` 身份列齐 | 一致 | PASS |
| TEAM-QA-B01-07 | 同租户两人**并发**认领 | 恰 1×201 + 1×409；owner 行=1；`team.created`=1 | 一致（行锁串行化生效） | PASS |
| TEAM-QA-B01-08 | 名称/slug 边界 | 名 121 字符/大写/首连字符/下划线/101 字符 slug → 400；中文名、120 字符名、99 字符 slug → 201；空串 slug 落 NULL | 一致 | PASS（附 OBS-1） |
| TEAM-QA-B01-09 | 无 bearer 调自助路径 | 401（自助≠公开） | 建团/两个 preview 均 401 | PASS |

## B02 — 邮箱邀请

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| TEAM-QA-B02-01 | 全生命周期（既有 `invitation_full_lifecycle`） | 建→预览→接受 joined→重复 409→已成员 already_member | 一致 | PASS |
| TEAM-QA-B02-02 | PENDING 防重（同上） | 409 `pending_invitation_exists` | 一致 | PASS |
| TEAM-QA-B02-03 | owner 角色不可邀请（同上） | 400 | 一致 | PASS |
| TEAM-QA-B02-04 | 邮箱不符（既有） | 403 `email_mismatch`；不带 email 放行（V1 尽力比对） | 一致 | PASS |
| TEAM-QA-B02-05 | 过期惰性翻转（既有） | 409 `invitation_expired`，库内 EXPIRED | 一致 | PASS |
| TEAM-QA-B02-06 | 撤销后接受（既有） | 409 `invitation_revoked`；二次撤销 409 | 一致 | PASS |
| TEAM-QA-B02-07 | 跨租户接受（既有） | 403 `invitation_tenant_mismatch` | 一致 | PASS |
| TEAM-QA-B02-08 | member 不可发邀请/看列表（既有） | 403 | 一致 | PASS |
| TEAM-QA-B02-09 | 邮箱格式校验 | `no-at-sign`/`a@nodot`/`@x.com`/含空格/空 → 400 | 一致 | PASS |
| TEAM-QA-B02-10 | token 只存哈希 + 默认 TTL | 64 位 hex；库内=SHA-256(token)；TTL 168h；列表不回显明文 | 一致 | PASS |
| TEAM-QA-B02-11 | 未知 token | preview/accept 404 `invitation_not_found` | 一致 | PASS |
| TEAM-QA-B02-12 | owner 邀请 admin | 接受后库内 admin；邀请 ACCEPTED + accepted_principal_id | 一致 | PASS |
| TEAM-QA-B02-13 | 已是 member 接受 admin 邀请 | already_member；**库内角色不变** | 一致（无静默提权） | PASS |
| TEAM-QA-B02-14 | 上例响应体 role | 应为实际角色 member | 回显 `admin`（邀请单角色） | PENDING-DECISION（PD-3） |
| TEAM-QA-B02-15 | 团队休眠时接受 | 403 `team_suspended` | 一致 | PASS |
| TEAM-QA-B02-16 | 撤销守卫 | 未知 id 404；member 403 | 一致 | PASS |
| TEAM-QA-B02-17 | 同一邀请两人**并发**接受 | 恰 1×200 + 1×409；激活人数=1 | 一致 | PASS |
| TEAM-QA-B02-18 | admin 发 role=admin 邀请 | 设计 §6.1 + 直接授权口径：403 | **201**（admin 可经邀请造 admin） | PENDING-DECISION（PD-1） |
| TEAM-QA-B02-19 | 已持久化 EXPIRED 再接受 | 仍 `invitation_expired` | `invitation_already_accepted`（前端会显示「已接受」） | PENDING-DECISION（PD-2） |

## B03 — 邀请链接

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| TEAM-QA-B03-01 | 配额/幂等/耗尽（既有 `invitation_link_quota_and_idempotency`） | 2 joined→重复 already_member 不占额→第 3 人 409 exhausted | 一致 | PASS |
| TEAM-QA-B03-02 | default_role 仅 member（同上） | admin → 400 | 一致 | PASS |
| TEAM-QA-B03-03 | 撤销后领用（既有） | 409 `invitation_revoked` | 一致 | PASS |
| TEAM-QA-B03-06 | **TDM-011** max_uses=3，10 人并发领用 | 恰 3 joined / 7 exhausted；used_count=3、EXHAUSTED、领用行=3 | 一致 | PASS |
| TEAM-QA-B03-07 | 参数校验 | max_uses 0/-1、过去 expires_at、default_role=owner → 400 | 一致 | PASS |
| TEAM-QA-B03-08 | 链接过期惰性翻转 | 409 `invitation_expired`；库内 EXPIRED | 一致 | PASS |
| TEAM-QA-B03-09 | 不限次数 + preview 余量 | 5 人均 joined；remaining_uses 2→1；未知 token 404 | 一致 | PASS |
| TEAM-QA-B03-10 | 撤销守卫 | member 403；二次撤销 409 `link_not_active` | 一致 | PASS |
| TEAM-QA-B03-11 | 跨租户领用 | 403 `invitation_tenant_mismatch` | 一致 | PASS |
| TEAM-QA-B03-12 | owner 领用本队链接 | already_member；库内仍 owner；used_count=0 | 一致 | PASS |

## B04 — 权限矩阵

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| TEAM-QA-B04-01 | admin 矩阵 | 改档案/邀请/链接/名册/成员详情 2xx；解散、转让 403 | 一致 | PASS |
| TEAM-QA-B04-02 | member 矩阵 | 档案、名册 200；成员详情/链接/增删改角色 403 | 一致 | PASS |
| TEAM-QA-B04-03 | 未激活主体 | 团队面 403，只能走自助路径 | 一致 | PASS |
| TEAM-QA-B04-04 | member 写档案/发邀请（既有 `member_is_readonly_…`） | 403 | 一致 | PASS |
| TEAM-QA-B04-05 | admin/member 不可解散（既有 `dissolve_is_owner_exclusive`） | 403 `team owner required` | 一致 | PASS |
| TEAM-QA-B04-06 | token 角色不授予租户权限（既有 `rv10`） | 以库内角色为准 | 一致 | PASS |

## B05 — 名册与成员

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| TEAM-QA-B05-01 | 唯一 owner 移除自己 | 409 | 一致 | PASS |
| TEAM-QA-B05-02 | **被移除成员**读名册 | 403/404（设计 §6.3「Verify Active Membership」） | **200**，仍可读名册与团队档案 | **FAIL → DEF-02（P1）** |
| TEAM-QA-B05-03 | 被移除成员 `/me` | memberships `[]` 时应 `onboarding_required=true` | `onboarding_required=false` + `team` 有值 + memberships `[]` | **FAIL → DEF-03（P2）** |
| TEAM-QA-B05-04 | admin 降级 admin / 不可提权（既有 rv1b） | 一致 | 一致 | PASS |
| TEAM-QA-B05-05 | 降级/移除即时生效（既有 rv1_rv2、rv3） | 同一 JWT 立即失去管理面 | 一致 | PASS |
| TEAM-QA-B05-06 | 只有转让能产生 owner（既有 rv5） | 一致 | 一致 | PASS |
| TEAM-QA-B05-07 | **TDM-013** 最后 owner 不可降级/移除（既有 rv6） | 409 | 一致 | PASS |
| TEAM-QA-B05-08 | 审计失败回滚（既有 rv8） | 原子 | 一致 | PASS |
| TEAM-QA-B05-09 | 并发降级不出现零 owner（既有 rv9） | 一致 | 一致 | PASS |
| TEAM-QA-B05-10 | **TDM-012** owner 转让原子 + 双审计（既有 tn4）；admin 不可转让（tn5） | 一致 | 一致 | PASS |
| TEAM-QA-B05-11 | 项目成员面 15 例（既有 `project_members.rs`：admin 拉人入项目、撤销即时、跨租户 404 等） | 全过 | 15/15 | PASS |

## B06 — 解散与重建

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| TEAM-QA-B06-01 | 解散后旧邀请/链接 token | 404；前成员回 onboarding 门；members_removed=2 | 一致 | PASS |
| TEAM-QA-B06-02 | 重建复用**同一** slug | 201，新 owner | 一致 | PASS |
| TEAM-QA-B06-03 | 解散他租户 | 404 | 一致 | PASS |
| TEAM-QA-B06-04 | 级联软删 + 休眠 + 重复解散 409 + 重建（既有 `dissolve_cascades_and_revives_on_reclaim`） | 一致 | 一致 | PASS |

## B07 — 跨租户

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| TEAM-QA-B07-01 | 他租户读写团队面（档案读/改、名册、建邀请、建链接） | 全 404，对方数据不变 | 一致 | PASS |
| TEAM-QA-B07-02 | 随机 tenant id（既有 `cross_tenant_access_…`）、成员面（既有 rv7） | 404 | 一致 | PASS |

## B08 — /me 与 onboarding 门

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| TEAM-QA-B08-01 | 经链接入团后 `/me` 契约 | onboarding 翻 false；team.id；memberships[0] {team_id, role=member, status=active} | 一致 | PASS |
| TEAM-QA-B08-02 | 被停用成员 | onboarding_required=true；memberships `[]`；名册 403 | 一致 | PASS |

## B09 — 前端 UI（真实后端）

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| TEAM-QA-B09-01 | 首登经 `/auth/signing-in` | 分流到 `/onboarding` | 一致 | PASS |
| TEAM-QA-B09-02 | 欢迎→建团→邀请页 | 横幅含团队名；后端 owner、档案落库 | 一致 | PASS |
| TEAM-QA-B09-03 | slug 被占用 | 页面报错不跳转；后端仍未入团 | 一致 | PASS |
| TEAM-QA-B09-04 | 建团后连续流程发邀请 | 页面列出一次性接受链接；后端 PENDING | 一致 | PASS |
| TEAM-QA-B09-05 | 跳过邀请→链接页→生成→完成 | 结果含 `type=link`；后端 ACTIVE/member；跳 `/dashboard` | 一致 | PASS |
| TEAM-QA-B09-06 | 被邀者预览并接受 | `data-state=success`；后端激活 member；继续→`/dashboard` | 一致 | PASS |
| TEAM-QA-B09-07 | 已撤销邀请 | 预览即阻断，无确认按钮 | 一致 | PASS |
| TEAM-QA-B09-08 | 登录邮箱与邀请不符 | 结果态 `emailMismatch` | 显示 **`invalid`（邀请无效）**；后端实返 403 `email_mismatch` | **FAIL → DEF-04（P2）** |
| TEAM-QA-B09-09 | 名额耗尽链接 | 预览即阻断 | 一致 | PASS |
| TEAM-QA-B09-10 | 跨租户接受 | 结果态 `tenantMismatch` | 显示 **`invalid`** | **FAIL → DEF-04** |
| TEAM-QA-B09-11 | 无效 token | `invalid` | 一致 | PASS |
| TEAM-QA-B09-12 | TeamSwitcher | 顶栏显示当前团队名 | 一致 | PASS |
| TEAM-QA-B09-13 | admin 改团队资料 | 保存后后端 description 更新；无 Danger Zone | 一致 | PASS |
| TEAM-QA-B09-14 | member 只读 | 通用设置无保存；名册只读提示；无移除按钮 | 一致 | PASS |
| TEAM-QA-B09-15 | 邀请管理页建+撤销 | 行消失；后端 REVOKED | 一致 | PASS |
| TEAM-QA-B09-16 | 名册移除成员 | 后端成员行消失 | 一致 | PASS |
| TEAM-QA-B09-17 | owner 解散 | 名称不符按钮禁用；确认后回登录页；后端 team=null、onboarding 门 | 一致 | PASS |
| TEAM-QA-B09-18 | 未入团深链 `/dashboard` | 引导回 `/onboarding` | 未拦截（只在登录中转页分流） | PENDING-DECISION（PD-4） |
| TEAM-QA-B09-19 | 链接页「默认角色」字段 | 显示 MEMBER | **空白**（`<Input value=…>` 未走 modelValue） | **FAIL → DEF-05（P3）** |
| TEAM-QA-B09-20 | 刷新/深链进入 `/onboarding/invite` 发邀请 | 横幅团队名正确，可发出邀请 | 横幅显示兜底「新团队」，提交**静默无动作** | **FAIL → DEF-06（P3）** |

## B11 — V1 不做项

| ID | 场景 | 依据 | 判定 |
|---|---|---|---|
| TEAM-QA-B11-01 | 一人多团队 / 不同团队不同角色 / 切换团队（TDM-003/004/018、spec §9-6/7/8） | V1 单团队裁决（上游 iam 单租户）；memberships[] 按多团队 shape 预留；TeamSwitcher 单团队置灰 | NOT IMPLEMENTED |
| TEAM-QA-B11-02 | 成员自行离开团队（spec §9-9） | 无「离开」端点；member 调 DELETE 自身 → 403（只有管理员能移人） | NOT IMPLEMENTED |
| TEAM-QA-B11-03 | 未注册用户先注册再接受（spec §9-5）/ 历史项目迁移（TDM-017） | 账号注册归上游 lmms；Team≡tenant 无数据迁移 | NOT IMPLEMENTED（不在本仓范围） |

## 观察项

- **OBS-1（P3/文档）**：`valid_slug` 允许 100 字符，而代码注释与设计稿正则 `^[a-z0-9][a-z0-9-]{0,98}$` 为 99；B01-08 只断言两边一致的边界（99 通过 / 101 拒绝），100 的归属待对齐。
- **OBS-2**：前端 `TeamInvitationAcceptView.test.ts` 未覆盖接受失败分支，这是 DEF-04 未被单测捕获的原因。
