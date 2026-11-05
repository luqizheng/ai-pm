# S01-08 · Project Knowledge Repository & Git Backup — 专家评审稿 V1.0

> 评审对象：`s01-08-knowledge-repository-git-backup-v1.md`（V1.0，2026-11-05）  
> 评审日期：2026-11-05  
> 评审性质：专家评审 · 对照本仓已实施事实与既有裁决  
> 评审范围声明：**原文在 Part D.2「自动同步」第 5 步处截断**。本评审只覆盖已到内容（Part A / B / C / D.1 / D.2 前半）。
> D.2 剩余步骤、冲突处理、失败重试、恢复等 Part D/E 章节（如有）到达后须补评。

---

## §0 评审前置事实（前因后果）

本评审不能孤立读稿，以下三组事实直接决定结论：

**事实一：本仓已实施「知识空间 V1/V2」，与本稿主题高度重叠。**
master 已落地：迁移 `0006`（`ai.project_document` + `ai.document_revision`，两表只存身份/路径/版本/Commit/哈希，**不存正文**）、迁移 `0014`（`document_proposal` / `document_index_state` / `knowledge_reference` + revision 补 `source_event_id` / `analysis_run_id` / `redline_confirmed_by`）、独立本地知识 Git 根（`AI_PM_KNOWLEDGE_ROOT`，首次写入自动 `git init` + 铺模板 + `core.autocrlf=false` + 路径白名单防穿越）、质量门 8 项、提案流（`document_proposal`）、红线三类（对外承诺 / 删除历史 / 改写已交付版本）人类事前确认、否决窗口（Git revert + `rolled_back` + 指针回退 + 审计）、`KnowledgeIndexer` + Markdown 树索引、REST + MCP 工具 + 前端 `KnowledgePage`。
已裁决的三权威分工：**Markdown+Git 管知识正文权威（本地知识根）、PostgreSQL 管业务状态权威、PageIndex 只做可重建派生索引**。

**事实二：本仓 AGENTS.md 硬规则——新项目无存量数据，不做迁移/兼容/双写。**
任何「旧存储→新存储」「双轨并存」式设计都不得实施。

**事实三：S01 目录存在未收敛的编号冲突。**
`docs/ui/s01/README.md` 既有「编号冲突（待裁决）」注的建议顺序中，**S01-08 预留给 Personal Home**；本稿占用 S01-08 = Knowledge Repository，两案冲突须用户裁决。

---

## §1 结论汇总

