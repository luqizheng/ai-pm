# I3 · Git 权限与仓库写入授权设计稿 V1.1

> 文档性质：实施基线（V1.0 已被 §0.0 简化裁决部分取代）  
> 所属工作包：S01  
> 状态：**V1.1 简化裁决生效**（2026-11-04，用户裁决）  
> 日期：2026-11-03（V1.0）/ 2026-11-04（V1.1）  
> 核心原则：Git 能力必须由明确授权产生，不能由自动化策略或 Agent 自行推导。

---

## 0.0 V1.1 简化裁决（2026-11-04）

> **效力**：本节为**最高优先级**，与下文 §1~§14 冲突时**一律以本节为准**。
> 下文保留 V1.0 原文作为**决策留痕**（当时为何这么设计、为何撤回），不再作为实施依据。

### 0.0.1 裁决原文与理由

用户裁决（2026-11-04）：

> 「不区分，都可以读写。因为这部分太复杂。」

理由：V1.0 的六能力模型 + 四层授权交集 + 依赖闭包 + 逐主体授权/撤销，
在 S01 阶段引入的**概念数与状态数远超实际收益**——平台在 S01 只区分
「能不能改这个仓库」，而 modify/commit/push/create_pr/merge 的分级授权
缺少真实业务用例支撑，却要付出：一张授权表、一套闭包判定、一组 REST 端点、
一次撤权语义、一套依赖校验规则和一份 24 条验收矩阵。

### 0.0.2 简化后的模型（**唯一实施依据**）

```
可访问仓库 = Task 关联仓库（delivery.task_repository）∩ 仓库状态 = verified
访问档位    = read_write（单一档位：可读即可写，不区分操作类型）
```

判定链（自上而下，任一不成立即拒绝）：

| # | 条件 | 依据 | 失败后果 |
|---|---|---|---|
| 1 | 租户内 + 项目内可见 | `core.project` / `core.project_member` | 404（不泄露存在性） |
| 2 | 仓库在 **Task 关联范围**内 | `delivery.task_repository` | 不在快照 → 拒绝 |
| 3 | 仓库状态 = `verified` | `integration.project_repository.status` | 不在快照 → 拒绝 |
| 4 | Project 未归档 | I1 归档门禁（`project_gate`） | 执行期拒绝（写入） |
| 5 | Run 未取消 | Run 生命周期 | 执行期拒绝 |

**写操作不再分级**：拿到 `read_write` 后，修改 / 提交 / 推送 / 建 PR / 合并
不再逐项判定，也不再做依赖闭包校验（不再有「允许 push 必须先有 commit」这类规则）。

### 0.0.3 删除项（V1.0 → V1.1）

| 删除项 | 原位置 | 处置 |
|---|---|---|
| 六能力枚举（read/modify/commit/push/create_pr/merge） | §3.1 | 废止；改为单一 `read_write` 档位 |
| 默认最小权限（默认拒绝写） | §3.2 | 废止；范围内仓库默认可读可写 |
| 能力依赖链与前置校验 | §3.3 | 废止（不再有分级） |
| 第一层「调用人授权」 | §2.2 | 废止（不再有逐主体授权） |
| 第二层「Project/Repository 策略」 | §2.3 | 废止（不再有逐仓能力策略） |
| 第四层「Run 执行快照」的**能力分级**语义 | §2.5 | 保留快照（改记**可访问仓库清单**） |
| 显式撤权（逐主体撤销） | §6.2 | 废止（无逐主体授权可撤）；**归档/取消仍即时阻断** |
| `integration.git_capability_grant` 授权表 | 迁移 0032 | **迁移 0033 删除** |
| `PUT/DELETE/GET .../capabilities/{principal_id}` | REST | **撤下**（见 §0.0.5） |

### 0.0.4 保留项（**不简化**）

这些不是「Git 能力分级」，而是**访问边界**，与 S01 的租户隔离、归档门禁、
交付闭环直接相关，一律保留：

