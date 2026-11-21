<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { HttpError } from "@digua/http";
import PageFrame from "@/components/shell/PageFrame.vue";
import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import {
  getKbDocument,
  getPcObject,
  getProjcObject,
  type KbDocumentDetail,
  type PcObjectDetail
} from "@/api/knowledge";
import { useShellStore } from "@/stores/shell";

/**
 * 知识对象与溯源页（/project/knowledge/objects/:domain/:objectId）。
 *
 * 对应 Figma 仓库智能画布 frames 21/39/40「Knowledge object & traceability」：
 * 正式对象卡 + 溯源链（FORMAL OBJECT → ANALYSIS REPORT → REPOSITORY → SOURCE
 * COMMIT）。数据单源 = 既有只读端点（getKbDocument / getPcObject /
 * getProjcObject），不新增契约：
 *   - PC / ProjC：PcObjectDetail.analysis_sources（最近 3 条已发布版本的
 *     报告→仓库→commit 反查）；report 为 null = 非报告通道发布。
 *   - KB：文档无 analysis_sources 投影——溯源链只到正式对象 + 修订历史，
 *     并显式标注「非报告通道发布」，不虚构报告跳。
 * 非法 domain / 跨项目 ID → 显式 404 语义态（与报告详情页同规）。
 */
const { t } = useI18n();
const route = useRoute();
const shell = useShellStore();
const currentProjectId = computed(() => shell.currentProjectId);

const TRACE_DOMAINS = ["kb", "pc", "projc"] as const;
type TraceDomain = (typeof TRACE_DOMAINS)[number];

const domain = computed(() => {
  const raw = String(route.params.domain ?? "");
  return (TRACE_DOMAINS as readonly string[]).includes(raw)
    ? (raw as TraceDomain)
    : null;
});
const objectId = computed(() => String(route.params.objectId ?? ""));

const kbDoc = ref<KbDocumentDetail | null>(null);
const pcObj = ref<PcObjectDetail | null>(null);
const loading = ref(false);
const loadError = ref("");
const notFound = ref(false);

const objectTitle = computed(() => {
  if (domain.value === "kb") return kbDoc.value?.document.doc_key ?? "";
  return pcObj.value?.name ?? "";
});

const revisionNo = computed(() => {
  if (domain.value === "kb") return kbDoc.value?.document.revision_no ?? null;
  return pcObj.value?.latest_revision?.revision_no ?? null;
});

/** 对象副题：类型 · 业务键（PC/ProjC）或文档类型（KB）。 */
const objectSubtitle = computed(() => {
  if (domain.value === "kb") return kbDoc.value?.document.doc_type ?? "";
  const parts = [pcObj.value?.object_type, pcObj.value?.object_key].filter(
    (v): v is string => Boolean(v)
  );
  return parts.join(" · ");
});

/** KB 确认发布元信息（confirmed_by + published_at）。 */
const kbConfirmedText = computed(() => {
  const doc = kbDoc.value?.document;
  if (!doc?.confirmed_by) return "";
  return t("s01.knowledge.trace.confirmedMeta", {
    date: formatTime(doc.published_at),
    by: doc.confirmed_by
  });
});

function formatTime(value: string | null): string {
  if (!value) return "—";
  return value.slice(0, 16).replace("T", " ");
}

function shortSha(sha: string | null): string {
  return sha ? sha.slice(0, 12) : "—";
}

function kbStatusVariant(
  status: string
): "success" | "warning" | "danger" | "neutral" {
  if (status === "published") return "success";
  if (status === "draft") return "warning";
  if (status === "rejected") return "danger";
  return "neutral";
}

function lifecycleVariant(
  status: string
): "success" | "warning" | "danger" | "neutral" {
  if (status === "active") return "success";
  if (status === "draft") return "warning";
  if (status === "deprecated" || status === "superseded") return "danger";
  return "neutral";
}

async function loadObject(): Promise<void> {
  if (!currentProjectId.value || !objectId.value || !domain.value) return;
  loading.value = true;
  loadError.value = "";
  notFound.value = false;
  kbDoc.value = null;
  pcObj.value = null;
  try {
    if (domain.value === "kb") {
      kbDoc.value = await getKbDocument(objectId.value, currentProjectId.value);
    } else if (domain.value === "pc") {
      pcObj.value = await getPcObject(objectId.value, currentProjectId.value);
    } else {
      pcObj.value = await getProjcObject(
        objectId.value,
        currentProjectId.value
      );
    }
  } catch (error) {
    if (error instanceof HttpError && error.status === 404) {
      notFound.value = true;
    } else {
      loadError.value = t("s01.knowledge.error.load");
    }
  } finally {
    loading.value = false;
  }
}

