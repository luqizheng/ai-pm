# 02 — 第二批修复（R5）：B02-01 答复入口 + B05-01 任务批契约字段

- 日期：2026-11-24
- 基线：v1.0-remediation 第一批（R1–R4，CONDITIONAL PASS）之后的遗留 P2
- 性质：代码修复 + 自动化回归；部署态复验见 §4

## 1. 修复范围

| 缺陷 | 修复 | 测试 |
|---|---|---|
| **B02-01**（P2）发现页「问题答复入口尚未在页面开放」 | `DiscoveryView.vue` 为每个 open 问题内联答复表单（Textarea+提交钮），直连既有命令 `answerDiscoveryQuestion`；i18n 双语言新增 answerPlaceholder/submitAnswer/answerFailed，answerNote 改为「答复由下一轮编排消费」 | `DiscoveryView.test.ts` +2 用例（提交调真实命令并清空草稿；空白禁提交、已答不给入口）9/9 绿 |
| **B05-01**（P2）AI 任务批 goal/deliverables 空 → S04 就绪 A 层全 ⚠ | ①`RawTask`/`ValidatedTask` 增 goal/deliverables；②分解 prompt JSON 样例要求产出；③`CreateTaskRequest` 契约扩展 goal/deliverables，`create_task` INSERT 同事务落库（deliverables 经 normalize 去重）；④写回映射带诚实回退（goal←description→title；deliverables←验收口径） | `discovery_writeback.rs` 夹具补字段 + 新断言（3 任务落库后 goal/deliverables 全非空、模型给定值不被回退覆盖）9/9 绿 |
| B03-03/B08-01（P2）访谈不收敛 | **核对后无需新代码**：第一批已在 DiscoveryView 提供 NEEDS_USER_INPUT 完结 / REVIEW_REQUIRED 退回+完结按钮（含测试）——人工可控收敛路径在位 | 既有 4 用例覆盖 |

## 2. 门禁

| 门禁 | 结果 |
|---|---|
| pm-ai `discovery_writeback` + `writeback_sweep` | 9+3 全绿 |
| pm-application 8 套件（task_read/task_ready_batch/task_revision/work_item/design_s03/task_dependency/task_product_acceptance/task_traceability） | 59 全绿 |
| 前端 `DiscoveryView.test.ts` | 9/9 绿 |
| `check:i18n` | zh-CN/en 6450 key 全对齐 |
| `vue-tsc --noEmit` | 干净 |
| `pnpm build`（production，release-guard 通过） | 2.32s 构建成功 |

**fmt 口径（用户裁决后更新）**：全仓 624 个 `.rs` 以 **CI 同配置**（`rustfmt --edition 2021 --config skip_children=true,reorder_modules=false`，即 pa08-registry-portability.yml 门禁原命令）全量格式化，165 文件归一；`--check` 全树干净，HEAD 存量漂移（约 85 文件，含本机 1.11 stable 默认口径与 CI 口径的历史差异）一并消除。fmt 后关键套件复跑全绿。

## 3. 明确不改的契约

- **B03-01 R1 契约保持**：本批曾实现「REVIEW_REQUIRED 续接自动回退 IN_PROGRESS（R2）」并通过 15/15 路由测试，因与第一批已运行态复验的 R1 横幅+人工按钮契约冲突而**撤销**，维持 R1。

## 4. 部署态复验（2026-11-24 15:40 实测）

| 项 | 结果 |
|---|---|
| pm-api / ai-pm-worker 镜像重建并滚动 | ✅（BuildKit 依赖层缓存命中，秒级重建）；ai-frontend-web `--no-cache` 重建后 bundle 含新代码（index-Bp01bco1.js 含 answerFailed key） |
| **B02-01 答复入口** | ✅ DiscoveryView 渲染 3 个 open 问题答复表单（dc-answer-form-*）；经表单提交真实答复 → 页面出现答案文本、未答 3→2、无错误；截图 `evidence/b2-answer-submitted.png` |
| B03-03 完结/退回按钮 | ✅ NEEDS_USER_INPUT 会话显示「复核通过并完结」，REVIEW_REQUIRED 显示退回+完结（第一批已交付，本批核对） |
| B05-01 落库断言 | 留待下一轮真实「需求→批准→分解」业务流（REQ-TASKFLOW-001 v1 已挂任务幂等跳过；v2 变更草案批准属真人工决策，不代行） |

### 运维动作（用户裁决，非业务伪造）

1. **leo 密码重置**：原口令失效（用户确认被改）。经 argon2id 生成器（m=19456,t=2,p=1 与原哈希参数一致）产出新 PHC 直写 `public.iam_users.password_hash`，`password_expired_at=NULL`，`verify()` 往返校验通过；新口令=仓库 e2e 标准 `Checkpoint@123456`。
2. **iam-service 重启**：写库后浏览器登录 200 但 `/users/me` 401 `unknown kid`（iam 的 JWKS 缓存不含 token-service 现用键）——重启刷新缓存后 login+users/me 全 200。
