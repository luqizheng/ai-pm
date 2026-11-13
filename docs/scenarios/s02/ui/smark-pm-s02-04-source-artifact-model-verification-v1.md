# S02-04 · 外部供稿《原始来源与需求治理模型 V1.0》增量核验（补充 13 点之外）

> 文档性质：对 `smark-pm-s02-04-initial-review-v1.md` 附录 A（13 点核验反馈）的**增量核验**，补入两处此前未发现的更严重事实 + 两处口径修正。
> 评审对象：外部供稿《S02 · 原始来源与需求治理模型 V1.0》（方案 B：五对象模型 + D-S02-01～10）。
> 输入基线：S02-01 V1.1（事实权威稿）+ S02-02 运行时核验报告 V1.0（DEF-01～04）+ S02-03 场景稿评审（C-1～C-8）+ 附录 A 13 点。
> 核验基线：`D:\kudi-project\templates\ai-pm` master @ 2933db8，迁移 0001–0028。
> 核验日期：2026-11-04
> 版本：V1.0
> 状态：待专家回应

---

## §0. 结论摘要

附录 A 的 13 点是**对照供稿稿逐条核**的结果。本稿补入的是**对照代码与既有设计资产横向核**的结果——两处新事实都比 13 点中的任何一条更影响落地：

| 编号 | 新发现 | 严重度 | 与 13 点的关系 |
|---|---|---|---|
| **E-1** | **`artifact` 一词在本仓已有三个互不相同的既定义义，稿的 `source.source_artifact` 是第四个** | **P0（命名地基）** | 13 点未涉及；比 C-2/C-4 更前置 |
| **E-2** | **`content_hash` 在本仓实际有三种计算语义**（REST 绑元数据 / 执行器绑正文 / 知识侧绑纯正文） | **P1** | 修订 C-2（13 点说的「两种语义」实为**三条路径**） |
| **E-3** | 供稿稿主张的 `source_artifact_id` **设计来源被查明**：架构稿 §6.2 把它定义为**交付物**指针，与稿的「原始来源」语义相反 | **P1（语义反转）** | 修正 13 点未察觉的**方向性错误** |
| **E-4** | 架构稿 §6.2 列出的 `artifact`（设计文档/PR/Commit/测试报告）与 `object-storage`（RustFS 计划）**从未实现**——稿的 `storage_ref` 方案有规划依据但无代码依据 | P2 | 修正 13 点「无上传设施」结论的完整表述 |

**总评不变**：事实断言全对，方向与既有裁定一致。E-1 是**必须先裁的命名问题**——在五套「artifact」语义并存的情况下新增第四套，会成为 S02-04 落地时最贵的返工点。

---

## §1. E-1 · `artifact` 一词在本仓已有三个既有义（新增 P0）

### 1.1 三个既有义（逐条带证据）

**义 ①：交付产物（架构稿 §6.2，V2 规划，未实现）**

`docs/architecture/ai-pm-architecture-principles.md:465`：
```
- artifact：设计文档、PR、Commit、测试报告。
```
上下文（`:458-467`）明确它是 `task_dependency` / `change_request` / `architecture_decision` 同级的**待建实体清单**——即**交付侧产物**。

**义 ②：AI 产出物 = 需求修订（已实现，REST + 前端）**

`apps/pm-api/src/pm_artifacts.rs:1-6`（模块头注释即定义）：
```
//! `/v1/pm/artifacts`：可验收「产物」的只读列表（AI 产物验收中心数据源）。
//! 首版产物 = **需求修订**（`delivery.requirement_revision`）：AI 产出并经审批的
//! 需求正文版本。
```
- 端点：`GET /v1/pm/artifacts`（`pm_artifacts.rs:23`），SQL 主体 `FROM delivery.requirement_revision rv`（`:51`、计数 `:67`）
- 返回体含 `content_hash` 字段（`:31`）
- 前端已占用：`frontend/src/api/artifacts.ts:8`（`ArtifactRecord`）、`:23`（`listArtifacts`）、路由 `frontend/src/shell/router/routes.ts:127-130`（`workspace-artifacts`）、页面 `frontend/src/workspace/pages/ArtifactsPage.vue:21`（注释：「按项目读产物（需求修订）」）

**义 ③：来源产物指针（已实现为**空列**，未写入）**

`migrations/0002_delivery.sql:22`：`source_artifact_id UUID` —— S02-01 V1.1 §2.2 已登记「代码中未见写入」。

### 1.2 稿的第四套义

