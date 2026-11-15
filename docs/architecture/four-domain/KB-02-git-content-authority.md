# KB-02 · Git 内容权威与存储实现审计（Q2 补审）

> commit `05ce33d8` · 2026-11-15 · 只读 · 回答专家定向问题 Q2
> 结论：**Git 写入能力只存在于旧 ai 栈且是 crate 私有实现**；新 kb/PC 正文权威=PG；专家的 Source Commit vs Content Commit 二分在现有代码中已有列级落位；kb/pc 迁 Git 各有一个硬阻碍（0023 不可变触发器 / 发布事务 hash 锚链）。

## Q2-1 旧 ai 栈 Git 实现（现役基准）

| 能力 | 实现 | 证据 |
|---|---|---|
| 仓库管理 | KnowledgeRoot.from_env（`AI_PM_KNOWLEDGE_ROOT`，缺省 `<cwd>/../ai-pm-knowledge`）；ensure_ready 首用 git init+拒绝嵌套 checkout；强制 autocrlf=false/longpaths=true（保 LF hash 口径） | knowledge.rs:147-194 |
| 目录布局 | `projects/{project_code}/{doc_type}/{doc_key}.md`（canonical_path+safe_join 防穿越） | knowledge.rs:270-283,704-712 |
| 写入/提交 | write_and_commit:582 / write_revision:610（fs::write→git add→git commit 单文件）；同内容幂等跳过 | knowledge.rs:582-631 |
| 读取 | read_document:539（读文件+hash 比对，Stale 拒）；git_read_at:742（`git show sha:path` 读历史） | knowledge.rs:539-579,742-748 |
| 并发防护 | 项目级会话 advisory lock（`ai.project_knowledge_write:{project_id}`，pg_try_advisory_lock 10s 超时，持锁连接不还池）+ operation_id 幂等账本（五命令）+ base_revision CAS | knowledge_write_coordinator.rs:69-118,157-229 |
| 回滚 | revert_commit:635（git revert --no-commit 幂等反向提交）+ veto.rs:78 + rolled_back 状态 | knowledge.rs:635-647, veto.rs:78-121 |
| 崩溃窗口 | Git 先提交、DB 后提交两段式，靠幂等重放补偿（:339-347） | knowledge.rs:390-457 |

## Q2-2 共享 Git Content Service：**不存在**

- 全仓无 git2 依赖、无 ContentService/ContentAuthority 抽象；仅两个互不复用的 git 子进程封装（旧栈可写 knowledge.rs:714 / 分析链只读 git_access.rs:1-5）。
- 旧栈 Git 能力全部 `pub(crate)` 私有，仅 KnowledgeRoot 类型导出（lib.rs:132-135）。**kb/pc 迁 Git 需新建共享抽象（D-CONTENT）或提升可见性改造。**

## Q2-3 kb.document_revision.content 迁 Git 可行性：部分可行，两个硬阻碍

**写入点**（全落 PG content）：document.rs:351（docproc 摄入）、:588-600（REST 摄入，hash=sha256(markdown) :557）、kb_pc_analysis.rs:683-703（worker，hash :490）。confirm 只翻状态不改 content。

**读取点**：get_document:510-517、indexer:41-55/335、retrieval/channels.rs:263-271（靠 content_hash 一致性守卫 :274）、analysis_report.rs:322-323、discovery_context.rs:535-539（repo-% 文档摘要）、repo_kb_pc_query.rs:74-84。

**hash 口径**：kb=sha256(原始 markdown 字节)，与旧栈同算法（pm_write/support.rs:4-10）——Git blob=原字节+LF+autocrlf=false 时 **hash 完全可保持**，无规范化裁剪。

**硬阻碍 ①**：0023 不可变触发器（0023:61-68）冻结 content/content_hash/document_id/revision_no 等——**迁移不能就地清空 content 改指 Git**，只能新列/新表/改列口径；rejected/draft 历史行（append-only 全留）也要有 Git 对应物，否则 hash 校验链断。
**硬阻碍 ②**：kb.document_revision **无 git_commit_sha 列**（Content Commit 缺位）；source_commit_sha 不在不可变清单可回填。

## Q2-4 kb.memory_revision.body：无 Git 机制，且 hash 口径特殊

- 表无任何 git 列；memory.rs 全文无 git 引用。
- content_hash=**结构化 JSON 规范 hash**（memory.rs:248-272：`{memory_type,title:trim,summary:trim,body:trim,refs:[排序四元组]}` 的 sha256）——**Git blob 无法直接充当 content_hash**，迁移需双轨（blob+保留原 hash+映射），无现成解法。D-CONTENT 裁决须单列此对象。

