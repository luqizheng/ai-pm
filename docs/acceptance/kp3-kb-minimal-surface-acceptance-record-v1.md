# KP-3 KB 三层最小面 · 验收记录 V1.0

| 项 | 值 |
| --- | --- |
| 日期 | 2026-11-08 |
| 批次 | V0.5 第四批收纳 · KP-3（范围基线 V1.3 §3.3/§五，WP-7 目标 4 主交付物） |
| 分支 | `feat/kp3-kb-minimal-surface`（基 master `be21542`；迁移→契约差异→端口→REST+注入→测试 五段提交） |
| 专用测试库 | Docker `aipm-kp3-pg`(16463) 干净库全量迁移 0001~0065（0065=本批，0064 归 KP-1）；并行会话（KP-1 / D-2=c / S02-I5-UI / ai-frontend）零干扰（全程 pathspec commit，未触碰 pc.* 与 `kb_pc_analysis.rs`） |
| 范围红线 | 未动 PC 17 表模型与分析写入面（KP-1 并行）；K1/K3/K6 与 P 系端点仍 501；知识检索全量面、S01-08 同步后置 V0.6 |

## 一、交付物与提交清单

| Commit | 内容 |
| --- | --- |
| `9e67a0e` | 迁移 **0065**（初号 0064，合入前因 KP-1 占 0064 顺移）：kb.source / kb.source_project（Original Sources，团队池统一身份 + 一源多项目）+ kb.memory / kb.memory_revision / kb.memory_ref（L1 Structured Memory 审核门 + 四元组引用）；编号注：0063 已由 D-2=c 批在 master 公开记账（91fe74e）→ 本批顺移 |
| `8271cda` | 契约差异说明 V1.0（D-KP3-1~8，先于端口改动落库；**端口方法签名零变更**） |
| `fca4f2f` | `pm_application::kb_service`（source/memory/retrieval/port）+ `PortError::Conflict/Internal` 补齐（D-KP3-1，对齐 V1.0 §12.1 错误码表）+ `KbServiceLive` 落真 K4/K5 |
| `d5de5a7` | REST：K4/K5 替换 `skeleton_501`（保持 require_project_visible 与「未接线≠404」先例）+ `kb_memory_api.rs` 管理面（D-KP3-5）+ `RunContract.discussion_memory` 注入（用户裁决 B，`AI_PM_DISCUSSION_MEMORY_INJECTION` 默认 off） |
| `e39ab40` | 测试：端口集成 11 例 + 注入 2 例 + REST 契约 3 例 |
| `eda2780` | clippy 清偿：kb_service::audit 8→7 参 + 类型别名 ×2 + master 存量 needless_borrow（ai_team_policies.rs:197） |
| `4f9d946` | 注入测试 clippy 清偿（失用助手移除 + 借引用比较） |
| `48fc816` | vendored digua-common-consul 钉 no_proxy（环境缺陷修复，D-KP3-9 / §九.2） |
| （本批） | 文档：迁移台账回填 0055~0065（含历史漏登补记；0064 行归 KP-1 PC-01B）+ D2 §4.2 `discussion_memory[]` 映射行 + 本记录 |

## 二、三层落点与目标 4 四职责对照（如实）

| 正式职责 | 本批兑现 | 落点 |
| --- | --- | --- |
| **资料管理**（Original Sources） | ✅ 最小面 | `kb.source`（内容不可改写触发器 + `UNIQUE(tenant, content_hash)` 团队池去重）+ `kb.source_project`（一源多项目）；REST 登记/详情 + K5 列表。**部分兑现**：文件类来源走 uri+声明 hash（拉取重算比对归 Phase 4 解析链）；对象存储直传未接 |
| **知识提取**（Structured Memory） | ✅ 提案-审核面 | `kb.memory*` L1 六类；AI/真人皆可提案，**审核仅真人**（K4，AI 主体 403）；approve/reject/revise 三动作；版本不可变 + SUPERSEDED 历史还原。**部分兑现**：自动提取（从资料产出记忆草稿）未建——本批的提取入口是人工/AI 提案 API；与 PC propose/confirm 链共用机制的目标形态未合并（零新建写入机制的裁决以独立 K4 门落地，差异已登记） |
| **讨论记忆**（Discussion Memory） | ✅ 最小面（裁决 B） | L2 原文零重复建设（留在 0060 `ai.conversation_message` / 0046 `task_interaction`，`kb.memory_ref` evidence 四元组指向）；L1 沉淀 → 真人审核 → 按 Task 检索注入（task + 关联需求主题，摘要级，开关默认 off） |
| **知识检索** | ❌ 未建（如实） | PageIndex 索引器未实施；本批只交付 `evidence_of` 来源回查与按 Task 的记忆检索两个窄读面，统一检索入口后置 V0.6 |

## 三、L1/L2 治理证据（fail-closed）

