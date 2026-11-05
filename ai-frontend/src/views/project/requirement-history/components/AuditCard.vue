<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  FileSearch,
  ScrollText,
  ShieldCheck,
  UserRound
} from "lucide-vue-next";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import type {
  RequirementEntry,
  RevisionRecord
} from "../../requirements/requirement-entry";

/**
 * 审计信息卡（S02-UI-05 §6 + Deliverable 4）。
 *
 * 谁发起 / 谁评审 / 谁确认（关联决策）/ 为什么改 / 相关来源证据。
 * 历史数据缺审计字段时回退到修订记录的 author / summary 并标注记录不全。
 */
interface Props {
  revision: RevisionRecord | null;
  entry: RequirementEntry;
}

const props = defineProps<Props>();
const { t } = useI18n();

const initiatedBy = computed(
  () => props.revision?.audit?.initiated_by ?? props.revision?.author ?? ""
);

const reviewedBy = computed(() => props.revision?.audit?.reviewed_by ?? "");

const confirmedBy = computed(() =>
  props.revision?.decision?.kind === "confirmed"
    ? props.revision.decision.actor
    : ""
);

const rejectedBy = computed(() =>
  props.revision?.decision?.kind === "rejected"
    ? props.revision.decision.actor
    : ""
);

const reason = computed(
  () => props.revision?.audit?.reason ?? props.revision?.summary ?? ""
);

const auditIncomplete = computed(() => !props.revision?.audit);

const sourceRefs = computed(() =>
  props.entry.sources
    .filter(source => source.ref)
    .map(source => ({ id: source.id, ref: source.ref!, title: source.title }))
);
</script>

<template>
  <section class="rha" :aria-label="t('pm.requirementHistory.auditTitle')">
    <h3 class="rha__title">{{ t("pm.requirementHistory.auditTitle") }}</h3>

    <EmptyState
      v-if="!props.revision"
      size="sm"
      :title="t('pm.requirementHistory.auditEmpty')"
    />

    <template v-else>
      <ul class="rha__facts">
        <li>
          <UserRound class="rha__icon" aria-hidden="true" />
          <span class="rha__label">{{
            t("pm.requirementHistory.auditInitiated")
          }}</span>
          <span class="rha__value">{{ initiatedBy }}</span>
        </li>
        <li v-if="reviewedBy">
          <FileSearch class="rha__icon" aria-hidden="true" />
          <span class="rha__label">{{
            t("pm.requirementHistory.auditReviewed")
          }}</span>
          <span class="rha__value">{{ reviewedBy }}</span>
        </li>
        <li v-if="confirmedBy || rejectedBy">
          <ShieldCheck class="rha__icon" aria-hidden="true" />
          <span class="rha__label">
            {{
              confirmedBy
                ? t("pm.requirementHistory.auditConfirmed")
                : t("pm.requirementHistory.auditRejected")
            }}
          </span>
          <span class="rha__value">{{ confirmedBy || rejectedBy }}</span>
        </li>
        <li>
          <ScrollText class="rha__icon" aria-hidden="true" />
          <span class="rha__label">{{
            t("pm.requirementHistory.auditReason")
          }}</span>
          <span class="rha__value">{{ reason }}</span>
        </li>
      </ul>

      <p v-if="auditIncomplete" class="rha__warn">
        {{ t("pm.requirementHistory.auditIncomplete") }}
      </p>

      <div class="rha__section">
        <span class="rha__section-label">
          {{ t("pm.requirementHistory.auditEvidence") }}
        </span>
        <ul v-if="sourceRefs.length > 0" class="rha__refs">
          <li v-for="source in sourceRefs" :key="source.id">
            <span class="rha__ref-code">{{ source.ref }}</span>
            <span class="rha__ref-title">{{ source.title }}</span>
          </li>
        </ul>
        <p v-else class="rha__empty">{{ t("pm.requirements.sourcesEmpty") }}</p>
      </div>
    </template>
  </section>
</template>

<style scoped>
.rha {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rha__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rha__facts {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rha__facts li {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
}

.rha__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  flex: none;
  color: var(--text-secondary);
}

.rha__label {
  flex: none;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  min-width: 64px;
}

.rha__value {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
  min-width: 0;
}

.rha__warn {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--warning-strong);
}

.rha__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.rha__section-label {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.rha__refs {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rha__refs li {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.rha__ref-code {
  font-family: var(--font-mono);
  color: var(--text-secondary);
}

.rha__ref-title {
  color: var(--text-primary);
}

.rha__empty {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-disabled);
}
</style>