1. 租户 / 项目隔离（跨项目、跨租户一律 404）；
2. **Task 仓库范围**（Task 未关联的仓库不可访问——这是"哪些仓库"，不是"什么操作"）；
3. **仓库状态 = `verified`**（未验证/已归档的仓库不可访问）；
4. **Run 授权快照 + SHA-256 内容寻址**（`task_run.policy_snapshot_hash` +
   `repository_policy_snapshots`）——快照改记「本次 Run 可访问的仓库清单」；
5. **Project 归档门禁**（归档后禁止一切新的写入与执行，I1）；
6. **Run 取消门禁**（凭据撤销后不得继续，I1/T-032 D3）。

### 0.0.5 副作用与已知取舍（**明示，不隐藏**）

1. **失去「只读仓 / 只写不推 / 禁合并」等细粒度控制**。缓解：Task 范围
   本身承担了「这个任务碰哪些仓库」的控制力；平台侧不再有单仓操作类型的
   二次开关。若将来出现真实用例（如"只读分析任务不应产生提交"），
   应在**交付规则**层实现（`delivery` 侧的只读仓驳回规则已存在），
   而不是重新引入能力分级。
2. **`delivery.task_repository.write_access` 不再决定访问档位**（列保留、
   历史数据保留）：范围内且 verified 的仓库一律 `read_write`。
   该列**仍是调度前置**（`run_dispatch::policy` 要求「至少一个可写仓库」才可领取）
   与**交付规则**（只读仓产出 commit 整单驳回）的输入——那两处与「访问档位」无关，
   不在本次简化范围内。
3. **MCP 校验仍不等于安全边界**（§6.4 结论保留）：执行环境对直接 Git 命令
   的约束未在本阶段实现，属 T-090 两机实测项。
4. **撤权即时生效的语义改由「状态变化」承担**：仓库被撤出 Task 范围、
   仓库状态变化、Project 归档、Run 取消——这四种状态变化即时阻断；
   不再有「撤销某个主体的某项能力」这一动作。

### 0.0.6 验收映射调整（§11 的 24 条如何落到简化模型）

| V1.0 验收项 | V1.1 处置 |
|---|---|
| I3-001（Agent 自申请更高权限 → 拒绝） | **保留**：无授权概念即无自申请入口 |
| I3-002（调用人无授权 → 拒绝） | **改写**：非 Task 范围 / 非 verified 仓库 → 拒绝 |
| I3-003（Project/Repository 策略拒绝） | **改写**：仓库不在 Task 范围 → 拒绝 |
| I3-004（Task 未关联 Repository → 拒绝） | **保留**（核心负向） |
| I3-005（Repository 已断开或不可用） | **保留**（`verified` 判定） |
| I3-006（四层授权全部允许 → 生成有效快照） | **改写**：范围内 verified 仓库 → 生成快照 |
| I3-007~010（Modify/Commit/Push/Merge 未授权 → 阻止） | **合并**：写操作不分级，统一由「范围内 + verified」决定 |
| I3-011（auto_merge=true 无 Merge 权限） | **改写**：auto_* 不再是也不产生 Git 权限 |
| I3-012（require_human_approval=false 无 Git 权限） | **改写**：同上 |
| I3-013（Task 只关联 backend → 不能访问 frontend） | **保留**（核心负向） |
| I3-014（多仓策略不同 → 分别执行） | **改写**：多仓按 Task 范围与状态分别判定 |
| I3-015（快照后策略变更 → 历史快照不被覆盖） | **保留**（快照不可变） |
| I3-016（快照后显式撤权 → 拒绝） | **改写**：撤权语义由状态变化承担（§0.0.4） |
| I3-017（Run 取消后 Git 操作 → 拒绝） | **保留** |
| I3-018（Project 归档后 Git 写操作 → 拒绝） | **保留**（核心负向） |
| I3-019（绕过 MCP 仍受执行环境约束） | **保留**（T-090 实测项） |
| I3-020（本地有凭据也需 Push 授权） | **改写**：写权限已统一放开，凭据隔离仍属执行环境项 |
| I3-021（policy_version INT 保持语义） | **保留** |
| I3-022（快照哈希独立保存） | **保留** |
| I3-023（撤权与操作并发的线性化） | **改写**：状态变化与操作并发的线性化规则保留 |
| I3-024（历史 Run 可还原当时授权依据） | **保留**（快照记仓库清单 + 哈希） |

