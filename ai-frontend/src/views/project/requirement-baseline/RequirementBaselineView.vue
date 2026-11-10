<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Card from "@/components/ui/card/Card.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import {
  getRequirementBaseline,
  type RequirementBaseline
} from "@/api/requirement-read";
import { useDesignProject } from "@/views/project/design/use-design-project";

/**
 * 需求基线页（S02-UI-04，接真）。
 *
 * 数据 = GET /projects/{id}/requirement-baseline。
 *
 * 关键语义（S02 裁决 R3）：**需求基线 = 全部已确认需求的当前正式修订
 * 集合**——它是**派生视图**，不是存储实体；也不与 S03 的实施基线混用
 * （后者是设计修订 + ADR + 仓库基准的冻结快照）。
 *
 * 因此本页没有「冻结基线」「版本比较」这类写动作：基线随需求确认状态
 * 实时变化，快照语义属于 S03 的实施基线。
 */
const { t } = useI18n();
const { projectId, hasProject } = useDesignProject();

const baseline = ref<RequirementBaseline | null>(null);
const loading = ref(true);
const loadError = ref(false);

async function load() {
  if (!hasProject.value) {
    baseline.value = null;
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    baseline.value = await getRequirementBaseline(projectId.value);
  } catch {
    loadError.value = true;
    baseline.value = null;
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(projectId, load);

const items = computed(() => baseline.value?.items ?? []);
const hasGap = computed(() => (baseline.value?.unconfirmed_count ?? 0) > 0);

function stateTone(
  state: string | null | undefined
): "success" | "warning" | "neutral" | "danger" {
  switch (state) {
    case "approved":
      return "success";
    case "pending_confirmation":
      return "warning";
    case "rejected":
      return "danger";
    default:
      return "neutral";
  }
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('pm.requirements.pageTitle') }]"
    :title="t('pm.requirementBaseline.pageTitle')"
    :description="t('pm.requirementBaseline.pageDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="rb-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("pm.requirementBaseline.refresh") }}
      </Button>
    </template>

    <Alert
      v-if="loadError"
      variant="danger"
      size="sm"
      :title="t('pm.requirementBaseline.errorTitle')"
    />

    <EmptyState
      v-if="!hasProject"
      :title="t('pm.requirementBaseline.noProjectTitle')"
      :description="t('pm.requirementBaseline.noProjectHint')"
    />

    <EmptyState
      v-else-if="!loading && items.length === 0"
      :title="t('pm.requirementBaseline.emptyTitle')"
      :description="t('pm.requirementBaseline.emptyHint')"
    />

    <template v-else>
      <div class="rb-stats" data-testid="rb-stats">
        <span class="rb-stat">
          <span class="rb-stat__label">
            {{ t("pm.requirementBaseline.statTotal") }}
          </span>
          <span class="rb-stat__value">{{ baseline?.total_count ?? 0 }}</span>
        </span>
        <span class="rb-stat">
          <span class="rb-stat__label">
            {{ t("pm.requirementBaseline.statConfirmed") }}
          </span>
          <span class="rb-stat__value is-success">
            {{ baseline?.confirmed_count ?? 0 }}
          </span>
        </span>
        <span class="rb-stat">
          <span class="rb-stat__label">
            {{ t("pm.requirementBaseline.statUnconfirmed") }}
          </span>
          <span class="rb-stat__value" :class="{ 'is-warning': hasGap }">
            {{ baseline?.unconfirmed_count ?? 0 }}
          </span>
        </span>
      </div>

      <Alert
        v-if="hasGap"
        variant="warning"
        size="sm"
        :title="t('pm.requirementBaseline.gapTitle')"
      >
        {{
          t("pm.requirementBaseline.gapHint", {
            count: baseline?.unconfirmed_count ?? 0
          })
        }}
      </Alert>

      <Card class="rb-table-card">
        <table
          class="rb-table"
          :aria-label="t('pm.requirementBaseline.pageTitle')"
        >
          <thead>
            <tr>
              <th scope="col">{{ t("pm.requirementBaseline.colCode") }}</th>
              <th scope="col">{{ t("pm.requirementBaseline.colTitle") }}</th>
              <th scope="col">{{ t("pm.requirementBaseline.colVersion") }}</th>
              <th scope="col">{{ t("pm.requirementBaseline.colState") }}</th>
              <th scope="col">{{ t("pm.requirementBaseline.colCriteria") }}</th>
              <th scope="col">{{ t("pm.requirementBaseline.colSources") }}</th>
              <th scope="col">
                {{ t("pm.requirementBaseline.colConfirmed") }}
              </th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="item in items"
              :key="item.requirement_id"
              :data-testid="`rb-row-${item.code}`"
            >
              <td class="rb-table__code">{{ item.code }}</td>
              <td>{{ item.title }}</td>
              <td class="rb-table__num">
                {{
                  item.version
                    ? t("pm.requirementBaseline.versionBadge", {
                        version: item.version
                      })
                    : t("pm.requirementBaseline.noRevision")
                }}
              </td>
              <td>
                <Badge :variant="stateTone(item.state)" size="sm">
                  {{
                    item.state
                      ? t(`pm.requirementBaseline.state.${item.state}`)
                      : t("pm.requirementBaseline.noRevision")
                  }}
                </Badge>
              </td>
              <td class="rb-table__num">
                {{ item.acceptance_criteria_count }}
              </td>
              <td class="rb-table__num">{{ item.source_count }}</td>
              <td class="rb-table__muted">
                {{
                  item.confirmed_by ?? t("pm.requirementBaseline.notConfirmed")
                }}
              </td>
            </tr>
          </tbody>
        </table>
      </Card>
    </template>
  </PageFrame>
</template>

<style scoped>
.rb-stats {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-6);
  padding: var(--space-3) var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-lg);
}

.rb-stat {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
}

.rb-stat__label {
  color: var(--ws-text-secondary);
}

.rb-stat__value {
  font-variant-numeric: tabular-nums;
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.rb-stat__value.is-success {
  color: var(--ws-color-success-text);
}

.rb-stat__value.is-warning {
  color: var(--ws-color-warning-text);
}

.rb-table-card {
  padding: 0;
  overflow: hidden;
}

.rb-table {
  width: 100%;
  border-collapse: collapse;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--ws-text-primary);
}

.rb-table th {
  padding: var(--space-3) var(--space-4);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
  text-align: left;
  color: var(--ws-text-secondary);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.rb-table td {
  padding: var(--space-3) var(--space-4);
  vertical-align: middle;
  border-bottom: var(--border-1) solid var(--ws-border);
}

.rb-table tbody tr:last-child td {
  border-bottom: none;
}

.rb-table__code {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}

.rb-table__num {
  font-variant-numeric: tabular-nums;
}

.rb-table__muted {
  color: var(--ws-text-secondary);
}
</style>
