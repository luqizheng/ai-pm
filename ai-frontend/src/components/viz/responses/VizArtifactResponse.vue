<script setup lang="ts">
/**
 * VizArtifactResponse —— 对话卡里的可视化产物卡（Response Envelope 第 9 类）。
 *
 * 契约（后端 `ai_interaction_envelope`：白名单第 9 类 `viz_artifact`）：
 * payload 必填 `artifactId` / `slotKey` / `previewUrl`——缺任一字段即整信封
 * 拒收并退化为纯文本（fail-closed，见 useAssistantConversation.ts）。
 *
 * 安全：卡内**不加载任何产物脚本**——预览以 `<img>` 引用（`previewUrl` 由后端
 * 下发的可直链地址；产物静态化保证零脚本执行，见 15 号安全审查）。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import AiResponseCard from "@/components/ui/ai-interaction/card/AiResponseCard.vue";
import { defaultAiIdentity } from "@/components/ui/ai-interaction/identity";
import type {
  AiIdentity,
  CardDensity,
  InteractionStatus
} from "@/components/ui/ai-interaction/types";

/** 信封 payload（必填字段已由信封层校验）。 */
export interface VizArtifactPayload {
  artifactId: string;
  slotKey: string;
  previewUrl: string;
  title?: string;
  state?: "draft" | "published" | "stale" | "rejected";
  nature?: "business" | "explanatory";
  versionNo?: number;
  nodeCount?: number;
  edgeCount?: number;
}

const props = withDefaults(
  defineProps<{
    payload: VizArtifactPayload;
    identity?: AiIdentity;
    timestamp?: string;
    density?: CardDensity;
    initialStatus?: InteractionStatus;
  }>(),
  {
    identity: () => defaultAiIdentity,
    timestamp: "",
    density: "default",
    initialStatus: "presented"
  }
);

const { t } = useI18n();

const isExplanatory = computed(() => props.payload.nature === "explanatory");
const stateLabel = computed(() =>
  t(`viz.artifact.state.${props.payload.state ?? "draft"}`)
);
</script>

<template>
  <AiResponseCard
    type="viz-artifact"
    :identity="identity"
    :timestamp="timestamp"
    :density="density"
    :initial-status="initialStatus"
    :title="payload.title || payload.slotKey"
  >
    <div class="viz-response" :data-testid="'viz-artifact-response'">
      <div class="viz-response__badges">
        <Badge variant="info" size="sm">
          {{ t("viz.artifact.version", { version: payload.versionNo ?? 1 }) }}
        </Badge>
        <Badge
          v-if="isExplanatory"
          variant="warning"
          size="sm"
          outline
          :data-testid="'viz-nature-badge'"
        >
          {{ t("viz.artifact.explanatory") }}
        </Badge>
        <Badge variant="neutral" size="sm">{{ stateLabel }}</Badge>
      </div>

      <figure class="viz-response__figure">
        <img
          :src="payload.previewUrl"
          :alt="payload.title || payload.slotKey"
          class="viz-response__image"
          :data-testid="'viz-response-image'"
        />
      </figure>

      <p class="viz-response__counts">
        {{
          t("viz.artifact.counts", {
            nodes: payload.nodeCount ?? 0,
            edges: payload.edgeCount ?? 0
          })
        }}
      </p>

      <p class="viz-response__hint" :data-testid="'viz-interaction-hint'">
        {{ t("viz.artifact.interactionHint") }}
      </p>

      <div v-if="!isExplanatory" class="viz-response__actions">
        <Button size="sm" variant="primary" :data-testid="'viz-accept'">
          {{ t("viz.artifact.accept") }}
        </Button>
        <Button size="sm" variant="ghost" :data-testid="'viz-reject'">
          {{ t("viz.artifact.reject") }}
        </Button>
      </div>
    </div>
  </AiResponseCard>
</template>

<style scoped>
.viz-response {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.viz-response__badges {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.viz-response__figure {
  margin: 0;
  display: flex;
  justify-content: center;
  align-items: center;
  max-height: var(--size-ai-artifact-preview-height);
  overflow: auto;
}

.viz-response__image {
  max-width: 100%;
  height: auto;
}

.viz-response__counts,
.viz-response__hint {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.viz-response__actions {
  display: flex;
  gap: var(--space-2);
}
</style>
