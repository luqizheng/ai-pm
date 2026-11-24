<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Plus, RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Card from "@/components/ui/card/Card.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import FormField from "@/components/ui/form-field/FormField.vue";
import Input from "@/components/ui/input/Input.vue";
import { PageFrame } from "@/components/shell";
import { translateError } from "@/api/base";
import {
  createAdr,
  listAdrs,
  listDesignApprovals,
  type AdrEntry,
  type DesignApprovalInboxRow
} from "@/api/designs";
import DesignFrame from "../DesignFrame.vue";
import { adrTone, formatDateTime } from "../design-contract";
import { useDesignProject } from "../use-design-project";

/**
 * S03-04 架构决策页（docs/ui/s03 页面结构 §8「Architecture Decisions」）：
 * ADR 列表 / 决策状态 / 影响关系 / **新建 ADR**（R4/B04-02 写入口）。
 *
 * ADR 是**独立实体**（§3.3），不是技术设计的「第八章」——一个决策可影响
 * 多个设计、关联多个需求、被新决策替代，因此独立检索与追溯。
 *
 * **阻塞标记**（blockers_implementation）是 Readiness Gate 条件 3 的输入：
 * 阻塞决策处于未决状态时不允许冻结实施基线——列表必须让这条可见。
 */
const { t } = useI18n();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const adrs = ref<AdrEntry[]>([]);
const approvals = ref<DesignApprovalInboxRow[]>([]);
const loading = ref(true);
const loadError = ref(false);

