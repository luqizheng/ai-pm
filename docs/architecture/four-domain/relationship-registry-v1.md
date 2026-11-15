# 四域关系与 DomainRef 注册表 V1.0（审计产出）

> commit `05ce33d8` · 2026-11-15 · 只读审计产物
> DomainRef 契约 = `(target_domain, target_type, target_id, target_revision_id)`（kb-pc-contracts/src/domain_ref.rs:21-29，ADR-07 禁 service 字段）

## 1. DomainRef 契约与校验现状

### 1.1 domain 注册表（registry.rs:35-56，共 9 个）

| domain | 归属服务 | 通道状态（registry.rs:91-102） |
|---|---|---|
| requirement / project_goal / design / adr | Platform | Open |
| knowledge_base | Kb | Open |
| project_cognition | Platform | **ReadOnly** |
| pc | Pc | Open |
| product_reality | Platform | **NotYetOpen** |
| product_plan | Platform | **PermanentlyExcluded** |

注意：通道表已预注册 `project_cognition`（ReadOnly）——四元组层面对第四域早有位格，但无对象注册与写入实现。

### 1.2 校验覆盖矩阵（V2.0 §8.2 六项）

| 校验项 | 覆盖位置 | 覆盖度 |
|---|---|---|
| domain | validate()（domain_ref.rs:70-82）+ parse_domain | ✓ 全覆盖 |
| type | **无统一注册表**；仅局部白名单：KB 记忆引用限 knowledge_base+type=source（memory.rs:206-211）；pc.source CHECK 兜底 kb_document 通道（BL:3410-3411） | ✗ 缺口 |
| id 存在性 | **从不验**（无 FK）；仅消费端反查时触库（kb_pc_handlers.rs:214-221，查不到即任务失败） | ✗ 缺口 |
| revision 存在性/有效性 | 从不验；记 revision_missing_reason 兜底 | ✗ 缺口 |
| tenant | validate() 不含；KB 自域引用 check_kb_refs_tx（memory.rs:226-）验本项目 | ✗ 仅局部 |
| project | 同上仅 KB 局部 | ✗ 缺口 |
| 事件 payload | validate_payload_references（events.rs:244-296）：禁 service 键+三键齐全+domain 可解析——**机械校验，不验 id** | 部分 |

### 1.3 构造与解析位置

- 结构化构造（生产）：memory.rs:1098（K4 返回）、pc/proposal.rs:1494-1506、pc/publish.rs:565-571、knowledge_services.rs:124（K5 解析）。
- **手拼 JSON 绕过结构**：kb_pc_handlers.rs:54-67/355-360、kb_pc_analysis.rs:59-66（ref_json）、intake/mod.rs:420-470、intake/signal.rs:231-239、kb_service/document.rs:261-266、memory.rs:805——事件 payload/任务列大量绕过 DomainRef 类型。
- 任务列裸透传：intake_task.signal_ref/result_ref（BL:2655-2656）由 complete_task 直写事件 JSON（intake/task.rs:48），不经过 payload 校验。

## 2. 跨域关系注册表（实存关系）

