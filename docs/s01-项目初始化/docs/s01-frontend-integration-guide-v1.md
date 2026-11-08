# S01 前端 ↔ 后端联调说明 V1.0

> **文档编号**：S01-FE-BE-02
> **配套**：`docs/s01-全局主菜单与信息架构设计基线/docs/s01-backend-api-requirements-v1.md`（后端需求清单 V1.0）
> **前端分支**：`feat/s01-frontend-design`
> **前端提交**：`ff68bf2`（契约层）→ `1a3e281`（类型 + API）→ `b748ac1`（页面 + i18n + 测试）

---

## 1. 前端已完成的交付

| 层 | 交付物 | 位置 |
|---|---|---|
| 类型 | 四切片 DTO | `frontend/src/api/types/initialization.ts` |
| API | 端点封装（逐条对齐需求清单） | `frontend/src/api/initialization.ts` |
| 页面 | 4 个视图 | `frontend/src/shell/views/s01/**` |
| 路由 | 4 条静态路由 | `frontend/src/shell/router/routes.ts` |
| i18n | `s01.initialization.*`（zh-CN + en） | `frontend/locales/{zh-CN,en}/s01/initialization.yaml` |
| 测试 | 19 条 vitest | 4 个 `*.test.ts` |

**验收状态**：`vue-tsc` 0 error · `eslint` 0 warning · `vitest` 96 文件 / 561 例全绿 · PRISM token 门禁通过 · i18n 双语 1431 key 对齐。

---

## 2. 路由一览（联调时直接访问）

| 路由 | 页面 | 依赖的后端端点 |
|---|---|---|
| `/projects/new` | 项目创建 | B2-1 · B2-2 · B2-3 |
| `/projects/:projectId/repositories-v2` | 仓库管理 | B3-1 · B3-2 · B3-3 |
| `/projects/:projectId/cognition` | AI-PM 认知 | B4-1 ~ B4-5 |
| `/projects/:projectId/baseline` | 基线确认 | B1-1 ~ B1-4 |

> 路由挂在 `/`（AppShell）下，静态注册，不依赖 DB 菜单 publish。

---

## 3. 端点 ↔ 页面映射（后端实现时的验收锚点）

### 3.1 S01-02 项目创建 → `ProjectCreationView.vue`

| 前端调用 | 期望响应 | 页面行为 |
|---|---|---|
| `GET /v1/pm/projects/suggest-code?name=` | `{code, available, conflict_code}` | 名称 debounce 300ms 触发；`available=false` 时禁用提交并提示冲突 |
| `POST /v1/pm/projects` | `{id, code, name, ..., init_status}` | 提交前二次确认弹窗；请求带 `idempotency_key`；成功后显示「进入初始化工作空间」按钮 |
| `GET /v1/pm/projects/{id}/initialization` | `InitializationState` | **尚未接入页面**（见 §5） |
| `GET /v1/pm/projects/{id}/profile` | `ProjectProfile` | **尚未接入页面**（见 §5） |

**页面强依赖的响应字段**：
- `create` 响应必须含 `id`（前端用它跳转到 `/projects/{id}/initialization`）。
- `suggest-code` 的 `code` 必须**非空**且 `available` 必须是布尔；`available=false` 时前端会锁死提交。

### 3.2 S01-03 仓库管理 → `RepositoryManagementView.vue`

| 前端调用 | 期望响应 | 页面行为 |
|---|---|---|
| `GET /v1/pm/projects/{id}/repositories` | `{items: RepositoryV2[]}` | 卡片列表；展示 role / connection_status / verification_status 三枚 Badge |
| `GET /v1/pm/projects/{id}/git-providers` | `GitProviderAuthorization[]` | 授权区；**只渲染 `scopes` / `status`** |
| `POST /v1/pm/repositories/{rid}/verify` | `RepositoryVerificationOutcome` | `head_changed=true` 时弹「仓库已有新提交」警告，**不自动改 `selected_commit`** |
| `POST /v1/pm/repositories/{rid}/set-primary` | `RepositoryV2` | 原 Primary 自动降级 |
| `POST /v1/pm/repositories/{rid}/update-version` | `RepositoryV2` | 人工确认后写 `selected_branch` + `selected_commit` |
| `POST /v1/pm/repositories/{rid}/disconnect` | `RepositoryV2` | 逻辑断开；列表仍显示该仓库 |
| `POST /v1/pm/repositories/{rid}/restore` | `RepositoryV2` | 恢复 |