### 0.0.7 实施拆分调整（§12）

| 任务 | V1.0 范围 | V1.1 范围 |
|---|---|---|
| I3-A 现状核验 | 保留（已完成） | — |
| I3-B 能力模型 | 六能力枚举 + 授权表 + REST | **简化为**：迁移 0033 撤下授权表；判定层改单一档位 |
| I3-C Task 仓库范围 | 保留 | 不变（`task_repository` 已在 0031） |
| I3-D Run 快照 | 保留 | 不变（快照内容改为仓库清单） |
| I3-E 执行期强校验 | 保留 | 不变（判定源改为快照 ∩ 现状） |
| I3-F 审计与回归 | 撤销授权/策略变更审计 | **改为**：快照生成审计 + 归档/取消联动回归 |

### 0.0.8 迁移影响

- `0032` 保留：`task_run.policy_snapshot_hash`（I3-D 的列**仍需要**）。
- `0033` 新增：`DROP TABLE integration.git_capability_grant`（撤下授权表）。
- 无回填（AGENTS.md 规则 1：无存量数据）。

---

## 0. 设计目标与边界

### 0.1 背景

平台允许 AI Agent 通过 Codex CLI 等执行环境完成代码开发。

一个 Project 可以关联多个 Repository。

同一个 Project 下，不同 Task 可能涉及不同仓库，且不同 Task 的操作范围也可能不同。

例如：

- Task A：只允许读取仓库，进行架构分析。
- Task B：允许修改代码，但不允许提交。
- Task C：允许修改并创建本地 Commit。
- Task D：允许推送到远程分支。
- Task E：允许创建 Pull Request，但不能直接合并。
- Task F：允许执行经过人工批准的合并操作。

如果只设置一个 `write_access=true`，无法表达上述差异。

如果仅依赖 Agent 自己声明需要什么权限，也无法保证执行边界。

因此 I3 建立四层授权模型：

1. 调用人授权；
2. Project/Repository 策略合成；
3. Task 仓库与操作范围约束；
4. Run 执行期授权快照。

四层必须同时成立，最终有效权限取最严格的交集。

### 0.2 设计目标

必须满足：

1. Git 权限有明确的人类授权来源。
2. Agent 不得自行提升权限。
3. 不同 Repository 可以拥有不同授权。
4. 不同 Task 可以拥有不同仓库范围。
5. Task 只能使用其明确关联的 Repository。
6. Run 执行期间授权不可被 Agent 修改。
7. 权限撤销能够影响后续 Git 操作。
8. Project 归档后所有 Git 写操作禁止。
9. auto_\* 策略不等于 Git 权限。
10. Run 的执行权限必须可追溯。
11. 数据库和执行器共同执行授权边界。
12. 授权失败必须默认拒绝。

### 0.3 不在本设计范围内

- Git Provider OAuth 完整授权体系。
- GitHub/GitLab App 安装管理。
- 远程分支保护规则的完整实现。
- Pull Request 审核工作流。
- 代码审查业务规则。
- Codex CLI 本身的权限实现细节。
- 组织级 Git 凭据托管服务的完整设计。

---

## 1. 现有代码事实

### 1.1 自动化策略字段

已核验，现有策略字段包括：

- `auto_accept`
- `auto_merge`
- `auto_publish_knowledge`
- `require_human_approval`
- `additional_checks`

**设计裁决：**

这些字段用于控制自动化流程，不表达 Git 仓库读写授权。

不得把：

- `auto_accept` 理解为允许 Git 写入；
- `auto_merge` 理解为允许推送或合并；
- `require_human_approval=false` 理解为自动授予 Git 权限；
- `auto_publish_knowledge` 理解为允许修改 Repository。

Git 权限必须建立独立授权表达。

### 1.2 现有 write_access 链路

已核验：

现有 `write_access` 读取链路已经定位。

但执行期强校验仍需在实施前完成核验。

**设计裁决：**

I3 不能只调整读取端或 UI。

