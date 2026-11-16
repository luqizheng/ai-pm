# R-1 裁决 × 现状实现差异登记 V1.0

> 日期：2026-11-16
> 性质：**差异登记，非缺陷清单**——R-1 是目标态架构裁决；现状是已实现的过渡态。差异项全部归属**后续技术设计与 Schema 演进**（裁决 §7 明确不在本次范围），本登记的目的是防止演进时重新发现或反向破坏裁决。
> 现状证据：全部为本日源码实证（Phase 0 复核 + 本轮 0022 迁移/应用层追踪）。

## 1. 现状实现（已实证）

| 维度 | 现状 | 证据 |
|---|---|---|
| 实体 | `integration.project_ssh_key`：**项目所有**（FK=project_id），无 team 维度 | `migrations/0022_ssh_key_multi.sql` |
| 池语义 | `is_default` 每项目唯一（部分唯一索引）；`UNIQUE(project_id, label)` 同名导入=替换 | 0022:27-32 |
| 回退链 | 未绑定仓库自动回退「项目默认 key → 最新 active」 | `repository/ssh_key.rs:3-4` |
| 自动创建 | `ensure_ssh_key` 项目初始化向导**幂等自动生成/复用默认 key**；第一把 active key 自动成默认 | `ssh_key.rs:37,64-68,157` |
| 绑定 | `project_repository.ssh_key_id` FK RESTRICT；`bind_repository_ssh_key`（人工操作） | 0022:36-38；`repo.rs:261` |
| 轮换 | rotate=同 row 换钥，绑定仓库自动跟随 | `ssh_key.rs:64-68` |
| 消费 | Git 访问链 `ssh -i "{key}"`（分析 clone + 执行链按 repo id 取 pem） | `git_access.rs:113` |
| 前端 | 独立页 `/project/settings/ssh-keys`（nav.ts:69 + router:306-309），项目级管理（导入/label/默认/吊销/绑定） | Phase 0 报告 |
| API | `/v1/pm/projects/{id}/ssh-keys*`（列表/ensure/import）+ `/v1/pm/ssh-keys/{key_id}`（吊销，key_in_use 409） | `pm_repositories.rs:65-78,512` |

## 2. R-1 目标模型（裁决）

**Team-owned Credential + Project-scoped Grant + Repository-level Binding + Execution-time Access**（裁决 §3）。

## 3. 差异登记（后续技术设计的强制输入）

| # | 差异 | 现状 | R-1 要求 | 处置方向 |
|---|---|---|---|---|
| Δ-1 | **所有权**：project → team | 实体 FK=project_id，无 team 维度 | Credential 归 Team，新增 Project Grant 授权关系（D1） | 新增 team credential 实体 + grant 关系实体；按仓库 AGENTS 规则 1 直接改契约/重建表，**不做双写兼容** |
| Δ-2 | **默认兜底语义（冲突）** | 未绑定仓库自动回退项目默认 key（is_default→最新） | **D2.5：不默认将某凭证应用于项目内全部仓库** | 回退链**废弃**：未绑定 = 明确失败/引导绑定，不得静默兜底。涉及 `ssh_key.rs` 解析函数、执行链取钥、向导文案 |
| Δ-3 | **自动创建（冲突）** | 初始化向导 `ensure_ssh_key` 幂等自动生成默认 key（系统自动创建） | **D3.1：创建必须人工**；D3.5 AI/系统不得自行创建 | 向导改为「引导人工创建/选择」步骤；自动生成逻辑移除或降级为「AI 建议 + 人工确认」 |
| Δ-4 | 授权检查（新增能力） | 绑定仅在项目内校验（repo.rs:278 校验同项目） | Repository 绑定不得突破 Team 授权范围（D2.6）；执行时按授权使用（§4.5） | 新增 grant 校验点：绑定写路径 + 执行取钥路径 |
| Δ-5 | 撤销联动 | 吊销按 key id，key_in_use 409 挡 verified 绑定（pm_repositories.rs:512） | 撤销后阻止新使用 + 在途失效清理（§4.7）+ Project 授权撤销联动（D1.6） | 技术设计补在途协议与 grant 撤销级联 |
| Δ-6 | 私钥存储 | 加密机制现状未核验（L2 测试链提示存在 AES 信封通道，**不妄断**） | §4.1 不得明文存业务库；§4.3 不得复制不受控副本 | 技术设计首项=核对现有存储是否满足 §4 全部安全约束 |
| Δ-7 | Read/Write 权限显式化 | 绑定不区分读写意图 | §4.8 Read/Write 必须明确，绑定≠默认写权限 | 技术设计补绑定模型读写意图字段（受 Provider 能力约束 §4.9） |

## 4. Phase 1 边界（本裁决的执行口径）

1. **仅导航配置**：Team 设置子树新增「Git 凭证（SSH Keys）」入口，**PLANNED 占位**（目标 UI 未实现，按纪律不得伪装已完成）；Project 设置**不新增**独立 SSH Keys 模块。
2. **现有 `/project/settings/ssh-keys` 页面/路由/API 原样保留**（过渡态仍在服役），不做 redirect——并入仓库分区的凭证绑定 UI 属「后续页面改造」，迁移完成前重定向会打断在用的密钥管理。兼容清单 C-8 按此口径。
3. **不做 Δ-1~Δ-7 的任何实现**——全部归后续技术设计与 Schema 演进（那时直接改契约，遵守 AGENTS 规则 1）。
4. 现有页面在过渡态继续按现状语义运行（含默认 key 兜底），**不因裁决立即改行为**——避免「裁决未实施先行改坏在用链路」；Δ-2/Δ-3 的行为切换随 Schema 演进一并落地。

## 5. 对既有文档的更正

- Phase 0 报告 §6 C-8、Phase 1 任务书 §5 Commit 04/§6 BLOCKED、IA 基线相关条目按本登记更新（任务书已随本文件同步修订）。
- 说明书残留缺口 **G-1 关闭**（SSH 归属已裁决）；G-5（《IA 基线 V1.1/V1.2 正文未落盘》）仍开放——本裁决附一即 V1.2 修订内容，建议正式 IA 基线 V1.2 文档落盘时与本登记互引。
