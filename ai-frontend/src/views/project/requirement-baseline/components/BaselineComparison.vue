<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Select from "@/components/ui/select/Select.vue";
import type { BaselineRecord } from "../baseline-contract";
import { compareBaselines } from "../fixtures";

/**
 * 基线比较（S02-UI-06 §7 + Deliverable 5）。
 *
 * 任意两基线比较：新增需求 / 移除需求 / 修改需求（修订版本变化 +
 * 两版验收标准差异陈述）。无差异时显示空态。
 */
interface Props {
  baselines: BaselineRecord[];
}

const props = defineProps<Props>();
const { t } = useI18n();

const ordered = computed(() =>
  [...props.baselines].sort((a, b) => b.version - a.version)
);

const oldId = ref(ordered.value[1]?.id ?? ordered.value[0]?.id ?? "");
const newId = ref(ordered.value[0]?.id ?? "");

const options = computed(() =>
  props.baselines.map(baseline => ({
    value: baseline.id,
    label: `${baseline.id} · ${t("pm.requirementBaseline.baselineVersion", { version: baseline.version })}`
  }))
);

const previous = computed(
  () => props.baselines.find(baseline => baseline.id === oldId.value) ?? null
);
const next = computed(
  () => props.baselines.find(baseline => baseline.id === newId.value) ?? null
);

const result = computed(() => {
  if (!previous.value || !next.value) return null;
  return compareBaselines(previous.value, next.value);
});

const hasDiff = computed(
  () =>
    result.value !== null &&
    (result.value.added.length > 0 ||
      result.value.removed.length > 0 ||
      result.value.modified.length > 0)
);

function baselineLabel(id: string) {
  const baseline = props.baselines.find(item => item.id === id);
  return baseline
    ? `${baseline.id} · ${t("pm.requirementBaseline.baselineVersion", { version: baseline.version })}`
    : id;
}
</script>

<template>
  <section class="rbc" :aria-label="t('pm.requirementBaseline.compareTitle')">
    <div class="rbc__head">
      <h3 class="rbc__title">{{ t("pm.requirementBaseline.compareTitle") }}</h3>
    </div>

    <div class="rbc__picks">
      <label class="rbc__pick">
        <span>{{ t("pm.requirementBaseline.compareOldLabel") }}</span>
        <Select
          :model-value="oldId"
          :options="options"
          size="sm"
          :aria-label="t('pm.requirementBaseline.compareOldAria')"
          @update:model-value="oldId = $event"
        />
      </label>
      <label class="rbc__pick">
        <span>{{ t("pm.requirementBaseline.compareNewLabel") }}</span>
        <Select
          :model-value="newId"
          :options="options"
          size="sm"
          :aria-label="t('pm.requirementBaseline.compareNewAria')"
          @update:model-value="newId = $event"
        />
      </label>
    </div>

    <EmptyState
      v-if="!result"
      size="sm"
      :title="t('pm.requirementBaseline.compareEmptyTitle')"
      :description="t('pm.requirementBaseline.compareEmptyDesc')"
    />

    <template v-else>
      <p class="rbc__scope">
        {{
          t("pm.requirementBaseline.compareScope", {
            from: baselineLabel(oldId),
            to: baselineLabel(newId)
          })
        }}
      </p>

      <EmptyState
        v-if="!hasDiff"
        size="sm"
        :title="t('pm.requirementBaseline.compareNoDiff')"
      />

      <div v-else class="rbc__groups">
        <div class="rbc__group">
          <div class="rbc__group-head">
            <Badge variant="success" size="sm">
              {{
                t("pm.requirementBaseline.compareAdded", {
                  count: result.added.length
                })
              }}
            </Badge>
          </div>
          <ul v-if="result.added.length > 0" class="rbc__items">
            <li
              v-for="entry in result.added"
              :key="`add-${entry.requirement_code}`"
            >
              <span class="rbc__code">{{ entry.requirement_code }}</span>
              <span class="rbc__item-title">{{ entry.title }}</span>
              <span class="rbc__rev">
                {{
                  t("pm.requirements.revisionVersion", {
                    version: entry.revision_version
                  })
                }}
              </span>
            </li>
          </ul>
        </div>

        <div class="rbc__group">
          <div class="rbc__group-head">
            <Badge variant="danger" size="sm">
              {{
                t("pm.requirementBaseline.compareRemoved", {
                  count: result.removed.length
                })
              }}
            </Badge>
          </div>
          <ul v-if="result.removed.length > 0" class="rbc__items">
            <li
              v-for="entry in result.removed"
              :key="`rm-${entry.requirement_code}`"
            >
              <span class="rbc__code">{{ entry.requirement_code }}</span>
              <span class="rbc__item-title">{{ entry.title }}</span>
              <span class="rbc__rev">
                {{
                  t("pm.requirements.revisionVersion", {
                    version: entry.revision_version
                  })
                }}
              </span>
            </li>
          </ul>
        </div>

        <div class="rbc__group">
          <div class="rbc__group-head">
            <Badge variant="warning" size="sm">
              {{
                t("pm.requirementBaseline.compareModified", {
                  count: result.modified.length
                })
              }}
            </Badge>
          </div>
          <ul v-if="result.modified.length > 0" class="rbc__modified">
            <li
              v-for="item in result.modified"
              :key="`mod-${item.requirement_code}`"
              class="rbc__modified-item"
            >
              <div class="rbc__modified-head">
                <span class="rbc__code">{{ item.requirement_code }}</span>
                <span class="rbc__item-title">{{ item.title }}</span>
                <span class="rbc__rev">
                  {{
                    t("pm.requirements.revisionVersion", {
                      version: item.old_revision
                    })
                  }}
                  →
                  {{
                    t("pm.requirements.revisionVersion", {
                      version: item.new_revision
                    })
                  }}
                </span>
              </div>
              <ul
                v-if="
                  item.criteria_removed.length > 0 ||
                  item.criteria_added.length > 0
                "
                class="rbc__criteria"
              >
                <li
                  v-for="statement in item.criteria_removed"
                  :key="`rm-${statement}`"
                  class="rbc__criterion rbc__criterion--removed"
                >
                  <span class="rbc__criterion-marker">−</span>
                  <span>{{ statement }}</span>
                </li>
                <li
                  v-for="statement in item.criteria_added"
                  :key="`add-${statement}`"
                  class="rbc__criterion rbc__criterion--added"
                >
                  <span class="rbc__criterion-marker">+</span>
                  <span>{{ statement }}</span>
                </li>
              </ul>
            </li>
          </ul>
        </div>
      </div>
    </template>
  </section>
</template>

<style scoped>
.rbc {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rbc__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.rbc__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rbc__picks {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.rbc__pick {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rbc__scope {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rbc__groups {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.rbc__group {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.rbc__group-head {
  display: flex;
  align-items: center;
}

.rbc__items {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rbc__items li {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  border-bottom: var(--hairline) solid var(--border-default);
  padding-bottom: var(--space-1);
}

.rbc__code {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rbc__item-title {
  flex: 1 1 0;
  min-width: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rbc__rev {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rbc__modified {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rbc__modified-item {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: var(--space-3);
}

.rbc__modified-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rbc__criteria {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rbc__criterion {
  display: flex;
  gap: var(--space-2);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.rbc__criterion-marker {
  font-family: var(--font-mono);
  flex: none;
}

.rbc__criterion--removed {
  color: var(--danger-strong);
  text-decoration: line-through;
}

.rbc__criterion--added {
  color: var(--success-strong);
}
</style>