必须确保最终 Git 操作发生前，执行器能够检查有效授权。

### 1.3 Run 执行快照

已核验：

`task_run.policy_version` 是 INT，定义来自 migration 0010。

此前 D2.1 设计提出使用 SHA-256 前缀内容寻址策略快照。

两者存在类型和语义缺口。

**设计裁决：**

不能直接把 SHA-256 字符串写入现有 INT 字段。

I3 必须单独设计兼容方案，不能假设现有 `policy_version` 可以承担策略内容哈希。

---

## 2. 四层授权模型

> ⚠️ **已被 §0.0 简化裁决取代**：四层交集（调用人授权 / 仓库策略 / Task 范围 /
> Run 快照）不再是实施依据——第一、二层已废止，第三、四层保留并简化。
> 以下 V1.0 原文仅作决策留痕。

### 2.1 总体模型

最终 Git 权限：

`EffectiveGitPermission = CallerGrant ∩ ProjectRepoPolicy ∩ TaskRepoScope ∩ RunSnapshot`

四层中任意一层拒绝，最终均拒绝。

任何一层缺失、无法解析或数据不一致，都按拒绝处理。

### 2.2 第一层：调用人授权

调用人授权回答：

**这个真人是否有权允许 Agent 对某个仓库执行某类 Git 操作？**

授权来源必须是可信的人类身份和平台权限体系。

授权至少需要区分：

- 仓库读取；
- 工作区修改；
- 本地 Commit；
- 远程 Push；
- PR 创建；
- Merge。

不能将这些操作全部压缩为一个 write 权限。

授权必须绑定：

- Tenant；
- Project；
- Repository；
- 授权人；
- 授权范围；
- 生效状态；
- 授权时间。

具体存储结构沿用现有权限体系，若现有模型无法表达，则在实施设计中新增明确的数据结构。

### 2.3 第二层：Project/Repository 策略

回答：

**这个项目允许 Agent 对关联仓库执行哪些操作？**

策略由 Project 管理者或有权限的真人配置。

同一 Project 下可以为不同 Repository 设置不同策略。

示例：

| Repository | Read | Modify | Commit | Push | PR | Merge |
|---|---:|---:|---:|---:|---:|---:|
| backend | 是 | 是 | 是 | 否 | 否 | 否 |
| frontend | 是 | 是 | 是 | 是 | 是 | 否 |
| infrastructure | 是 | 否 | 否 | 否 | 否 | 否 |

策略不得通过 Agent 自己的 Task 描述或自然语言请求自动提升。

### 2.4 第三层：Task 仓库范围

回答：

**当前 Task 可以使用哪些 Repository，以及在这些仓库中允许做什么？**

一个 Project 可以有多个 Repository。

Task 必须明确声明其涉及的 Repository。

Task 仓库范围不能默认等于 Project 下全部 Repository。

示例：

Project 有三个仓库：

- backend；
- frontend；
- infrastructure。

Task 只关联 backend 和 frontend，则不得读取或修改 infrastructure。

Task 范围可以进一步限制操作：

| Task | Repository | 允许操作 |
|---|---|---|
| TASK-001 | backend | Read、Modify |
| TASK-001 | frontend | Read |
| TASK-002 | infrastructure | Read |

Task 不得提升 Project/Repository 策略，也不得提升调用人授权。

### 2.5 第四层：Run 执行快照

回答：

**本次 Run 实际获得了什么权限？**

Run 创建时计算有效权限，并生成不可变快照。

Run 执行时只能使用快照中明确授予的权限。

Run 不得：

- 自行修改快照；
- 在执行过程中扩大仓库范围；
- 通过 MCP 申请更高权限后直接生效；
- 将其他 Run 的权限复制给自己。

快照必须能够追溯到：

- 调用人授权；
- Project/Repository 策略版本；
- Task 仓库范围；
- 最终有效权限。

---

## 3. Git 操作权限模型

> ⚠️ **已被 §0.0 简化裁决取代**：六能力拆分（§3.1）、默认最小权限（§3.2）、
> 能力依赖（§3.3）**全部废止**；写操作不再分级（单一 `read_write` 档位）。
> 以下 V1.0 原文仅作决策留痕。

