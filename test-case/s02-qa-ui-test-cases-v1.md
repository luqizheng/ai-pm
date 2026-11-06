# S02 QA · UI 验收测试用例 V1.1（执行完毕回填版）

> **承接**：S02 发布验收（`s02-qa-execution-report-v1.md`，PASS WITH CONDITIONS）的 UI 面补充测试。
> **执行载体**：Playwright 1.65.0（`@playwright/test`，workspace root 依赖）+ 系统 MS Edge（channel，免浏览器下载）+ vite dev server :8848；视口 1440×900（设计基线档位）。
> **脚本**：`ai-frontend/playwright.config.ts` + `ai-frontend/e2e/s02-ui.spec.ts`；证据 `test-case/evidence/s02-ui/playwright-report/`（HTML 报告 520K）。
> **认证策略**：路由守卫仅判凭据存在性（`router/index.ts` beforeEach → `prism_*` localStorage 键）；用 `addInitScript` 注入测试凭据，未登录用例用全新 context。
> **执行结果**：**14/14 PASS**（26s，2026-11-06）。

## 0. 执行前 QA 判定口径（关键发现）

| 发现 | 事实 | 判定影响 |
|---|---|---|
| **UI↔API 集成缺口（NI-4）** | S02 全部 6 页（requirements/requirement-detail/requirement-history/requirement-review/requirement-baseline/requirement-impact）数据源为**本地 fixtures**（`RequirementsView.vue:52-54` 等），状态迁移在页内本地完成，未接真实 pm-api（后端 S02 API 已验收 PASS） | 本套件判定口径 = 「交互闭环与状态机演示态」；**NI-4 登记为范围发现**（前端接入真实 API 归 S02-UI 后续切片），不判后端缺陷 |
| 状态机 UI 面 | `canEdit`=draft / `canPropose`=draft\|confirmed / `canConfirm`=pending_confirmation / `canResolve`=conflict（`RequirementDetail.vue:177-184`） | U-07 实测证明 pending 状态正确隐藏 propose 按钮——UI 状态机与后端语义一致 |
| 数据锚点 | fixtures 四状态齐全：req-001=pending / req-002=confirmed / req-003=draft / req-004=conflict | 断言基于 fixtures 已知值 |

## 1. 用例明细（14 条，全部回填）

| ID | 场景 | 预期 | 实际 | 判定 |
|---|---|---|---|---|
| S02-QA-UI-01 | 工作区加载 | 壳+列表+摘要条渲染 fixtures | REQ-001/User authentication 可见，列表 aria 齐全 | PASS |
| S02-QA-UI-02 | 行选中→详情 | 详情面板显示所选 | 点击后详情显示所选需求 | PASS |
| S02-QA-UI-03 | 状态筛选 | 行集随筛选变化 | 自定义 Select（reka-ui）选「已确认」→REQ-001 消失；回「全部状态」恢复 | PASS |
| S02-QA-UI-04 | 视图密度切换 | 列表保持可见 | 切换后列表可见 | PASS |
| S02-QA-UI-05 | Confirm 取消路径 | 弹窗取消状态不变 | `.ui-confirm__panel` 出现，取消（footer 首位按钮）后弹窗关闭、确认按钮仍在（状态未变） | PASS |
| S02-QA-UI-06 | Confirm 确认路径 | 状态迁移为已确认 | 弹窗确认（footer 末位按钮）后「已确认」出现 | PASS |
| S02-QA-UI-07 | propose 链路 | 确认弹窗→修订抽屉 | **req-002（confirmed）**：「提交确认」→弹窗→确认→RevisionSheet 出现；pending 状态正确隐藏该按钮 | PASS |
| S02-QA-UI-08 | **AI 红线（UI 面）** | AI 动作不改需求状态 | AI 动作前后「待确认」计数不减——人工审批边界 UI 面成立 | PASS |
| S02-QA-UI-09 | 历史查看 | 抽屉+路由双验证 | 「查看历史」=页内 RevisionSheet 抽屉（不跳路由）；直访 `/project/requirements/req-001/history` 渲染正常 | PASS |
| S02-QA-UI-10 | 基线页 | 渲染正常 | `/project/requirements/baselines` main 可见 | PASS |
| S02-QA-UI-11 | 评审页 | 渲染正常 | `/project/requirements/review` main 可见 | PASS |
| S02-QA-UI-12 | 守卫重定向 | 无凭据→login | 未登录访问业务页重定向登录页 | PASS |
| S02-QA-UI-13 | 已登录直访 | 不拦截 | 注入凭据后直通 | PASS |
| S02-QA-UI-14 | 壳冒烟：侧栏折叠 | data-sidebar 属性切换 | 属性存在且可切换 | PASS |

## 2. 迭代记录（脚本修正，非产品缺陷）

| 轮次 | 结果 | 修正内容 |
|---|---|---|
| R1 | 7 过 7 失败 | 选择器基线建立：自定义 Select 非原生 select；ConfirmationDialog 无 dialog role（`role="presentation"`）改用 `.ui-confirm__panel`；footer 取消在前确认在后；i18n 实值（actionPropose=提交确认、actionHistory=查看历史）；main/body 歧义改 getByRole("main") |
| R2 | 12 过 2 失败 | 断言口径修正：「查看历史」=页内抽屉不路由跳转；「提交确认」先弹 proposeDialog 确认后才开 RevisionSheet |
| R3 | 13 过 1 失败 | U-07 状态机事实：pending 状态 propose 按钮正确隐藏（canPropose=draft\|confirmed）——改用 req-002（confirmed） |
| R4 | **14 过 0 失败** | — |

## 3. 结论

1. **UI 交互闭环 14/14 PASS**：fixtures 演示态下的列表/筛选/详情/显式确认弹窗/修订抽屉/子页导航/认证守卫全部符合设计意图。
2. **人工审批边界的 UI 面成立**：AI 动作只追加提案池不改状态（U-08）；确认类动作必经显式弹窗（U-05/06/07）；状态机按 status 正确显隐操作按钮（R3 发现）。
3. **核心范围发现 NI-4**：S02 六页未接真实 API（fixtures mock 先行）——UI↔后端集成验收待 S02-UI 接入切片后执行（沿用本套件骨架，替换数据锚点即可）。
4. **无 UI 层缺陷**；3 轮脚本修正均为测试侧选择器/口径问题，已沉淀为可复用的选择器基线（见 §2）。

## 4. 复现

```bash
cd ai-frontend
pnpm exec playwright test        # 自动拉起 vite dev（reuseExistingServer=true）
# 证据：../test-case/evidence/s02-ui/playwright-report/index.html
```