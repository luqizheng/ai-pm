<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Card from "@/components/ui/card/Card.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import {
  listDesignAnalysis,
  listDesignApprovals,
  listDesigns,
  type DesignAnalysisRow,
  type DesignApprovalInboxRow,
  type DesignEntry
} from "@/api/designs";
import DesignFrame from "../DesignFrame.vue";
import { formatDateTime, revisionTone } from "../design-contract";
import { useDesignProject } from "../use-design-project";

/**
 * S03-03 技术设计页（docs/ui/s03 页面结构 §8「Technical Designs」）：
 * 设计列表 / Revision / 评审状态。
 *
 * 三个项目级读端点拼出列表（无 N+1）：
 *   - GET /designs              → 设计行（当前正式版本 + 最新版本指针）；
 *   - GET /design-approvals     → 按 design_id 归并的待办审批（评审状态）；
 *   - GET /design-analysis      → 每个设计的未闭评审问题数。
 *
 * **评审状态 ≠ 已批准**：pending 审批只表示「等待人类裁决」，只有修订
 * state=approved 才是设计已通过评审（§4/§5）。
 */
const { t } = useI18n();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const designs = ref<DesignEntry[]>([]);
const approvals = ref<DesignApprovalInboxRow[]>([]);
const analysis = ref<DesignAnalysisRow[]>([]);
const loading = ref(true);
const loadError = ref(false);