**页面强依赖的响应字段**：
- `verify` 响应**必须含 `head_changed`**（布尔）。缺失则前端无法提示 HEAD 变化。
- `verify.checks[]` 的 `code` 取值须为：`repository_identity` / `access_permission` / `branch_availability` / `commit_availability` / `repository_metadata`。
- `role` 取值须为：`primary` / `supporting` / `infrastructure` / `test`（**不是**现库的 `main/docs/service/infra`）。
- `connection_status`：`pending` / `connected` / `failed` / `disconnected`。
- `verification_status`：`pending` / `passed` / `warning` / `failed`。

### 3.3 S01-04 AI-PM 认知 → `CognitionView.vue`

| 前端调用 | 期望响应 | 页面行为 |
|---|---|---|
| `GET /v1/pm/projects/{id}/cognition/preparation` | `CognitionPreparation` | 准备区；`repositories` 为空时提示「没有仓库也可以进行基础认知分析」 |
| `GET .../cognition/runs` | `{items: CognitionRun[]}` | 执行列表；展示 `item_count` 与 `confirmed_count` |
| `GET .../cognition/items` | `{items: CognitionItem[]}` | 逐条审核；支持 `status` / `domain` 筛选 |
| `PATCH .../cognition/items/{id}` | `CognitionItem` | 逐项处理；**无批量端点**（前端逐条提交） |
| `POST .../cognition/runs` | `CognitionRun` | 人工启动；启动前弹窗复述范围/仓库/资料/缺失 |
| `GET .../knowledge` | `{items: ProjectKnowledge[]}` | 正式知识列表；`is_core` 标记 |
| `PATCH .../knowledge/{id}/core` | `ProjectKnowledge` | 设/取消核心知识 |
| `GET .../agent-context` | `AgentContext` | 四层上下文（base / task / repository / execution） |

**页面强依赖的响应字段**：
- `CognitionItem.status`：`proposed` / `confirmed` / `rejected` / `investigation_required` / `deferred` / `superseded`。
- `CognitionItem.domain`：6 个领域枚举（`project_identity` / `repository_understanding` / `domain_understanding` / `architecture_understanding` / `development_convention` / `project_evolution`）。
- `CognitionItem.evidence_type`：`code_fact` / `git_commit` / `doc_fact` / `ai_inference` —— 前端按此渲染不同 Badge，**事实与推断视觉必须可区分**。
- `confidence` 为 0~1 浮点（前端按 ×100 渲染百分比）。
- `has_conflict=true` 的条目**不提供批量勾选框**。

### 3.4 S01-01 基线确认 → `BaselineReviewView.vue`

| 前端调用 | 期望响应 | 页面行为 |
|---|---|---|
| `GET /v1/pm/projects/{id}/baseline` | `BaselineOverview` | 当前生效基线卡片，**始终可见** |
| `GET .../baseline/proposals` | `{items: BaselineProposal[]}` | 提案下拉选择 |
| `GET .../baseline/proposals/{pid}` | `BaselineProposal` | 四任务渲染主体 |
| `POST .../baseline/proposals/{pid}/confirm` | 确认结果 | 二次确认弹窗复述：提案版本 / 来源 Commit / 差异统计 / 未解决风险 / 将生成的版本 |
| `POST .../baseline/proposals/{pid}/risks/{rid}/decide` | `BaselineRisk` | 逐条裁决；P0 必填意见 |
| `POST .../baseline/proposals/{pid}/revise` | `BaselineProposal` | 要求修订 → 新 Proposal Version |
| `POST .../baseline/proposals/{pid}/reject` | `BaselineProposal` | 拒绝，记录保留 |

**页面强依赖的响应字段**：
- `proposal.status`：`generating` / `pending_review` / `needs_revision` / `rejected` / `confirmed` / `expired` / `failed`。
  **非 `pending_review` 时前端禁用确认按钮**并显示对应原因。