## Q2-5 pc.document_revision.content：预留位齐全但零接线

- 写入：object.rs:232-244 / :408-435（append_revision，hash=sha256_hex :73）；读取：repo_kb_pc_query.rs:156/185/211、channels.rs:52-55/175-179、analysis_report.rs:179-183/359-363、discovery_context.rs:544-557、indexer.rs:335。
- **启用 git_committed 中间态需改**：publish_inner（publish.rs:91-615）①建 write_operation 后插 Git 提交步骤 ②completed 更新前补 git_committed 迁移+git_commit_sha 回填（:611-615）③幂等重放分支识别 git_committed 续跑（:124-167）。CHECK 0001:3470 的 pc_commit_object/relationship_revision 两命令现役零写入。
- 附加缺口：pc draft 面无写锁（advisory 仅发布入口 :104-108）。

## Q2-6 Source Commit vs Content Commit 列级落位

| 概念 | 现役列 | 写入者 |
|---|---|---|
| Source Commit（知识来自哪个源码提交） | kb.document_revision.source_commit_sha（+0023 report.commit_sha/since_commit、project_repository.last_analyzed_commit_sha） | 仅 kb_pc_analysis.rs:684（docproc 通道不写，用 source_ref/checksum） |
| Content Commit（正文在哪个 Git 提交） | ai.document_revision.git_commit_sha（NOT NULL，现役唯一真实现） | knowledge.rs:390,436-447 |
| Content Commit（预留未用） | pc.document_revision.git_commit_sha 恒 NULL（pc/mod.rs:26） | 无写入点 |
| **缺口** | kb.document_revision 无 Content Commit 列 | — |

## Q2-7 canonical_path / doc_slug：全部虚拟路径，无文件对应

- kb.document.canonical_path 写值 `kb/{doc_key}`（document.rs:339,577）——kb/pc 栈全文无 fs::write 到该路径；pc doc_slug=`pc/object/{key}.md`（object.rs:31-52，字符集已受控）同样仅入库。
- 唯一真实文件对应=旧 ai 栈 `projects/...`（knowledge.rs:563-565）。
- 迁 Git 时虚拟路径可直接升格为仓内路径（kb 格式已似路径、pc 字符集受控），成本在唯一键与回填。

## Q2-8 迁移风险清单

| 风险 | 已有解法（旧栈可复用） | 缺口 |
|---|---|---|
| 并发写 | 项目级 advisory lock+幂等账本 | kb 需锁键升 tenant+project；pc 需扩到 draft 面；共享 Git 服务（远端）时 DB advisory 模式全部要重做（Git 仓无并发 index 保护） |
| 崩溃/双写不一致 | Git 先提交+重放补偿 | —（模式可复制） |
| 读一致性 | Stale hash 校验+autocrlf=false | kb/pc 需保留同款语义 |
| 历史回滚 | revert_commit 幂等+rolled_back | kb 0023 触发器挡就地改列+历史行整链迁移；pc snapshot_hash 纳入 Git commit 会破坏 CAS 与审批锚（proposal_content_hash，publish.rs:201-213） |
| memory_revision | 无 | 结构化 JSON hash 与 blob 不对应，需双轨 |
| 闲置机制 | pc git_committed/pc_commit_* 预留位，启用成本集中在 publish.rs 单文件 | write_operation 需补 commit sha 落位 |

## 三栈对照总表

| 维度 | 旧 ai 栈 | 新 kb 栈 | PC 栈 |
|---|---|---|---|
| 正文权威 | **Git**（合规基准） | PG content | PG content |
| Content Commit | ✓ 现役 | ✗ 缺列 | 列在恒 NULL |
| Source Commit | —（不适用） | ✓ source_commit_sha | 经 pc.source 间接 |
| 回滚 | ✓ revert+veto | ✗（rejected 终态） | ✗（版本只前进） |
| 并发防护 | advisory+幂等+CAS | PG 行级 | 发布 advisory+CAS |
| 迁移阻碍 | —（基准） | 0023 触发器+缺列 | hash 锚链+幂等分支改造 |

## 对裁决的输入

- D-CONTENT：必须新建共享 Content Service（现状无可复用抽象）；memory_revision 的结构化 hash 双轨问题与 kb 0023 触发器是裁决时要点名的两个例外项。
- U13 顺序参考：pc 预留位最齐（启用成本低）但 hash 锚链风险最高；kb hash 口径最干净但触发器挡路——顺序裁决需权衡「预留位成本 vs 锚链风险」。
- U19/审计口径更正采纳：canonical_path 虚拟、source_commit_sha=Source Commit——authority-matrix 的 PG 权威判定维持，且补充「kb 缺 Content Commit 列」为新事实。
