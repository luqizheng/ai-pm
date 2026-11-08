# KP-1 — PC-01B 正式数据模型实施验收记录 V1.0

| 项 | 值 |
|---|---|
| 批次 | KP-1（V0.5 范围基线 V1.3 §3.3 第四批：PC/KB 应用完善） |
| 分支 | `feat/kp1-pc01b-data-model`（worktree `ai-pm-kp1`，基 350f748） |
| 提交 | migrations → 服务层 → 链路适配 三批 pathspec commit（54c8f95 / pc 服务层 / e103492 + 本文档批） |
| 设计权威 | `docs/architecture/product-cognition/pc-01b/` 七主题文件 V1.2（C-1~C-4 已采纳） |
| 日期 | 2026-11-08 |

---

## 一、用户裁决（本批输入）

1. **0053 存量处置 = 原地收敛保留**（2026-11-08 AskUserQuestion）：ALTER + 回填，AI 草稿全保留；同迁移对全新库（0053 空表）自然退化。
2. **KP-1/KP-2 边界 = KP-1 含发布服务**：§18.1 十四步 + ⑫b outbox 作为服务层函数落地（无端点、AI 不可达）；提案 create/submit/approve/reject 命令、p4/p5 REST、UI 与分析→提案接线归 KP-2。

## 二、交付物

| 层 | 内容 | 位置 |
|---|---|---|
| 迁移 | `0063_pc_formal_model.sql`：pc 正式 18 表（01B §15.2 全清单）+ 0053 五表原地收敛 + 触发器/种子/闭合约束 + `pc.v_revision_versioned` 视图；台账取号 0063（取号时目录 max=0062），台账已回填 | `migrations/0063_pc_formal_model.sql` |
| 服务层 | `pm-application::pc/` 四文件（mod/object/relationship/evidence/publish；mod.rs 再导出公共 API）：对象草稿 upsert（object_key 幂等 + 内容寻址 + 行锁推号 + CAS）、identity 占位、关系端点解析/类型合法性/跨域通道、来源/证据登记、发布事务 §18.1 + 规范哈希 | `crates/pm-application/src/pc/` |
| 链路 | `kb_pc_analysis::derive_pc_cognition` 走服务层（V-6）；pc_analysis_ref 改锚 pc.source；`pc.cognition.version_published` 生产者（⑫b）+ 消费者（Platform 留痕）接真，事件目录 has_handler 翻真；P3 新形状 + legacy result_ref 兼容 | `crates/pm-infrastructure/src/kb_pc_analysis.rs`、`kb_pc_handlers.rs`、`apps/pm-api/src/knowledge_services.rs` |
| 测试 | `migration_0063.rs`（fresh 负向 + 收敛回填两用例）、`pc_model.rs`（五用例）、kb_pc_repo_flows/intake_flows/audit_actor_coverage/outbox 套件适配 | 各 tests |

## 三、PC-I1/I2 验收条目对照（09 稿 §17）

**PC-I1 认知骨架**

| 验收条目 | 兑现 | 证据 |
|---|---|---|
| 不依赖大模型 | ✓ | 服务层纯结构化 SQL；analysis_mode 仅归因，不参与判定 |
| 人工能够建立基础 Product Plan | ✓（数据面） | `create_object_identity`（骨架）+ `upsert_object_draft`（草稿）；REST/UI 入口归 KP-2 |
| 对象具有稳定身份 | ✓ | UUID PK + `UNIQUE(tenant,project,id)` + object_key 项目内幂等 + 复合 FK 锚；18 类种子（14 正典 + 4 snapshot 偏差 b） |
| 基础对象可查看、编辑和组织 | ✓（数据面） | 查看=P3/SQL/视图；编辑=upsert 推号（终态拒绝复活）；组织=object_type 层级字典 |

**PC-I2 关系与来源**

