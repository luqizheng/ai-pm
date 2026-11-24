# 01 — 修复回归验收结果

- 日期：2026-11-24
- 被验收代码：`remediation/integration`（= master `3a50c7cd` + 四修复合并 + 回归期修复 `a2cd87e7`）
- 部署态：full-dev 27 容器；**pm-api / ai-pm-worker / ai-frontend-web 三镜像自集成分支重建**（迁移 0059 已应用；worker 双循环 On）
- 操作者：leo（OWNER，用户提供的真实账号经产品 UI 登录执行）

## 1. 自动化门禁（全部通过）

| 门禁 | 结果 |
|---|---|
| 后端全量（pm-api/pm-application/pm-infrastructure/pm-mcp/pm-ai，`--skip q10`） | **215 个测试二进制 / 1832 passed / 0 failed**（cargo exit 0） |
| q10 系列单独跑（AGENTS 规则 6） | 18/18 绿（14+2+2） |
| 前端全量 vitest | 230 文件 / 1386 用例全绿（R4 分支口径） |
| vue-tsc --noEmit | 干净 |
| check:i18n | zh-CN/en 6447 key 全对齐（含回归期修复后的 catalog 块） |
| 前端发布构建 | `pnpm build`（production，release-guard 通过） |

## 2. 业务流运行态重验（leo 真实登录，2026-11-24 14:00–14:30）

证据：`evidence/`（8 张截图 + 1 份 AI 回复全文）。

| # | 场景 | 结果 | 证据 |
|---|---|---|---|
| 1 | **R1 死信反转**：会话置 REVIEW_REQUIRED（产品 API 两跳，NUI→IP→RR 合法边）→ chat 发消息 → **横幅如实渲染**（原因+「退回继续访谈」「复核通过并完结」双按钮）→ **110s 无消费**（worker 日志同步证实零编排轮）→ 点「退回继续访谈」→ 状态 IN_PROGRESS → **编排器下一轮即消费**（AI 回复引用 REQ-TASKFLOW-001 并提澄清问题）→ 随后编排器升级 NEEDS_USER_INPUT，横幅条件渲染正确（只给完结按钮） | **通过** | r1-review-required-banner.png、r1-resumed-consumed.png |
| 2 | **R2 项目记忆**：新会话发盘点问题（避开路由关键词措辞）→ 回复在 chat 段（未被路由吞）→ **引用真实记录**：REQ-TASKFLOW-001·v1·approved、TD-001·v3·approved、ADR-001·accepted、ADR-002 阻塞标记、任务计数 9；缺失类别（里程碑/缺陷/发布记录）如实说明「无法盘点」 | **通过**（与修复前同项目同问题「没有可确认的已完成事项」直接对照） | r2-inventory-reply.png/.txt |
| 3 | **R3 就绪探针与引导**：① 就绪态（目录齐备）→ 无横幅；② 经产品 API 暂删 general 策略 → `ai-readiness` ready=false 且 `general_policy_set:false` → 刷新后**横幅出现**：原因行（尚未配置 general 模型策略）+「配置前其他项目功能不受影响」+ 设置入口链接 → 恢复策略 → ready=true | **通过** | r3-not-ready-banner.png |
| 4 | **R3 目录 UI**：/settings/team/ai-providers 渲染目录区（deepseek-chat 行、授权态=撤销钮、策略区、general 缺失警告条件正确） | **通过**（发现 i18n 嵌套缺陷→当场修复→重部署后翻译正常） | r3-model-catalog-ui.png |
| 5 | **R4 Design 全流程 UI**：新建设计（TD-002）→ 跳详情 → 关联需求 REQ-TASKFLOW-001 v1（**后端正确拒绝未批准需求**「requirement has no current revision to link」，UI 如实报错——闸门运行态有效）→ 新修订 v2 → 提交（待确认）→ 请求审批 → **leo 在审批面板人工批准**（理由+SC-06 双令牌）→ v2 已批准 | **通过** | r4-design-approved.png |
| 6 | **R4 ADR 全流程 UI**：新建 ADR-003（决策问题必填）→ 提交（评审中）→ 请求人工确认 → **leo 人工接受**（发现并修复面板匹配缺陷，见 §3）→ 已接受 | **通过** | r4-readiness-gate.png（右上时序可见） |
| 7 | **就绪门禁**：未就绪 + 逐条件如实（设计已批准/需求依据已确认/关键决策已解决=通过；实施范围已明确/验收标准已齐备=未通过，逐修订给出 content.implementation_scope missing 等阻断原因）；gate 标签翻译完整（P3 B04-03 同区缺口已修） | **通过**（诚实呈现） | r4-readiness-gate.png |
| 8 | 需求梳理/访谈编排（后台行为）：编排器多轮真实运行（findings/questions 增长、answers_consumed、NEEDS_USER_INPUT 升级、overreach 拦截） | **通过**（worker 日志） | — |
| 9 | 项目创建 UI、知识沉淀 KB、任务规划、变更影响分析 | **未重操作**（链路代码零改动；自动化回归全绿；原始验收相应阶段结论未被本批修复触碰） | — |
| 10 | 工程执行 / 交付审查 | **未验收**（无真实 Repository/Runner/MCP/Codex——同原始验收口径，不伪造） | — |

## 3. 回归期发现并修复的缺陷（均已运行态复验）

| 缺陷 | 修复 | 复验 |
|---|---|---|
| catalog i18n 块嵌套错误（settings.team.catalog ≠ settings.team.ai.catalog）→ 目录全部文案回显原始 key | 程序化重写块至正确路径（42 key × 2 语言） | 重部署后 titleOk=true、key 回显清零（r3-model-catalog-ui.png） |
| ADR 确认面板过滤条件错（target_id===修订 id；实际 adr_acceptance 的 target_id=ADR id）→ 面板永不渲染 | 改按 `item.adr_id === adrId` 匹配（单测 15/15 绿） | ADR-003 人工接受成功（决策状态=已接受） |

## 4. 部署态核验

| 项 | 结果 |
|---|---|
| 迁移 0059 | 已应用（`_sqlx_migrations` version=59）✓ |
| 前端 bundle | index-DcPZ-XIE.js（含全部修复；强刷生效——nginx 对 index.html 的缓存需注意）✓ |
| worker 开关 | REPLY=on / DISCOVERY_ORCHESTRATOR=on / RUN_DISPATCH=off ✓ |
| 部署态数据 | 回归中的临时改动（策略删/恢复、会话状态跃迁）均已恢复或为真实业务对象 ✓ |
