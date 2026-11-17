# 08 — P3-11 落地约束证据（F 节）

**证据基准**：master `e05693afdc0a9ae534735c9b6f67117a6b92de8d`。本包只读取证，未修改任何文件。
**文档简称**（均在 `docs/audit/ia-2026-11-16/`）：D1=裁决稿 V1.1；D2=基线校准与裁决台账 V1.0；D3=T1T2T3 交付报告 V1.0；D4=验收补充报告 V1.0；D5=架构裁决与实现一致性核查报告 V1.0；D7=close 契约影响核查 V1.0。

## 1. 最终冻结设计与正式裁决记录（F.1）

- **架构基线正式冻结令（2026-11-17，用户下达）**：D2 §10.8，冻结类型=「**有边界的条件冻结（Bounded Conditional Freeze）**」。三节效力链：「§10.6 工程交付验收、§10.7 正式裁决、§10.8 冻结令三者依次成立」（D2 L334）。
- **正式冻结范围（12 项，D2 §10.8 L343-356 逐字）**：
  1. PC / ProjC 双域正式认知权威边界；
  2. ProjC 独立 `projc.*` schema 及来源语义约束；
  3. 仓库分析认知分流判据、模块消歧及候选认知原则；
  4. 有界过渡模型及过渡期间行为约束；
  5. `delivery_activity` 不进入 ProjC 正式认知字典；
  6. ProjC 独立 Conflict 模型及 `pc.conflict` Legacy 处理原则；
  7. DomainRef 四字段契约及其与 `relationship_type.owner_domain` 的职责分离；
  8. 统一检索三层架构原则；
  9. close → publish 双域分流原则；
  10. 历史 snapshot 的 Legacy Transitional 处理原则；
  11. 共享技术基础设施不得产生共享业务权威；
  12. 跨域引用不构成跨域授权，以及跨域权限闭环原则。
- **边界条件**：O-01~O-09 九项开放项**不得因冻结视为完成**（D2 §10.8 L360-368）；「R-04④ 保持 Partial…R-06 的 Conflict 保持 IMPLEMENTED…R-07 的引用完整性细则仍待 P3-D04 闭合；统一检索原则已批准，不代表统一检索实现已经完成」（L370）。
- **冻结效力**：成为后续设计与实施正式基线；P3-D03/D04/D05/P3-10 必须遵循；修改须「提交变更理由/影响分析/重新裁决」（L372-378）。
- **R-01~R-10 正式裁决（D2 §10.7）**：7 批准（R-01/02/03/05/08/09/10）+3 修改后批准（R-04/06/07）+0 不批准。关键条目：
  - R-02（双域权威边界）五条边界（D1 §1 L38-44 冻结表述）：①不共享正式知识对象身份；②不共享未经定义的状态机；③不允许一个领域直接修改另一个领域的正式认知；④不允许两个领域同时成为同一知识正文的权威；⑤跨域关联只走引用契约。总原则「共享数据库基础设施 ≠ 共享领域权威」。
  - R-06（修改后批准，D1 §6 L177-189）：ProjC 自建 projc.conflict；**pc.conflict=Legacy**（留表不解冻 L3 守卫、不手工插数、注册表 Legacy/Not for New Use、不改名不伪装迁移）；归属原则「冲突对象的所有权由主要冲突认知所属领域决定；涉及其他领域时通过 DomainRef 引用对方对象，双方各自履行确认责任」；暂不新增 Global Conflict 域。
  - R-07（修改后批准）：DomainRef 四字段契约成立；引用完整性细则归 P3-D04；revision_context↔target_revision_id 命名统一=O-09。
  - R-09（批准）：close 成功≠全域发布成功，必须可区分 PC/ProjC 各自发布结果与失败待人工。