### 3.1 操作能力拆分

建议使用以下能力集合：

| 能力 | 含义 | 风险等级 |
|---|---|---|
| `read` | 读取仓库内容、分支和提交信息 | 低 |
| `modify` | 修改本地工作区文件 | 中 |
| `commit` | 创建本地 Git Commit | 中 |
| `push` | 推送远程分支 | 高 |
| `create_pr` | 创建 Pull Request | 高 |
| `merge` | 执行远程合并 | 很高 |

这些能力相互独立。

不能因为允许 `modify` 就自动允许 `commit`。

不能因为允许 `commit` 就自动允许 `push`。

不能因为允许 `push` 就自动允许 `merge`。

### 3.2 默认权限

默认采用最小权限：

- `read`：可以由 Project/Repository 的明确策略授予；
- `modify`：默认拒绝；
- `commit`：默认拒绝；
- `push`：默认拒绝；
- `create_pr`：默认拒绝；
- `merge`：默认拒绝。

任何未配置、未识别或无法解析的权限均视为拒绝。

### 3.3 能力依赖

部分操作存在前置条件：

- `modify` 需要 `read`；
- `commit` 需要 `modify`；
- `push` 需要 `commit`；
- `create_pr` 需要对应远程仓库和分支的访问能力；
- `merge` 需要独立的 Merge 授权。

实施时应将依赖作为服务端校验规则。

不能仅在 UI 隐藏不合理的权限组合。

### 3.4 不允许的隐式授权

以下情况均不能产生 Git 权限：

- Agent 自称需要写入权限；
- Task 描述中出现“请修改代码”；
- `auto_merge=true`；
- `require_human_approval=false`；
- Repository 已连接；
- 当前用户有仓库读取权限；
- Codex CLI 本地已经配置 Git 凭据；
- 其他 Run 曾经获得过写权限。

---

## 4. 权限合成规则

### 4.1 集合交集

对于每一个 Repository、每一种 Git 操作，分别计算四层授权。

例如：

调用人授权：

`read, modify, commit`

Project/Repository 策略：

`read, modify`

Task 范围：

`read, modify`

Run 最终快照：

`read, modify`

最终权限：

`read, modify`

Commit 不会因为调用人允许就自动获得，因为 Project 策略没有授予。

### 4.2 仓库范围

最终仓库范围：

`EffectiveRepositories = CallerRepositories ∩ ProjectRepositories ∩ TaskRepositories`

Task 没有声明的 Repository 不得进入 Run 授权快照。

不存在于 Project 的 Repository 不能因为 Task 指定了它就获得授权。

### 4.3 缺失策略

如果以下任一项无法读取：

- 调用人授权；
- Project/Repository 策略；
- Task 仓库范围；
- Repository 连接状态；

则拒绝对应操作。

不得降级为“默认允许”。

### 4.4 Repository 状态

Repository 必须处于允许访问的连接状态。

如果 Repository 已断开、已禁用或关联关系失效，相关操作必须拒绝。

具体状态枚举以现有 Repository 模型为准，不在设计稿中预设数据库状态值。

---

## 5. Run 授权快照

### 5.1 快照目的

Run 快照解决两个问题：

1. 让每次执行具有可审计的授权依据。
2. 避免执行过程中权限配置变化导致行为无法解释。

### 5.2 快照内容

每个 Run 的 Git 授权快照至少包含：

- `run_id`；
- `task_id`；
- `project_id`；
- Repository ID 列表；
- 每个 Repository 的有效操作权限；
- 授权人或授权来源；
- Project/Repository 策略版本；
- Task 仓库范围；
- 快照生成时间；
- 快照格式版本；
- 内容哈希。

建议采用规范化 JSON 内容。

内容哈希使用 SHA-256。

### 5.3 不可变性

Run 快照创建后不可修改。

如果需要扩大权限：

1. 由有权限的人类重新授权；
2. 更新对应策略或 Task 范围；
3. 重新计算有效权限；
4. 创建新的 Run。

