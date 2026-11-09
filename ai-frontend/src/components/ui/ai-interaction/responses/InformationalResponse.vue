<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Check, Copy, FileText, MessageSquare } from "lucide-vue-next";

import MarkdownViewer from "@/components/ui/markdown-editor/MarkdownViewer.vue";
import { resolveAttachmentUrl } from "@/api/base/attachments";
import AiResponseCard from "../card/AiResponseCard.vue";
import { defaultAiIdentity } from "../identity";
import type {
  AiEvidence,
  AiIdentity,
  CardDensity,
  FooterAction,
  InformationalPayload,
  InteractionStatus,
  SubmitResult
} from "../types";
import { useAiInteraction } from "../useAiInteraction";

/**
 * 信息卡 —— 纯信息与分析，只读。
 *
 * 交互只有「复制正文」与「继续追问」两个信息性动作：
 * 不出现接收 / 拒绝 / 审批类按钮，避免把「阅读信息」误读成「业务决策」。
 */
const props = withDefaults(
  defineProps<{
    payload: InformationalPayload;
    identity?: AiIdentity;
    timestamp?: string;
    density?: CardDensity;
    submit?: (payload: unknown) => Promise<SubmitResult>;
    initialStatus?: InteractionStatus;
    evidence?: AiEvidence;
  }>(),
  {
    identity: () => defaultAiIdentity,
    timestamp: "",
    density: "default",
    submit: undefined,
    initialStatus: "presented",
    evidence: undefined
  }
);

const emit = defineEmits<{
  action: [key: string];
  submitted: [result: SubmitResult];
}>();

const { t } = useI18n();

const state = useAiInteraction({ initialStatus: props.initialStatus });
const status = state.status;

const copied = ref(false);
let copiedTimer: ReturnType<typeof setTimeout> | null = null;

const sources = computed(() => props.payload.sources ?? []);

const actions = computed<FooterAction[]>(() => [
  {
    key: "copy",
    label: copied.value
      ? t("ai.informational.copied")
      : t("ai.informational.copy"),
    variant: "ghost",
    icon: copied.value ? Check : Copy
  },
  {
    key: "ask-follow-up",
    label: t("ai.informational.followUp"),
    variant: "secondary",
    icon: MessageSquare
  }
]);

async function copyContent() {
  try {
    await navigator.clipboard?.writeText(props.payload.contentMarkdown);
  } catch {
    // 剪贴板不可用（非安全上下文 / 无权限）时仍给出反馈，复制动作不阻断阅读
  }
  copied.value = true;
  if (copiedTimer) clearTimeout(copiedTimer);
  copiedTimer = setTimeout(() => {
    copied.value = false;
  }, 1600);
  emit("action", "copy");
}

function onAction(key: string) {
  if (key === "copy") {
    void copyContent();
    return;
  }
  emit("action", key);
}

onBeforeUnmount(() => {
  if (copiedTimer) clearTimeout(copiedTimer);
});
</script>

<template>
  <AiResponseCard
    type="informational"
    :identity="props.identity"
    :evidence="evidence"
    :status="status"
    :timestamp="props.timestamp"
    :density="props.density"
    :actions="actions"
    @action="onAction"
  >
    <MarkdownViewer
      :model-value="props.payload.contentMarkdown"
      :resolve-url="resolveAttachmentUrl"
    />

    <section v-if="sources.length" class="ai-info__sources">
      <p class="ai-info__sources-label">
        {{ t("ai.informational.sources") }}
      </p>
      <ul class="ai-info__source-list">
        <li v-for="source in sources" :key="source.id" class="ai-info__source">
          <FileText :size="12" aria-hidden="true" />
          <span class="ai-info__source-label">{{ source.label }}</span>
          <span v-if="source.meta" class="ai-info__source-meta">
            {{ source.meta }}
          </span>
        </li>
      </ul>
    </section>
  </AiResponseCard>
</template>

<style scoped>
.ai-info__sources {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin-top: var(--space-3);
  padding-top: var(--space-2);
  border-top: var(--border-1) solid var(--ws-border);
}

.ai-info__sources-label {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
}

.ai-info__source-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ai-info__source {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: var(--size-control-sm);
  padding: 0 var(--space-2);
  color: var(--ws-text-secondary);
  background: var(--ws-elevated);
  border-radius: var(--radius-xs);
}

.ai-info__source-label {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ai-info__source-meta {
  margin-left: auto;
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  white-space: nowrap;
}

@media (prefers-reduced-motion: reduce) {
  .ai-info__source {
    transition: none;
  }
}
</style>
