# AI Collaboration Workspace V1.0 — Figma AI 页面设计提示词包

**产品**：Smark PM · **模块**：AI Collaboration Workspace（AI-01~07）
**上位文档**：[设计基线](../../product/ai-pm-ai-collaboration-workspace-v1.md)
**裁决真源**：[95 裁决记录](smark-pm-aic-95-decision-record-v1.md)（DR-C1~C6，与本包其他文件冲突时以裁决记录为准）
**性质**：统一产品界面系统——七个页面共享同一主 UI 框架（Navigation / Main Area / AI Chat / Context Panel 四区），只改变三区内容与状态。不是七张独立 UI。
**版本**：v1-1（2026-11-11 专家裁决后修订；v1 见 git 历史）

## 文件清单与使用顺序

| 序 | 文件 | 用途 |
|---|---|---|
| 00 | [smark-pm-aic-00-design-master-prompt-v1-1.md](smark-pm-aic-00-design-master-prompt-v1-1.md) | 全局主提示词：产品定位、四区尺寸、V2 视觉令牌、交互原则、卡名映射、权限审批模型、会话绑定。**每页投喂 Figma AI 前必须先给本文件** |
| 95 | [smark-pm-aic-95-decision-record-v1.md](smark-pm-aic-95-decision-record-v1.md) | 正式裁决记录（DR-C1~C6）。**设计/前端/后端/QA 的共同依据** |
| 01 | [smark-pm-aic-ui-01-project-ai-workspace-v1-1.md](smark-pm-aic-ui-01-project-ai-workspace-v1-1.md) | AI-01 Project AI Workspace（布局母版，其余六页复制其 Shell） |
| 02 | [smark-pm-aic-ui-02-context-management-v1-1.md](smark-pm-aic-ui-02-context-management-v1-1.md) | AI-02 Context Management（Available / Attached / Read-only） |
| 03 | [smark-pm-aic-ui-03-ai-response-cards-v1-1.md](smark-pm-aic-ui-03-ai-response-cards-v1-1.md) | AI-03 AI Response Cards（七类卡 ↔ 八类 Envelope） |
| 04 | [smark-pm-aic-ui-04-draft-review-v1-1.md](smark-pm-aic-ui-04-draft-review-v1-1.md) | AI-04 Draft & Review（Draft → Approval Pending → 正式 Revision） |
| 05 | [smark-pm-aic-ui-05-ai-execution-control-v1-1.md](smark-pm-aic-ui-05-ai-execution-control-v1-1.md) | AI-05 AI Execution Control（授权 ≠ 审批） |
| 06 | [smark-pm-aic-ui-06-exception-permission-v1-1.md](smark-pm-aic-ui-06-exception-permission-v1-1.md) | AI-06 Exception & Permission（六异常场景 + 硬审批边界） |
| 07 | [smark-pm-aic-ui-07-conversation-continuity-v1-1.md](smark-pm-aic-ui-07-conversation-continuity-v1-1.md) | AI-07 Conversation Continuity（Project 绑定 + 历史快照） |
| 90 | [smark-pm-aic-90-cross-page-prototype-spec-v1-1.md](smark-pm-aic-90-cross-page-prototype-spec-v1-1.md) | 跨页 Prototype 契约 + 执行顺序 + 16 步演示流 + 验收清单 + 实施边界 |
| — | [smark-pm-aic-open-conflicts-v1.md](smark-pm-aic-open-conflicts-v1.md) | 冲突清单（**已全部裁决**，留档追溯） |

## 实际执行顺序（十步，禁止一次性生成七页）

一次性生成七页易致布局漂移、组件不一致、交互断裂。按 [90 §A](smark-pm-aic-90-cross-page-prototype-spec-v1-1.md) 十步执行：**Design System → Workspace Shell → AI-01 → AI-02~07 → Prototype Flows**。

**特别建议：先完成 AI-01，再复制其 Workspace Shell 到其他六个页面；不要让 Figma AI 每次重新设计四栏布局。**

最终验收以 [90 §B 三成败场景](smark-pm-aic-90-cross-page-prototype-spec-v1-1.md) + [Part 7 完整清单](smark-pm-aic-90-cross-page-prototype-spec-v1-1.md)为门；任一失败即整体返工。

## 正式布局尺寸（DR-C5）

画板 1440 × 1000px · Navigation 188px · Main Area 518px · AI Chat 446px · Context Panel 288px。
Main Area 仅 518px，**不强制双栏 Diff**，采用 Inline / Unified Diff 或单列切换。

## 裁决要点速查

| 裁决 | 一句话 |
|---|---|
| DR-C1 | 视觉以本包 V2 深色为准（专家稿浅色蓝为旧版，不采纳） |
| DR-C2 | Default Deny + 权限交集 + 真人审批强制；Approval Pending 是强拦截态 |
| DR-C3 | 右栏四主 Tab + History Drawer（五类历史） |
| DR-C4 | 八类 Envelope 不变；七类 UI 卡映射；Risk 须结构化字段，Progress 须真实执行状态 |
| DR-C5 | 188 / 518 / 446 / 288；518px 禁双栏 Diff |
| DR-C6 | V1 无未绑定 Project 的会话；Global AI 只是 Project 选择入口 |

## 与设计基线的关系

设计基线（`docs/product/ai-pm-ai-collaboration-workspace-v1.md`）是**上游骨架**；本包是**可执行设计规范**。两者的冲突以 95 裁决记录为准，已同步修订基线 §1.1 / §3.1 / §5.1 / §5.3 与附录 B。