不允许直接修改已有 Run 的快照。

### 5.4 policy\_version 兼容

现有：

`task_run.policy_version INT`

不能直接承载 SHA-256 内容哈希。

因此：

- 不修改该字段的数据类型来强行兼容；
- 不将哈希截断后伪装成原 policy\_version；
- 不改变该字段现有含义；
- Git 授权快照应使用独立的内容哈希字段或独立快照记录。

推荐设计：

独立的 Run Git 授权快照记录，使用 `run_id` 唯一关联。

具体表结构在实施任务中根据现有 Run 快照设施决定。

如果已有通用 Run policy snapshot 结构，应优先扩展；否则新增独立结构。

### 5.5 快照查询

历史 Run 必须能够查询：

- 当时允许访问哪些 Repository；
- 当时允许哪些 Git 操作；
- 授权依据是什么；
- 是否因后续撤权而失效。

不能只展示当前 Project 策略来代替历史 Run 快照。

---

## 6. 权限撤销与执行期强校验

### 6.1 基本原则

Run 快照是执行授权的基准，但不是永久不可撤销的许可证。

人类可以撤销授权。

撤权必须影响后续 Git 操作。

### 6.2 两种变化

**策略变更：**

Project/Repository 策略发生变化。

默认不自动改变已经创建的 Run 快照。

新 Run 使用新策略。

**显式撤权：**

有权限的人类撤销某个调用人、Repository 或操作能力的授权。

撤权必须阻止后续相关 Git 操作。

### 6.3 执行期检查

执行器在实际 Git 操作发生前必须检查：

1. Run 仍然有效；
2. Run 未取消；
3. Run 凭据有效；
4. Project 未归档；
5. Repository 仍属于该 Project；
6. Task 仍授权该 Repository；
7. 当前操作存在于 Run 快照；
8. 当前操作未被显式撤销。

任何一项失败，都必须拒绝操作。

### 6.4 强制检查位置

检查必须覆盖实际执行 Git 命令的路径。

不能只在 MCP 工具层校验。

如果 Codex 子进程能够绕过 MCP，直接使用本地 Git 命令、远程凭据或其他执行通道，则 MCP 校验不能构成完整安全边界。

实施时必须明确：

- Git 命令由谁执行；
- Codex 是否可以直接执行 Git；
- Git 凭据由谁持有；
- Push 使用什么凭据；
- 本地文件修改如何约束；
- 是否存在其他绕过授权的通道。

如果无法拦截所有 Git 写入路径，则必须通过操作系统隔离、凭据隔离、受控 Git 代理或其他等效手段实现强制执行。

不能声称仅有数据库快照就能限制本地 Git 命令。

---

## 7. Git 权限与 Codex 执行环境

### 7.1 本地工作区

Codex 执行环境应只挂载 Task 授权范围内的 Repository。

不在 Task 范围内的仓库，不应暴露给该 Run。

### 7.2 读权限

Read 授权控制：

- 工作区读取；
- Git 历史读取；
- 分支信息读取；
- Commit 信息读取。

### 7.3 写权限

Modify 授权控制本地文件修改。

如果 Agent 能直接写文件，必须通过执行环境隔离或文件系统权限保证其不能修改未授权 Repository。

### 7.4 Commit

Commit 权限必须独立判断。

禁止通过本地 Git 配置或 Agent 自行执行 Git 命令绕过授权。

### 7.5 Push

Push 必须使用受控凭据。

远程凭据不应直接暴露给 Agent。

没有 Push 授权时，即使本地存在可用 Git 凭据，也必须拒绝远程写入。

### 7.6 Merge

Merge 是独立的高风险操作。

必须具备明确授权。

是否还要求额外人工审批，属于业务审批策略，与 Git Merge 能力授权是两个独立条件。

---

## 8. 与自动化策略的关系

### 8.1 自动化策略不授予 Git 权限

保持现有字段语义：

- `auto_accept`
- `auto_merge`
- `auto_publish_knowledge`
- `require_human_approval`
- `additional_checks`

它们不参与 Git 权限集合交集。

### 8.2 auto\_merge

