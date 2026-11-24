# R2 — 项目记忆与正式记录读取（B09-01 / B03-02 修复）

- 日期：2026-11-24
- 分支：`remediation/r2-project-memory`
- 对应缺陷：**B09-01（P1）** chat AI 无项目业务记录上下文，总结与真实状态完全脱节（09-project-summary.md §3）；连带 **B03-02（P2）**（03-knowledge.md §3）
- 基线：`00-baseline.md` §3 B09-01

## 1. 根因

chat 注入面 =「最小两节」：S1 项目画像（2k）+ S2 仓库事实（4k，按锚点）+ 用户**手工附加**的对象节（2k，`ai.conversation_context_item`）。库内的需求/设计/ADR/任务/风险/待人工事项**永不自动进入** chat 上下文——「AI 项目负责人」只能当「会话咨询师」。

## 2. 修复方案

新增**项目正式记录投影**注入节（`project_records`），chat 每次回复前**实时**从数据库投影（无缓存、无持久化副本——不是「聊天摘要冒充项目记忆」）：

```
S1 project_profile   ← core.project（原有，2k）
S2 project_records   ← delivery/design/discovery/governance（新，6k）★
S3 repository_facts  ← integration.project_repository（原有，4k，按锚点）
（对象节 = 用户手工附加，2k，不变）
总预算 8,000 → 16,000（访谈 36,000 不动）
```

记录节内容（行级来源可定位）：
- **需求**：正式（approved，`code · vN · approved · 标题`）与草案/待确认（draft/pending_confirmation 等，显式标注「未经人工确认，不得当正式决策」）分组列出
- **技术设计**：当前修订状态（draft/pending_confirmation/approved…）
- **ADR**：状态（proposed…accepted）+ 阻塞实施标记
- **任务**：状态计数 + 最近 5 条
- **风险 / 阻塞**：未关闭项
- **待人工处理**：需求待确认/设计待审批/ADR 待审/访谈待人工 四项计数
- **最近记录变更**：审计投影最近 5 条（action · target · 时间）
- 空域显式「（无）」；头部语义声明「未列出的记录即不存在」

## 3. 关键设计约束（对照任务书 §五）

| 要求 | 实现 |
|---|---|
| 项目上下文严格隔离 | 全部 SQL 首过滤 `tenant_id AND project_id`（测试钉死跨项目不可见） |
| 区分正式/草稿/AI 建议 | state 驱动分组；非 approved 修订一律带「未经人工确认」标注；被拒绝的修订只以其真实状态呈现 |
| 关联真实记录来源 | 每行带 code/version/状态（AI 可直接引用 REQ-001 v2 approved） |
| 记录不存在明确说明 | 空域显式（无）+ 头部「未列出即不存在」 |
| 不凭空生成状态 | 投影全部来自库；缺失项目 → 零注入（防探测，与画像同语义） |
| 版本/状态变化感知 | 无缓存实时投影；测试钉死「pending_confirmation → approved 后下一轮即见」 |
| 不无差别注入 | 每域限行（需求 20/设计 10/ADR 10/任务 5/风险 8/阻塞 5/审计 5）；节预算 6k 内**整行裁剪**（自下而上：审计→任务→风险→ADR→设计→草案需求；「待人工处理」永不裁；头部永不裁）；不复制正文（approved 正文仅经既有 `ai_context_items` 显式附加通道注入） |
| Markdown+Git 权威不破坏 | 本投影只读 PostgreSQL 的状态/版本/计数权威面，不写任何域、不动 Git |

超预算退化路径：仓库节先删（CHAT_DROP_ORDER[0]），记录节其次（[1]），逐节截断兜底，全部记账进 `sections_dropped`（进 `llm_call_record.metadata`）。

## 4. 变更清单

- `crates/pm-ai/src/project_records.rs`（新）：记录投影构建器（6 条只读 SQL + 预算内整行裁剪装配）
- `crates/pm-ai/src/chat_context.rs`：新节接线（存在项目才投影）、预算 16k、节上限常量、模块文档更新
- `crates/pm-ai/src/discovery_context.rs`：CHAT_SECTION_ORDER 3 节、CHAT_DROP_ORDER 2 项、assemble_chat 记录节 cap
- `crates/pm-ai/src/lib.rs`：注册模块

**无前端/无 API 契约变更**（注入在 worker 回复链内部；`metadata.injection` 记账自动带新节）。

## 5. 测试

**新增 `crates/pm-ai/tests/project_records.rs`（6 用例全绿）**
1. 正式/草案分组 + 「未经人工确认」标注 + ADR 状态/阻塞标记 + 可定位元数据
2. 项目隔离：他项目 REQ-SECRET 不可见
3. 实时性：pending_confirmation → approved 后下一轮投影即反映
4. 空项目：≥6 个显式（无）+ 计数为零
5. 长度控制：60 条长需求超预算 → 总长 ≤6k、最新行保留最旧行被裁、头部与待人工不可裁
6. build_chat_context 装配：records 节进注入与 metadata 记账

**既有测试更新（`chat_context.rs` 10/10 绿）**
- 节序断言 2→3 节；预算断言 8k→16k；DROP_ORDER 断言（仓库先删、记录次之）
- A-7 凭据静态扫描扩展覆盖 project_records.rs SQL 区段

**回归**：`cargo test -p pm-ai` 全量（conversation_reply 12、discovery_context 18、orchestrator 37、chat_context 10、project_records 6 等）。

## 6. 安全基线核对

- 只读投影：6 条 SELECT，无写路径；D1 凭据扫描扩展到新 SQL（通过）
- 跨项目隔离：SQL 双过滤 + 测试钉死
- 正式/草案边界：非 approved 显式标注（测试钉死）；审批闸门无任何变动
- 注入隔离：外层 `<context>` 数据化处理不变

## 7. 验收标准对照（任务书 §五）

- AI 可准确回答项目事实 → 待运行态验证（回归批次，用 TaskFlow 真实记录提问）
- AI 能引用/定位记录 → code+version+状态进投影（测试钉死）
- 不把草稿当正式 → 「未经人工确认」标注（测试钉死）
- 不跨项目读取 → 测试钉死
- 新对话读取项目知识 → 注入按项目实时构建（不依赖会话历史）
- 变更后取最新版本 → 实时投影（测试钉死）
- 上下文选择与长度控制 → 行数上限+节预算+整行裁剪（测试钉死）
