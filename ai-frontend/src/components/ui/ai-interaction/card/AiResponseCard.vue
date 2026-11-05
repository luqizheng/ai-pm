<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  ChevronDown,
  ChevronUp,
  MoreHorizontal,
  Sparkles
} from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import type { BadgeVariant } from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import IconButton from "@/components/ui/icon-button/IconButton.vue";
import AiEvidencePanel from "./AiEvidencePanel.vue";
import AiStatusPill from "./AiStatusPill.vue";
import AiSubmissionStatus from "./AiSubmissionStatus.vue";
import type {
  AiEvidence,
  AiIdentity,
  AiResponseType,
  CardDensity,
  FooterAction,
  InteractionStatus,
  SubmissionPhase,
  SubmitResult
} from "../types";

/**
 * Interactive Response Card —— 8 类 AI 响应的统一容器。
 *
 * 段位：Header（身份/类型/状态/时间/更多）→ Summary（标题/描述/结论）
 *      → Content → Evidence → Interaction → Submission → Footer。
 * 每段都可缺省：只有传入数据或插槽时才渲染，避免所有卡都变成大块头。
 */
const props = withDefaults(
  defineProps<{
    type: AiResponseType;
    status?: InteractionStatus;
    identity: AiIdentity;
    title?: string;
    description?: string;
    conclusion?: string;
    /** 展示用时间文案（如 10:01） */
    timestamp?: string;
    /** 机器可读时间 */
    datetime?: string;
    density?: CardDensity;
    /** 长内容可折叠 */
    collapsible?: boolean;
    defaultCollapsed?: boolean;
    evidence?: AiEvidence;
    /** 页脚动作；传了渲染为按钮行 */
    actions?: FooterAction[];
    /** 提交中：禁用所有动作 */
    submitting?: boolean;
    submissionPhase?: SubmissionPhase;
    submissionResult?: SubmitResult | null;
    submissionError?: string | null;
    /** 是否允许页脚换行（默认单行右对齐） */
    footerWrap?: boolean;
  }>(),
  {
    status: "presented",
    title: "",
    description: "",
    conclusion: "",
    timestamp: "",
    datetime: "",
    density: "default",
    collapsible: false,
    defaultCollapsed: false,
    evidence: undefined,
    actions: () => [],
    submitting: false,
    submissionPhase: "idle",
    submissionResult: null,
    submissionError: null,
    footerWrap: false
  }
);

const emit = defineEmits<{
  action: [key: string];
  more: [];
  retry: [];
}>();

const { t } = useI18n();

const collapsed = ref(props.defaultCollapsed);

const TYPE_KEY: Record<AiResponseType, string> = {
  informational: "ai.card.type.informational",
  recommendation: "ai.card.type.recommendation",
  proposal: "ai.card.type.proposal",
  question: "ai.card.type.question",
  decision: "ai.card.type.decision",
  artifact: "ai.card.type.artifact",
  "action-request": "ai.card.type.actionRequest",
  "execution-result": "ai.card.type.executionResult"
};

const TYPE_BADGE: Record<AiResponseType, BadgeVariant> = {
  informational: "neutral",
  recommendation: "accent",
  proposal: "info",
  question: "accent",
  decision: "warning",
  artifact: "info",
  "action-request": "warning",
  "execution-result": "success"
};

const typeLabel = computed(() => t(TYPE_KEY[props.type]));
const typeBadge = computed(() => TYPE_BADGE[props.type]);

const actionVariant = (
  action: FooterAction
): "primary" | "secondary" | "ghost" | "danger" =>
  action.variant ?? "secondary";

function onAction(action: FooterAction) {
  if (props.submitting || action.disabled) return;
  emit("action", action.key);
}
</script>

