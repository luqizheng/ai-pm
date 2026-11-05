<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Ban, Compass, Outdent, Target } from "lucide-vue-next";
import type { Component } from "vue";
import type { RequirementSpecification } from "../../requirements/requirement-entry";

/**
 * 需求规格卡（S02-UI-02 §3B）。
 *
 * 详细描述之外的结构化规格：业务理由 / 预期结果 / 范围 / 明确排除。
 * 明确排除用独立图标与虚线边框呈现，避免与范围混淆。
 */
interface Props {
  specification: RequirementSpecification;
  description: string;
}

defineProps<Props>();
const { t } = useI18n();

const sections = computed<
  Array<{ key: keyof RequirementSpecification; title: string; icon: Component }>
>(() => [
  {
    key: "business_rationale",
    title: t("pm.requirementDetail.specRationale"),
    icon: Target
  },
  {
    key: "expected_outcome",
    title: t("pm.requirementDetail.specOutcome"),
    icon: Outdent
  },
  { key: "scope", title: t("pm.requirementDetail.specScope"), icon: Compass },
  {
    key: "explicit_exclusions",
    title: t("pm.requirementDetail.specExclusions"),
    icon: Ban
  }
]);
</script>

<template>
  <section class="reqd-card" :aria-label="t('pm.requirementDetail.specTitle')">
    <h3 class="reqd-card__title">{{ t("pm.requirementDetail.specTitle") }}</h3>
    <p class="reqd-spec__desc">{{ description }}</p>
    <div class="reqd-spec__grid">
      <div
        v-for="section in sections"
        :key="section.key"
        class="reqd-spec__section"
        :class="{
          'reqd-spec__section--exclusion': section.key === 'explicit_exclusions'
        }"
      >
        <div class="reqd-spec__section-head">
          <component
            :is="section.icon"
            class="reqd-spec__icon"
            aria-hidden="true"
          />
          <span class="reqd-spec__section-title">{{ section.title }}</span>
        </div>
        <p class="reqd-spec__section-body">{{ specification[section.key] }}</p>
      </div>
    </div>
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

.reqd-spec__desc {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.reqd-spec__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
}

.reqd-spec__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: var(--space-3);
  min-width: 0;
}

.reqd-spec__section--exclusion {
  border-style: dashed;
}

.reqd-spec__section-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.reqd-spec__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--text-secondary);
}

.reqd-spec__section--exclusion .reqd-spec__icon {
  color: var(--warning-strong);
}

.reqd-spec__section-title {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.reqd-spec__section-body {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

@media (max-width: 1100px) {
  .reqd-spec__grid {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