`auto_merge=true` 只表示自动化流程允许尝试执行自动合并策略。

它不能自动授予：

- `read`；
- `modify`；
- `commit`；
- `push`；
- `merge`。

实际执行 Merge 仍须通过 Git 权限授权。

### 8.3 人工审批

`require_human_approval` 决定业务流程是否要求人工审批。

它不代表 Git 操作能力。

即使 `require_human_approval=false`，也不能绕过 Git 授权。

### 8.4 职责分离

最终执行某项操作必须同时满足：

- Git 能力授权；
- 业务流程审批要求；
- Run 生命周期允许；
- Project 生命周期允许。

这几项是独立条件，不得混为一谈。

---

## 9. 并发与安全要求

### 9.1 权限计算

Run 创建时必须在一致性事务或可证明一致的读取协议中计算授权快照。

不能读取一半旧策略、一半新策略后生成无法解释的授权。

### 9.2 授权撤销

撤权与 Git 操作并发时，必须明确线性化语义。

推荐：

- Git 操作开始前检查有效授权；
- 显式撤权通过统一授权版本或撤销记录生效；
- Git 操作在不可中断阶段开始后，撤权至少必须阻止下一次操作；
- 高风险远程操作必须在真正调用 Provider 前再次检查。

不能承诺一个已经发送给远程 Git Provider 的网络请求一定能够被撤回。

### 9.3 Project 归档

遵循 I1：

Project 归档后：

- 禁止创建新 Run；
- 禁止新 Git 写操作；
- 现有 Run 凭据撤销后不能继续操作；
- 历史快照继续保留。

### 9.4 Run 取消

遵循 T-032 D3：

Run 凭据撤销后，后续 Git 操作必须拒绝。

进程终止是后续资源清理，不替代授权撤销。

---

## 10. 数据模型与接口实施要求

本节规定逻辑要求，不预先声称新增结构已经存在。

### 10.1 授权数据

必须能够表达：

- 授权主体；
- Tenant；
- Project；
- Repository；
- Git 操作能力；
- 授权状态；
- 授权来源；
- 生效时间；
- 撤销时间。

### 10.2 Project/Repository 策略

必须能够按 Repository 单独设置操作能力。

不能只在 Project 级使用一个全局布尔值。

### 10.3 Task 仓库范围

必须能够表达：

- Task 关联的 Repository；
- Task 在各 Repository 上的操作限制。

不得默认授权 Project 下所有仓库。

### 10.4 Run 快照

必须保存不可变授权快照及内容哈希。

`task_run.policy_version INT` 继续保持现有语义。

Git 授权快照的哈希不能写入该字段。

### 10.5 审计

授权授予、修改、撤销和 Run 快照生成均应可追溯。

优先复用现有审计基础设施。

---

## 11. 验收矩阵

> ⚠️ **映射调整见 §0.0.6**：以下 24 条为 V1.0 原案；V1.1 实施时按 §0.0.6
> 的「保留 / 改写 / 合并」处置执行，**合并后的验收项以 §0.0.6 为准**。

| 编号 | 验收项 | 预期结果 |
|---|---|---|
| I3-001 | Agent 自行申请更高 Git 权限 | 拒绝 |
| I3-002 | 调用人无授权 | 拒绝 |
| I3-003 | Project/Repository 策略拒绝 | 拒绝 |
| I3-004 | Task 未关联 Repository | 拒绝 |
| I3-005 | Repository 已断开或不可用 | 拒绝 |
| I3-006 | 四层授权全部允许 | 生成有效快照 |
| I3-007 | Modify 未授权 | 本地修改被阻止 |
| I3-008 | Commit 未授权 | Commit 被阻止 |
| I3-009 | Push 未授权 | 远程推送被阻止 |
| I3-010 | Merge 未授权 | Merge 被阻止 |
| I3-011 | auto\_merge=true 但无 Git Merge 权限 | 拒绝 |
| I3-012 | require\_human\_approval=false 但无 Git 权限 | 拒绝 |
| I3-013 | Task 只关联 backend | 不能访问 frontend |
| I3-014 | Project 多 Repository 策略不同 | 分别执行 |
| I3-015 | Run 快照创建后策略变更 | 历史快照不被覆盖 |
| I3-016 | Run 快照创建后显式撤权 | 后续相关操作被拒绝 |
| I3-017 | Run 取消后 Git 操作 | 拒绝 |
| I3-018 | Project 归档后 Git 写操作 | 拒绝 |
| I3-019 | Git 操作绕过 MCP | 仍受执行环境约束 |
| I3-020 | 本地存在远程 Git 凭据 | 无 Push 授权仍不能推送 |
| I3-021 | policy\_version INT | 保持现有语义 |
| I3-022 | Git 快照哈希 | 独立保存，不写入 INT |
| I3-023 | 授权撤销与操作并发 | 符合明确线性化规则 |
| I3-024 | 历史 Run 查询 | 可还原当时授权依据 |