稿 §3.2 提议新增 `source.source_artifact`，定义为**原始材料容器**：
> 原始来源：用户输入、邮件、会议纪要、外部反馈、上传文件等原始材料。

### 1.3 冲突判定

| 维度 | 既有三义 | 稿的第四义 |
|---|---|---|
| 方向 | 义①交付侧 · 义②产出侧（AI 产出物） | **输入侧（原始材料）** |
| 时点 | 需求定稿**之后**产生 | 需求定稿**之前**产生 |
| 载体 | 义①规划为独立实体 · 义②即 `requirement_revision` 行 | 稿提议独立 `source.source_artifact` 表 |
| 名称 | `artifact` / `source_artifact_id` / `/v1/pm/artifacts` | `source.source_artifact` |

**冲突点（三个）**：

1. **`source_artifact_id` 列名已被占**：该列现挂在 `requirement_revision` 上。若 E-3 成立的语义（该列原设计指向交付物）保持不变，则**同名列将分别指向两种相反的对象**——旧列指向交付物、新表指向原始材料。

2. **REST API 路径已被占**：`GET /v1/pm/artifacts` 是已实现的产出物列表。稿的新来源材料若走 `/v1/pm/sources` 尚可，但若也用 `artifacts` 路径即正面冲突。前端 `ArtifactsPage` 已把「产物=需求修订」写进注释与 fixtures。

3. **`artifact_type` / `source_channel` 两个枚举与义①的字段语义重叠**：义①的 artifact 是「设计文档、PR、Commit、测试报告」——即**有明确产出形态的交付物**；稿的 `artifact_type` 含 `document`（上传文档）、`other`。二者在「文档」这一类上无法区分。

### 1.4 建议（提请 S02-04 裁决）

**选项 A（推荐）**：来源材料**不叫 artifact**。改用 `source.source_input` / `source.source_material`（或 `source.provenance_source`），并把列名从 `source_artifact_id` 改为 `source_input_id`。
- 优点：彻底避开三套既有义，零冲突；`/v1/pm/artifacts` 保持「AI 产出物」语义不动，前端与 fixtures 零改动
- 代价：`source_artifact_id` 列需改名（当前**零数据零引用**→改名成本为零，E-4 结论支持）

**选项 B**：保留 `artifact` 命名，但**重新定义全仓 artifact 语义**为「一切产物」，稿的来源材料作为其子类型。
- 优点：术语统一
- 代价：须同时改架构稿 §6.2 的规划语义、`/v1/pm/artifacts` 端点语义、前端 `ArtifactRecord`/`ArtifactsPage`/fixtures/i18n；且「原始输入」归入「产物」在概念上是反的（产物是输入的结果）

**选项 C**：保留 `source.source_artifact` 表名，但把 `GET /v1/pm/artifacts` 改名为 `/v1/pm/deliverables`。
- 优点：表名不动
- 代价：仍与义①「artifact=交付物」在文档层冲突；前端路由 `workspace-artifacts` 连带改动

**本稿倾向 A**，理由：唯一零返工点（空列可改名）、且「来源材料」与「产出物」本就是相反方向，强行统一术语的收益低于成本。

### 1.5 新增待裁项

> **Q-18（新增）**：来源材料实体的**命名与既有 `artifact` 语义的隔离方式**——选项 A / B / C 三选一。
> 关联：E-1 / E-3 / 附录 A D-2（迁移号）。

---

## §2. E-2 · `content_hash` 实为三种语义（修订附录 A C-2）

### 2.1 附录 A C-2 的表述需修正

C-2 原文：「`content_hash` 两种语义并存（现有=`sha256("{rid}:{version}:{markdown}")` 绑元数据 vs 供稿=正文哈希）」。

**实为三条路径三种语义**——「现有」一侧本身就是两条：

| # | 路径 | 代码位置 | 计算式 | 绑定内容 |
|---|---|---|---|---|
| ① | REST 起草修订 | `crates/pm-application/src/pm_write/revision.rs:34-35` | `sha256("{requirement_id}:{version}:{markdown}")` | **rid + version + 正文**（绑元数据） |
| ② | 命令执行器建修订 | `crates/pm-application/src/command_executor/executor.rs:432-433` | `knowledge::content_hash(&serde_json::to_string(&content))` → `sha256(json)` | **title + markdown + priority 的 JSON 串**（`content` 见 `:427-431`） |
| ③ | 知识侧文档修订 | `crates/pm-application/src/knowledge.rs:646-653` | `sha256(markdown)` | **纯正文**（知识域，与需求域不同表） |