| Relation Code | Source 域 | Source 对象 | Target 域 | Target 对象 | 含义 | 版本引用 | 当前实现 |
|---|---|---|---|---|---|---|---|
| R-01 | KB | kb.document_revision.source_ref | docproc | docproc.artifact | 文档处理产物锚 | checksum 复检（document.rs:132-163） | 0017 两列（裸引用+复检） |
| R-02 | KB | kb.memory_ref | 任意登记域 | 四元组目标 | 知识证据/主题锚 | target_revision_id 必钉或记原因 | 不可变触发器+validate_refs |
| R-03 | KB | memory_ref→knowledge_base/source | KB | kb.source | 域内证据 | 四元组 | memory.rs:206 白名单 |
| R-04 | ANALYSIS | repo_analysis_report.kb_revision_id | KB | kb.document_revision | 报告正文载体 | **裸 UUID 无 FK 无解析** | 0023:30；mark_analyzed 回填 :228-238 |
| R-05 | ANALYSIS | repo_analysis_report.pc_source_id | PC | pc.source | 报告-PC 来源锚 | 裸 UUID（经 result_ref JSON 强转） | 0023:31 |
| R-06 | ANALYSIS | repo_import.analysis_run_id | ANALYSIS | ai.analysis_run | 导入-Run 关联 | 裸列无 FK | BL:2772 区段；proposal.rs:374 |
| R-07 | ANALYSIS | ai.change_proposal.analysis_run_id | ANALYSIS | ai.analysis_run | 提案-Run | **FK NOT NULL**（BL:745） | 唯一强 FK 关联 |
| R-08 | ANALYSIS | worker 产物 pc.evidence.target_* | KB | kb.document_revision | 分析证据钉 KB 修订 | 四元组+evidence_hash | kb_pc_analysis.rs:936 |
| R-09 | ANALYSIS | ai.cognition_item.run_id | ANALYSIS | ai.cognition_run | 认知候选来源 | FK | BL:7202 |
| R-10 | PROJECT_COGNITION | ai.project_knowledge.source_item_id | PROJECT_COGNITION | ai.cognition_item | 正式认知→候选 | FK | BL:7642 |
| R-11 | PROJECT_COGNITION | ai.baseline_proposal.source_run_id | PROJECT_COGNITION | ai.cognition_run | 基线提案挂认知 Run | FK 但**零填充**（仅 viz 读侧 join） | BL:7174 |
| R-12 | PC | pc.source.target_* / pc.evidence.target_* | KB 等 | 四元组 | 认知来源/证据 | TEXT 四元组（无 FK） | BL:3391/3182 |
| R-13 | PC | pc.proposal.source_id | PC | pc.source | 提案来源 | 域内 | proposal.rs:644 |
| R-14 | PC | pc.version_member.object_id/relationship_id | PC | 对象/关系修订 | 版本成员闭合 | 裸 UUID+DEFERRED 闭合触发器兜底 | BL:3421；trg_pc_vm_closure |
| R-15 | PC←ANALYSIS | pc.proposal ← propose_from_analysis | ANALYSIS | 分析产物 | 分析→PC 候选 | 提案成员钉对象修订 | proposal.rs:1401-1520 |
| R-16 | PROJECT_COGNITION→PC | identity-synthesis 端点产 PC 提案 | PC | pc.proposal | 身份综合三提案 | 走 pc 服务层（合规） | pm_cognition.rs:168-228→identity.rs:178-206 |
| R-17 | 四域通用 | integration.outbox_event payload | 各域 | 事件 | 跨域通知 | payload 内四元组（机械校验） | outbox 同事务 |
| R-18 | 交付桥 | delivery.requirement_finding.finding_id | discovery→delivery | discovery_finding | 发现挂需求（人工桥） | 裸引用 | requirement_traceability.rs:306 |

**未建立的关系（目标态缺口）**：
- KB Source→Document（两表无任何引用列）——R 无。
- KB Document→Extraction→Memory（Extraction 对象不存在）。
- Analysis Run→Report（无 analysis_run_id，靠 intake 间接）。
- Report→Report Revision（Revision 不存在）。
- Project Cognition→PC（无正式关系类型；R-16 是唯一的代码级桥且无关系对象）。
- kb.memory↔一切分析链（自动生成侧缺位）。

## 3. 关系字典归属（pc.relationship_type）

| 事实 | 证据 | 判定 |
|---|---|---|
| 字典种子 12 条（0001:10280-10291）+ 0007 增 8 条，owner_domain∈{pc,kb} CHECK | 0007:36-37 | — |
| **owner_domain='kb' 共 2 行语义**：requirement-implemented-by-design（两端均非 PC 对象）+ design-constrained-by-adr / adr-documented-in（KB 域内边） | 0007:63-74 | **越位成立**：PC 表承载 KB 域关系确认权威（V2.0 §11 已列差异）；U18 裁决归属 |
| owner_domain 语义终裁=「该类型关系实例的确认权威域，非谁可创建」 | 0007:10-17 头注 | 审计时不得误读 |
| **写入侧零校验**：create_relationship_draft 不读 owner_domain（relationship.rs:219-310）；全仓读取仅测试与注释 | grep 证据 | 缺口 CONFIRMED（但该函数无生产调用者，当前不可达） |
| **full-dev 字典 0 行**：0007 已执行但清库重测清空种子 | `_sqlx_migrations` 含 0007；实测 count=0 | 运维缺陷：种子未与业务数据分级 |
| 端点隔离 DB 函数 pc.endpoint_in_scope | 0007:82-151 | 检索遍历双端隔离已就位 |

## 4. DomainRef 缺陷清单（供 decision-register 引用）

1. type 无注册表（D-REF-1）：任意 type 字符串可通过 validate()。
2. id/revision 无存在性与租户/项目校验（D-REF-2）：跨租户/跨项目裸引用不可被契约层拦截，仅靠各写入方自发检查（KB 有、PC 无）。
3. 任务列绕过 payload 校验（D-REF-3）：intake_task.signal_ref/result_ref。
4. 裸 UUID 清单：R-04/R-05/R-06/R-14 + pc.version_member；强 FK 仅 R-07/R-09/R-10。
5. TEXT 型四元组（pc.source.target_*）与 UUID 型裸列并存——同一系统三种引用形态（四元组 TEXT / 裸 UUID / FK）。