| 验收条目 | 兑现 | 证据 |
|---|---|---|
| 认知对象关系 | ✓ | `create_relationship_draft` + 12 关系类型种子（17 矩阵行折叠 §15.5）+ 端点快照自包含（§6.4） |
| 外部 Requirement 引用 | ✓ | `specified_by/constrains/informs` → `requirement.requirement_revision` 四元组；服务层组合键存在性校验 |
| Design / ADR 引用 | 按设计未启用 | 通道状态表 NotYetOpen（01B §13.1 V1 后续跨域集成）——服务层显式拒绝，非缺陷 |
| Project Goal 支撑关系 | ✓ | `supports` → `project_goal.project_goal`；doc_key='project-goal' 存在性探测 |
| Source Evidence | ✓ | `register_source/register_evidence`（判别器 CHECK + 主体二选一 + uq_pc_evidence_identity 含 commit_sha 偏差 d） |
| Requirement Revision 关联依据 | ✓（Q-PC-03） | Revision 级引用必填（服务层 + 库层通道收窄 CHECK 双保险） |
| 关系可以查看和追溯 | ✓（数据面） | RR 端点快照 + content_hash + append-only；evidence 反向索引 idx_pc_evidence_subject |
| 来源可以定位 | ✓ | 结构化四元组列 + section_anchor/start_line/locator + source_content_hash |
| Requirement 关联保留历史 Revision | ✓ | 端点/来源均钉 revision_id（UUID↔TEXT 桥接），身份与修订双记录 |
| 基础 Product Map 读取对象及关系 | ✓（数据面） | 版本成员集 + 关系端点可查；UI Map 归 PC-I4 |

## 四、偏差登记（全部显式裁决/默认，迁移头注同源）

| # | 偏差 | 依据 |
|---|---|---|
| a | PCGIT 正文权威后置：`pc.document_revision.content` 暂存 PG、`git_commit_sha` 本批恒 NULL；`pc.write_operation` 仅发布幂等用（账本状态机字段按设计建齐） | 范围基线 V1.3 §3.3 将「KBGIT/PCGIT 正文权威」显式切出 V0.6 |
| b | checkpoint 四派生类型保留：repository/tech_stack/delivery_activity/module 以 `layer='snapshot'` 入字典（14 正典 + 4 = 18 行） | P2-13 裁决 D-2=A「对象形态不变」；01B §5.1 新增类型走种子迁移同款受控路径 |
| c | `analysis_mode` 列保留且放宽可空（0062 列）：分析产物带 derived/derived_fallback/llm，人工草稿 NULL（语义真值） | 0062/D-3=A；非分析产物不应默认误标 derived |
| d | `uq_pc_evidence_identity` 增 `coalesce(commit_sha,'')`：01B §9.3 原式在同 subject 多 commit 证据下撞键 | commit 类证据身份必须含锚 |
| e | doc_key 兜底形态 `pc-object-{object_id}`、canonical_path `pc/{object_type}/{object_id}.md`（0053 存量 object_key 含 `:`/`/` 非 §11.2 slug） | §11.2-3 兜底条款 |
| f | P3 响应形状变化（显式登记）：`objects[].name`=display_name（键名不变）、`latest_revision.status` 移除（`versioned` 布尔派生，§5.3 不冗余 revision_status）、`summary` 改由 change_summary 承载、新增 `analysis_mode`；pc_analysis_ref 改锚 pc.source；旧任务 legacy result_ref 走兼容分支 | K2/P3 契约变化须显式登记（任务书要求） |
| g | ADR-06b 时序闸落实：PC 正文自 0063 起域内闭环（pc.document\*），PC 对 `ai.document*` 零新增引用（`guard_ai_container_users.rs` 守卫持续断言）；KB 域 kb.\* 本批零触碰 | 六问的 PC 侧引用点结论先行于 Migration |
| h | 视图 `pc.v_revision_versioned` 实现勘误：01B §15.3 片段 LATERAL 仅 SELECT 1 致外层引用解析失败，迁移内改为 LATERAL 显式投影该列（语义不变） | 迁移 0063 内注记 |
| i | CAS 失配补偿归并主事务（§18.1 ② 伪码「proposal → failed; COMMIT」）：独立事务补偿与 proposal FOR UPDATE 行锁自死锁，实施期发现并修复 | 锁序纪律 §17.3 |
| j | project_goal 端点存在性探测不落地：01B §13.1 原文的「ai.project_document 探测」与 ADR-06a 守卫冲突（C-1：PC 禁止接入共享容器，过渡期规约 §7.1-1）——守卫为准（用户裁决），本批仅通道校验；探测归后续 goal 域只读接口（K3/PK1 同款模式）。守卫测试在实施期真实拦截一次（含注释字面量），已按其口径修正 | guard_ai_container_users（ADR-06a 立法） |