①与②的差异是**结构性的**，不是实现瑕疵：

- 公式不同（`{rid}:{version}:{markdown}` vs `JSON 串`）
- 输入不同（①只取 markdown，②取 title+markdown+priority 的序列化）
- ②对 JSON 序列化顺序敏感（`serde_json::to_string` 对 struct/map 的字段序依赖）

**后果**：同一张 `delivery.requirement_revision` 表，行间 `content_hash` **不可比**——REST 建的行与执行器建的行，哈希无法互相验证，也无法用哈希判断「正文是否相同」。

### 2.2 当前是否已造成实际故障

**尚未**——需求域的 `content_hash` 全仓**只被写入和展示，从未被用于比对**：

- `pm_artifacts.rs:31` 仅作为响应字段回显
- `pm_lists.rs:511` 仅 SELECT 回显
- 唯一按哈希查重的地方是**知识域**（`knowledge_quality.rs:78`，查 `ai.document_revision.content_hash`）——与需求域不同表、不同语义

**但风险已逼近**：S02-04 若引入 `requirement_revision_source`（来源关联）+ RequirementBaseline（基线冻结），两者都会自然产生「比对某修订正文是否与基线一致」的需求——那时哈希不可比会直接变成 S02-04 的实现障碍。

### 2.3 建议

1. **S02-04 裁决需求域 `content_hash` 的唯一语义**（建议：纯正文 `sha256(markdown)`，与知识域一致，且**不含 rid/version**——因为 rid/version 已在同表的 `requirement_id`/`version` 列中，无需进哈希）
2. **已有行的兼容处理**：当前无存量数据（AGENTS 规则 1「无存量数据」优先于合并规格的回填要求）+ 哈希无人比对 → **可定义新语义而不回填**，但须在迁移中显式声明「旧行哈希语义为 legacy，不保证可比」
3. **若保留双路径**：则两条写路径必须共用**同一个哈希函数**（收敛到 `pm-application` 的单一 helper），且输入集合必须一致——这属 DEF-04（双入口分叉）的子项

> **Q-19（新增）**：需求域 `content_hash` 的唯一语义定义——纯正文 / 保留当前 REST 语义 / 统一收敛为共用 helper；以及存量行是否回填。
> 关联：E-2 / 附录 A C-2 / DEF-04（Q-1）。

---

## §3. E-3 · `source_artifact_id` 的设计来源已查明——语义与稿相反（新增 P1）

### 3.1 查明过程与结论

`migrations/0002_delivery.sql:1` 头部注释即写明该文件是**架构稿 §6.2–6.3 的逐字复制**：
```sql
-- 0002_delivery: requirements, revisions, tasks, milestones (architecture doc §6.2–6.3, verbatim)
```

架构稿 `docs/architecture/ai-pm-architecture-principles.md` 中：
- `:359-372` = `delivery.requirement_revision` 的 DDL，**含 `source_artifact_id UUID`（`:363`），与 0002 完全一致**
- `:465` = `artifact：设计文档、PR、Commit、测试报告。`（V2 待建实体清单，与 `task_dependency` / `change_request` / `architecture_decision` 同级）

**结论**：架构稿里 `source_artifact_id` 指向的是 §6.2 规划中的 **`artifact` 实体**——而该实体被定义为**交付物**（设计文档、PR、Commit、测试报告），**不是原始输入材料**。

**这意味着稿 §3.4 的兼容处理方案存在方向性错误**：

> 稿 §3.4：「如果旧字段已经有数据，迁移时将其转入关联表。」

该方案默认旧列指向的是「原始来源」。按架构稿原义，旧列指向的是「交付物」——若真有数据，转入会把**交付物**错标成**原始来源**。

### 3.2 缓解事实

**该列零数据零引用**（S02-01 V1.1 §2.2 + 本轮全仓扫描确认）：
- 全仓 `source_artifact_id` 仅 1 处命中 = `migrations/0002_delivery.sql:22` 自身
- 零代码引用、零种子数据、零测试引用

→ 所以**实际风险为零**，但**语义陷阱真实存在**：任何读到架构稿的人（包括未来的实施者）会自然以为「已有来源模型可继承」，而实际是一个从未实现的、指向相反方向的空指针。

### 3.3 建议

