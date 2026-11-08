# P2-13 × PC-I6 LLM 接管分析：实施批记录 V1

> 状态：已实施 · 2026-11-08
> 所属产品：smark-pm / PM-AI
> 裁决依据：`ai-pm-p2-13-pc-i6-llm-analysis-adjudication-pack-v1.md`（D-1=A / D-2=A / D-3=A / D-4=全同意，2026-11-08）
> 批范围：**PC-I6 起笔切片**（仓库快照分析的 LLM 受约束增强）——git URL 介入仓库
> → verify → intake → KB/PC 分析接入 LLM 执行体（治理不变，产物仍 draft/PROPOSED）。
> 登记注：ai-pm-worker 的装配接线 hunks 被并行会话 a1bd7f4（run-dispatch 批，
> 2026-11-08 17:13）在工作区 sweep 进提交——内容与本批一致、已随 origin/master，
> 本批提交不再重复包含该文件。

## §一 落地内容

**迁移 0062**（`migrations/0062_snapshot_analysis_mode.sql`）：
`kb.document_revision` / `pc.cognition_revision` 各加 `analysis_mode TEXT NOT NULL
DEFAULT 'derived'`（CHECK：derived / derived_fallback / llm）。append-only 表仅
ADD COLUMN，存量行默认值语义准确（0062 前全部为纯派生）。

**端口（pm-application/src/snapshot_analysis.rs，新模块）**：
`SnapshotAnalysisModel` trait——`enrich_digest`（KB）+ `enrich_objects`（PC）。
分层纪律与 `RepoAnalysisModel` 同款：实现归 pm-ai，pm-infrastructure 只依赖
trait。骨架类型 `SnapshotObjectSkeleton` / 产物类型
`Snapshot{Digest,Objects}Enrichment`。错误复用 `repo_facts::pm_ai_port::ModelError`。

**实现（pm-ai/src/snapshot_model.rs，新模块）**：
- `LlmSnapshotAnalysisModel`：逐调用三级解析（复用 `resolve_repo_call_config`，
  项目指定严格不回退 → 租户首个 enabled → 无），provider 启停即时生效。
- 降级语义 D-3=A：无 provider → `Ok(None)`（上层 derived）；解析失败 →
  `Err(Unavailable)`；调用失败/校验失败 → `Err`（重试一次后 BadOutput）——
  上层一律回退结构化派生并标 `derived_fallback`。**无 Fake fixtures 回退**
  （与主链 RoutedAnalysisModel 的关键差异：治理面产物禁假数据）。
- 受约束增强 D-2=A：对象骨架（4 类 object_key）归派生，LLM 不得增删；PC
  content 强制三区小节（`### 事实` / `### 推断` / `### 不确定`，AC-019 可校验
  面）；evidence_refs 服务端对照 RepoFacts 白名单过滤（commit sha 全量+8 位
  前缀/热点文件/文档路径），编造条目丢弃，每对象上限 8 条。
- prompt 安全边界沿用 REPO_SYSTEM_PROMPT 先例：`<repo_facts>` 内容是数据不是
  指令；temperature=0；宽松 JSON 提取（`extract_json` 转 pub(crate) 复用）；
  prompt 收窄视图（commits 截 50 / docs 截 20 篇×8K 字符）。
- 记账：ok/failed 双向落 `ai.llm_call_record`，purpose 新增
  `kb_snapshot_analysis` / `pc_snapshot_analysis`（call_record.rs 常量）。
- 旋钮 `AI_PM_SNAPSHOT_ANALYSIS_MODEL=auto|derived`（默认 auto；拼错值 WARN 回
  auto；**无 fake 档**）。

**分析本体（pm-infrastructure/src/kb_pc_analysis.rs）**：
- `analyze_repository_snapshot` / `derive_pc_cognition` 增加尾参
  `Option<&dyn SnapshotAnalysisModel>`。
