# Checkpoint-1 验证记录 V1.0

> **第一个检查点收官记录**（2026-11-08，feat/checkpoint-1-kb-pc @ 02352d0+）。
> 检查点目标（用户定义）：① 用户注册成功，创建第一个 team；② 创建第一个项目
> （ai-pm）并关联本项目仓库 URL，系统根据代码/commit 真实创建 PC 与 KB。
> 结论：**全链 PASS**（含幂等与负向证据）。

## 一、环境

| 项 | 值 |
|---|---|
| 分支 | `feat/checkpoint-1-kb-pc`（4e95d1c 纵切 + 02352d0 c180 跨批补登记，rebase 于 master 3a354ab） |
| 栈 | 本地 `infrastructure/` + **ckpt overlay**（`docker-compose.ckpt-override.yml`）：专用 PG `aipm-ckpt-pg`(16453) 两库 `lmms_ckpt`（lmms 全家）+ `pm_ckpt`（pm-api/workers） |
| 镜像 | `aipm/pm-api:0.1.0` / `aipm/pm-worker:0.1.0` / `aipm/ai-pm-worker:0.1.0` 重建（含 0053 + worker `git openssh-client`） |
| 迁移 | pm_ckpt 全量 49 项至 **0053**；lmms_ckpt 35 表（各服务自迁移） |
| 前置修复 | 容器内 rsproxy CDN DNS 黑洞 → `--add-host` 钉 IP（lmms iam 重发同款坑）；runtime 镜像补 `openssh-client`（`--no-install-recommends` 装 git 不带 ssh-client → ssh-keygen/git-over-ssh 不可用） |

## 二、端到端步骤与证据（/tmp/ckpt1-evidence/ 留档）

| # | 步骤 | 结果 |
|---|---|---|
| 1 | `GET :3000/v1/iam/auth/setup-status` | `{"setup_required":true,"user_count":0}` ✓ 全新系统 |
| 2 | `POST /v1/iam/auth/register`（leo） | 首用户注册成功，id=`WX6UUCxwu4VYSiUPTpFNX` |
| 3 | 登录 → `GET /v1/pm/me` | `is_active=false` + `onboarding_required=true` ✓ **onboarding 门生效**（JIT principal 落库 inactive；本次刻意不配 FIRST_ADMIN 对，走产品正常建团路径） |
| 4 | `POST /v1/pm/teams`（checkpoint-team/slug=checkpoint） | 建团成功 → `is_active=true`、`tenant_role=owner`、onboarding_required=false ✓ **第一个 team** |
| 5 | `POST /v1/pm/projects`（code=ai-pm） | 201，project_id=`764906ca-6e6f-47b7-98a2-abc6d6233d10` ✓ **第一个项目** |
| 6 | `POST …/ssh-keys` | ed25519 keypair 生成（公钥人工挂 GitLab .16 作只读 deploy key——**唯一人工步骤**） |
| 7 | `POST …/repositories`（`git@192.168.2.16:tooling/ai-pm.git`） | 201，repo_id=`74e99167-dad5-437a-b49c-127782dfd7d9` ✓ **关联本项目 URL** |
| 8 | verify（挂钥前） | `unreachable / auth_failed`（200+状态字段，负向证据：诚实失败不抛错） |
| 9 | verify（挂钥后） | `verified`、default_branch=master、`head_commit_sha=d9d02f76…` ✓ **真 ls-remote** |
| 10 | 触发链自动受理 | intake `51657a5e…`（`engineering_artifact`/`system`，幂等键 `repo_verify:{repo}:{head}`）pm-api 日志 `intake submitted replayed=false` |
| 11 | worker 消费全链（pm-worker 日志） | `kb: repository snapshot analyzed (revision 1)` → 信号（confidence=high，rationale="repository snapshot d9d02f76: 300 commits / 2105 files analyzed"）→ RT 路由 pc_analyze → `pc: cognition drafted … objects=8` → `pc: analysis completed (PROPOSED)` → intake **COMPLETED**（17:50:16 受理 → 17:50:24 收口，约 8s） |

## 三、产物断言（0053 最小表）

| 表 | 计数 | 断言 |
|---|---|---|
| `kb.document` | 1 | doc_key=`repo-{repo_id}`、doc_type=knowledge |
| `kb.document_revision` | 1 | revision_no=1、**status=draft**、`source_commit_sha=d9d02f76…`、content_hash=sha256(digest)、正文=仓库快照 Markdown（技术栈 Top8/近期交付 30 条/热点文件/文档清单） |
| `pc.cognition_object` | 8 | snapshot+tech_stack+delivery_activity+5×module，**全部 lifecycle_status=draft** |
| `pc.cognition_revision` | 8 | rev1 draft |
| `pc.source` | 1 | 四元组结构化列 → `knowledge_base/document/{doc_id}@{rev_id}`（禁单 JSONB 混用纪律） |
| `pc.evidence` | 8 | `verification_status=unverified`、confidence=medium、locator 带 intake/task/repo/head（可对账） |
| `governance.audit_log` | 2 | source_kind=`kb_pc_analysis`（KB 完成 + PC 派生，系统面 dispatch principal） |
| `integration.intake_record` | 1 | 幂等键唯一 |

REST 读面：K2 `GET /v1/kb/analyses/{task_id}` 返回任务态 + 文档/修订明细（含 digest 正文）；P3 `GET /v1/pc/analyses/{task_id}` 返回任务态 + 8 对象明细。存量骨架入口（K1/K4/K6/P1/P2/P4~P7）保持 501。

## 四、幂等与范围切割

- **同 HEAD 重复 verify**：不新建 intake（日志无第二次 submit——`previous_head == head` 直接短路）、kb revision 仍 1、evidence 仍 8 ✓
- **新 commit → 新 intake**：幂等键锚 `{repo}:{head}`（集成测试 `kb_pc_repo_flows` 已验 revision_no=2 + 内容寻址去重）
- **范围切割**（checkpoint-1 显式登记）：仅 `repository_snapshot` 输入走真实分析；其余 intake 输入保持 Phase 3 桩语义；产物全部 draft/unverified 等**人工确认**（AI 禁自动确认，M3 门不破）；KBGIT/PCGIT 正文权威后置（正文暂存 PG，偏差登记于迁移头注）

## 五、门禁与遗留

- 后端全量 **1028 例：1023 绿 + c180 已修绿**；余 4 红判 **baseline**（非本批）：
  - `devenv revoked_device…`（+visibility 一例同文件）：0049 `uq_device_tenant_name` 打破「撤销后重注册」假设 → 归 client-registry 批跟进
  - `team_onboarding_qa b05_02/b05_03`：被移除者仍可读名册（200）→ 已知挂账「DEF-01 修复后默认模式 QA 复跑未做」，归团队 QA 会话
- 集成测试：`kb_pc_repo_flows` 3 例 + `intake_flows` 10 例 + pm-api `intake` 7 例全绿；审计守卫 C174/C180 绿
- **回滚/恢复**：摘 ckpt overlay（不加 `-f docker-compose.ckpt-override.yml` 重 up）即恢复原栈指向（lmms→aipm-pg 旧库、pm→.10:16433）；ckpt 卷 `infrastructure_aipm_ckpt_pg_data` 可整体删除

## 六、注册→建团语义注记

首用户经**首管理员通道**（`AI_PM_FIRST_ADMIN_*`）会跳过建团门（JIT 直接 active+owner，租户占位名 `lmms:1`）；本验证刻意不配该对，走**产品正常路径**：注册 → inactive → `POST /v1/pm/teams` 认领建团激活。两路径均可用，检查点以正常路径为准。
