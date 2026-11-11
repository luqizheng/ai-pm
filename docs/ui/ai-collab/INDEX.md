# AI Collaboration Workspace V1.0 — Figma AI 页面设计提示词包

**产品**：Smark PM · **模块**：AI Collaboration Workspace（AI-01~07）
**上位文档**：`docs/product/ai-pm-ai-collaboration-workspace-v1.md`（设计基线）
**性质**：统一产品界面系统——七个页面共享同一主 UI 框架（Navigation / Main Area / AI Chat / Context Panel 四区），只改变三区内容与状态。不是七张独立 UI。

## 文件清单与使用顺序

| 序 | 文件 | 用途 |
|---|---|---|
| 00 | [smark-pm-aic-00-design-master-prompt-v1.md](smark-pm-aic-00-design-master-prompt-v1.md) | 全局主提示词：产品定位、四区布局、V2 视觉令牌、交互原则、卡名词表映射。**每页投喂 Figma AI 前必须先给本文件** |
| 01 | [smark-pm-aic-ui-01-project-ai-workspace-v1.md](smark-pm-aic-ui-01-project-ai-workspace-v1.md) | AI-01 Project AI Workspace（布局基础页，其余六页继承） |
| 02 | [smark-pm-aic-ui-02-context-management-v1.md](smark-pm-aic-ui-02-context-management-v1.md) | AI-02 Context Management（Available / Attached / Read-only） |
| 03 | [smark-pm-aic-ui-03-ai-response-cards-v1.md](smark-pm-aic-ui-03-ai-response-cards-v1.md) | AI-03 AI Response Cards（七类卡组件体系） |
| 04 | [smark-pm-aic-ui-04-draft-review-v1.md](smark-pm-aic-ui-04-draft-review-v1.md) | AI-04 Draft & Review（草稿→正式确认流） |
| 05 | [smark-pm-aic-ui-05-ai-execution-control-v1.md](smark-pm-aic-ui-05-ai-execution-control-v1.md) | AI-05 AI Execution Control（计划/授权/监控） |
| 06 | [smark-pm-aic-ui-06-exception-permission-v1.md](smark-pm-aic-ui-06-exception-permission-v1.md) | AI-06 Exception & Permission（六异常场景） |
| 07 | [smark-pm-aic-ui-07-conversation-continuity-v1.md](smark-pm-aic-ui-07-conversation-continuity-v1.md) | AI-07 Conversation Continuity（连续性/历史快照） |
| 90 | [smark-pm-aic-90-cross-page-prototype-spec-v1.md](smark-pm-aic-90-cross-page-prototype-spec-v1.md) | 跨页 Prototype 交互规则 + 端到端演示流 + Figma 文件组织 |

## 实际执行顺序（十步，禁止一次性生成七页）

一次性生成七页易致布局漂移、组件不一致、交互断裂。按 [90 §A](smark-pm-aic-90-cross-page-prototype-spec-v1.md) 十步执行：**Design System → Workspace Shell → AI-01 → AI-02~07 → Prototype Flows**。

**特别建议：先完成 AI-01，再复制其 Workspace Shell 到其他六个页面；不要让 Figma AI 每次重新设计四栏布局。**（00 主提示词已内置此规则，每次投喂自动携带。）

最终验收以 [90 §B 三成败场景](smark-pm-aic-90-cross-page-prototype-spec-v1.md)为门：①切页对话与 Attached 不丢 ②Proposal 后三区分工可编辑/可查源/可追问 ③正式确认后 Chat、Main Area、Context Panel 三处状态一致更新。任一失败即整体返工。

## 与设计基线的三处细化（以本包为准）

1. **视觉锁定 V2**：基线未定稿视觉；本包锁定 V2「Slate & Muted Teal」（`docs/design/pm-ai-design-tokens-v2.css` 唯一真源，深色为默认渲染目标，附浅色变体）。按 S04/S05 同款裁决收敛，简报浅色蓝方向不采用；禁紫、禁渐变。
2. **History 收进 Drawer**：基线 §3.1 为五 Tab（含 History）；本包右栏主 Tab 收敛为 4 个（Context / Sources / Results / Actions），History 经二级入口 / Drawer 访问，不增加第五主区。
3. **卡名词表映射**（响应基线评审 F3）：基线 §5.1 七卡名与实现侧 8 类 `RESPONSE_TYPES` 的映射见 00 §0.5；Figma 内以卡名为准，实现以信封类型为准。

## 四区布局速览（1440px 参考比例，源自已确认线框）

| 区域 | 比例 | ≈ 宽度 | 默认宽度 |
|---|---|---|---|
| Navigation | 13% | ~190px | 200px 固定 |
| Main Area | 36% | ~520px | ~500px |
| AI Chat | 31% | ~440px | ~420px |
| Context Panel | 20% | ~290px | ~280px |

比例仅为 1440px 初始设计参考，实际宽度支持调整；四区格局恒定，不出现第五主区，AI Chat 不全屏化。