1. **审核门**：一切内容变化（CREATE/UPDATE/WITHDRAW）先 `PENDING_REVIEW`；**插入即带审核结论/版本态被库层触发器拒绝**；`APPROVED ⇔ version_status 非空`、`reviewed_by/reviewed_at` 同进同出（CHECK）；SQL 场景 14 项（G1~G14）在干净库验证 + Rust 侧 `l1_gate_db_rejects_unreviewed_version_and_mutation` 复验六种绕过全拒。
2. **版本不可变**：修订内容列 UPDATE 拒绝、行 DELETE 拒绝、审核结论单向一次、version_no 分配后不可改、状态迁移白名单（NULL→CURRENT|WITHDRAWN、CURRENT→SUPERSEDED）——G6/G8/G9/G11 逐项触发。
3. **单 pending / 单 CURRENT**：部分唯一索引；并发第二提案 409（应用层转译，G3 + REST 用例）。
4. **真人门**：K4 三动作 AI 主体一律 403（`requirement::require_human` 同款）；`reviewed_by` 落真人 principal；审计 `source_kind='kb_service'` 四身份列全写，`audit_actor_coverage`（C174/C180）清单登记 `kb_service/mod.rs` 并通过。
5. **版本语义**：批准同事务写 outbox `kb.memory.approved`（目录三字段，`validate_payload_references` 前置，幂等键 `kb_memory_approved:{revision_id}`）；同记忆重复批准的消费端去重瑕疵（`KbMemoryApprovedHandler` 按 memory_id 去重）属并行会话热点文件，只登记不改（差异说明 D-KP3-8）。

## 四、注入面（用户裁决 B，2026-11-08）

- `pm_get_run_context` 契约新增**可选** `discussion_memory[]`（memory_id/version_id/version_no/memory_type/title/summary/matched_by；无 body）；`knowledge_refs[]` 与其余字段原样。
- 开关 `AI_PM_DISCUSSION_MEMORY_INJECTION=off|on` **默认 off**，拼写错误告警按 off；off **不查询**、字段不序列化——契约输出与 KP-3 前逐字节一致（`get_run_context_with` 同租约双读测试：开关是两次读取唯一差异源）。
- 主题集合 = task 本身 + `task_requirement` 关联需求（实体级 subject）；命中只取 **CURRENT**（待审改写、被替代、撤回、其他项目同 id 主题均不进入——`on_injects_current_versions_by_task_and_requirement_subjects` 全覆盖）。
- 未命中 = 空集合/无字段；上层语义只能是「当前浅层记忆中没有找到相关记录」（定义稿 §5.3），测试断言与文档均按此口径。
- D2 契约 §4.2 补映射行；MCP 四工具签名与 `CONTRACT_VERSION` 零变更。

## 五、门禁证据

| 门 | 结果 |
| --- | --- |
| 迁移 | 干净库 0001~0064 全量应用成功 + 顺移 0065 后复放验证（aipm-kp3-pg 实跑；本迁移中途两次结构性修订均重放验证） |
| 端口/应用 | `pm-application` kb_service 集成 11/11、注入 2/2、`audit_actor_coverage` 2/2、contracts 16/16 |
| REST | `pm-api` kb_memory 契约 3/3（登记/审核生命周期/提案负向与撤回闭环） |
| 全量回归 | 最终基线见 §五.1（无白名单外新增红） |
| fmt/clippy | 本批文件 rustfmt 零 diff；`scripts/clippy.sh --all` 结果见 §五.2。**基线事实**：master 存量 11 文件 rustfmt 漂移（ai_team_policies / customers / files / tenant_members / discovery/web / project_import/lifecycle / review/pipeline / team_llm / team_onboarding / worker_command 等）——非本批引入，未代改 |
| 守卫 | `guard_ai_container_users` 绿（零 `ai.document*` 新增使用方）；事件目录 13 项断言绿（本批零新增事件） |

### §五.1 全量回归（最终数字，跑毕回填）

见文末「§五.1 回归结果」。

### §五.2 clippy --all（跑毕回填）

见文末「§五.2 clippy 结果」。

## 六、实施期偏差登记（设计未覆盖处的就地裁定）

1. **审核动作三元**：K4 契约 `approve|reject|revise`，`revise` 落为 `REVISION_REQUESTED`（退回关闭，另提新修订）；定义稿 §8.1 的 PENDING_REVIEW 状态词表吸收。
2. **撤回提案内容自描述**：WITHDRAW 不携带新内容，summary/body/引用原样承接自被撤回版本（被批准后 v_n=WITHDRAWN 行自身可还原「为什么撤回、撤的是什么」）；change_reason 记撤回理由。
3. **content_hash 口径**：记忆版本锚 = 规范化（type/title/summary/body + 排序引用四元组）sha256；来源 = `knowledge::content_hash` 同款裸 hex（服务端对内联计算、对 uri 校验声明）。
4. **平台对象引用域**：沿 Phase 1-3 偏差⑧挂 `requirement` 命名空间（D-KP3-6）；会话消息 evidence 限提案人自己的会话（属主隔离不泄露）。
5. **K5 evidence_of wire**：`<memory|memory_revision>:<uuid>`（D-KP3-4）。
6. **端口返回类型**：K4 沿骨架 `KbAnalysisStatus`（D-KP3-2，签名不动），REST 层出 §12.2 形状。

