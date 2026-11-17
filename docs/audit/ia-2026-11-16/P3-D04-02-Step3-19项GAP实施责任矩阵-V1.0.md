# P3-D04-02 Step 3 — 19 项 GAP 实施责任矩阵 V1.0

- 日期：2026-11-17
- 口径：活跃 19 项（总账 V1.1 §一；§六清单勘误后含 GAP-32）= 裁决齐备可实施 17 + 待 B 级裁决 2（GAP-22/GAP-32）
- 〔增记 2026-11-17〕GAP-31 键来源已经《显式补发布键来源裁决归档记录 V1.0》裁定=服务端确定性派生；31-A 验收定稿（裁决齐备），31-B 缩窄为 wire 命名+校验实现位置（Q-06 B 级维持）
- 代码事实列均经本轮独立复核（非仅转引）；行号=当前 master 实测
- 实施授权状态：**全部未授权**；本矩阵仅为实施前归属界定

---

## 一、逐项矩阵

| GAP | 当前代码事实（CODE/SCHEMA FACT） | 目标设计要求 | 缺失/不一致 | 实施落点（唯一归属） | 依赖 GAP | 依赖人工裁决 | 验收标准 |
|---|---|---|---|---|---|---|---|
| **01 起草段账本** | 起草/提交无账本行：write_operation 仅发布段使用（command CHECK 三值 0001:3470） | Q-01 方案 A：propose/submit 账本登记，同事务 all-or-nothing | 分析链/身份链调用点零登记 | `pc/analysis.rs`(propose_from_analysis) 与 `pc/identity.rs` 调用点插账本写（projc 镜像同） | 27（CHECK+指纹列） | 无（Q-01 已裁） | 同 op_id 重放幂等返回既有 proposal；中途失败账本留痕可续 |
| **02 身份链幂等** | identity.rs 三段独立事务、无 slot 键、无 round_id（E-22） | op_id=`{pc}-identity:{round_id}:{slot}`，round_id 调用方生成透传 | 无任何幂等锚（同轮重跑=重复提案） | identity.rs+调用方 round_id 透传+键构造；slot 校验随 27 校验器 | 01/27 | 无 | 同轮重跑零重复提案；异 slot 异键各自成立 |
| **04 revision_no DB 兜底** | 行锁推号在位；(object_id,revision_no) 唯一约束缺位（C-1） | DB 层兜底 | 无约束 | 新迁移加 UNIQUE(object_id,revision_no)（pc+projc cognition revision 表） | 无 | 无（C 级） | 并发首插冲突被 DB 拒绝并映射 409 |
| **05 decision_reason 强制化** | decide reason=Option+trim 后可空（pc/proposal.rs:1041-1046） | 决策理由必填 | 可空 | decide 入口（pc/proposal.rs:1041 同族，projc:1039 镜像）必填校验 | 无 | 无（C 级；与 expired_reason=GAP-36 平行不同字段） | null/空/纯空白拒绝（Invalid） |
| **06 proposal 转换 DB 约束化** | proposal.status 无触发器守卫（仅 approval/version/object/write_op 有，E-07 清单） | T2 全局 allowlist 规则 3：DB 锚列触发器兜底 | 状态转换纯服务层守卫 | 新触发器（按 §D1.2 allowlist 拒绝非法转换，纯拒绝无推进逻辑） | 无 | 无（C 级） | 非法转换直写 DB 被拒；合法 9 转换不受影响 |
| **09 审批/expire 入口接线** | decide/publish 无 HTTP 路由（ports.rs:361-392 NotWired；仅 close 有 POST） | A-2：NotWired≠设计不完整，接线归 Schema/API 阶段 | 无外部入口 | pm-api 路由+PcServicePort/ProjcServicePort 实装（去 NotWired） | 35/36 收敛后接线（建议顺序，非硬依赖） | 无（A-2 已定性） | 真人 A3 可经 HTTP decide/expire；AI 恒 403 |
| **10 draft 修改入口+重锚定** | V1 无修改入口；外部改动由 submit 时 hash 重算失配 409 拦截（fail-safe 在位） | 修改产生新锚定（重算+更新 content_hash） | 无入口 | 新服务命令（成员增删+重锚，同事务） | 无 | 无（C 级） | draft 期修改后 submit 不触发 fail-safe 409 |
| **11 draft upsert 审计动作** | 草稿物化无独立审计（仅 created_by/updated_by 留痕） | `pc_object.draft_upsert`（projc 同）审计动作 | 无审计行 | object.rs upsert_object_draft 加 append_audit | 无 | 无（C 级） | 每次草稿落库有同事务审计行 |
| **21 对账巡检** | 无巡检（原 Q-09 移入 C 级） | 只读运维差分（completed 账本×version 存在性） | 无 | 运维面只读任务（不属本批服务代码） | 无 | 无（C 级） | 差分报告可产出；零写路径 |
| **22 账本保留/清理** | completed 行永久保留 | 保留策略 | 无策略 | —— | 无 | **Q-07（B 级未裁）→ 不可实施** | 待裁决后定 |
| **23 分析链/身份链事件幂等** | 分析链已满足（analysis_outcome_key+写侧去重，E-22/E-28）；身份链缺失 | 两链均有幂等 | 身份链半边 | **全部归 02 实施**；23 无独立实施条目（总账口径维持） | 02 | 无 | 随 02 验收 |
| **27 账本扩展实施** | command CHECK 三值（0001:3470）；无指纹列/slot 列；UNIQUE(tenant,project,operation_id) 在位（0001:5746；projc=0026）；**发布段取证（V1.2 勘误）**：completed 短路不比对指纹+expected_content_hash 仅占位串（publish.rs:114/:147-163） | Q-01+Q-04：新命令入 CHECK+九要素指纹列+slot 结构/匹配校验+malformed fail-closed+起草段重放+默认归一+**发布段严格指纹判定（同 op 同 fp 确认+异 fp 409）** | 全部缺位 | 新迁移（仓库惯例=追加编号迁移）扩 pc+projc write_operation CHECK/列；校验器函数（op_id 解析+slot 匹配） | 无（被 01/02/30/31 共用） | 无（Q-01/Q-04 已裁） | D6.6 V1.1 场景矩阵 11 行全过（含场景 10/11）；异指纹同 op_id 409（含发布段） |
| **29 PC 侧冲突用例** | conflict 冻结/解冻测试仅 projc 侧（conflict_freezes_and_resolves） | PC 镜像逻辑回归（pc.conflict=Legacy 空集，测门禁代码非数据） | PC 用例缺 | tests/ 新增 PC 冲突门禁用例（镜像 projc 同构） | 无 | 无 | PC 侧三落点门禁用例绿 |
| **30 起草段幂等用例** | 无（起草段无账本） | GAP-01 配套测试 | 无 | tests/ 起草段重放/失败续行/异指纹 409 用例 | 01/27 | 无 | 同 op 重放/异指纹 409/部分失败续行用例绿 |
| **31 发布键固定校验** | close 链派生键在位（analysis_report.rs:599/663=CF）；publish_version 入参任意串无校验（PublishInput.operation_id；测试传 "op-publish-1" 等任意值） | §D6.5+裁决归档 V1.0：确定性派生（**全部发布入口，含显式补发布**）+非规范键入口 fail-closed 拒绝 | 入口校验器缺位 | **31-A**：publish_version 入口键格式校验（非 `{pc\|projc}-decision:{uuid}` 拒 INVALID_CONTRACT，close 链回归不变；**含 operation_id 入参内化+测试改造**）；**31-B**：wire 命名+校验位置 | 27（校验器共用） | 31-A=无（键来源已裁）；**31-B 待 Q-06（B 级）** | 31-A=任意键入口被拒+close 回归绿（**已定稿**） |
| **32 close/decide 竞态** | approved 清单选取无 FOR UPDATE（analysis_report.rs:586-591）+显式补发布兜底 | 竞态处置政策 | 政策未裁 | —— | 无 | **Q-10（B 级未裁）→ 不可实施** | 待裁决后定 |
| **33 R38 时间字段分离** | proposal 仅有 submitted_at；无 expired_at（0001:3240-3264） | expired_at=决策事实时间独立列（≠updated_at 流程时间） | 列缺位 | **随 GAP-36 迁移同批落列**（无独立实施条目） | 36 | 无 | expired_at 与 updated_at 语义分离（R38 复核） |
| **35 决策终局语义** | decide 守卫允许 submitted/approved/rejected 三态互转（pc:972）；跨态写点=pc:1064/projc:1062 各一处（E-51 清点 12 UPDATE 唯二）；测试 decide_is_provisional（pc_proposal.rs:1432）断言互放行 | Q-02b+Q-11：仅 submitted→approved/rejected+同态幂等；跨态双向 409 | 守卫过宽+测试过时 | decide 守卫收敛（:972 改 submitted-only+同态分支保留）+projc 镜像+**decide_is_provisional 测试改写** | 无 | 无（Q-02b/Q-11 已裁） | 跨态 8 组合全 409；同态幂等返回原 receipt；终态/非裁决态 409 |
| **36 expire 触发与承载** | 零实现：无 setter/无 expired_at/expired_reason 列/无入口（grep 证实 pc/projc 无 expired 写路径；状态 CHECK 含 expired 值域） | Q-12 模式 A+Q-03：A3 真人 fail-closed+reason 必填+四写同事务+D4.10 八保证 expire 半边 | 全部缺位 | 新迁移（列）+expire 服务命令（复用 ensure_formal_authorized，pc/mod.rs:152 同款）+同事务审计 `{domain}_proposal.expire`+测试矩阵 | 33（同迁移）；09（接线） | 无（Q-12/Q-03 已裁；reason 长度=可选确认项） | 专项报告 §二 12 问逐项+并发测试矩阵全过 |