| 编号 | 级别 | 问题 | 落点 | 根因 | 处置建议 |
|---|---|---|---|---|---|
| R-08-01 | **P0** | 与已实施知识空间 V2 的关系全文未声明，Part C 按「新建存储」行文 | B.2 / C.2 全部 | 稿件视角只对齐 S01-02~07，未盘点仓内既有实现 | 增加「与 Knowledge Space 关系」章节；第二层 Knowledge File Storage **必须复用** `AI_PM_KNOWLEDGE_ROOT`，不得重建第二套（详见 §2.1） |
| R-08-02 | **P0** | 自动同步可能绕过质量门/红线治理 | D.1 / D.2 | 「文件变化→入队→Push」管道未声明同步对象是「已治理版本」还是「工作区原始文件」 | 明确：只同步**已提交到本地知识根的 commit**（以 `document_revision` 为准）；红线类落 `redline_pending` 期间不得进入远程同步（详见 §2.2） |
| R-08-03 | **P0** | C.2.3 元数据建模与 `document_revision` 既有字段重叠，存在双表双权威风险 | C.2.3 | 字段清单按通用文件模型罗列，未对照既有表 | 不得新增与 `ai.project_document` / `document_revision` 平行的第二套主表；增量仅为「远程仓库关联配置 + 同步状态」两类表（详见 §2.3） |
| R-08-04 | **P0** | 远程目录布局与本地知识根结构的关系未定义 | C.2.2 | `project-knowledge/` 示意树与知识根现行 `projects/{project_key}/...` 布局不一致 | 须裁决：镜像（推荐）还是重排。重排=同步器做路径映射，漂移风险高（详见 §2.4） |
| R-08-05 | P1 | 编号冲突：README 建议顺序 S01-08 = Personal Home | 全文 / README | 既有编号冲突注未随本稿收敛 | 登记 README，待用户统一裁决，不自行改名（详见 §3.1） |
| R-08-06 | P1 | 一个远程仓库可否被多个 Project 关联未定义 | C.1 | 只约束了 Project→仓库方向（≤1），反向未约束 | 建议 V1 明确 1:1（唯一约束），须裁决（详见 §3.2） |
| R-08-07 | P1 | 冲突语义缺闭环：远程 diverge 检测字段缺失 | C.1.1 / R-S01-08-02 | `Last Commit` ≠「本地已知的远程同步基点」 | 增补 `synced_remote_commit_sha` 字段；定义「fetch 后 HEAD 漂移 → 停自动同步 → Waiting for Human」状态（详见 §3.3） |
| R-08-08 | P1 | Sync Mode=Manual 与裁决「自动同步为主」的语义边界 | C.1.1 | 配置项允许完全关闭自动，与 R-S01-08-02 措辞存在两种读法 | 澄清：Manual 模式合法但须显式告警 Pending 积压；或 Automatic 唯一 + Sync Now 为补充（详见 §3.4） |
| R-08-09 | P1 | 认证/配置变更审计与告警未覆盖 | C.1.2 / B.4 | B.4 审计只提「文件操作」 | 配置变更（换仓/换分支/换凭据）落审计事件；认证失效走 notification-service 提醒（详见 §3.5） |
| R-08-10 | P1 | 二进制附件（PDF/图片）体积治理缺失 | C.2.1 | 进 Git 的附件无上限/白名单机制 | 单文件上限 + 格式白名单；与现有附件机制对齐，不得双轨（详见 §3.6） |
| R-08-11 | P2 | 「最多关联一个**默认** Knowledge Repository」措辞 | C.1 | 「默认」暗示未来多仓，与「本版本不引入多个」矛盾 | 删「默认」二字 |
| R-08-12 | P2 | `Default Branch` 与 S01-03 术语撞车 | C.1.1 | S01-03 已裁决 Default Branch=Git 自身默认，本稿语义实为 Selected Branch | 改名 `Sync Branch` |
| R-08-13 | P2 | 目录树 `technical-decisions` 双归口 | C.2.2 | `architecture/` 与 `decisions/` 下各有一份 | 保留一处（建议归 `decisions/`），须裁决 |
| R-08-14 | P2 | 同步失败积压的用户可见性未定义 | R-S01-08-02 / C.1.1 | 「保留待同步」若无提醒会静默腐化 | 失败/积压超阈值走通知（可在补发部分落地） |

---

## §2 P0 详述

### §2.1 R-08-01：必须声明与知识空间 V2 的关系，禁止第二套存储

**问题**：Part B.2 模块关系表只列 S01-02~07；Part C.2「Knowledge File Storage」按新能力设计文件类型、目录、元数据。若按字面实施，会新建一套与 `ai.project_document` / `document_revision` / `AI_PM_KNOWLEDGE_ROOT` 平行的存储——直接违反三权威分工（正文单一权威）与 AGENTS.md「不做双轨」硬规则。

**正确的三层映射**（建议补入 B 章作 B.5）：

| 本稿层次 | 已实施对应物 | 本稿真正增量 |
|---|---|---|
| Knowledge Domain | `pm-application` knowledge 模块 + 质量门/提案/红线/否决 | 无（复用） |
| Knowledge File Storage | 本地知识 Git 根 `AI_PM_KNOWLEDGE_ROOT` + `document_revision` 登记 | 无（复用） |
| Knowledge Git Backup | **不存在** | 远程仓库关联、认证、同步队列、冲突治理（本稿核心价值） |

即：**本稿的合法定位是「给已有本地知识 Git 根增加远程异地备份」，而不是「建设知识文件存储」**。C.2 应改写为「对既有存储的约束声明 + 远程备份增量」。

