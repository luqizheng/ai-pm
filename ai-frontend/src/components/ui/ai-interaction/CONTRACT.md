# AI Interaction Component Library — 协作契约（V1.0）

> 本文件是并行开发的接口唯一来源。所有 teammate 先读本文件，再动代码。
> 工作目录：`D:\kudi-project\templates\ai-pm-ai-interaction\ai-frontend`

## 1. 目标与交付

把 Figma 需求《AI Interaction Component Library V1.0》落成 **生产级 Vue 组件 + `/design/ai-interaction` 目录页**：

- 一个统一响应卡基座 + 8 类响应卡；
- 21 个可复用交互控件（8 种状态）；
- 会话面板 + 会话集成目录页；
- 12 个目录页覆盖交付清单 16 帧；
- `zh-CN` / `en` 双语文案；示例数据放 `fixtures.ts`（豁免 i18n 门禁）。

## 2. 硬约束（违反即返工）

1. 配色只用 `src/styles/tokens.css` 变量；禁止裸 hex / rgb() / hsl() / 颜色名。
2. 样式一律 `<style scoped>`；禁止 Tailwind / reset / 全局选择器。
3. 图标只用 `lucide-vue-next`。
4. 源码禁止中文（组件/页面/逻辑）；中文示例只允许出现在：
   - `src/components/ui/ai-interaction/fixtures.ts`
   - `*.test.ts`
   - `locales/**.yaml`
5. 注释可以中文（门禁会剥离注释），但请精简。
6. 每个页面 / 组件自带 `:focus-visible`（`box-shadow: var(--ws-focus-ring)`），动画走 duration token，并带 `prefers-reduced-motion` 降级。
7. 不改 `src/components/shell/`、`src/styles/index.css`、`src/router/navigation.ts`。
8. 只改自己的写作用域（见 §8），不要动别人文件；公共底座由 Lead 维护。

## 3. 目录与所有权

```
src/components/ui/ai-interaction/
  types.ts                 # Lead（唯一契约，禁止改）
  useAiInteraction.ts      # Lead
  submitAdapter.ts         # Lead
  fixtures.ts              # Lead（可追加？否：只读；缺样例找 Lead）
  card/                    # Lead
  controls/                # T1 控件
  responses/               # T2 四类卡 / T3 四类卡
  conversation/            # T4 会话面板
src/views/design/ai-interaction/
  _shared/CatalogPage.vue  # Lead（只读）
  AiInteractionIndexView.vue ... 共 12 页
  # T5 负责 index/base/informational/recommendation/proposal
  # T4 负责 question/decision/artifact/action/execution/controls/conversation
locales/zh-CN/ai/*.yaml + locales/en/ai/*.yaml
  card.yaml                # Lead
  controls.yaml            # T1
  informational.yaml ...   # T2（每类卡一个文件）
  question.yaml ...        # T3（每类卡一个文件）
  conversation.yaml        # T4
```

## 4. 卡片基座 API

```vue
<AiResponseCard
  type="recommendation"            <!-- AiResponseType，必填 -->
  :identity="aiIdentity"           <!-- AiIdentity，必填 -->
  status="presented"               <!-- InteractionStatus，默认 presented -->
  title="…"
  description="…"
  conclusion="…"                   <!-- 关键结论；accent 语义块 -->
  timestamp="10:01"
  datetime="2026-11-05T10:01:00+08:00"
  density="conversation"           <!-- default | conversation | compact -->
  :collapsible="false"             <!-- 长内容折叠 -->
  :evidence="projectEvidence"      <!-- AiEvidence；自动渲染证据区 -->
  :actions="actions"               <!-- FooterAction[] -->
  :submitting="state.submitting.value"
  :submission-phase="state.phase.value"
  :submission-result="state.result.value"
  :submission-error="state.error.value"
  @action="onAction"
  @retry="state.retry"
  @more="…"
>
  <template #summary>…</template>      <!-- 可选，追加在摘要区 -->
  <template #interaction>…</template> <!-- 类型专属控件区 -->
  <template #footer>…</template>      <!-- 可选，替换 actions 行 -->
</AiResponseCard>
```

- `AiResponseCard` 已含：AI 身份章、类型 Badge、状态徽标、时间、更多按钮、结论块、证据区、提交结果条、页脚动作。
- 8 类响应卡统一额外接受 `evidence?: AiEvidence`，透传给基座证据区（缺省不渲染）。
- 内容默认插槽 = 正文（Markdown 预览用 `MarkdownViewer`，表格用 `Table`，代码块自行 `<pre>` + `--font-mono`）。
- 页脚动作：`FooterAction = { key, label, variant?: primary|secondary|ghost|danger, disabled?, loading?, icon? }`，点击 `emit('action', key)`。

```ts
// 状态机
const state = useAiInteraction({
  initialStatus: "presented",
  submit: demoSubmit // 或 createMockSubmit({ failTimes: 1 })
});
state.status /
  phase /
  result /
  error /
  draft /
  submitting /
  submitted /
  isTerminal;
state.begin() /
  saveDraft(v) /
  defer() /
  accept(p) /
  reject(p) /
  submit(p) /
  resolve(p) /
  retry() /
  reset();
```

