<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { ChevronRight } from "lucide-vue-next";
import { HttpError } from "@digua/http";
import PageFrame from "@/components/shell/PageFrame.vue";
import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import {
  listAnalysisReports,
  type AnalysisReportRow,
  type AnalysisReportStatus
} from "@/api/analysis-reports";
import { useShellStore } from "@/stores/shell";

/**
 * Analysis 独立首页（/project/analysis，P2-02）——报告列表。
 *
 * - 状态分区迁移自 AnalysisReportsPanel（D-5 报告门模型）：analyzing /
 *   analyzed（待审理，默认展示）/ closed / failed（历史，「显示历史」开关
 *   默认隐藏）。`cancelled` 在 API 类型存在但 UI 不分区——终态不占门，
 *   沿用旧面板展示策略，不自创分区（P2-02 登记）。
 * - analyzing 行存在时 3s 轮询（分析中 → 待审理自动浮现，行为等价迁移）；
 *   行点击进详情页 /project/analysis/reports/{id}（analyzing 行不可进，
 *   与旧面板「不展开」一致）。
 * - 深链承接：?report={id} → 定位校验后跳详情页（P2-06 起为知识页兼容跳转的
 *   最终落点；不丢 ID 与项目上下文）。ID 不在列表中（非法/跨项目）→ 404
 *   语义错误态，不静默——GAP-6 列表侧呈现；详情页直链的显式错误态随 P2-03。
 * - 列表数据唯一来源 = GET /v1/pm/analysis-reports（契约零改动）；无分页
 *   继承现状（GAP-2 独立登记）。「来源」列不自造归并（analysis_run 双写入者
 *   分叉属既有登记，UNKNOWN 不自填）。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const currentProjectId = computed(() => shell.currentProjectId);

const rows = ref<AnalysisReportRow[]>([]);
const loading = ref(false);
const loadError = ref("");
/** 404 语义态：项目不可见/无权限（后端 404 不泄露存在性）。 */
const notFound = ref(false);

/** 已关闭/失败历史默认隐藏（审阅流 v2：默认只看待办）。 */
const showHistory = ref(false);

const GROUPS: AnalysisReportStatus[] = [
  "analyzing",
  "analyzed",
  "closed",
  "failed"
];

function statusVariant(
  status: string
): "success" | "warning" | "danger" | "neutral" {
  if (status === "analyzed") return "warning";
  if (status === "closed") return "success";
  if (status === "failed") return "danger";
  return "neutral";
}

function shortSha(sha: string): string {
  return sha.slice(0, 12);
}

function formatTime(value: string | null): string {
  if (!value) return "—";
  return value.slice(0, 16).replace("T", " ");
}

const grouped = computed(() =>
  GROUPS.filter(
    status =>
      showHistory.value || status === "analyzing" || status === "analyzed"
  )
    .map(status => ({
      status,
      items: rows.value.filter(row => row.status === status)
    }))
    .filter(group => group.items.length > 0)
);

/** 默认视图下的可见行数（历史隐藏时用于空态判定）。 */
const visibleRowCount = computed(
  () =>
    rows.value.filter(
      row =>
        showHistory.value ||
        row.status === "analyzing" ||
        row.status === "analyzed"
    ).length
);

const hasAnalyzing = computed(() =>
  rows.value.some(row => row.status === "analyzing")
);

function pendingTotal(row: AnalysisReportRow): number {
  return row.pending_proposals + (row.has_pending_kb ? 1 : 0);
}

async function loadLists(silent = false) {
  if (!currentProjectId.value) return;
  if (!silent) {
    loading.value = true;
    loadError.value = "";
    notFound.value = false;
  }
  try {
    const result = await listAnalysisReports(currentProjectId.value);
    rows.value = result.items;
  } catch (error) {
    if (error instanceof HttpError && error.status === 404) {
      // 不可见项目统一 404 → 无权限语义态，不泄露存在性
      notFound.value = true;
    } else if (!silent) {
      loadError.value = t("analysis.error.load");
    }
  } finally {
    if (!silent) loading.value = false;
  }
}

/** analyzing 行存在时 3s 轮询（分析中 → 待审理自动浮现）。 */
let pollTimer: ReturnType<typeof setInterval> | null = null;