## 七、挂账（不阻塞本批验收）

| # | 事项 | 归宿 |
| --- | --- | --- |
| 1 | 文件类来源对象存储直传 + 拉取 hash 重算（§7.4 ADR-05 实测四项） | Phase 3/4 解析链 |
| 2 | 自动知识提取（资料→记忆草稿）与 PC propose 链合流 | V0.6（KP-3 后续批） |
| 3 | `KbMemoryApprovedHandler` 去重键 memory_id→memory_revision_id | 并行会话热点，登记 D-KP3-8 后改 |
| 4 | kb.source 登记走 Intake 受理链（K1）的形态统一 | Phase 4 KB 独立化评审 |
| 5 | MERGED 版本状态（合并流程） | V0.6（词表保留，无写入方） |
| 6 | 知识检索统一入口（PageIndex 消费） | V0.6 |
| 7 | `digua-common-consul` no_proxy 修复反哺 lmms 上游（SDK 通道） | lmms 仓下一批 |
| 8 | 测试容器命名卷+重启策略纪律推广（aipm-kp3-pg 配方） | 各会话自用配方 |

## 八、权威源

1. 定义与治理：`docs/product-cognition-knowledge-base/product-cognition-knowledge-base-definition-architecture-v1.md` §5/§6/§9。
2. 端口与所有权：`docs/architecture/knowledge-services/kb-pc-independent-services-architecture-v1.1.md` §7/§12/§13；`crates/kb-pc-contracts`。
3. 本批差异：`docs/architecture/knowledge-services/kp3-kb-minimal-surface-contract-deviations-v1.md`（D-KP3-1~8，含注入裁决记录）。
4. 范围：`docs/project-ai-team-v2/ai-pm-v05-mvp-scope-baseline-v1.md` §3.3/§五/§九。
5. 表结构：迁移 0048 / 0053 / 0060 / 0064（KP-1）/ **0065**（本批）。

---

## 九、实施期环境事故与处置（存档，2026-11-08 晚）

两起与代码无关的环境事故在门禁轮被捕获，均按既有配方恢复：

1. **Docker Desktop 重启杀容器**（KP-1 记忆「容器自灭假红」同类）：专用测试库 `aipm-kp3-pg`(16463) 随宿主 Docker 重启消失（匿名卷+无重启策略）→ 回归中途挂死/假红。处置：重建容器（命名卷 `kp3-pgdata` + `--restart unless-stopped`）→ 干净库重放 0001~0065（64 文件，0029 历史空号）→ 门禁重跑。
2. **v2rayN 进程退出暴露系统代理劫持**：reqwest 0.12.28 Windows 系统代理探测读取注册表残留 `ProxyServer=127.0.0.1:10808`（`ProxyEnable=0` 仍生效）→ `vendor/digua-common-consul` 全部 wiremock 用例 10061 拒连（v2rayN 活着时放行故基线绿）。处置：vendored crate `make_client()` 与全部测试 client 钉 `no_proxy()`（`openai_adapter.rs:58` 同款先例；用户现场裁决「全链 127.0.0.1 直连，不用 localhost」——wiremock 0.6.5 本就绑 `127.0.0.1:0`，无 ::1 分歧）。10/10 三连绿，套件 6.3s→0.12s（commit `48fc816`，差异说明 D-KP3-9，**挂账反哺 lmms 上游**）。
3. 顺带清偿 master 存量 clippy 漂移一处：`apps/pm-api/src/ai_team_policies.rs:197` needless_borrow（cfaa9c6 引入、clippy 1.99 新拦），随 `eda2780` 清偿。

## §五.1 回归结果

| 项 | 值 |
| --- | --- |
| 日期 | 2026-11-09（合入前终验） |
| 基线 | `c272488`（feat/kp3-kb-minimal-surface，rebased on master `5ecb0e3`；迁移终号 **0065**——0063 归 D-2=c、0064 归 KP-1 PC-01B，两度顺移） |
| 执行 | `bash scripts/cargo-test.sh` 全量模式（nextest `--no-fail-fast` + `cargo test --doc`）；专用库 `aipm-kp3-pg`(16463，显式 127.0.0.1)；template1 预建 64 迁移快档 |
| 结果 | **1214 tests run: 1214 passed, 0 failed, 4 skipped**；nextest 段 125.0s，全链 253s |
| 首轮记录 | 1213/1214——唯一红 = KP-1 `migration_0064` 收敛用例的 head 钉点（`head migration must be 0064`，被 0065 顺移合法推进）；修复 `c272488`（按版本号精确摘除 0064，不再按尾部截断 + head 断言更新 65）后套件 2/2 绿，终轮全绿 |
| consul 结论 | `digua-common-consul` registration 套件 10/10（no_proxy 修复 `48fc816` 后三连绿 + 本轮全量内含全绿）——上一完整轮的 5 红清偿闭合 |
| 白名单外新增红 | **0** |

## §五.2 clippy 结果

`scripts/clippy.sh --all`（-D warnings 全仓，独立 .target-clippy 目录）：**PASS**（修复 vendored consul 后复核轮 clippy4 复跑 PASS）。