## 二、专项检查结论（任务书 §二 特别检查项）

| 检查 | 结论 |
|---|---|
| GAP-01 vs GAP-27 重复/遗漏 | **无重复**：27=账本机制（CHECK/指纹列/校验器=表结构+共享函数）；01=起草段调用点接线（分析链/身份链插账本写）。指纹九要素定义归 27，键构造（业务锚格式）归 01 按已冻结的 D6.6 §4 规则执行。遗漏=无 |
| GAP-02 vs GAP-23 幂等边界 | **清楚**：23=分析链（已满足，无实施条目）；身份链全部归 02。23 在 19 项活跃中的存在仅为「半边活跃」记账口径，实施对象唯一=02 |
| GAP-29 vs GAP-30 测试归属 | **明确**：29=PC 冲突门禁镜像用例（独立）；30=起草段幂等用例（随 01/27 验收）。无交叉 |
| GAP-35 跨态禁止覆盖所有写路径 | **覆盖**：E-51 全量 12 UPDATE 中，submit（draft-only）、withdraw（draft/submitted）、close withdraw（draft/submitted 清单）、publish failed（approved+CAS 失配）、publish applied（approved 守卫）均结构上不可能产生跨态翻转；唯一跨态源=decide :1064/:1062。收敛该守卫即全覆盖（另 GAP-06 触发器作 DB 兜底） |
| GAP-36 expire 完整性 | 12 问逐项见《GAP35/GAP36 专项报告》§三：全部可实施，无缺口；唯一待确认=reason 长度（可选项，默认不限） |

## 三、批次依赖图（授权后实施序）

```
27(账本机制) ──→ 01(起草段接线) ──→ 30(起草段测试)
 │              ↘ 02(身份链)      ↘ 29(PC 冲突用例, 独立可并行)
 └─→ 31-A(发布键校验, 待 I-1 收口定稿验收)
35(decide 收敛+测试改) ── 独立可先行
36(expire 命令+列+审计+测试) ←─ 33(同迁移落列)；09(接线) 建议在 35/36 后
31-A(发布键校验) ── 键来源已裁(归档 V1.0)，验收定稿、裁决齐备
22/Q-07、32/Q-10 ── 待 B 级裁决，不进本批
```

## 四、零实施声明

本矩阵为文档交付：未修改代码/Schema/Migration/API/测试/数据库；实施未授权。