function syncPolling(): void {
  if (hasAnalyzing.value && pollTimer === null) {
    pollTimer = setInterval(() => {
      void loadLists(true);
    }, 3000);
  } else if (!hasAnalyzing.value && pollTimer !== null) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

watch(hasAnalyzing, syncPolling, { immediate: true });

watch(
  currentProjectId,
  () => {
    rows.value = [];
    loadError.value = "";
    notFound.value = false;
    void loadLists();
  },
  { immediate: true }
);

// 深链承接（P2-02）：/project/analysis?report={id} → 校验定位后跳详情页。
// 保留其余 query（project_id 等），report 自身升级为路径参数——不退化为丢参 redirect。
// 同时监听项目上下文：深链先于项目注入到达时，项目就绪后重放（旧面板兜底语义）。
watch(
  [() => route.query.report, currentProjectId],
  async ([reportId]) => {
    if (typeof reportId !== "string" || !reportId || !currentProjectId.value)
      return;
    if (!rows.value.length) await loadLists();
    const row = rows.value.find(item => item.id === reportId);
    if (!row) {
      // 非法/跨项目 ID：404 语义错误态，不静默（GAP-6 列表侧）
      notFound.value = true;
      return;
    }
    const { report: _report, ...rest } = route.query;
    await router.replace({
      path: `/project/analysis/reports/${reportId}`,
      query: rest
    });
  },
  { immediate: true }
);

/** 行点击 → 详情页；analyzing 行不可进（与旧面板不展开一致）。 */
function openReport(row: AnalysisReportRow): void {
  if (row.status === "analyzing") return;
  void router.push(`/project/analysis/reports/${row.id}`);
}

onBeforeUnmount(() => {
  if (pollTimer !== null) clearInterval(pollTimer);
});
</script>

<template>
  <PageFrame
    :title="t('analysis.page.title')"
    :description="t('analysis.page.description')"
  >
    <div class="analysis-page">
      <div class="an-toolbar">
        <Button
          size="sm"
          variant="secondary"
          class="an-history-toggle"
          @click="showHistory = !showHistory"
        >
          {{
            showHistory
              ? t("analysis.list.hideHistory")
              : t("analysis.list.showHistory")
          }}
        </Button>
      </div>

      <EmptyState
        v-if="!currentProjectId"
        size="sm"
        :title="t('analysis.state.noProject')"
      />
      <template v-else>
        <Alert v-if="loadError" variant="danger" size="sm" role="alert">
          {{ loadError }}
        </Alert>
        <EmptyState
          v-if="notFound"
          size="sm"
          :title="t('analysis.error.notFound')"
        />
        <p v-else-if="loading" class="an-hint">
          {{ t("analysis.list.loading") }}
        </p>
        <EmptyState
          v-else-if="!visibleRowCount"
          size="sm"
          :title="t('analysis.list.empty')"
        />
        <template v-else>
          <section
            v-for="group in grouped"
            :key="group.status"
            class="an-group"
          >
            <h4 class="an-group-title">
              {{ t(`analysis.status.${group.status}`) }}
              <span class="an-group-count">{{ group.items.length }}</span>
            </h4>
            <ul class="an-list">
              <li v-for="row in group.items" :key="row.id" class="an-row">
                <button
                  class="an-head"
                  :disabled="row.status === 'analyzing'"
                  @click="openReport(row)"
                >
                  <span class="an-title">
                    {{ row.repository_name }} @ {{ shortSha(row.commit_sha) }}
                  </span>
                  <Badge :variant="statusVariant(row.status)">
                    {{ t(`analysis.status.${row.status}`) }}
                  </Badge>
                  <Badge
                    v-if="row.run_mode === 'incremental'"
                    variant="neutral"
                  >
                    {{ t("analysis.list.incrementalBadge") }}
                  </Badge>
                  <span class="an-meta">
                    <template v-if="row.status === 'analyzing'">
                      {{ row.stage_detail || t("analysis.list.analyzingHint") }}
                    </template>
                    <template v-else-if="row.status === 'analyzed'">
                      {{
                        pendingTotal(row) > 0
                          ? t("analysis.list.pendingItems", {
                              n: pendingTotal(row)
                            })
                          : t("analysis.list.allDisposed")
                      }}
                    </template>
                    <template v-else-if="row.status === 'closed'">
                      {{
                        t("analysis.list.closedAt", {
                          time: formatTime(row.closed_at)
                        })
                      }}
                    </template>
                    <template v-else-if="row.status === 'failed'">
                      {{ row.error_code || t("analysis.list.failedReason") }}
                    </template>
                    <template v-else>{{ formatTime(row.updated_at) }}</template>
                  </span>
                  <ChevronRight
                    v-if="row.status !== 'analyzing'"
                    class="an-chevron"
                    aria-hidden="true"
                  />
                </button>

                <p
                  v-if="row.status === 'failed' && row.error_detail"
                  class="an-error"
                  role="alert"
                >
                  {{ row.error_detail }}
                </p>
              </li>
            </ul>
          </section>
        </template>
      </template>
    </div>
  </PageFrame>
</template>

<style scoped>
.analysis-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  max-width: 860px;
}

.an-toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.an-history-toggle {
  margin-left: auto;
}

.an-group-title {
  margin: 0 0 var(--space-2);
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--text-secondary);
}

.an-group-count {
  margin-left: var(--space-2);
  color: var(--text-tertiary);
}

.an-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.an-row {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-card);
  display: flex;
  flex-direction: column;
}

.an-head {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  padding: var(--space-3) var(--space-4);
  background: transparent;
  border: 0;
  cursor: pointer;
  text-align: left;
}

.an-head:hover:not(:disabled) {
  background: var(--ws-hover-veil);
}

.an-head:disabled {
  cursor: default;
}

.an-title {
  font-weight: 600;
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.an-meta {
  margin-left: auto;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
  text-align: right;
}

.an-chevron {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--text-tertiary);
  transition: transform var(--duration-fast) var(--ease-decelerate);
}

.an-error {
  margin: 0;
  padding: 0 var(--space-4) var(--space-3);
  font-size: var(--font-size-caption);
  color: var(--text-warning);
  white-space: pre-wrap;
}

.an-hint {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}
</style>
