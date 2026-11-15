# PM-AI 四域对象注册表与数据库映射审计 · 证据基准与过程记录

> 任务书 V1.0 · 只读审计 · 未修改任何代码/迁移/数据
> 基线：《PM-AI 四域对象边界与协作架构设计基线 V2.0》（D01~D04 + B01~B10 已冻结；U01~U25 待裁决）

## 证据基准

| 项 | 值 |
|---|---|
| Git commit | `05ce33d8dfbd70a672b27c1a6ce20a1ee5d405c6`（2026-11-15 19:18 +0800） |
| 工作区 | clean（仅未跟踪 docs/audit/、docs/architecture/four-domain/ 审计产物） |
| 数据库 | full-dev `superpowers-full-postgres-1`，db=`postgres`，user=`postgres`（docker exec 只读访问） |
| 迁移最大版本 | 0024_analysis_report_run_mode.sql（master 共 24 条，`_sqlx_migrations` 1~24 全执行） |
| Schema | 16 业务 schema：ai, core, delivery, design, devenv, discovery, docproc, governance, integration, kb, pc, plan, platform, public, source, viz |
| 对象总量 | 209 表/视图/物化视图 · 1941 列 · 1041 约束 · 36 触发器 · 527 索引（raw/ 目录全量导出） |
| 审计执行时间 | 2026-11-15 19:41 ~ 当日晚 |

## 运行事实总览（精确行数）

- **三/四域核心正式对象全部 0 行（NO_PRODUCTION_DATA）**：kb.memory、pc.cognition_object、pc.cognition_revision、pc.cognition_version、pc.proposal、pc.approval、pc.evidence、pc.relationship、pc.conflict、pc.document(_revision)、ai.project_knowledge、ai.cognition_item、ai.cognition_run。
- 少量真实数据（清库重测后）：kb.document=1、kb.document_revision=1、kb.document_section_index=6、pc.source=1、integration.repo_analysis_report=1（failed）、integration.repo_import=6、integration.intake_task=27、delivery 任务域若干、ai.analysis_run 若干（结果见 raw/inv_rowcounts.tsv）。
- **pc.relationship_type = 0 行**：迁移 0007 种子（8 行，owner_domain∈{pc,kb}）已执行，但清库重测把字典清空——种子数据与业务数据未分级，清库脚本一律全清。

## raw/ 导出清单

| 文件 | 内容 |
|---|---|
| inv_objects.tsv | 209 对象（schema.name + 类型） |
| inv_rowcounts.tsv | 全表精确行数 |
| inv_rowcounts_est.tsv | pg_class 估计（全 -1，弃用留档） |
| inv_columns.tsv | 1941 列（schema.table\|column\|type\|nullable\|default） |
| inv_constraints.tsv | 1041 约束（含 CHECK 状态枚举全集） |
| inv_triggers.tsv | 36 触发器全定义 |
| inv_indexes.tsv | 527 索引定义 |
| inv_migration_origin.tsv | 162 个 CREATE → 出生迁移映射 |

## DB 侧关键事实（写入权威矩阵素材）

1. 触发器分布：pc 14 个（revision/version/version_member/write_operation/approval/proposal 锚定/relationship 端点闭包）、kb 3 个（document_revision/memory_revision/memory_ref/source 4 个守卫）、delivery 5、design 3、docproc/viz/plan/governance 其余；**ai.* 与 integration.* 零触发器**。
2. 状态枚举大小写混用实录：大写系（discovery_finding.verification_status、discovery_session、kb.memory_revision.review_status/version_status、integration.intake_*）vs 小写系（kb.document_revision、pc.*、ai.change_proposal、delivery.*）。
3. `ai.analysis_run.status` 无 CHECK、无触发器：状态权威完全在应用层。
4. `ai.project_knowledge` 有 version+supersedes_id+confirmed_by/at（非"无版本"），`source_item_id` 回指 ai.cognition_item。
5. `ai.cognition_item` 状态机 CHECK：proposed/confirmed/rejected/investigation/deferred/superseded；带 evidence jsonb、run_id、knowledge_id、has_conflict。
6. `pc.source` 自带 DomainRef 四列（target_domain/type/id/revision_id）+ revision_missing_reason。
7. `kb.document_revision` 带 source_commit_sha/source_ref/source_checksum（Git 相邻字段）但正文权威 = content TEXT 列。
8. pc.evidence CHECK 强制三类定位形态：代码类必须 file_path+commit_sha、document_section 必须 target_domain='knowledge_base'+target_revision_id、口述类必须 excerpt/description。

## 代码事实来源

五路并行 Explore 审计（KB+旧ai栈 / Analysis 三链 / ProjectCognition / PC / 跨域 DomainRef+API+事件+前端），结论汇入 docs/architecture/four-domain/ 七份交付物，证据均标注 commit 05ce33d8 下的 file:line。

## 审计纪律遵守记录

- 数据库访问全部只读（SELECT / 系统目录查询），未执行任何 DDL/DML。
- 未修改迁移、业务代码、测试数据；未生成 Proposal；未确认任何认知；未提交 Git。
- 交付物写入 docs/architecture/four-domain/，原始证据留 docs/audit/four-domain-2026-11-15/raw/（不入正式架构目录视为已批准基线）。