watch(
  [domain, objectId, currentProjectId],
  () => {
    if (!domain.value) {
      // 非法域参数 → 404 语义态，不发请求
      notFound.value = true;
      return;
    }
    void loadObject();
  },
  { immediate: true }
);

/** 报告跳链接：已发布版本的来源分析报告详情页。 */
function reportLink(reportId: string): string {
  return `/project/analysis/reports/${reportId}`;
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      {
        label: t('s01.knowledge.page.title'),
        to:
          domain === 'projc'
            ? '/project/knowledge?tab=projc'
            : domain === 'pc'
              ? '/project/knowledge?tab=pc'
              : '/project/knowledge'
      }
    ]"
    :title="objectTitle || t('s01.knowledge.page.title')"
    :description="t('s01.knowledge.trace.chainLabel')"
  >
    <div class="trace-page">
      <EmptyState
        v-if="!currentProjectId"
        size="sm"
        :title="t('analysis.state.noProject')"
      />
      <template v-else>
        <p v-if="loading" class="trace-hint">
          {{ t("s01.knowledge.hint.loading") }}
        </p>
        <EmptyState
          v-else-if="notFound || !domain"
          size="sm"
          :title="t('s01.knowledge.trace.notFound')"
        />
        <template v-else>
          <Alert v-if="loadError" variant="danger" size="sm" role="alert">
            {{ loadError }}
          </Alert>

          <template v-if="domain === 'kb' && kbDoc">
            <!-- 正式对象卡（设计稿 Knowledge object card） -->
            <section class="trace-card" aria-labelledby="trace-object">
              <p id="trace-object" class="ws-caption">
                {{ t("s01.knowledge.trace.objectLabel") }}
              </p>
              <div class="trace-card-head">
                <h2 class="trace-title">{{ kbDoc.document.doc_key }}</h2>
                <Badge variant="neutral">
                  v{{ kbDoc.document.revision_no }}
                </Badge>
                <Badge :variant="kbStatusVariant(kbDoc.document.status)">
                  {{ t(`s01.knowledge.status.${kbDoc.document.status}`) }}
                </Badge>
              </div>
              <p class="trace-subtitle">
                {{ kbDoc.document.doc_type }}
                <template v-if="kbConfirmedText">
                  · {{ kbConfirmedText }}
                </template>
              </p>
              <p v-if="kbDoc.revisions.length" class="trace-meta">
                {{ t("s01.knowledge.field.revisions") }}
                {{ kbDoc.revisions.length }}
              </p>
            </section>

            <!-- KB 溯源链：无报告跳投影——显式标注，不虚构 -->
            <section class="trace-chain" aria-labelledby="trace-chain-kb">
              <p id="trace-chain-kb" class="ws-caption">
                {{ t("s01.knowledge.trace.chainLabel") }}
              </p>
              <ol class="trace-hops">
                <li class="trace-hop">
                  <span class="trace-hop-label">
                    {{ t("s01.knowledge.trace.objectLabel") }}
                  </span>
                  <span class="trace-hop-value">
                    {{ kbDoc.document.doc_key }} · v{{
                      kbDoc.document.revision_no
                    }}
                  </span>
                </li>
                <li class="trace-hop">
                  <span class="trace-hop-label">
                    {{ t("s01.knowledge.trace.revisionsLabel") }}
                  </span>
                  <ul class="trace-revisions">
                    <li
                      v-for="rev in kbDoc.revisions"
                      :key="rev.revision_id"
                      class="trace-revision"
                    >
                      <Badge :variant="kbStatusVariant(rev.status)">
                        {{ t(`s01.knowledge.status.${rev.status}`) }}
                      </Badge>
                      v{{ rev.revision_no }} ·
                      {{ formatTime(rev.published_at ?? rev.created_at) }}
                    </li>
                  </ul>
                </li>
              </ol>
              <p class="ws-footnote">
                {{ t("s01.knowledge.trace.noReportChannel") }}
              </p>
              <p class="ws-footnote">
                {{ t("s01.knowledge.trace.chainHint") }}
              </p>
            </section>
          </template>

          <template v-else-if="pcObj">
            <!-- 正式对象卡（PC / ProjC 同构） -->
            <section class="trace-card" aria-labelledby="trace-object">
              <p id="trace-object" class="ws-caption">
                {{ t("s01.knowledge.trace.objectLabel") }}
              </p>
              <div class="trace-card-head">
                <h2 class="trace-title">{{ pcObj.name }}</h2>
                <Badge v-if="revisionNo !== null" variant="neutral">
                  v{{ revisionNo }}
                </Badge>
                <Badge :variant="lifecycleVariant(pcObj.lifecycle_status)">
                  {{ t(`s01.knowledge.lifecycle.${pcObj.lifecycle_status}`) }}
                </Badge>
              </div>
              <p v-if="objectSubtitle" class="trace-subtitle">
                {{ objectSubtitle }}
              </p>
              <p v-if="pcObj.latest_revision?.summary" class="trace-meta">
                {{ pcObj.latest_revision.summary }}
              </p>
            </section>

            <!-- 溯源链：正式对象 → 分析报告 → 仓库 → 来源提交（analysis_sources） -->
            <section class="trace-chain" aria-labelledby="trace-chain">
              <p id="trace-chain" class="ws-caption">
                {{ t("s01.knowledge.trace.chainLabel") }}
              </p>
              <ol class="trace-hops">
                <li class="trace-hop">
                  <span class="trace-hop-label">
                    {{ t("s01.knowledge.trace.objectLabel") }}
                  </span>
                  <span class="trace-hop-value">
                    {{ pcObj.name }}
                    <template v-if="revisionNo !== null">
                      · v{{ revisionNo }}
                    </template>
                  </span>
                </li>
                <li
                  v-for="source in pcObj.analysis_sources"
                  :key="source.version_no + ':' + (source.report_id ?? 'none')"
                  class="trace-hop"
                >
                  <span class="trace-hop-label">
                    v{{ source.version_no }} ·
                    {{ t("s01.knowledge.trace.reportLabel") }}
                  </span>
                  <span v-if="source.report_id" class="trace-hop-value">
                    <router-link
                      class="trace-link"
                      :to="reportLink(source.report_id)"
                    >
                      {{ t("s01.knowledge.detail.openReport") }} ·
                      {{ source.repository_name }} @
                      {{ shortSha(source.commit_sha) }}
                    </router-link>
                  </span>
                  <span v-else class="trace-hop-value">
                    {{ t("s01.knowledge.detail.sourceAnalysisNone") }}
                  </span>
                  <template v-if="source.report_id">
                    <span class="trace-hop-label">
                      {{ t("s01.knowledge.trace.repositoryLabel") }}
                    </span>
                    <span class="trace-hop-value ws-mono">
                      {{ source.repository_name }}
                    </span>
                    <span class="trace-hop-label">
                      {{ t("s01.knowledge.trace.commitLabel") }}
                    </span>
                    <span class="trace-hop-value ws-mono">
                      {{ shortSha(source.commit_sha) }}
                      <span class="trace-hop-hint">
                        {{ t("s01.knowledge.trace.commitImmutable") }}
                      </span>
                    </span>
                  </template>
                </li>
              </ol>
              <p v-if="!pcObj.analysis_sources.length" class="ws-footnote">
                {{ t("s01.knowledge.trace.noReportChannel") }}
              </p>
              <p class="ws-footnote">
                {{ t("s01.knowledge.trace.chainHint") }}
              </p>
            </section>
          </template>
        </template>
      </template>
    </div>
  </PageFrame>