**收益**：避免重建存储（预计节省一整条存储/索引/检索线的重复实施）；避免「本地根 vs 第二存储」内容漂移这一类最难排查的一致性事故。

### §2.2 R-08-02：同步对象必须锚定已治理版本

**问题**：D.1/D.2 管道是「文件变化→事件→队列→Commit→Push」。若事件源是「文件写入动作」，则红线条目（对外承诺/删除历史/改写已交付版本）在人类事前确认完成前就会被推到用户远程服务器——对外承诺类内容一旦离开系统边界，否决窗口的「Git revert」只能追加历史，**收不回已暴露的内容**。

**要求**（补入 D，截断部分到达后核对是否已覆盖）：

1. 远程同步的输入 = 本地知识根的已落 commit（`document_revision` 登记链），不是工作区原始文件。
2. 红线类提案处于 `redline_pending` 期间不产生可同步版本。
3. 否决窗口的 revert 落本地根后，作为**新增提交**进入同步队列；远程历史不删除、不 Force Push，与本稿 C.1.3 一致。
4. 同步器自身不得生成知识内容，只做版本搬运。

### §2.3 R-08-03：元数据不得平行建表

**问题**：C.2.3 字段清单（file_id / content_hash / size / created_by / deleted_at…）与 `document_revision` 既有字段（version / content_hash / commit / source_event_id / analysis_run_id / redline_confirmed_by）大面积重叠。

**裁决建议**：身份/路径/版本/哈希一律以 `ai.project_document` + `document_revision` 为唯一真源；本稿仅新增两类增量表——①`project_knowledge_remote`（远程仓库关联 + 认证 + Sync Mode + 连接状态），②`knowledge_sync_state`（同步队列/最近成功同步/synced_remote_commit_sha/错误记录）。字段是否加列、列名是否独立，按「代码事实核验」执行；迁移号实施时核验当前 migrations 最大号顺延占用，不抢号。

### §2.4 R-08-04：远程目录 = 本地知识根镜像（待裁决）

**问题**：C.2.2 示意树 `project-knowledge/…` 与知识根现行布局（`projects/{project_key}/knowledge/…`，已有种子文档 `projects/SHIELD/knowledge/ARCH-001.md`）不同。若远程采用另一套布局，同步器必须做路径映射：多一条映射规则就多一处漂移点，且恢复（从远程拉回）时要反向映射，复杂度翻倍。

**倾向**：远程仓库目录 = 本地知识根的**原样镜像**，C.2.2 改写为「本地知识根现行结构的分类扩展建议」；`technical-decisions` 双归口在此一并收敛（见 R-08-13）。**须用户裁决**。

---

## §3 P1 详述

### §3.1 R-08-05 编号冲突
README 既有建议把 S01-08 给 Personal Home。本稿占用后形成第二处 S01-08 主张。处置：两案并存在 README 登记，**待用户统一裁决编号目录**，不自行改名（与既有「编号冲突待裁决」注同性质）。

### §3.2 R-08-06 远程仓库与 Project 的反向约束
C.1 只约束「每个 Project ≤1 个知识仓库」。若允许多个 Project 共享一个远程仓库，提交署名/目录/恢复边界会跨 Project 混杂。建议 V1 明确 1:1：一个远程仓库至多归属一个 Project（数据库唯一约束兜底）。**须裁决**。

### §3.3 R-08-07 冲突检测缺「同步基点」字段
R-S01-08-02 第 6 条禁止未经人工确认覆盖远程冲突，但 C.1.1 没有「本地已知的远程位置」字段。`Last Commit`（最近成功提交的本地侧 commit）不能替代它。建议增补 `synced_remote_commit_sha`：每次成功 Push/Pull 对齐后更新；fetch 发现远程 HEAD ≠ 该字段 → 判定 diverge → 暂停自动同步 → 转 Waiting for Human + 差异预览。该状态机应在补发的 Part D 中闭环。