- `accept()` 成功 = `accepted`，只代表「选择已记录」；文案不得写「已生效」。
- `resolve()` 成功 = `resolved`，用于正式决策提交。

其他基础件：

| 组件                 | Props                                                                        |
| -------------------- | ---------------------------------------------------------------------------- |
| `AiStatusPill`       | `status: InteractionStatus`                                                  |
| `AiEvidencePanel`    | `evidence?: AiEvidence`, `defaultOpen?: boolean`（卡片已内置，一般不用手写） |
| `AiSubmissionStatus` | `phase`, `result?`, `errorMessage?`（卡片已内置）                            |

## 5. 类型与样例数据

全部类型在 `types.ts`：`InformationalPayload` / `RecommendationPayload` / `ProposalPayload` / `QuestionPayload` / `DecisionPayload` / `ArtifactPayload` / `ActionRequestPayload` / `ExecutionResultPayload`，另有 `DiffPayload`、`ConversationMessage`。

样例数据在 `fixtures.ts`（只读）：`aiIdentity`、`projectEvidence`、`informationalFixture`、`recommendationFixture`、`proposalFixture`、`proposalDiffFixture`、`questionFixture`、`decisionFixture`、`artifactFixture`、`actionRequestFixture`、`executionResultFixture`、`executionFailureFixture`、`conversationFixture`。

## 6. 目录页规范

每页：

```vue
<CatalogPage
  title-key="design.aiInteraction.recommendation.title"
  subtitle-key="design.aiInteraction.recommendation.subtitle"
>
  <DesignFrame
    label="03 — Recommendation / Pending"
    note="接收 ≠ 审批；接收后卡片保留并转为只读结果摘要。"
    width="default"
  >
    <RecommendationResponse … />
  </DesignFrame>
  …
</CatalogPage>
```

- `CatalogPage`：统一页框（已建，只读）。
- `DesignFrame`：`src/views/design/design-kit/DesignFrame.vue`，props `label` / `note` / `width: narrow|default|wide|full`。
- 页面标题/副标题 key 已在 `locales/*/design.yaml` 注册；页面文件已存在（stub），**替换内容即可，不要改路由**。
- 每页至少 2 个状态（按交付清单帧），并真实可交互（点击后状态变化）。
- 页面必须挂 1 条 vitest（在 `src/views/design/ai-interaction/ai-interaction-pages.test.ts` 追加自己的用例；该文件由 Lead 建，按页追加 `it()`，不要删别人用例）。

## 7. i18n

- 命名空间 = 文件路径：`locales/zh-CN/ai/controls.yaml` → key `ai.controls.*`。
- **zh-CN 与 en 的 key 必须完全一致**（缺一个就挂门禁），文件也必须成对。
- 只允许在自己负责的 yaml 文件里加 key；`ai/card.yaml`、`design.yaml` 由 Lead 维护。
- 计数/插值写 `{count}` / `{reference}` 形式。

## 8. 写作用域与并行纪律

| 谁  | 只改这些路径                                                                                                                                                                   |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| T1  | `src/components/ui/ai-interaction/controls/**`、`locales/*/ai/controls.yaml`                                                                                                   |
| T2  | `src/components/ui/ai-interaction/responses/Informational*`、`Recommendation*`、`Proposal*`、`Artifact*`、`locales/*/ai/{informational,recommendation,proposal,artifact}.yaml` |
| T3  | `responses/Question*`、`Decision*`、`ActionRequest*`、`ExecutionResult*`、`locales/*/ai/{question,decision,actionRequest,executionResult}.yaml`                                |
| T4  | `conversation/**`、`locales/*/ai/conversation.yaml`、目录页 question/decision/artifact/action/execution/controls/conversation                                                  |
| T5  | 目录页 index/base/informational/recommendation/proposal                                                                                                                        |

共用：`fixtures.ts`、`types.ts`、`card/**`、`design.yaml`、`design-kit/**` 只读；缺东西找 Lead。

## 9. 完成标准（每个组件 / 页面）

- 8 类卡片至少覆盖：待处理、提交中、成功（只读摘要）、失败（可重试）；问题卡另加校验错误；决策卡区分推荐/选择/正式提交/已记录；动作卡用 `ConfirmationDialog` 二次确认。
- 控件至少覆盖：默认 / 悬停 / 聚焦 / 选中 / 禁用 / 校验错误 / 提交中 / 已提交（可用目录页组合展示，组件本身给 props）。
- 文案全部 i18n；无裸色值；无中文硬编码。
- 写完先跑：`pnpm exec vue-tsc --noEmit --skipLibCheck`（慢，可只跑一次）或让 Lead 统一跑。
- 测试：`pnpm exec vitest run <自己的文件>` 必须过。

## 10. 可用 token（新增部分）

`--size-ai-card-min-width` / `-max-width` / `-compact-width` / `-conversation-width`、
`--size-ai-content-collapsed`、`--size-ai-evidence-max-height`、`--size-ai-artifact-preview-height`、
`--size-ai-log-max-height`、`--size-ai-diff-line`、`--size-ai-avatar`、`--size-ai-avatar-lg`、
`--size-ai-option-min-height`、`--size-ai-conversation-max-width`、`--size-ai-message-gap`。
