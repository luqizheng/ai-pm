# 04 — 阶段 4：技术设计与 ADR

- 日期：2026-11-24 08:02–08:09
- 证据目录：`evidence/04-design-adr/`

## 1. 实际执行步骤

| # | 操作 | 结果 |
|---|---|---|
| 1 | 打开 /project/design | ✅ 分析/技术设计/架构决策/实施准备四页签 + 指标卡；⚠️ 空态文案是未翻译 i18n key（s03.design.analysis.emptyTitle） |
| 2 | chat 发送任务书设计话术 | ⚠️ 含「需求」关键词被升级为第 3 个需求访谈（设计请求被吞，B04-01）；会话 3 收尾（4 发现确认+完结） |
| 3 | **改写话术避开路由关键词**后重发 | ✅ chat 收到完整架构提案：7 模块划分、概念数据模型（全表带 tenant_id）、权限模型 A/B/C 三选项+权衡、技术风险、5 个待确认决策点，「未做任何自行批准」 |
| 4 | 创建设计对象 | ⚠️ **UI 无创建入口**（前端 designs.ts 无 create 函数；后端 API 齐全）→ 经产品 API 创建 TD-001（B04-02，P1） |
| 5 | TD-001 v1 关联 REQ-TASKFLOW-001 | ✅ 201，is_current=true，需求修订状态 approved |
| 6 | 创建 ADR-001（权限模型选 A，含 options/rationale/consequences） | ✅ 201（API） |
| 7 | ADR-001 关联需求 + 设计 | ✅ 204 ×2（双向追溯链 REQ↔ADR↔TD） |
| 8 | 设计 v1 提交→请求审批→**人工批准** | ✅ approval decided_by=leo，理由+target_hash 留痕 |
| 9 | ADR 提交→审批→**人工接受** | ✅ adr_acceptance approved |
| 10 | 创建 v2 修订（决策落定） | ✅ 版本链 v1→superseded、v2→approved；已批修订不可改（409 only draft editable，不可变语义正确） |
| 11 | 就绪门禁评估 | ✅ **ready=false**（scope_defined/acceptance_ready 未过，逐条件给出 detail）；ADR-002（blockers_implementation=true，未接受）在列 |
| 12 | v3 终版（scope.included 规范化 + AC + 重链需求）→提交→批准 | ✅ **ready=true** 全绿 |
| 13 | UI 复核 | ✅ TD-001 v3已批准；ADR-001 已接受/不阻塞；ADR-002 已提出/**阻塞实施** 标记可见 |

截图：`01-design-list.png`、`02-adr-list.png`

## 2. 检查项结论

| 检查项 | 结论 | 说明 |
|---|---|---|
| Design 独立业务对象 | ✅ | TD-001 独立编号/生命周期/版本链/列表页 |
| 一个 Design 关联多个 Requirement | ⚠️ 未完全验证 | 关联按修订承载、API 支持多次 link；本项目仅 1 条需求，多需求关联留待阶段 8 变更场景验证 |
| 修改设计形成新 Revision | ✅ | v1→v2→v3，旧版 superseded，已批不可变 |
| ADR 独立对象 | ✅ | ADR-001/002 独立列表、决策状态（已提出/已接受）、supersede/deprecate 生命周期 API |
| ADR 关联需求与技术设计 | ✅ | requirements + designs 双链接（204） |
| AI 提方案并等人类确认 | ✅ | chat 提案（多方案+权衡+待确认点）；审批必须真人 decide（AI 无路径触达审批） |
| 识别不具备实施条件的设计 | ✅ | Readiness Gate ready=false→true 全程可解释（缺 scope/AC/需求依据逐项报因） |

## 3. 缺陷记录

### B04-01（P2）
- 设计类请求话术含路由关键词（需求/支持等）即被升级为需求访谈；chat 中无法获得设计咨询回复，设计请求被「吞」进访谈（会话 3）
- 影响：AI 设计咨询的可用性依赖用户措辞避开关键词；无降级回 chat 的机制

### B04-02（P1）
- 技术设计与 ADR 的创建/修订/提交/审批/关联**全部无 UI 入口**（前端只读 API）；后端全生命周期 API 齐全
- 影响：普通用户无法在产品内建立设计与 ADR；「设计进入正式业务流程」只对 API 用户成立
- 本验收经产品 REST API 完成（操作员路径，非数据篡改）

### B04-03（P3）
- 设计分析页空态文案未翻译（显示 i18n key `s03.design.analysis.emptyTitle/emptyHint`）

### B04-04（P3）
- Readiness Gate 的 `adrs_resolved` 条件在存在「已提出且阻塞实施」的 ADR-002 时仍判 passed（语义疑似应为：阻塞 ADR 未接受 → 不就绪）；实际阻断靠 scope/AC 条件兜住
- 注：也可能是「仅已接受的阻塞 ADR 才参与判定」的预期语义，建议产品澄清

### B04-05（P3）
- design_approval 请求体的 `expected_current_revision_id/expected_row_revision` 乐观锁字段为可选——缺省时审批未强制校验版本（SC-06 弱化）；API 层未见强制

## 4. 阶段结论

**部分通过（领域模型通过、产品化不足）**：Design/Revision/ADR/审批/就绪门禁的领域语义与人工边界全部正确且可追溯；但设计与 ADR 全链路无 UI（B04-02）+ 设计请求被路由误吞（B04-01），使该能力对普通用户不可达。