- `proposal.diff[].diff_type`：`added` / `modified` / `removed` / `unchanged`。差异视图**默认只显示 changed**（不含 unchanged）。
- `proposal.risks[]`：`risk_level`（`P0`/`P1`/`P2`）、`ai_status`、`human_decision`。
  **`human_decision` 为空且 `risk_level=P0` → 前端禁用确认按钮并提示**。
  后端也必须独立校验（前端禁用只是提示，不是安全边界）。
- `proposal.sources[].source_type`：`code_fact` / `git_commit` / `doc_fact` / `ai_inference`。

---

## 4. 错误码约定（前端按字符串匹配）

前端统一处理以下 `error` 字段值（来自 `{error: "..."}` 响应体）：

| error | 前端表现 |
|---|---|
| `baseline_conflict` | 「基线已变更，请刷新后重新审核」 |
| `proposal_version_conflict` | 「提案已修订，请刷新后重新审核」 |
| `baseline_expired` | 「仓库基准已过期，请重新核验」 |
| `unresolved_p0_risk` | 「存在未裁决的 P0 风险」+ 刷新提案 |
| `project_code_conflict` | 「该标识已被占用」 |
| `idempotency_conflict` | 「该创建请求已提交过」 |

**关键约束**：确认失败 / 冲突 / 过期 / 权限不足时，**响应不得带「已生效」语义**（前端据 HTTP 状态 + `error` 判定，200 + 成功体才会显示「基线已生效」）。

---

## 5. 尚未接入页面的能力（后端可先行实现）

| 能力 | 端点 | 状态 |
|---|---|---|
| 初始化工作空间（状态聚合 + 状态更新） | `GET/PATCH /v1/pm/projects/{id}/initialization` | API 层已封装，**页面未做**（等后端契约确认后再补） |
| 项目背景信息读写 | `GET/PATCH /v1/pm/projects/{id}/profile` | API 层已封装，**页面未做** |
| 基线历史版本与确认记录 | `GET /v1/pm/projects/{id}/baseline/history[/{bid}]` | API 层已封装，**页面未做** |
| 认知变更提案专项视图 | `GET /v1/pm/projects/{id}/knowledge/{id}/history` | API 层已封装，**页面未做** |

> 这四项**不阻塞联调**——后端实现后前端补视图即可，契约已定。

---

## 6. 联调顺序建议

| 批次 | 后端交付 | 前端动作 | 可验收现象 |
|---|---|---|---|
| B1 | B2-1 + B2-2 | 联调创建向导 | 输入名称 → 自动出 Key → 创建 → 跳初始化工作空间 |
| B2 | B3-1 + B3-2 + B3-3 | 联调仓库管理 | 多仓库卡片 → 设 Primary → 核验看到 `head_changed` → 断开/恢复 |
| B3 | B4-1 ~ B4-5 | 联调认知 | 准备区 → 启动 Run → 逐条审核 → 知识设核心 → Context 四层 |
| B4 | B1-1 ~ B1-4 | 联调基线确认 | 看提案 → 审差异 → 裁决 P0 → 二次确认 → 基线生效 |

**每批次前端联调前先跑**：
```bash
cd frontend && npx vue-tsc --noEmit --skipLibCheck && npx vitest run
```

---

## 7. 前端侧已知取舍（供后端理解）

1. **仓库页是新路由 `/repositories-v2`**，未改动既有 `/repositories`（老页仍走老 API）。后端 v2 端点就绪后，老页可下线或重定向。
2. **`selected_commit` 与 `head_commit_sha` 是两个独立字段**，前端不会用后者覆盖前者——这是 UX §6.2「只提示、由人类确认」的实现方式。
3. **`ai_status` 与 `human_decision` 是两个独立字段**，前端分别渲染成不同 Badge，不合并。风险裁决写入的是 `human_decision`。
4. **认知批量确认走的是单条端点**（前端循环调用），不是后端批量接口。
5. **前端 `idempotency_key` 由 `crypto.randomUUID()` 在页面挂载时生成**，同一次创建重试复用同一 key；用户重新进入页面才生成新 key。

---

**文档状态**：V1.0，随前端实现同步更新。
