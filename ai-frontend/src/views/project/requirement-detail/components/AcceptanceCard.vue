<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { AcceptanceCriterion } from "../../requirements/requirement-entry";

/**
 * 验收标准卡（S02-UI-02 §3C）。
 *
 * 结构化验收标准：每条独立可读、可测、可验证。
 */
interface Props {
  criteria: AcceptanceCriterion[];
}

defineProps<Props>();
const { t } = useI18n();
</script>

<template>
  <section
    class="reqd-card"
    :aria-label="t('pm.requirementDetail.acceptTitle')"
  >
    <h3 class="reqd-card__title">
      {{ t("pm.requirementDetail.acceptTitle") }}
    </h3>
    <ol v-if="criteria.length > 0" class="reqd-accept__list">
      <li
        v-for="criterion in criteria"
        :key="criterion.id"
        class="reqd-accept__item"
      >
        <span class="reqd-accept__index" aria-hidden="true" />
        <span class="reqd-accept__statement">{{ criterion.statement }}</span>
      </li>
    </ol>
    <p v-else class="reqd-accept__empty">
      {{ t("pm.requirements.criteriaEmpty") }}
    </p>
  </section>
</template>

<style scoped>
.reqd-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.reqd-card__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.reqd-accept__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
  counter-reset: accept;
}

.reqd-accept__item {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: var(--space-2) var(--space-3);
  counter-increment: accept;
}

.reqd-accept__index {
  flex: none;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.reqd-accept__index::before {
  content: "AC-" counter(accept);
}

.reqd-accept__statement {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.reqd-accept__empty {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-disabled);
}
</style>