### §3.4 R-08-08 Sync Mode 语义澄清
两种读法：a) Automatic/Manual 二选一，Manual 合法；b) Automatic 唯一，Sync Now 只是手动补充。裁决 R-S01-08-02「自动为主」更贴近 b，但 C.1.1 按 a 可实施。建议：保留双模式，但 Manual 模式下 Pending Changes > 0 须持续可见并告警（与 R-08-14 合并处理）。

### §3.5 R-08-09 审计与通知
B.4「文件操作审计 = 系统审计记录」不覆盖配置面。换仓库/换分支/换凭据/初始化确认/冲突人工处置，均须落审计事件（who / when / before / after）。认证失效提醒走既有 notification-service 通道（2026-11-05 已裁决的跨仓对接）。

### §3.6 R-08-10 附件治理
PDF/图片进 Git 无上限会拖垮同步与仓库体积。建议：单文件上限（如 ≤10MB）+ 格式白名单（C.2.1 已有雏形）+ 项目级附件总量提示；是否复用既有附件存储须代码核验，不得双轨。

---

## §4 待用户裁决清单

| 编号 | 问题 | 倾向 |
|---|---|---|
| Q-08-01 | 远程目录布局：镜像本地知识根（推荐）还是独立布局（C.2.2 现稿） | 镜像 |
| Q-08-02 | 一个远程知识仓库可否被多个 Project 共享 | V1 禁止，1:1 |
| Q-08-03 | C.2.2 `technical-decisions` 双归口收敛到哪一处 | `decisions/technical-decisions/` |
| Q-08-04 | Sync Mode 双模式保留（Manual 须告警积压）还是 Automatic 唯一 | 双模式 + 积压告警 |
| Q-08-05 | S01-08 编号归 Knowledge Repository 还是 Personal Home（README 既有冲突） | 待用户定 |
| Q-08-06 | 附件单文件上限数值与是否复用既有附件存储 | 上限 10MB；存储复用待代码核验 |

---

## §5 与既有裁决一致性核对

| 既有裁决/规则 | 本稿表现 | 结论 |
|---|---|---|
| R-S01-01-01 · Native AI-PM Independence（不依赖 Repository/MCP） | R-S01-08-04 知识仓库非前置条件 | ✅ 一致 |
| S01-03 Q1-Q4 · 多代码仓库 / Primary 可空 / OAuth 优先 / HEAD 仅提示 | C.1.2 OAuth 优先；知识仓库独立于代码仓库 | ✅ 一致；`Default Branch` 术语撞车见 R-08-12 |
| S01-04 · AI 只提案、人类确认；未确认 Cognition 不入正式 Knowledge | B.3「Git Push ≠ 人工确认」「Commit ≠ 产品决策」 | ✅ 一致 |
| 知识空间 V2 三权威分工 | B.4 权威表与「本地根=正文权威」可统一 | ⚠️ 方向一致但未点名既有实现（R-08-01） |
| V2 红线三类事前确认 | 未声明同步与红线的关系 | ⚠️ 缺口（R-08-02） |
| AGENTS.md · 无存量数据、不做双轨 | C.2.3 保留核验措辞 | ⚠️ 须按 §2.3 收敛，禁止第二套表 |
| S01-05 基线 · 正式基线确认机制不变 | B.2 明确「基线文档可成知识文件，确认机制不变」 | ✅ 一致 |

---

## §6 后续动作

1. **用户补发 Part D 剩余内容**（D.2 第 5 步之后至文末）→ 补充落库 + 补评（重点：冲突处理、失败重试、恢复/迁移、Delete 语义）。
2. 用户裁决 §4 六项（Q-08-01~06）。
3. 裁决后本稿升 V1.1：收编裁决 + 补 B.5（与 Knowledge Space 关系）+ C.1.1 增补 `synced_remote_commit_sha` + C.2.2 按裁决改写。
4. 实施前按统一技术核验清单核验：`document_revision` 字段现状、现有 Provider 接入能力（S01-03 同源核验）、附件存储现状、migrations 最大号。