async function load() {
  if (!hasProject.value) {
    adrs.value = [];
    approvals.value = [];
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    adrs.value = await listAdrs(projectId.value);
    approvals.value = await listDesignApprovals(projectId.value);
  } catch {
    loadError.value = true;
    adrs.value = [];
    approvals.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(projectId, load);

/** 按 ADR 归并的待办审批（adr_acceptance）。 */
const pendingByAdr = computed(() => {
  const map = new Map<string, number>();
  for (const row of approvals.value) {
    if (row.status !== "pending" || !row.adr_id) continue;
    map.set(row.adr_id, (map.get(row.adr_id) ?? 0) + 1);
  }
  return map;
});

const statusFilter = ref("all");
const STATUS_FILTERS = [
  "all",
  "proposed",
  "under_review",
  "accepted",
  "rejected",
  "superseded",
  "deprecated"
] as const;

const filtered = computed(() =>
  adrs.value.filter(row => {
    if (statusFilter.value === "all") return true;
    return row.status === statusFilter.value;
  })
);

function openAdr(adrId: string) {
  void router.push(`/project/design/adrs/${adrId}`);
}

// ── 新建 ADR（R4/B04-02 写入口）──────────────────────────────────────

const showCreate = ref(false);
const creating = ref(false);
const createError = ref("");
const createForm = reactive({
  title: "",
  decision_question: "",
  context: "",
  rationale: "",
  consequences: "",
  blockers_implementation: false
});
const createInvalid = computed(
  () => !createForm.title.trim() || !createForm.decision_question.trim()
);

async function submitCreate() {
  if (!projectId.value || createInvalid.value) return;
  creating.value = true;
  createError.value = "";
  try {
    const detail = await createAdr(projectId.value, {
      title: createForm.title.trim(),
      decision_question: createForm.decision_question.trim(),
      context: createForm.context.trim() || undefined,
      rationale: createForm.rationale.trim() || undefined,
      consequences: createForm.consequences.trim() || undefined,
      blockers_implementation: createForm.blockers_implementation
    });
    showCreate.value = false;
    createForm.title = "";
    createForm.decision_question = "";
    createForm.context = "";
    createForm.rationale = "";
    createForm.consequences = "";
    createForm.blockers_implementation = false;
    void router.push(`/project/design/adrs/${detail.adr.id}`);
  } catch (error) {
    createError.value = translateError(String(error));
  } finally {
    creating.value = false;
  }
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('s03.design.module') }]"
    :title="t('s03.design.page.adrsTitle')"
    :description="t('s03.design.page.adrsDesc')"
  >
    <template #actions>
      <Button
        variant="primary"
        size="sm"
        data-testid="adrs-create"
        @click="showCreate = !showCreate"
      >
        <Plus :size="14" />
        {{ t("s03.design.adrs.create") }}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        data-testid="adrs-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("s03.design.common.refresh") }}
      </Button>
    </template>

    <DesignFrame>
      <!-- 新建 ADR（R4/B04-02）：独立业务实体；决策问题必填。 -->
      <Card v-if="showCreate && hasProject" class="create-card">
        <template #header>
          {{ t("s03.design.adrs.createTitle") }}
        </template>
        <FormField :label="t('s03.design.adrs.fieldTitle')">
          <Input v-model="createForm.title" data-testid="adr-create-title" />
        </FormField>
        <FormField :label="t('s03.design.adrs.fieldQuestion')">
          <Input
            v-model="createForm.decision_question"
            data-testid="adr-create-question"
          />
        </FormField>
        <FormField :label="t('s03.design.adrs.fieldContext')">
          <textarea
            v-model="createForm.context"
            class="create-card__text"
            rows="4"
            data-testid="adr-create-context"
          />
        </FormField>
        <FormField :label="t('s03.design.adrs.fieldRationale')">
          <textarea
            v-model="createForm.rationale"
            class="create-card__text"
            rows="3"
            data-testid="adr-create-rationale"
          />
        </FormField>
        <FormField :label="t('s03.design.adrs.fieldConsequences')">
          <textarea
            v-model="createForm.consequences"
            class="create-card__text"
            rows="3"
            data-testid="adr-create-consequences"
          />
        </FormField>
        <label class="create-card__check">
          <input
            v-model="createForm.blockers_implementation"
            type="checkbox"
            data-testid="adr-create-blocking"
          />
          {{ t("s03.design.adrs.fieldBlocking") }}
        </label>
        <Alert v-if="createError" variant="danger" size="sm">
          <span data-testid="adr-create-error">{{ createError }}</span>
        </Alert>
        <div class="create-card__actions">
          <Button
            variant="primary"
            size="sm"
            :loading="creating"
            :disabled="createInvalid"
            data-testid="adr-create-submit"
            @click="submitCreate"
          >
            {{ t("s03.design.adrs.create") }}
          </Button>
          <Button variant="ghost" size="sm" @click="showCreate = false">
            {{ t("s03.design.common.cancel") }}
          </Button>
        </div>
      </Card>

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
        v-else-if="!loading && adrs.length === 0"
        :title="t('s03.design.adrs.emptyTitle')"
        :description="t('s03.design.adrs.emptyHint')"
      />

      <template v-else>
        <nav class="adrs-filters" :aria-label="t('s03.design.common.filters')">
          <button
            v-for="value in STATUS_FILTERS"
            :key="value"
            type="button"
            class="adrs-filters__item"
            :class="{ 'is-active': statusFilter === value }"
            :aria-pressed="statusFilter === value"
            :data-testid="`adrs-filter-${value}`"
            @click="statusFilter = value"
          >
            {{
              value === "all"
                ? t("s03.design.common.all")
                : t(`s03.design.adrStatus.${value}`)
            }}
          </button>
        </nav>

        <Card class="adrs-table-card">
          <table
            class="adrs-table"
            :aria-label="t('s03.design.page.adrsTitle')"
          >
            <thead>
              <tr>
                <th scope="col">{{ t("s03.design.adrs.colCode") }}</th>
                <th scope="col">{{ t("s03.design.adrs.colTitle") }}</th>
                <th scope="col">{{ t("s03.design.adrs.colStatus") }}</th>
                <th scope="col">{{ t("s03.design.adrs.colBlocking") }}</th>
                <th scope="col">{{ t("s03.design.adrs.colReview") }}</th>
                <th scope="col">{{ t("s03.design.adrs.colUpdated") }}</th>
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
                :data-testid="`adr-row-${row.code}`"
              >
                <td class="adrs-table__code">{{ row.code }}</td>
                <td>{{ row.title }}</td>
                <td>
                  <Badge :variant="adrTone(row.status)" size="sm">
                    {{ t(`s03.design.adrStatus.${row.status}`) }}
                  </Badge>
                  <span v-if="row.current_version" class="adrs-table__version">
                    {{
                      t("s03.design.analysis.version", {
                        version: row.current_version
                      })
                    }}
                  </span>
                </td>
                <td>
                  <Badge
                    v-if="row.blockers_implementation"
                    variant="danger"
                    size="sm"
                  >
                    {{ t("s03.design.adrs.blocking") }}
                  </Badge>
                  <span v-else class="adrs-table__muted">
                    {{ t("s03.design.adrs.notBlocking") }}
                  </span>
                </td>
                <td>
                  <Badge
                    v-if="(pendingByAdr.get(row.id) ?? 0) > 0"
                    variant="warning"
                    size="sm"
                  >
                    {{
                      t("s03.design.designs.pendingApproval", {
                        count: pendingByAdr.get(row.id) ?? 0
                      })
                    }}
                  </Badge>
                  <span v-else class="adrs-table__muted">
                    {{ t("s03.design.designs.noPendingApproval") }}
                  </span>
                </td>
                <td class="adrs-table__muted">
                  {{ formatDateTime(row.updated_at) }}
                </td>
                <td>
                  <Button
                    variant="ghost"
                    size="sm"
                    :data-testid="`adr-open-${row.code}`"
                    @click="openAdr(row.id)"
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
.adrs-filters {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-1);
}

.adrs-filters__item {
  padding: var(--space-1) var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
  background: transparent;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.adrs-filters__item:hover {
  color: var(--ws-text-primary);
}

.adrs-filters__item:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.adrs-filters__item.is-active {
  color: var(--ws-text-primary);
  border-color: var(--ws-accent);
}

.adrs-table-card {
  padding: 0;
  overflow: hidden;
}

.adrs-table {
  width: 100%;
  border-collapse: collapse;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--ws-text-primary);
}

.adrs-table th {
  padding: var(--space-3) var(--space-4);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
  text-align: left;
  color: var(--ws-text-secondary);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.adrs-table td {
  padding: var(--space-3) var(--space-4);
  vertical-align: middle;
  border-bottom: var(--border-1) solid var(--ws-border);
}

.adrs-table tbody tr:last-child td {
  border-bottom: none;
}

.adrs-table__code {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}

.adrs-table__version {
  margin-left: var(--space-2);
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-secondary);
}

.adrs-table__muted {
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
.create-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin-bottom: var(--space-4);
}

.create-card__text {
  width: 100%;
  padding: var(--space-2);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-body);
  color: var(--ws-text-primary);
  background: var(--surface-base);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.create-card__check {
  display: flex;
  gap: var(--space-2);
  align-items: center;
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.create-card__actions {
  display: flex;
  gap: var(--space-2);
}

</style>
