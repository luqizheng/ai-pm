# V0.5 Demo 演示脚本 V1.0（DK-F 收官）

| 项 | 值 |
|---|---|
| 适用栈 | 本机 DK-F 演示栈（`infrastructure/docker-compose.dkf.yml`）；.10 部署见验收记录 Stage 3 登记 |
| 前置 | pm-api/pm-worker/ai-pm-worker 镜像 `:dkf`（≥ a12e290）；pm_dkf 空库自动迁移 0001→0004；身份面 = .10 lmms（iam/token-service 可达）；DeepSeek API Key |
| 执行体 | 本脚本 §四 的自动化复跑器 `~/.dkf/evidence/dkf_full_run.py` + `s2_rerun.py`（全部 REST/psql，可逐步手工执行） |
| 开关 | `AI_PM_CONVERSATION_ROUTER=on`（pm-api）、`AI_PM_DISCOVERY_ORCHESTRATOR=on` + `AI_PM_CONVERSATION_REPLY=on`（ai-pm-worker，pm-worker 同值）、`AI_PM_RUN_DISPATCH=on` |
| 权威口径 | 设计 `docs/product/ai-pm-dialog-orchestration-design-v1.md` §8.1/§8.2/§8.3；验收 `ai-pm-dk-f-acceptance-record-v1.md` |

台词与操作一一对应；「预期」栏全部为 2026-11-09 真栈（DeepSeek deepseek-chat，非 fixture）实测。

---

## 一、准备（操作者，一次性）

```bash
cd infrastructure
export AI_PM_SECRET_KEY='<与 .10 一致的封私钥主密钥>'
export DK_F_FIRST_ADMIN_SUBJECT='<.10 登录 JWT 的 sub>'
docker compose -f docker-compose.dkf.yml up -d          # 空库 → pm-api 自动迁移 0001→0004
# 登录（首个 REST 调用会以首管理员对 JIT 开 active+owner）
curl -s -X POST http://192.168.2.10:3000/v1/iam/auth/login \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"Admin@123456"}'    # → access_token
# 注册并启用租户 Provider（DeepSeek）
curl -s -X POST http://127.0.0.1:18090/v1/pm/tenants/<tid>/llm-providers -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"name":"dkf-deepseek","provider_type":"openai_compatible","endpoint":"https://api.deepseek.com/v1",
       "configuration":{"model":"deepseek-chat"},"api_key":"<key>","timeout_secs":120}'
curl -s -X POST http://127.0.0.1:18090/v1/pm/tenants/<tid>/llm-providers/<pid>/status \
  -H "Authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"action":"enable","expected_revision":<rev>}'
# Provider 连通自检
curl -s -X POST .../llm-providers/<pid>/test -d '{}'     # → {"ok":true,"latency_ms":…}
```

## 二、场景 1：口述需求 → 澄清 → 起草 → 人类确认 → 分解 → 整批 ready（≈8 分钟）

| # | 台词 / 操作者动作 | 预期（真栈实测） |
|---|---|---|
| 1 | 建项目：`POST /v1/pm/projects {"code":"DEMO-1","name":"Demo Shop",…}` | 201；调用者自动成为 Lead（owner） |
| 2 | **（用户）**对话框发首条消息：`POST …/conversations` + `POST …/conversations/{cid}/messages {"content":"本项目需要手机登录"}` | **201，回包 `landed:"interview"`**、`handoff.trigger:"implicit_intent"`、`intent:"requirement_intake"`、`notice:"已切换到需求澄清"`、`undo_until`（30 分钟）；`discovery_session_id` 已置 |
| 3 | 验证 A1：查 `ai.conversation_message` 与 `discovery.discovery_message` | chat 表**只有一条** system 边界消息（「已切换到需求澄清」，`metadata.handoff`），**无**需求句重复行（无双写）；discovery 表 seq1=交接摘要、seq2=用户原话 |
| 4 | 等 30~60s（第八循环 + DeepSeek） | 会话 → `NEEDS_USER_INPUT`；落 1 条 `user_confirmed` finding（槽位标签 `slot:core_behavior`）+ 3 个 open 问题（`why_blocking` 必填） |
| 5 | **（用户）**逐轮回答：REST 答题 + 线程里继续发口径（六槽文本，见复跑器 `SLOT_TEXT`）；每轮后等编排器回应 | 每轮 `landed:"interview"`（A2：不回落 chat）；findings 按槽位累积去重；问题去重 |
| 6 | 三条件全绿 | 会话 → `REVIEW_REQUIRED`（本轮 questions=0、blocking=0、六槽全绿） |
| 7 | **（人类）**裁决 findings：逐条 `POST …/findings/{fid}/verification {"status":"CONFIRMED","answer":"人工复核确认…"}` | 全部 `CONFIRMED`；此步之前**不会**起草（写回硬门：只认 CONFIRMED） |
| 8 | 等 ≤60s（写回扫描面 sweep_write_back） | worker 日志 `requirement draft submitted for human confirmation`；需求落库 **`pending_confirmation`**、`basis_finding_ids` = CONFIRMED 全集、markdown 已绑 Git（`markdown_version_ref` 非空）；同轮 B5 生效：`task decomposition … is not approved — 请先在需求页确认该需求（approved）后再生成任务` |
| 9 | **（人类）**一键物化 basis：`POST /v1/pm/requirements/{rid}/basis/materialize {"finding_ids":[…],"register_source":true,…}` | 201：finding 全部挂链 + `source_input` 登记（S02D-20 人类唯一登记人） |
| 10 | **（Approver）**确认：`POST …/revisions/{rid}/confirm {"expected_content_hash":"…"}` | 200；修订 → **`approved`**（旧 approved→superseded）；`criteria_drift:false` |
| 11 | 等 ≤60s（写回分解） | worker 日志 `task batch drafted`：**draft 任务 ×8**（前置 ×3 + 主 ×5），每条带验收口径；超限截断/角色越界丢弃计数可见 |
| 12 | 验证 E1/E3 | DB：`task_requirement` 8 行全部钉 **approved** 修订；依赖 9 边落 `task_dependency`（非标题字符串） |
| 13 | **（人类）**整批确认：`POST /v1/pm/projects/{pid}/tasks/mark-ready {"task_ids":[…8]}` | 200 `{"ready":[×8],"skipped":[]}` |
| 14 | 验证 E5 + 彩蛋 | `integration.outbox_event` `TaskReady` = **8**（=确认数）；`delivery.task_run` queued=8（入调度；本栈 executor=noop，仅演示入队） |

