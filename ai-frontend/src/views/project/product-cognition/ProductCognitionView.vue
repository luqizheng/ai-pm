<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import PageFrame from "@/components/shell/PageFrame.vue";
import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import MarkdownViewer from "@/components/ui/markdown-editor/MarkdownViewer.vue";
import {
  getPcObject,
  listPcObjects,
  type ObjectAnalysisSource,
  type PcObjectDetail,
  type PcObjectRow
} from "@/api/knowledge";
import { useShellStore } from "@/stores/shell";

/**
 * Product Cognition — 独立业务入口。
 *
 * PC 对象仍由 /v1/pc/objects 提供；本页只聚焦产品认知对象及其正式版本、
 * 来源报告和溯源链。对象是只读发布视图，不在此页直接编辑权威认知正文。
 * 提案审批继续由统一人工审核中心负责，避免出现第二套审批入口。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const shell = useShellStore();

const projectId = computed(() => {
  const query = route.query.project_id;
  if (typeof query === "string" && query) return query;
  return shell.currentProjectId || "";
});

const rows = ref<PcObjectRow[]>([]);
const details = ref<Record<string, PcObjectDetail>>({});
const expanded = ref<Record<string, boolean>>({});
const loading = ref(false);
const detailLoading = ref<string | null>(null);
const error = ref("");

function lifecycleVariant(status: string): "success" | "warning" | "danger" | "neutral" {
  if (status === "active") return "success";
  if (status === "draft" || status === "identity") return "warning";
  if (status === "rejected" || status === "failed") return "danger";
  return "neutral";
}

function formatTime(value: string | null): string {
  if (!value) return "—";
  return value.slice(0, 16).replace("T", " ");
}

function shortSha(value: string | null): string {
  return value ? value.slice(0, 12) : "—";
}

async function load(): Promise<void> {
  if (!projectId.value) {
    rows.value = [];
    details.value = {};
    expanded.value = {};
    loading.value = false;
    return;
  }

  loading.value = true;
  error.value = "";
  try {
    const result = await listPcObjects(projectId.value);
    rows.value = result.items;
    const existing = new Set(result.items.map(item => item.object_id));
    details.value = Object.fromEntries(
      Object.entries(details.value).filter(([id]) => existing.has(id))
    );
    expanded.value = Object.fromEntries(
      Object.entries(expanded.value).filter(([id]) => existing.has(id))
    );
  } catch (cause) {
    rows.value = [];
    error.value =
      cause instanceof Error ? cause.message : t("s01.knowledge.error.load");
  } finally {
    loading.value = false;
  }
}

async function toggle(row: PcObjectRow): Promise<void> {
  const next = !expanded.value[row.object_id];
  expanded.value[row.object_id] = next;
  if (!next || details.value[row.object_id] || !projectId.value) return;

  detailLoading.value = row.object_id;
  error.value = "";
  try {
    details.value[row.object_id] = await getPcObject(
      row.object_id,
      projectId.value
    );
  } catch (cause) {
    expanded.value[row.object_id] = false;
    error.value =
      cause instanceof Error ? cause.message : t("s01.knowledge.error.load");
  } finally {
    detailLoading.value = null;
  }
}

function sources(row: PcObjectRow): ObjectAnalysisSource[] {
  return details.value[row.object_id]?.analysis_sources ?? [];
}

async function openSourceReport(reportId: string): Promise<void> {
  await router.push({
    path: `/project/analysis/reports/${encodeURIComponent(reportId)}`,
    query: projectId.value ? { project_id: projectId.value } : {}
  });
}

function openTrace(row: PcObjectRow): void {
  void router.push({
    path: `/project/knowledge/objects/pc/${encodeURIComponent(row.object_id)}`,
    query: projectId.value ? { project_id: projectId.value } : {}
  });
}

async function openProposalQueue(): Promise<void> {
  await router.push({
    path: "/project/review-center",
    query: {
      ...(projectId.value ? { project_id: projectId.value } : {}),
      kind: "proposal"
    }
  });
}

onMounted(load);
watch(projectId, () => {
  details.value = {};
  expanded.value = {};
  void load();
});
</script>

<template>
  <PageFrame
    :title="t('shell.app.side.item.productCognition')"

  >
    <div class="product-cognition">
      <div class="product-cognition__actions">
        <span class="product-cognition__count">
          {{ rows.length }} · {{ t("s01.knowledge.tab.pc") }}
        </span>
        <Button variant="secondary" size="sm" @click="openProposalQueue">
          {{ t("review.tabs.proposal") }}
        </Button>
        <Button variant="ghost" size="sm" :disabled="loading" @click="load">
          {{ t("environments.action.refresh") }}
        </Button>
      </div>

      <Alert v-if="error" variant="danger" role="alert">
        {{ error }}
      </Alert>

      <p v-if="loading" class="product-cognition__hint">
        {{ t("s01.knowledge.hint.loading") }}
      </p>
      <EmptyState
        v-else-if="!projectId"
        :title="t('s01.cognition.state.noProject.title')"
        :description="t('s01.cognition.state.noProject.description')"
      />
      <EmptyState
        v-else-if="!error && rows.length === 0"
        :title="t('s01.knowledge.hint.emptyPc')"
      />

      <ul v-else-if="rows.length > 0" class="product-cognition__list">
        <li
          v-for="row in rows"
          :key="row.object_id"
          class="product-cognition__item"
          :data-testid="`product-cognition-${row.object_id}`"
        >
          <button
            class="product-cognition__head"
            type="button"
            :aria-expanded="Boolean(expanded[row.object_id])"
            @click="toggle(row)"
          >
            <span class="product-cognition__name">{{ row.display_name }}</span>
            <Badge v-if="row.revision_no" variant="neutral">
              v{{ row.revision_no }}
            </Badge>
            <span class="product-cognition__type">{{ row.object_type }}</span>
            <Badge :variant="lifecycleVariant(row.lifecycle_status)">
              {{ t(`s01.knowledge.lifecycle.${row.lifecycle_status}`) }}
            </Badge>
            <span class="product-cognition__updated">
              {{ formatTime(row.updated_at) }}
            </span>
          </button>

          <div v-if="!expanded[row.object_id]" class="product-cognition__summary">
            <p v-if="row.change_summary">{{ row.change_summary }}</p>
            <p v-if="row.excerpt">{{ row.excerpt }}</p>
          </div>

          <div v-else class="product-cognition__detail">
            <p
              v-if="detailLoading === row.object_id && !details[row.object_id]"
              class="product-cognition__hint"
            >
              {{ t("s01.knowledge.hint.loading") }}
            </p>
            <template v-else-if="details[row.object_id]">
              <p
                v-if="!details[row.object_id].latest_revision"
                class="product-cognition__hint"
              >
                {{ t("s01.knowledge.detail.noRevision") }}
              </p>
              <template v-else>
                <div class="product-cognition__revision">
                  <span>
                    {{ t("s01.knowledge.field.revision") }}
                    v{{ details[row.object_id].latest_revision?.revision_no }}
                  </span>
                  <Badge
                    :variant="details[row.object_id].latest_revision?.versioned ? 'success' : 'neutral'"
                  >
                    {{
                      details[row.object_id].latest_revision?.versioned
                        ? t("s01.knowledge.detail.versioned")
                        : t("s01.knowledge.detail.unversioned")
                    }}
                  </Badge>
                  <span v-if="details[row.object_id].latest_revision?.analysis_mode">
                    {{ t("s01.knowledge.detail.analysisMode") }}:
                    {{ details[row.object_id].latest_revision?.analysis_mode }}
                  </span>
                </div>
                <p
                  v-if="details[row.object_id].latest_revision?.summary"
                  class="product-cognition__summary-text"
                >
                  {{ details[row.object_id].latest_revision?.summary }}
                </p>
                <MarkdownViewer
                  :model-value="details[row.object_id].latest_revision?.content ?? ''"
                />
              </template>

              <section v-if="sources(row).length" class="product-cognition__sources">
                <h3>{{ t("s01.knowledge.detail.sourceAnalysis") }}</h3>
                <ul>
                  <li
                    v-for="source in sources(row)"
                    :key="`${source.version_no}:${source.report_id ?? ''}`"
                  >
                    <span>v{{ source.version_no }}</span>
                    <button
                      v-if="source.report_id"
                      type="button"
                      class="product-cognition__link"
                      @click="openSourceReport(source.report_id)"
                    >
                      {{ source.repository_name ?? "—" }} @{{ shortSha(source.commit_sha) }}
                    </button>
                    <span v-else>
                      {{ t("s01.knowledge.detail.sourceAnalysisNone") }}
                    </span>
                  </li>
                </ul>
              </section>

              <div class="product-cognition__detail-actions">
                <Button variant="secondary" size="sm" @click="openTrace(row)">
                  {{ t("s01.knowledge.trace.viewTrace") }}
                </Button>
              </div>
            </template>
          </div>
        </li>
      </ul>
    </div>
  </PageFrame>
</template>

<style scoped>
.product-cognition {
  display: grid;
  gap: var(--space-4);
}

.product-cognition__actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-2);
}

.product-cognition__count {
  margin-right: auto;
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
}

.product-cognition__list {
  display: grid;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.product-cognition__item {
  overflow: hidden;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-base);
}

.product-cognition__head {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-3) var(--space-4);
  border: 0;
  background: transparent;
  color: var(--text-primary);
  cursor: pointer;
  text-align: left;
}

.product-cognition__head:hover {
  background: var(--surface-hover-veil);
}

.product-cognition__name {
  font-weight: var(--weight-semibold);
}

.product-cognition__type,
.product-cognition__updated,
.product-cognition__hint {
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
}

.product-cognition__updated {
  margin-left: auto;
}

.product-cognition__summary,
.product-cognition__detail {
  padding: 0 var(--space-4) var(--space-4);
}

.product-cognition__summary p,
.product-cognition__summary-text {
  margin: var(--space-1) 0 0;
  color: var(--text-secondary);
  font-size: var(--font-size-body);
}

.product-cognition__detail {
  display: grid;
  gap: var(--space-3);
}

.product-cognition__revision {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
}

.product-cognition__sources {
  border-top: 1px solid var(--border-subtle);
  padding-top: var(--space-3);
}

.product-cognition__sources h3 {
  margin: 0 0 var(--space-2);
  font-size: var(--font-size-body);
}

.product-cognition__sources ul {
  display: grid;
  gap: var(--space-2);
  margin: 0;
  padding-left: var(--space-4);
}

.product-cognition__sources li {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  color: var(--text-secondary);
  font-size: var(--font-size-micro);
}

.product-cognition__link {
  border: 0;
  padding: 0;
  background: transparent;
  color: var(--accent);
  cursor: pointer;
  text-decoration: underline;
}

.product-cognition__detail-actions {
  display: flex;
  justify-content: flex-end;
}

.product-cognition__hint {
  margin: 0;
}
</style>