<template>
  <article
    class="ai-card"
    :class="[`ai-card--${density}`, { 'is-collapsed': collapsed }]"
    :data-type="type"
    :data-status="status"
  >
    <!-- Header：AI 身份 / 响应类型 / 状态 / 时间 / 更多 -->
    <header class="ai-card__header">
      <span class="ai-card__identity">
        <span class="ai-card__avatar" aria-hidden="true">
          <Sparkles :size="14" />
        </span>
        <span class="ai-card__identity-text">
          <span class="ai-card__identity-name">{{ identity.name }}</span>
          <span v-if="identity.role" class="ai-card__identity-role">{{
            identity.role
          }}</span>
        </span>
      </span>

      <Badge :variant="typeBadge" size="sm">{{ typeLabel }}</Badge>
      <AiStatusPill :status="status" />

      <span class="ai-card__meta-spacer" />

      <time
        v-if="timestamp"
        class="ai-card__timestamp"
        :datetime="datetime || undefined"
        >{{ timestamp }}</time
      >

      <slot name="header-actions" />

      <IconButton
        class="ai-card__more"
        :label="t('ai.card.more')"
        variant="ghost"
        size="sm"
        @click="emit('more')"
      >
        <MoreHorizontal :size="16" aria-hidden="true" />
      </IconButton>
    </header>

    <!-- Summary：标题 / 描述 / 关键结论 -->
    <div
      v-if="title || description || conclusion || $slots.summary"
      class="ai-card__summary"
    >
      <h3 v-if="title" class="ai-card__title">{{ title }}</h3>
      <p v-if="description" class="ai-card__description">{{ description }}</p>
      <div v-if="conclusion" class="ai-card__conclusion">
        <span class="ai-card__conclusion-label">{{
          t("ai.card.conclusion")
        }}</span>
        <p class="ai-card__conclusion-text">{{ conclusion }}</p>
      </div>
      <slot name="summary" />
    </div>

    <!-- Content -->
    <div v-if="$slots.default" class="ai-card__content">
      <div
        class="ai-card__content-body"
        :class="{ 'is-limited': collapsible && collapsed }"
      >
        <slot />
      </div>
      <button
        v-if="collapsible"
        type="button"
        class="ai-card__collapse"
        :aria-expanded="!collapsed"
        @click="collapsed = !collapsed"
      >
        <component
          :is="collapsed ? ChevronDown : ChevronUp"
          :size="14"
          aria-hidden="true"
        />
        {{ collapsed ? t("ai.card.expand") : t("ai.card.collapse") }}
      </button>
    </div>

    <!-- Evidence -->
    <AiEvidencePanel v-if="evidence || $slots.evidence" :evidence="evidence">
      <slot name="evidence" />
    </AiEvidencePanel>

    <!-- Interaction：类型专属控件与输入 -->
    <div v-if="$slots.interaction" class="ai-card__interaction">
      <slot name="interaction" />
    </div>

    <!-- 提交结果 -->
    <AiSubmissionStatus
      v-if="submissionPhase !== 'idle'"
      :phase="submissionPhase"
      :result="submissionResult"
      :error-message="submissionError"
      @retry="emit('retry')"
    />

    <!-- Footer：提交 / 接收 / 拒绝 / 取消 / 稍后 / 详情 -->
    <footer
      v-if="actions.length || $slots.footer"
      class="ai-card__footer"
      :class="{ 'is-wrapped': footerWrap }"
    >
      <slot name="footer">
        <Button
          v-for="action in actions"
          :key="action.key"
          :variant="actionVariant(action)"
          size="sm"
          :loading="
            action.loading || (submitting && action.variant === 'primary')
          "
          :disabled="submitting || action.disabled"
          @click="onAction(action)"
        >
          <component
            :is="action.icon"
            v-if="action.icon"
            :size="14"
            aria-hidden="true"
          />
          {{ action.label }}
        </Button>
      </slot>
    </footer>
  </article>
</template>

<style scoped>
.ai-card {
  display: flex;
  flex-direction: column;
  width: 100%;
  min-width: var(--size-ai-card-min-width);
  max-width: var(--size-ai-card-max-width);
  overflow: hidden;
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.ai-card--conversation {
  max-width: var(--size-ai-card-conversation-width);
}

.ai-card--compact {
  max-width: var(--size-ai-card-compact-width);
}

/* 会话内卡片不重复画布底色：靠边框区分层级 */
.ai-card--conversation {
  background: var(--ws-surface);
}

/* Header */
.ai-card__header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: var(--size-control-lg);
  padding: var(--space-2) var(--space-3) var(--space-2) var(--space-4);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.ai-card__identity {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.ai-card__avatar {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: var(--size-ai-avatar);
  height: var(--size-ai-avatar);
  color: var(--ws-accent);
  background: var(--ws-color-accent-surface);
  border-radius: var(--radius-xs);
}

.ai-card__identity-text {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  min-width: 0;
}

.ai-card__identity-name {
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  white-space: nowrap;
}

.ai-card__identity-role {
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  white-space: nowrap;
}

.ai-card__meta-spacer {
  flex: 1;
}

.ai-card__timestamp {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  white-space: nowrap;
}

.ai-card__more {
  flex: none;
}

/* Summary */
.ai-card__summary {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-3) var(--space-4) var(--space-2);
}

.ai-card__title {
  margin: 0;
  font-size: var(--font-size-h4);
  font-weight: var(--weight-semibold);
  line-height: var(--line-height-h4);
}

.ai-card__description {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-card__conclusion {
  display: flex;
  gap: var(--space-2);
  margin-top: var(--space-1);
  padding: var(--space-2) var(--space-3);
  background: var(--ws-color-accent-surface);
  border-left: var(--size-indicator) solid var(--ws-accent);
  border-radius: var(--radius-xs);
}

.ai-card__conclusion-label {
  flex: none;
  color: var(--ws-accent);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  line-height: var(--line-height-micro);
}

.ai-card__conclusion-text {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

/* Content */
.ai-card__content {
  display: flex;
  flex-direction: column;
  padding: 0 var(--space-4) var(--space-3);
}

.ai-card__content-body {
  min-width: 0;
  overflow: hidden;
}

.ai-card__content-body.is-limited {
  max-height: var(--size-ai-content-collapsed);
  mask-image: linear-gradient(to bottom, transparent, var(--ws-canvas) 96%);
}

.ai-card__collapse {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-control-sm);
  padding: 0;
  color: var(--ws-text-secondary);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  cursor: pointer;
  background: transparent;
  border: none;
}

.ai-card__collapse:hover {
  color: var(--ws-text-primary);
}

.ai-card__collapse:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

/* Interaction */
.ai-card__interaction {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-top: var(--border-1) solid var(--ws-border);
  background: var(--ws-surface);
}

/* Footer */
.ai-card__footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border-top: var(--border-1) solid var(--ws-border);
  background: var(--ws-surface);
}

.ai-card__footer.is-wrapped {
  align-items: stretch;
}

@media (prefers-reduced-motion: reduce) {
  .ai-card * {
    transition: none;
  }
}
</style>