- **实施交付与提交（F.2/F.3）**：D3 §1——前置 `391c57e`（测试对齐）；阶段一 `ea9aafb`（迁移 0026+projc 服务层镜像五文件+事件目录 16 项+留痕 handlers+formal_chain 6/6，「现网零行为变化」）；阶段二 `0785f97`（push 前 amend 为 **`e9f66ec`**：W4 worker 重挂/G-1 双源/G-2 close 双域分流/W6 并行端点+前端/W7 迁移 0027/W8 矩阵）。Stage 1 先行批（D6）：迁移 0025+projc/{mod,object,evidence}.rs+3 用例。验收：D4 全绿（8+5+10+5 项 PASS，无一 FAIL/BLOCKED；附 dist i18n 文案残留 2 处登记）；一致性核查 D5（二/三/四=PASS，一=NEEDS_DECISION 后经 §10.4 D-1 选项 B 闭合，五=措辞偏差经 D-3 降级闭合，注册表 V1.3）。

## 2. P3-11 正式链当前状态、调用入口和状态转移（F.4）

- **调用入口**：仓库分析→`projc::propose_from_analysis`（origin_kind='repo_analysis'）→提案 submit（同事务 outbox `projc.change_proposed`）→人工 decide（P5 决策路由 `POST /v1/pm/projc/proposals/{id}/decide`，I-6）→close_report 分流块（analysis_report.rs:643-698：approved→`projc::publish_version` 幂等键 `projc-decision:{id}` 独立计数；draft/submitted→withdrawn+逐条审计；NULL projc_source_id→Legacy 跳过+审计 `projc_branch:"legacy_no_source"`）。
- **状态转移**：与 pc 同构（draft→submitted→approved/rejected 暂记互改→applied/failed；withdrawn 集同 draft/submitted）。**conflict 处置**：projc/conflict.rs create（真人+A3）/resolve/dismiss 强制 note/reopen；冻结三处同口径（submit/approve/publish ⑥）。
- **运行库实态**：projc.cognition_object=2、cognition_version=1、proposal=1、relationship=1（种子级负载，见 06 号文件 §7）。
- **G-1 双源登记**：mark_analyzed 单事务 pc_source_id+projc_source_id 四写合一；projc 源缺失硬失败+审计 `analysis_report.analyze_gate_failed`（D3 §2 逐字见 12 号文件）。

## 3. P4-03 V1.1 中引用 P3-11 的具体位置（F.5）

| V1.1 位置 | 引用内容 | 与 P3-11 冻结文本核对 |
|---|---|---|
| 头部引注 L6 | 「P3-11 冻结令 12 项（台账 §10.8）」 | ✅ 12 项清单一致（本文件 §1） |
| §9.1 表（L429-439） | H1~H9 逐项兼容结论 | H1~H9 为审计 V1.1 §6.1 对冻结令 12 项+裁决稿的归纳映射（审计在仓库可核）；逐条语义与冻结令一致 |
| §2.2（L73） | 「P3-11 五条权威边界约束的是域间关系，PC-P/PC-J 是域内作用域分层，不触犯五条（§9.1 逐条核验）」 | 五条边界原文见 D1 §1；「域内分层不触犯域间约束」是 P4-03 的**解释主张（PROPOSED）**，非已裁决事实——复审应独立判断 |
| §7.5/§9.3-I1/I2 | identity 路由分流、close pc 分支语义收窄=「实施期再裁决项」 | 与冻结令效力条款一致（修改须影响分析+重新裁决）；P4-03 明示不在本切片执行 ✅ |
| §7.6/§9.1-H5 | PC-P conflict 新建=R-06 归属原则域内新对象；pc.conflict Legacy 不解冻 | 与 R-06 原文一致 ✅ |
| §11.2/§9.1-H7/H9 | 不搬运、不双写、有界过渡 | 与冻结令第 4/10 项一致 ✅ |
| 附录 A | 引用 D3/D4/D5 | 文件均在仓库 ✅ |

## 4. 逐项核对（F 节四问，只报告事实）