1. **稿 §3.4 的迁移步骤 3 改写**：不是「转入关联表」，而是「**确认为空列后直接废弃**（无数据可转）」——本轮核验已把「步骤 3」提前完成
2. **S02-04 裁决表须显式记载**：旧列原设计语义 = 交付物指针，与新来源模型**语义不同源**，禁止以「延续既有设计」为由复用旧列名（回到 E-1 选项 A）
3. 归档建议：在迁移文件或 S02-04 裁决中留一句「`source_artifact_id` 原为架构稿 V2 `artifact`（交付物）预留指针，从未实现，与 S02 来源模型无继承关系」

> **Q-20（新增）**：旧列 `source_artifact_id` 的处置口径——废弃 / 改名复用 / 保留为 NULL 兼容位。
> 关联：E-1 / E-3 / E-4（零数据结论）。

---

## §4. E-4 · 架构稿的 `object-storage` 计划与「无上传设施」结论的完整表述（修正）

### 4.1 附录 A 表述需补全

附录 A 写「全仓无上传/对象存储设施（storage_ref 须接 lmms file-service）」。**结论正确，但缺规划层依据**——架构稿有明确规划：

`docs/architecture/ai-pm-architecture-principles.md:102`：
```
| object-storage | 业务附件、大型 AI 产物、CI 报告 | S3 兼容存储（采用 RustFS） |
```
`:141`（F2 节点）：
```
F2[("Object Storage<br/>附件、文档、测试产物")]
```

### 4.2 完整判定

| 层 | 状态 | 证据 |
|---|---|---|
| 架构规划 | ✅ **有**（S3 兼容 / RustFS） | 架构稿 `:102`、`:141` |
| 迁移 DDL | ❌ 无（无 artifact/attachment 表） | `migrations/*.sql` 扫描零命中 |
| 应用代码 | ❌ 无（无 multipart / S3 / presign / storage_ref） | `apps/` + `crates/` 扫描零命中 |
| 编排/部署 | ❌ 无（无 rustfs/minio service） | compose/yml 扫描零命中 |
| **可复用性** | **零**——稿的 `storage_ref` 方案在 ai-pm 仓内无任何可复用设施 | 同上 |

### 4.3 对稿的影响

稿 §3.2 说「附件正文可以存对象存储，数据库保存 storage_ref 和哈希，不必把大文件全部塞进 JSONB」——**方向与架构规划一致**（不是稿的发明），但稿把它写成了现成可用的能力。事实是：**这是本仓尚未开工的第一个存储设施**。

**建议**：
1. S02-04 裁决表把 `storage_ref` 标注为「**依赖未实现的 object-storage 能力**（架构稿 §6.2 规划为 RustFS/S3 兼容），非 S02 内部可完成项」
2. 二选一裁决（**Q-21 新增**）：
   - **选项 A（推荐）**：S02 第一批**只落 `source_artifact` 的文本类来源**（`original_content` JSONB + `content_hash`），`storage_ref` 字段**先建后用**（预留列，不接存储）；文件上传类来源推迟到 object-storage 落地
   - **选项 B**：S02 同步落地 object-storage（RustFS 编排 + S3 客户端）——**跨 S02 边界，须评估是否另立 I 切片**
3. 无论哪个选项：`original_content` 的**大小上限**须在 S02-04 明确（否则 JSONB 装大文件的风险敞口无界）

> **Q-21（新增）**：附件存储的 S02 边界——只落文本来源（`storage_ref` 预留）/ S02 内落地 object-storage / 推迟整个来源实体。
> 关联：E-4 / 附录 A D-2。

---

## §5. 对附录 A 13 点的口径修正汇总

| 附录 A 条目 | 修正 | 修正后表述 |
|---|---|---|
| **C-2** | 修订 | `content_hash` **三种**语义（REST 绑 rid+version+markdown / 执行器绑 title+markdown+priority 的 JSON / 知识域绑纯正文），非两种；且需求域哈希**当前无人比对**（仅回显），风险尚未爆发但 S02-04 的来源关联与基线比对会触发 → 见 E-2 / Q-19 |
| **「新增事实 4 条」第 1 条** | 补全 | 「全仓无上传/对象存储设施」→ 应补：**架构稿 §6.2 已规划 RustFS/S3 兼容 object-storage，但迁移/代码/编排三层均零实现** → 见 E-4 / Q-21 |
| **C-4**（FK 对齐 0019） | 细化 | 0019 惯例的具体形态：`UNIQUE (tenant_id, id)`（`:157-158`）+ `fk_*_tenant_project` 复合外键（`:241-254`）+ `ADD COLUMN tenant_id UUID NOT NULL`（`:103-107`）；新表须同时满足这三项，非仅「复合 FK」 |
| **D-1**（编号） | 确认 | 与本轮一致：供稿稿=S02-04 领域模型输入，不单独占号 |