---

## 12. 实施任务拆分建议

### I3-A：现有权限链路核验

范围：

- 核验 `write_access` 完整读取链路；
- 核验授权数据来源；
- 核验现有权限校验入口；
- 核验 Repository 与 Task 关联关系；
- 核验执行器实际 Git 命令路径；
- 核验 Codex 是否可绕过 MCP 直接执行 Git。

交付：

Git 权限现状矩阵和安全边界说明。

### I3-B：Git 操作能力模型

范围：

- 定义 read/modify/commit/push/create\_pr/merge；
- 明确现有授权字段映射；
- 明确授权主体；
- 明确 Project/Repository 策略表达。

交付：

Git 权限模型与数据库迁移设计。

### I3-C：Task 仓库范围

范围：

- Task 多 Repository 关联；
- 每个 Repository 的操作限制；
- Repository 归属检查；
- Task 与 Project 的一致性检查。

交付：

Task 仓库范围授权模型。

### I3-D：Run 授权快照

范围：

- 快照结构；
- SHA-256 内容寻址；
- `policy_version INT` 兼容方案；
- 不可变快照；
- 历史查询。

交付：

Run Git 授权快照协议。

### I3-E：执行期强校验

范围：

- Git 操作前检查；
- MCP 执行入口；
- Codex 本地 Git 执行约束；
- Push 凭据隔离；
- 显式撤权即时生效。

交付：

可强制执行的 Git 权限边界。

### I3-F：审计与回归

范围：

- 授权授予；
- 策略变更；
- 授权撤销；
- Run 快照生成；
- 直接 Git 命令绕过测试；
- 归档与取消联动测试。

交付：

Git 授权全链路验收报告。

---

## 13. 最终设计裁决

> ⚠️ **已被 §0.0 简化裁决取代**（第 1~4、10、16 条及六能力表述作废；
> 第 5~9、11~15 条中与简化模型不冲突者保留）。以下为 V1.0 原裁决，留档。

1. Git 权限采用四层授权模型：调用人授权、Project/Repository 策略、Task 仓库范围、Run 执行快照。
2. 四层权限采用交集，不允许任一层提升其他层权限。
3. Git 操作能力拆分为 read、modify、commit、push、create\_pr、merge。
4. 默认拒绝写操作。
5. Task 必须明确声明使用哪些 Repository。
6. 不得默认把 Project 下全部 Repository 授权给每个 Task。
7. Agent 不得自行授予或扩大 Git 权限。
8. auto\_\* 字段不表达 Git 授权。
9. Run 快照不可变。
10. 显式撤权必须阻止后续相关 Git 操作。
11. `task_run.policy_version INT` 保持现有含义。
12. Git 授权快照哈希独立保存。
13. MCP 校验不能代替执行环境对直接 Git 命令的约束。
14. Push 凭据不得直接暴露给 Agent。
15. Project 归档和 Run 取消必须立即阻断后续 Git 操作。
16. 授权失败一律默认拒绝。

**最终结论：I3 的授权模型、权限合成、Run 快照、撤权原则和安全验收标准至此定稿。实施前必须完成实际 Git 命令执行路径和 `write_access` 完整链路核验，确保权限约束不只是数据库和 MCP 层面的逻辑约束，而是真正覆盖 Codex 执行环境。**