**Q1：P4-03 是否改变或隐含覆盖 P3-11 已落地行为？**
- 现行行为：未改变——V1.1 全文 Recommendation=PROPOSED、I1~I5 显式登记为「实施期再裁决项」（§12 明示「不修改 P3-11 已冻结正式链的现行行为」）；代码/迁移/数据库经本包全程核查零改动。
- 隐含覆盖风险点（供复审判断，非认定）：①I2 将 close pc 分支发布对象语义收窄——触及冻结令第 9 项「close→publish 双域分流原则」下已验收的 pc 分支契约语义（机械结构不变、语义变更），P4-03 已自我登记为再裁决项，但其成立以「产品级权限主体」裁决（DR-01/DR-08）为前提；②§7.9 dissolve 行将「PC-P 全部对象终态冻结、未决提案→withdrawn」纳入清扫集扩展=实施期登记项（沿 P4-02 DR-06 先例）——与 P3-11 无直接冲突（pc/projc 行为未动），但「系统级撤回通道」无现状代码。

**Q2：是否把目标态误写成当前已实现行为？**
- 发现一处：§7.9「close_report 批量撤回先例已覆盖含 approved 的未决态撤回」与代码不符（close 撤回集=draft/submitted；approved→applied；approved→withdrawn 无代码路径）。详证见 05 号文件 §15 / 11 号文件 D-1。其余现状引用（18 表、A3 闸、bootstrap.rs:325、upsert_object_draft 同口、close 批量发布 approved、0026/0027 内容）经逐项核对**均与代码一致**。

**Q3：是否存在与 P3-11 正式链冲突的身份、状态、权限或版本语义？**
- 身份：PC-P/PC-J 域内分层主张不触碰「不共享正式知识对象身份」（域间）；DomainRef 四字段契约原样引用 ✅（命名映射 O-09 开放项须在 C5 显式处理）。
- 状态：§7.9 声明不引入新状态值——但「approved→withdrawn」在现状状态机中不是合法转移（见 Q2）；若按 V1.1 字面执行 dissolve 清扫，需新增该转移或修正表述——此为与 P3-11 状态机（提案 8 态 CHECK+转移集）的**潜在冲突点**，须 DR-09 显式裁决。
- 权限：PC-P 产品级门=新权限面（Product Owner∪tenant admin），不改动现有 A3 闸（PC-J 沿用）——无冲突；消费授权=新资源面（H14 边界声明）。
- 版本：版本不可变纪律、快照哈希、predecessor 链与现状机制同构镜像 ✅。

**Q4：P4-03 迁移计划是否会影响 P3-11 已落地数据或接口？**
- §11.2 声明：0026/0027 零触碰（经 06 号文件 §1 核对属实——两迁移内容与 PC-P/PC-J 增量正交）；identity/bootstrap/close 为「显式一次性切换」序列，无双写双读。
- 事实补充：pc 正式链零负载、单例骨架近零、引用链零负载（运行库实测，06 号文件 §7）——「数据近零直改」的前提在当前运行库成立；但该前提是**时点事实**非恒久保证，实施时点须重测。
- G-1 双源登记中 pc 源改指 PC-P 源（I5）：触及 D3 §2 G-1「pc 源反查（identity 通道，口径不变）」的已验收语义——P4-03 已登记为随 I1 同任务包再裁决 ✅。

## 5. P4-03 未改变 P3-11 的物理证据

- `git status`：crates/、migrations/、apps/ 无任何未提交改动；改动仅 ai-frontend design 导航文件与 docs/ 审计文档。
- P4-03 V1.1 为未跟踪 .md 文件；P3-11 相关代码文件的 HEAD 内容与 D3/D4/D5 记录的交付态一致（projc 7 文件、close 分流块、事件 2 项、G-1 列）。
- 运行库无任何 PC-P 痕迹（06 号文件 §4/§5）。
