# R4 — Design / ADR 完整 UI（B04-02 修复）

- 日期：2026-11-24
- 分支：`remediation/r4-design-adr-ui`
- 对应缺陷：**B04-02（P1）** 技术设计与 ADR 的创建/修订/提交/审批/关联全链无 UI 入口（前端只读），普通用户无法在产品内建立设计与 ADR
- 基线：`00-baseline.md` §3 B04-02；领域模型与 API **全部复用**（未新建并行模型）

## 1. 根因

`api/designs.ts` 13 个函数全 GET（自述「写路径一律走后端命令，前端只调不改状态」——但命令封装也从未建立）；四个视图全部只读（TechnicalDesignDetailView 注释明示「本页只读」）。后端命令面完整（create/revision/submit/link/approvals/decide/supersede/deprecate/blocking/freeze），含 require_human 闸门与 SC-05 AI 禁批。

## 2. 修复方案（复用既有后端，零领域模型变更）

### API 层（api/designs.ts +16 写函数）

- Design：createDesign / createDesignRevision / updateDesignRevisionDraft / submitDesignRevision / link±unlinkDesignRequirement / requestDesignApproval
- ADR：createAdr / createAdrRevision / submitAdrRevision / linkAdrDesign / linkAdrRequirement / requestAdrApproval / supersedeAdr / deprecateAdr / setAdrImplementationBlocking
- 审批（人工）：decideDesignApproval（design_revision / adr_acceptance / baseline_freeze 同一端点；SC-06 双令牌可选透传）
- 返回形状与后端逐一核对（204 No Content / {approval} / {revision} / AdrDetail）

### 视图接线（四个既有视图，状态驱动 UI）

| 视图 | 新增能力 |
|---|---|
| TechnicalDesignsView | 「新建设计」表单（标题+首版 Markdown）→ 创建后跳详情 |
| TechnicalDesignDetailView | 动作条（新修订/编辑草稿/提交/请求审批，按选中修订状态渲染）；修订表单（每次编辑=新修订，旧批准版不动）；**人工审批面板**（pending 审批行存在时：批准/拒绝+理由，透传 SC-06 `expected_current_revision_id/expected_row_revision`）；需求关联卡（draft 修订可链/解链，需求选择器=项目需求列表） |
| AdrView | 「新建 ADR」表单（决策问题必填、阻塞标记可选） |
| AdrDetailView | 动作条（新修订/提交/请求确认/阻塞标记切换）；新修订表单（决策问题必填）；**人工确认面板**（pending adr_acceptance：接受/拒绝+理由）；关联设计（挂该设计最新修订）与需求（当前正式版）表单 |

### 业务规则落实（任务书 §七）

- 每次编辑 = 新 Design Revision（后端不可变触发器兜底）✅
- 关联仅 draft 修订可改（后端闸门同口径，UI 同步收口）✅
- 需求关联缺省绑当前正式版本（须 approved）✅
- **UI 不推断状态**：批准按钮只在 pending 审批行存在时出现；approved/ready 只读后端状态；AI 无审批入口（后端 require_human 403 兜底）✅
- 就绪门禁：既有 ImplementationReadinessView 只读展示（gate key `scope_defined`/`acceptance_ready` 翻译缺口顺带补齐——验收 P3 B04-03 同区）
- supersede/deprecate/blocking 的真人门命令已接（UI 完整流程中按需使用）

## 3. 变更清单

- `ai-frontend/src/api/designs.ts`：+16 写函数 + payload 接口
- `TechnicalDesignsView.vue` / `TechnicalDesignDetailView.vue` / `AdrView.vue` / `AdrDetailView.vue`：写入口接线
- `locales/{zh-CN,en}/s03/design.yaml`：+56 key（common.save/cancel、detail 18、designs 4、adrs 20、gate 2）
- 无后端变更、无新路由、无新依赖组件（复用 FormField/Input/Card/Button）

## 4. 测试

**前端（全绿）**
- TechnicalDesignsView 4 例（+2：新建设计真实命令+跳转、空标题禁提交）
- TechnicalDesignDetailView 7 例（+5：draft 新修订/draft 提交/已批准无入口（状态闸门）/pending 审批面板+SC-06 双令牌/draft 关联需求）
- AdrView 4 例（+2：新建 ADR 真实命令+跳转、决策问题缺失禁提交）
- AdrDetailView 4 例（+3：新修订命令/阻塞真人门/pending 接受真实裁决）
- 全量 vitest + vue-tsc + check:i18n（见回归记录）

**安全相关（后端闸门零改动，回归覆盖）**
- AI 禁批（SC-05）：后端 require_human 未动；design_s03.rs `sc01_ai_drafts_design_human_approves_ai_cannot_decide` 既有绿
- SC-06 乐观锁：decide 请求透传双令牌；后端既有测试绿

## 5. 验收标准对照（任务书 §七 R4）

Design UI 11 项：列表✅(既有)/创建✅/编辑(新修订+草稿)✅/修订历史✅(既有版本链)/逐版内容✅(既有)/关联多需求✅/查看关联✅(含 superseded 警示)/审批状态✅/提交审批✅/人工确认✅/就绪门禁展示✅(既有)
ADR UI 10 项：列表✅(既有)/创建✅/编辑草稿✅/历史✅(既有版本链)/关联需求✅/关联设计✅/状态✅(既有)/提交人工确认✅/人工接受或拒绝✅/决策结果✅(既有状态+审批行)
业务流程（TaskFlow 全流程）→ 运行态回归批次执行。
