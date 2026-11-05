<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Badge from "@/components/ui/badge/Badge.vue";
import type { BadgeVariant } from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Sheet from "@/components/ui/sheet/Sheet.vue";
import type { RequirementEntry, RevisionRecord } from "../requirement-entry";

/**
 * 修订历史抽屉（S02-UI-01 §11 状态 7：修订比较）。
 *
 * 完整修订列表（version 倒序）+ 两版并排比较（旧版左 / 新版右）。
 * 只读展示：修订内容来自 Knowledge Markdown 快照，此处不得编辑正文。
 */
interface Props {
  open: boolean;
  entry: RequirementEntry | null;
}

const props = defineProps<Props>();
const emit = defineEmits<{ (e: "update:open", value: boolean): void }>();

const { t } = useI18n();

const compareNew = ref("");
const compareOld = ref("");

const revisions = computed<RevisionRecord[]>(() =>
  props.entry
    ? [...props.entry.revisions].sort((a, b) => b.version - a.version)
    : []
);

watch(
  () => [props.open, props.entry?.id] as const,
  ([open]) => {
    if (!open || !props.entry) return;
    const ordered = revisions.value;
    compareNew.value = ordered[0]?.id ?? "";
    compareOld.value = ordered[1]?.id ?? "";
  },
  { immediate: true }
);

const compareNewRevision = computed(() =>
  revisions.value.find(revision => revision.id === compareNew.value)
);
const compareOldRevision = computed(() =>
  revisions.value.find(revision => revision.id === compareOld.value)
);
const canCompare = computed(() =>
  Boolean(compareNewRevision.value && compareOldRevision.value)
);

const revisionStateLabel = (state: string) =>
  ({
    draft: t("pm.requirements.revStateDraft"),
    pending_confirmation: t("pm.requirements.revStatePending"),
    approved: t("pm.requirements.revStateApproved"),
    rejected: t("pm.requirements.revStateRejected"),
    superseded: t("pm.requirements.revStateSuperseded")
  })[state] ?? state;

const revisionStateVariant = (state: string): BadgeVariant =>
  (
    ({
      draft: "neutral",
      pending_confirmation: "warning",
      approved: "success",
      rejected: "danger",
      superseded: "neutral"
    }) as const
  )[state] ?? "neutral";
</script>

<template>
  <Sheet
    :open="props.open"
    :title="t('pm.requirements.historyTitle')"
    :description="props.entry ? `${props.entry.code} ${props.entry.title}` : ''"
    side="right"
    @update:open="emit('update:open', $event)"
  >
    <div v-if="props.entry" class="req-history">
      <ul class="req-history__list">
        <li
          v-for="revision in revisions"
          :key="revision.id"
          class="req-history__item"
        >
          <div class="req-history__item-head">
            <span class="req-history__version">
              {{
                t("pm.requirements.revisionVersion", {
                  version: revision.version
                })
              }}
            </span>
            <Badge :variant="revisionStateVariant(revision.state)" size="sm">
              {{ revisionStateLabel(revision.state) }}
            </Badge>
            <span class="req-history__meta"
              >{{ revision.author }} · {{ revision.created_at }}</span
            >
          </div>
          <p class="req-history__summary">{{ revision.summary }}</p>
          <pre class="req-history__markdown">{{ revision.markdown }}</pre>
        </li>
      </ul>

      <section
        class="req-history__compare"
        :aria-label="t('pm.requirements.historyCompare')"
      >
        <h4 class="req-history__compare-title">
          {{ t("pm.requirements.historyCompare") }}
        </h4>
        <div v-if="canCompare" class="req-history__compare-grid">
          <div class="req-history__compare-col">
            <div class="req-history__compare-head">
              <span class="req-history__compare-label">
                {{ t("pm.requirements.historyCompareOld") }}
              </span>
              <span class="req-history__compare-version">
                v{{ compareOldRevision?.version }}
              </span>
            </div>
            <pre class="req-history__markdown">{{
              compareOldRevision?.markdown
            }}</pre>
          </div>
          <div class="req-history__compare-col">
            <div class="req-history__compare-head">
              <span class="req-history__compare-label">
                {{ t("pm.requirements.historyCompareNew") }}
              </span>
              <span class="req-history__compare-version">
                v{{ compareNewRevision?.version }}
              </span>
            </div>
            <pre class="req-history__markdown">{{
              compareNewRevision?.markdown
            }}</pre>
          </div>
        </div>
        <EmptyState
          v-else
          :title="t('pm.requirements.historyCompareEmpty')"
          :description="t('pm.requirements.historyCompareEmptyDesc')"
          size="sm"
        />
        <div class="req-history__compare-picks">
          <label class="req-history__pick">
            <span>{{ t("pm.requirements.historyCompareOld") }}</span>
            <select v-model="compareOld" class="req-history__pick-select">
              <option
                v-for="revision in revisions"
                :key="`old-${revision.id}`"
                :value="revision.id"
              >
                v{{ revision.version }}
              </option>
            </select>
          </label>
          <label class="req-history__pick">
            <span>{{ t("pm.requirements.historyCompareNew") }}</span>
            <select v-model="compareNew" class="req-history__pick-select">
              <option
                v-for="revision in revisions"
                :key="`new-${revision.id}`"
                :value="revision.id"
              >
                v{{ revision.version }}
              </option>
            </select>
          </label>
        </div>
      </section>

      <div class="req-history__footer">
        <Button
          variant="secondary"
          size="sm"
          @click="emit('update:open', false)"
        >
          {{ t("pm.requirements.historyClose") }}
        </Button>
      </div>
    </div>
  </Sheet>
</template>

<style scoped>
.req-history {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.req-history__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.req-history__item {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: var(--space-3);
}

.req-history__item-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.req-history__version {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.req-history__meta {
  margin-left: auto;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.req-history__summary {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.req-history__markdown {
  margin: 0;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-xs);
  background: var(--surface-raised);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  white-space: pre-wrap;
  word-break: break-word;
}

.req-history__compare {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding-top: var(--space-3);
  border-top: var(--hairline) solid var(--border-default);
}

.req-history__compare-title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.req-history__compare-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
}

.req-history__compare-col {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
}

.req-history__compare-head {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
}

.req-history__compare-label {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.req-history__compare-version {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.req-history__compare-picks {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.req-history__pick {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.req-history__pick-select {
  height: var(--size-control-sm);
  padding: 0 var(--space-2);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.req-history__pick-select:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.req-history__footer {
  display: flex;
  justify-content: flex-end;
}
</style>