</template>

<style scoped>
.trace-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  max-width: 860px;
}

.trace-card,
.trace-chain {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-card);
  padding: var(--space-3) var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.trace-card-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.trace-title {
  margin: 0;
  font-size: var(--font-size-lg, 1.25rem);
  font-weight: 600;
  color: var(--text-primary);
}

.trace-subtitle {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.trace-meta {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.trace-hops {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.trace-hop {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  border-top: 1px dashed var(--border-subtle);
  padding-top: var(--space-2);
}

.trace-hop:first-child {
  border-top: 0;
  padding-top: 0;
}

.trace-hop-label {
  font-size: var(--font-size-micro);
  font-weight: 600;
  letter-spacing: 0.05em;
  color: var(--text-tertiary);
}

.trace-hop-value {
  font-size: var(--font-size-caption);
  color: var(--text-primary);
}

.trace-hop-hint {
  margin-left: var(--space-2);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.trace-link {
  color: var(--accent);
  text-decoration: underline;
  text-underline-offset: 2px;
}

.trace-revisions {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.trace-revision {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.ws-mono {
  font-family: var(--font-family-mono, monospace);
}

.ws-caption {
  margin: 0;
  font-size: var(--font-size-micro);
  font-weight: 600;
  letter-spacing: 0.05em;
  color: var(--text-tertiary);
}

.ws-footnote {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
  line-height: 1.5;
}

.trace-hint {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}
</style>
