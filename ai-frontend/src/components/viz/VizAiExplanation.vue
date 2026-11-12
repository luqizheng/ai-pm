<script setup lang="ts">
/**
 * VizAiExplanation —— AI 图形解释面板（V0-8.12）。
 *
 * 依据 18 号 UX 规格 §4.1 D：图形表达了什么、主要关系、关键发现、
 * **Direct / Indirect Relationships**、潜在风险、AI 推断与待确认事项。
 *
 * 数据来源：信封 payload 的结构化字段（**不是模型自由文本直出**）——
 * 后端负责把解释落成受控字段，前端只渲染，避免不可控文本混入卡片。
 */
import { useI18n } from "vue-i18n";
import Badge from "@/components/ui/badge/Badge.vue";

/** 解释面板载荷（受控字段）。 */
export interface VizExplanation {
  summary?: string;
  keyFindings?: string[];
  directRelations?: string[];
  indirectRelations?: string[];
  risks?: string[];
  pendingConfirmations?: string[];
}

interface Props {
  explanation: VizExplanation;
}

const props = defineProps<Props>();

const { t } = useI18n();
</script>

<template>
  <section class="viz-explain" :data-testid="'viz-explanation'">
    <p v-if="props.explanation.summary" class="viz-explain__summary">
      {{ props.explanation.summary }}
    </p>

    <div
      v-if="props.explanation.keyFindings?.length"
      class="viz-explain__block"
    >
      <h4 class="viz-explain__heading">
        {{ t("viz.explanation.keyFindings") }}
      </h4>
      <ul class="viz-explain__list">
        <li v-for="(item, i) in props.explanation.keyFindings" :key="i">
          {{ item }}
        </li>
      </ul>
    </div>

    <!-- 关系块：字段存在即渲染——两者皆空时也要显式说明「无」，
         而不是让整块消失（沉默会被读成「没查」）。 -->
    <div
      v-if="
        'directRelations' in props.explanation ||
        'indirectRelations' in props.explanation
      "
      class="viz-explain__block"
    >
      <h4 class="viz-explain__heading">
        {{ t("viz.explanation.relationships") }}
      </h4>
      <div class="viz-explain__group">
        <Badge variant="neutral" size="sm">
          {{ t("viz.explanation.direct") }}
        </Badge>
        <ul class="viz-explain__list">
          <li
            v-for="(item, i) in props.explanation.directRelations ?? []"
            :key="`d-${i}`"
          >
            {{ item }}
          </li>
          <li
            v-if="!props.explanation.directRelations?.length"
            class="viz-explain__muted"
          >
            {{ t("viz.explanation.none") }}
          </li>
        </ul>
      </div>
      <div class="viz-explain__group">
        <Badge variant="warning" size="sm" outline>
          {{ t("viz.explanation.indirect") }}
        </Badge>
        <ul class="viz-explain__list">
          <li
            v-for="(item, i) in props.explanation.indirectRelations ?? []"
            :key="`i-${i}`"
          >
            {{ item }}
          </li>
          <li
            v-if="!props.explanation.indirectRelations?.length"
            class="viz-explain__muted"
          >
            {{ t("viz.explanation.none") }}
          </li>
        </ul>
      </div>
    </div>

    <div v-if="props.explanation.risks?.length" class="viz-explain__block">
      <h4 class="viz-explain__heading">{{ t("viz.explanation.risks") }}</h4>
      <ul class="viz-explain__list">
        <li v-for="(item, i) in props.explanation.risks" :key="`r-${i}`">
          {{ item }}
        </li>
      </ul>
    </div>

    <div
      v-if="props.explanation.pendingConfirmations?.length"
      class="viz-explain__block"
    >
      <h4 class="viz-explain__heading">
        {{ t("viz.explanation.pending") }}
      </h4>
      <ul class="viz-explain__list">
        <li
          v-for="(item, i) in props.explanation.pendingConfirmations"
          :key="`p-${i}`"
        >
          {{ item }}
        </li>
      </ul>
    </div>
  </section>
</template>

<style scoped>
.viz-explain {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.viz-explain__summary {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.viz-explain__block {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.viz-explain__heading {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.viz-explain__group {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.viz-explain__list {
  margin: 0;
  padding-left: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
}

.viz-explain__muted {
  color: var(--text-tertiary);
  list-style: none;
  margin-left: calc(-1 * var(--space-4));
}
</style>