## 五、测试与门禁证据

- **迁移**：`migration_0063` 两用例——fresh 路径（01B §20 负向子集：T-03/04/06/08/13/14/15/26/27/28 + 种子清单 + 终态/append-only/账本状态机）与收敛路径（0053 形态存量 → 正文迁移/hash/证据钉修订/来源收窄/对象指针全链断言）。
- **服务层**：`pc_model` 五用例（对象幂等/去重/推号/终态、关系端点规则全集、来源证据、发布 happy path【成员集+端点自动并入+snapshot_hash 一致性+生命周期+审计+outbox+账本+幂等重放】、门禁矩阵【AI 403 / 非授权 403 / L3 冻结 403 / CAS 409+提案 failed+账本 failed】）。
- **链路回归**：kb_pc_repo_flows 5/5（全链 verify→intake→KB→PC、同 HEAD 幂等、新 commit 新 intake、LLM 增强/降级）、intake_flows 全量、outbox 三套件、pm-ai 79、pm-api intake REST 7。
- **审计门禁**：C174/C180 清单驱动测试补登记 `pc/publish.rs`（2 处：pc_version.publish 四列发布者身份；publish_failed 系统面四列显式 NULL）。
- **checkpoint-1 复跑证据**：同 HEAD 幂等短路 = `reverify_same_head_is_idempotent`（intake 计数=1 + KB/PC 零新增）；新 commit 新 intake = `new_commit_advances_revision_with_content_dedup`（KB rev2 锚新 HEAD、snapshot 追加修订、tech_stack 内容寻址去重、evidence 5→10）。真栈 E2E（`scripts/checkpoint1-e2e.sh`）待 pm-api 镜像重建后拉起（与 S02-I5 同一待办），本批以套件级证据交付。
- **fmt/clippy**：改动面单文件 rustfmt；`cargo clippy -p pm-application -p pm-infrastructure -p pm-api --all-targets` = 0 告警（含借道清偿 `ai_team_policies.rs:197` needless_borrow 一处跨批存量 lint，语义零变更）。
- **ADR-06a 守卫**：`guard_ai_container_users` 全绿（19/19）——PC 对共享容器零引用（守卫曾拦截 relationship.rs 探测与注释字面量，均已按守卫口径修正，见偏差 j）。
- **代理免疫**：`scripts/cargo-test.sh` 增 loopback 恒免代理（NO_PROXY 追加 localhost/127.0.0.1/::1）——本机系统代理（v2rayN）开启时 reqwest 劫持 loopback 致 digua wiremock 五例连红（lmms 侧同款坑家族），修复后隔离复跑绿。
- **全量回归**：`bash scripts/cargo-test.sh`（nextest 全量）**1179/1179 全绿 + doctest 绿，EXIT=0**（2026-11-08 终态；基线同容器 1169/1170，唯一红 c23 为编译负载时序 flake、隔离双跑绿——本批终态全绿含该例）。

## 六、遗留与移交（KP-2 输入清单）

1. 提案 create/submit/approve/reject 命令服务（复用 `ProposalMemberPin`/`canonical_proposal_hash`/端点自动并入构建器——已在 pc::publish 公开形状内）；p4/p5 REST + ensure_trusted_context；UI 确认入口。
2. Profile/Vision 项目初始化接线（`create_object_identity` 已备，未挂 project creation hook——挂接点在 project 域共享文件，归 KP-2 一并处理）。
3. PCGIT 正文权威与 write_operation Git 两态（V0.6，随 ADR-06b 框架）。
4. 真栈 E2E 复跑（pm-api 镜像重建后 checkpoint1-e2e.sh；provider 步已备）。
5. 跨域引用周期对账（R-13；数据源=K3/PK1 只读 API）与 reconcile 后台任务（01B §12.5）——V1 范围外。