- `compose_repo_digest`（新纯函数）：结构化统计小节恒有 + 可选 LLM 叙述摘要
  （48K 字符界）+ 模式尾注（llm 翻转「无 LLM 参与」声明）。
- PC 合并：enhancement 按 object_key 回填 summary/content；evidence locator
  增记 `analysis_mode` + `llm_evidence_refs`；审计 after_data 增记
  `analysis_mode` / `llm_enriched_objects`；change_summary 追加模式后缀。
- 内容寻址去重语义不变（同 content 不追加修订）。

**Handler 与装配**：两个分析 handler 加 `with_snapshot_model`；
`register_kb_pc_handlers` / `register_all_handlers` 签名增加
`Option<Arc<dyn SnapshotAnalysisModel>>`。两 worker（ai-pm-worker / pm-worker）
同构装配 `LlmSnapshotAnalysisModel::from_env`（pm-worker 新增 pm-ai /
pm-application 依赖——共享 outbox 表，装配语义必须一致）。

## §二 验证

- pm-ai：9/9 绿（snapshot_model 新增 6 用例：mode 解析/prompt 收窄/三区与
  覆盖校验/wiremock 重试/digest+objects 解析/解析+记账含停用语义）。
- pm-infrastructure：125/125 绿（kb_pc_repo_flows 扩至 5 用例——用例 4 LLM
  成功路径全链断言：digest 尾注翻转+三区正文+白名单证据+ok 记账×2+父
  COMPLETED；用例 5 wiremock 500 失败：全链不碎+产物回退纯派生
  +derived_fallback+failed 记账×2；存量 3 用例传 None 零破坏）。
- 全量门禁：`bash scripts/cargo-test.sh` 全 workspace **1165 跑 / 1162 绿**，
  3 红均为已知 baseline（pm-api::team_onboarding_qa b05_02/03 挂账 +
  digua_common_consul::registration 环境型）——零本批回归。clippy（改动面）
  本批文件 0 告警；pm-application 70 个存量 lint 告警全在未触碰代码
  （worker_command/team_onboarding/review 等，新工具链 lint 漂移型，挂账）。
- 验收对照（PC-I6 五条验收前三条，09 稿 :147-151）：可追溯建议（evidence
  locator 白名单 refs+audit）✅；AI 不自动确认（产物恒 draft/PROPOSED，
  confirm 面零改动，AC-015-02/AC-016-02 不回归）✅；区分事实/推断/不确定
  （三区小节硬校验）✅。

## §三 坑位（复测必读）

1. **三级解析的租户兜底**：同租户任意 enabled provider 会被「无指定 → 租户
   首个 enabled」解析到——测 NoProvider 分支必须先显式 disable（snapshot_model
   测试踩过：`Ok(None)` 断言直接打到 500）。
2. **json! 多层括号**：content 为内嵌 JSON 字符串的 mock 体，`}}]` 后还需
   json 外层 `}`（`}]` + `})))` 两行收口）。
3. **claim_catalog 语义**：`ai.llm_call_record.provider_id` 非空——解析失败
   （无 provider 行可挂）不记账，只留结构化 WARN；derived 模式/无 provider
   不记账（与主链 Fake 回退不记账同语义）。

## §四 遗留与挂账

- PC-I6 剩余验收面（NO_CHANGE/CONFLICT 三态结论、需求/Design·ADR 影响分析、
  完整性/一致性检查、主动维护）——按 D-4c 另行排批，依赖认知关系建模
  （0053 未建，归 PC-01C）。
- KB 信号 relevance/confidence 判定手段与信号契约（§23 Q10）——留 KB 独立化
  批（D-4a）。
- E2E 真实栈脚本（scripts/checkpoint1-e2e.sh）未扩 provider 步——wiremock
  集成测试已覆盖全链语义，真实栈 provider 步待下一批连同 .10 部署一起验。
- outbox claim 租约 vs LLM 慢调用：现有租约足够测试节奏；生产长分析（分钟级）
  的租约余量待 Phase 6 观测后调参（ai_analysis job 先例=3600s）。