---

## §6. 修订后的新增待裁项索引

| # | 问题 | 来源 | 影响 |
|---|---|---|---|
| **Q-18** | 来源材料实体**命名**与既有三套 `artifact` 语义的隔离方式（选项 A/B/C） | E-1 | **命名地基**，五套语义并存=落地返工点；建议 A（`source.source_input` + 列改名，空列零成本） |
| **Q-19** | 需求域 `content_hash` 的唯一语义定义 + 存量行是否回填 | E-2 | 修订附录 A C-2；来源关联与基线比对的实现前提 |
| **Q-20** | 旧列 `source_artifact_id` 处置口径（废弃/改名复用/保留 NULL 位） | E-3 | 须在裁决表显式记载「与新来源模型无继承关系」 |
| **Q-21** | 附件存储的 S02 边界（只落文本 / 内建 object-storage / 推迟） | E-4 | 与 E-4 共同决定 M1（SourceArtifact）的实际范围 |

> 连同附录 A 已提的三项（来源继承语义 / SourceArtifact 捕获权限与红线 / 拒绝语义并入 Q-6），**S02-04 待裁项总数**：Q-1～Q-17 + 新增 Q-18～Q-21 + 三项待裁登记 = **21 项编号 + 3 项待裁**。

---

## §7. 本轮核验的检索方法与可复现性

| 核验项 | 命令要点 | 结果 |
|---|---|---|
| `source_artifact_id` 全仓引用 | `grep -rn "source_artifact_id" migrations/ apps/ crates/` | 仅 `0002_delivery.sql:22` 一处（列定义自身）；代码/测试/种子零引用 |
| 通用 Artifact/Attachment 表 | `grep -rniE "CREATE TABLE .*(artifact\|attachment\|source_)" migrations/*.sql` | **零命中** |
| 上传/对象存储设施 | `grep -rniE "multipart\|object_storage\|minio\|s3\|blob\|storage_ref\|presign\|file_upload" apps/ crates/ migrations/` | **零命中** |
| RustFS/MinIO 编排 | `grep -rniE "rustfs\|minio\|object-storage" --include="*.yml" --include="*.yaml" --include="*.rs" --include="*.toml"`（排除 docs/） | **零命中** |
| 基线类冻结快照先例 | `grep -rniE "snapshot" migrations/*.sql` | 仅 `config_snapshot`（`0003:114`）/ `context_snapshot`（`:115`）/ `policy_snapshot`（`0010:30`）/ `repository_policy_snapshots`（`:35`）——均为**配置快照**，非需求/交付集合冻结，无形态可抄 |
| `artifact` 既有语义 | `grep -niE "artifact\|产物" docs/architecture/ai-pm-architecture-principles.md` + `pm_artifacts.rs` + 前端 `api/artifacts.ts` + `routes.ts` + `ArtifactsPage.vue` | 三个既有义（§1.1） |
| `content_hash` 计算式 | `revision.rs:34-35` / `executor.rs:432-433` + `knowledge.rs:646-653` | 三种语义（§2.1） |
| `content_hash` 比对用途 | `grep -rn "content_hash" apps/ crates/`（排除 INSERT/bind/SELECT 回显） | 需求域**零比对**；唯一查重在知识域 `knowledge_quality.rs:78`（`ai.document_revision`，异表） |

**核验边界（诚实声明）**：本稿全部结论基于**静态检索与源码阅读**，未实跑。S02-02 报告的 V-1～V-6 实跑结论作为**已实证事实**引用，未重复验证。`content_hash` 三语义的实际影响需在修复 DEF-04（双入口分叉）时实跑确认哈希是否已在测试中断言。

---

## §8. 给专家的三个问题（须答）

1. **Q-18**：来源材料实体叫什么？在本仓已有三套 `artifact` 语义（交付物 / AI 产出物 / 空指针）的前提下，是否接受改名为 `source.source_input` 一类？
2. **Q-19**：需求域 `content_hash` 收敛为哪种语义？存量（空）行是否需要声明为 legacy？
3. **Q-21**：附件存储是否留在 S02 第一批内？还是先只落文本类来源、`storage_ref` 预留后用？

---

参见 `smark-pm-s02-04-initial-review-v1.md`（附录 A 13 点）、`smark-pm-s02-01-requirement-domain-asset-inventory-v1-1.md`（事实权威稿）、`docs/scenarios/s02/s02-02-requirement-runtime-verification-v1.md`（运行时核验）。