async function load() {
  if (!hasProject.value) {
    designs.value = [];
    approvals.value = [];
    analysis.value = [];
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    designs.value = await listDesigns(projectId.value);
    approvals.value = await listDesignApprovals(projectId.value);
    analysis.value = await listDesignAnalysis(projectId.value);
  } catch {
    loadError.value = true;
    designs.value = [];
    approvals.value = [];
    analysis.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(projectId, load);

/** 按设计归并的待办审批数（评审状态列）。 */
const pendingByDesign = computed(() => {
  const map = new Map<string, number>();
  for (const row of approvals.value) {
    if (row.status !== "pending" || !row.design_id) continue;
    map.set(row.design_id, (map.get(row.design_id) ?? 0) + 1);
  }
  return map;
});

/** 按设计归并的未闭评审问题数。 */
const findingsByDesign = computed(() => {
  const map = new Map<string, number>();
  for (const row of analysis.value) {
    map.set(row.design_id, row.open_finding_count);
  }
  return map;
});

/** 状态筛选（全部 / 当前正式版本已批准 / 待评审 / 草稿）。 */
const stateFilter = ref("all");
const STATE_FILTERS = [
  "all",
  "approved",
  "pending_confirmation",
  "draft"
] as const;

const filtered = computed(() =>
  designs.value.filter(row => {
    if (stateFilter.value === "all") return true;
    return row.current_state === stateFilter.value;
  })
);

function openDesign(designId: string) {
  void router.push(`/project/design/technical-designs/${designId}`);
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('s03.design.module') }]"
    :title="t('s03.design.page.designsTitle')"
    :description="t('s03.design.page.designsDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="designs-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("s03.design.common.refresh") }}
      </Button>
    </template>

    <DesignFrame>
      <Alert
        v-if="loadError"
        variant="danger"
        size="sm"
        :title="t('s03.design.common.errorTitle')"
      />

      <EmptyState
        v-if="!hasProject"
        :title="t('s03.design.common.noProjectTitle')"
        :description="t('s03.design.common.noProjectHint')"
      />

      <EmptyState
        v-else-if="!loading && designs.length === 0"
        :title="t('s03.design.designs.emptyTitle')"
        :description="t('s03.design.designs.emptyHint')"
      />

      <template v-else>
        <nav
          class="designs-filters"
          :aria-label="t('s03.design.common.filters')"
        >
          <button
            v-for="value in STATE_FILTERS"
            :key="value"
            type="button"
            class="designs-filters__item"
            :class="{ 'is-active': stateFilter === value }"
            :aria-pressed="stateFilter === value"
            :data-testid="`designs-filter-${value}`"
            @click="stateFilter = value"
          >
            {{
              value === "all"
                ? t("s03.design.common.all")
                : t(`s03.design.revisionState.${value}`)
            }}
          </button>
        </nav>

        <Card class="designs-table-card">
          <table
            class="designs-table"
            :aria-label="t('s03.design.page.designsTitle')"
          >
            <thead>
              <tr>
                <th scope="col">{{ t("s03.design.designs.colCode") }}</th>
                <th scope="col">{{ t("s03.design.designs.colTitle") }}</th>
                <th scope="col">
                  {{ t("s03.design.designs.colCurrentVersion") }}
                </th>
                <th scope="col">
                  {{ t("s03.design.designs.colLatestVersion") }}
                </th>
                <th scope="col">{{ t("s03.design.designs.colReview") }}</th>
                <th scope="col">{{ t("s03.design.designs.colFindings") }}</th>
                <th scope="col">{{ t("s03.design.designs.colUpdated") }}</th>
                <th scope="col">
                  <span class="sr-only">{{
                    t("s03.design.common.actions")
                  }}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="row in filtered"
                :key="row.id"
                :data-testid="`design-row-${row.code}`"
              >
                <td class="designs-table__code">{{ row.code }}</td>
                <td>{{ row.title }}</td>
                <td>
                  <span class="designs-table__version">
                    {{
                      row.current_version
                        ? t("s03.design.analysis.version", {
                            version: row.current_version
                          })
                        : t("s03.design.common.noRevision")
                    }}
                  </span>
                  <Badge :variant="revisionTone(row.current_state)" size="sm">
                    {{
                      row.current_state
                        ? t(`s03.design.revisionState.${row.current_state}`)
                        : t("s03.design.common.none")
                    }}
                  </Badge>
                </td>
                <td>
                  <span class="designs-table__version">
                    {{
                      row.latest_version
                        ? t("s03.design.analysis.version", {
                            version: row.latest_version
                          })
                        : t("s03.design.common.noRevision")
                    }}
                  </span>
                  <Badge :variant="revisionTone(row.latest_state)" size="sm">
                    {{
                      row.latest_state
                        ? t(`s03.design.revisionState.${row.latest_state}`)
                        : t("s03.design.common.none")
                    }}
                  </Badge>
                </td>
                <td>
                  <Badge
                    v-if="(pendingByDesign.get(row.id) ?? 0) > 0"
                    variant="warning"
                    size="sm"
                  >
                    {{
                      t("s03.design.designs.pendingApproval", {
                        count: pendingByDesign.get(row.id) ?? 0
                      })
                    }}
                  </Badge>
                  <span v-else class="designs-table__muted">
                    {{ t("s03.design.designs.noPendingApproval") }}
                  </span>
                </td>
                <td>
                  <Badge
                    v-if="(findingsByDesign.get(row.id) ?? 0) > 0"
                    variant="danger"
                    size="sm"
                  >
                    {{
                      t("s03.design.designs.openFindings", {
                        count: findingsByDesign.get(row.id) ?? 0
                      })
                    }}
                  </Badge>
                  <span v-else class="designs-table__muted">
                    {{ t("s03.design.designs.noFindings") }}
                  </span>
                </td>
                <td class="designs-table__muted">
                  {{ formatDateTime(row.updated_at) }}
                </td>
                <td>
                  <Button
                    variant="ghost"
                    size="sm"
                    :data-testid="`design-open-${row.code}`"
                    @click="openDesign(row.id)"
                  >
                    {{ t("s03.design.common.open") }}
                  </Button>
                </td>
              </tr>
            </tbody>
          </table>
        </Card>
      </template>
    </DesignFrame>
  </PageFrame>
</template>

<style scoped>
.designs-filters {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.designs-filters__item {
  padding: var(--space-1) var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
  background: transparent;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.designs-filters__item:hover {
  color: var(--ws-text-primary);
}

.designs-filters__item:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.designs-filters__item.is-active {
  color: var(--ws-text-primary);
  border-color: var(--ws-accent);
}

.designs-table-card {
  padding: 0;
  overflow: hidden;
}

.designs-table {
  width: 100%;
  border-collapse: collapse;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--ws-text-primary);
}

.designs-table th {
  padding: var(--space-3) var(--space-4);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
  text-align: left;
  color: var(--ws-text-secondary);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.designs-table td {
  padding: var(--space-3) var(--space-4);
  vertical-align: middle;
  border-bottom: var(--border-1) solid var(--ws-border);
}

.designs-table tbody tr:last-child td {
  border-bottom: none;
}

.designs-table__code {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}

.designs-table__version {
  margin-right: var(--space-2);
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-secondary);
}

.designs-table__muted {
  color: var(--ws-text-secondary);
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border-width: 0;
}
</style>