## 三、场景 2：导入 → 分析 → 自动访谈 → 注入问答 → 更正（≈6 分钟）

| # | 台词 / 操作者动作 | 预期（真栈实测） |
|---|---|---|
| 1 | **（操作者）**把演示仓库放入卷：`docker run -v infrastructure_aipm_dkf_demo_repo:/repos … cp demo-shop`（仓库含 README：认证写 JWT + `src/api.js` 等） | — |
| 2 | 关联仓库：`POST …/repositories {"name":"demo-shop","url":"git@demo.local:team/demo-shop.git"}`，再把库内 url 改 `file:///repos/demo-shop`（REST 白名单只收 ssh 形态，仓内测试同款做法） | 201 |
| 3 | `POST /v1/pm/repositories/{rid}/verify` | `verified` + head sha（真 `git ls-remote file://…`） |
| 4 | 等分析链（worker bare clone → repo_facts → KB digest LLM 增强 → `kb.analysis.completed`） | `kb.document_revision` **`analysis_mode='llm'`**（真 DeepSeek）；PC 对象落库 |
| 5 | DK-E 自动开访谈：等 10~30s | 出现 **`INITIAL_ONBOARDING`** 会话（source=SYSTEM）；低置信/触顶派生问题（`analysis_mode!=llm` 时） |
| 6 | **（用户）**显式进入访谈发首条：`{"content":"帮我了解下这个仓库…","mode":"interview","discovery_session_id":…}` | `landed:"interview"`；编排器按 S1~S6 注入作答（引用 README 内容）+ 按 INITIAL_ONBOARDING 五槽提问 |
| 7 | **（用户）**回答背景（REST 答题 + 线程陈述） | `user_confirmed` findings 落库（slot 标签） |
| 8 | **（用户）**注入式问答：「这个项目的认证是怎么做的？」 | 回答**引用 KB digest 小节**（JWT/Bearer/Redis TTL 逐条），并诚实标注「这是文档描述，不是实现事实」（D3/D4 纪律） |
| 9 | **（用户）**更正：「不对，README 写的 JWT 已过时；实际是 session cookie：服务端保存会话、客户端只带 session id、30 分钟失效」 | **F1**：`contradicted` 行（evidence `contradicts:{旧id}`）+ `user_confirmed` 行（更正后事实），**旧 finding 原样保留（PENDING，零改写）**；落 **`[更正待知识应用]`** deferred 问题（why_blocking=NULL，携带被更正对象 doc_key，如 `repo:…:module:README.md`）；**F3**：`discovery_proposal` = **0 行**，`kb.*`/`pc.*` 零变化（F2，DK-E 裁定口径）；仍有 open 阻塞问题时会话**不**跃 REVIEW_REQUIRED（C1） |
| 10 | **（人类）**裁决：更正相关 finding `CONFIRMED` / 旧行 `REJECTED`；知识应用演示=经 `POST /v1/kb/memories` 人提一条「认证方式=session cookie」讨论记忆 → K4 `POST /v1/kb/memories/{id}/review` approve | KB 侧走**人工记忆评审**落地（K4 评审 `kb.memory_revision`）；digest 重写与 PC 对象更正属 KP-3/KP-2 续作（第 0 步核实：标记消费链=只写+可查看，不虚构） |

## 四、自动化复跑器（一条命令重放两场景）

```bash
python ~/.dkf/evidence/dkf_full_run.py     # 场景 1 + 场景 2 + A 组探针（id 自动派生）
python ~/.dkf/evidence/s2_rerun.py         # 场景 2 定向复跑（等待条件修正版）
```

证据 JSON 落 `~/.dkf/evidence/full_run_*.json` / `s2r_*.json`；验收记录引用。

## 五、失败语义演示（可选加分项）

| 动作 | 预期 |
|---|---|
| 停用 provider 后发需求句 | `landed:"chat"`（A3 fail-closed）：不建会话、不写 finding；线程链接 NULL |
| 升级后 30 分钟内 `POST …/conversations/{cid}/handoff/undo` | session → `PAUSED`、线程链接回退 NULL（A4） |
| 访谈 COMPLETED 后同线程发纯问答 | `landed:"chat"`（A6，不新建会话） |
| AI 起草后未确认即强行分解 | B5 拒绝文案指向「请先在需求页确认」 |
