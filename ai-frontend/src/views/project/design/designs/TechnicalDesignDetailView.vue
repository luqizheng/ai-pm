<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { ArrowLeft, RefreshCw } from "lucide-vue-next";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Card from "@/components/ui/card/Card.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { PageFrame } from "@/components/shell";
import {
  getDesign,
  listDesignFindings,
  listDesignRequirements,
  type DesignDetail,
  type DesignRequirementLink,
  type ReviewFinding
} from "@/api/designs";
import DesignFrame from "../DesignFrame.vue";
import {
  formatDateTime,
  revisionMarkdown,
  revisionSummary,
  revisionTone,
  severityTone
} from "../design-contract";

/**
 * S03-03 技术设计详情（版本链 / 正文 / 需求覆盖 / 评审问题）。
 *
 * 数据 = GET /v1/pm/designs/{id}（设计 + 全部修订 + 审批）；
 * 选中某一修订后按需读该修订的需求关联与评审问题
 * （GET .../revisions/{rid}/requirements 与 .../findings）。
 *
 * 版本不可变（§4.2）：进入 pending_confirmation 的修订正文冻结；版本链上
 * 只有 draft 态可改——本页只读，不提供改稿入口（写路径走命令端点）。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();

const designId = computed(() => String(route.params.designId ?? ""));
const detail = ref<DesignDetail | null>(null);
const selectedRevisionId = ref("");
const requirements = ref<DesignRequirementLink[]>([]);
const findings = ref<ReviewFinding[]>([]);
const loading = ref(true);
const loadError = ref(false);

async function loadDetail() {
  if (!designId.value) return;
  loading.value = true;
  loadError.value = false;
  try {
    detail.value = await getDesign(designId.value);
    const revisions = detail.value.revisions;
    const current = detail.value.design.current_revision_id;
    const initial =
      revisions.find(rev => rev.id === current)?.id ?? revisions[0]?.id ?? "";
    selectedRevisionId.value = initial;
    await loadRevisionSide(current ? initial : "");
  } catch {
    loadError.value = true;
    detail.value = null;
  } finally {
    loading.value = false;
  }
}

async function loadRevisionSide(revisionId: string) {
  if (!revisionId || !designId.value) {
    requirements.value = [];
    findings.value = [];
    return;
  }
  try {
    const [reqs, founds] = await Promise.all([
      listDesignRequirements(designId.value, revisionId),
      listDesignFindings(designId.value, revisionId)
    ]);
    requirements.value = reqs ?? [];
    findings.value = founds ?? [];
  } catch {
    requirements.value = [];
    findings.value = [];
  }
}

onMounted(loadDetail);
watch(designId, loadDetail);

function selectRevision(revisionId: string) {
  selectedRevisionId.value = revisionId;
  void loadRevisionSide(revisionId);
}

const selectedRevision = computed(
  () =>
    detail.value?.revisions.find(rev => rev.id === selectedRevisionId.value) ??
    null
);

const openFindings = computed(() =>
  findings.value.filter(item => item.status === "open")
);

function goBack() {
  void router.push("/project/design/technical-designs");
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('s03.design.module') },
      {
        label: t('s03.design.page.designsTitle'),
        to: '/project/design/technical-designs'
      }
    ]"
    :title="detail?.design.title ?? t('s03.design.page.designDetailTitle')"
    :description="detail?.design.code ?? t('s03.design.page.designDetailDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="design-detail-back"
        @click="goBack"
      >
        <ArrowLeft :size="14" />
        {{ t("s03.design.common.back") }}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        data-testid="design-detail-refresh"
        @click="loadDetail"
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
        v-else-if="!loading && !detail"
        :title="t('s03.design.designs.notFoundTitle')"
        :description="t('s03.design.designs.notFoundHint')"
      />

      <div v-else-if="detail" class="detail-layout">
        <Card class="detail-revisions" data-testid="design-revision-list">
          <template #header>
            {{ t("s03.design.detail.revisionsTitle") }}
          </template>
          <p v-if="detail.revisions.length === 0" class="detail-empty">
            {{ t("s03.design.detail.noRevisions") }}
          </p>
          <ul v-else class="revision-list">
            <li
              v-for="rev in detail.revisions"
              :key="rev.id"
              class="revision-item"
            >
              <button
                type="button"
                class="revision-item__button"
                :class="{ 'is-active': rev.id === selectedRevisionId }"
                :aria-pressed="rev.id === selectedRevisionId"
                :data-testid="`design-revision-${rev.version}`"
                @click="selectRevision(rev.id)"
              >
                <span class="revision-item__version">
                  {{
                    t("s03.design.analysis.version", { version: rev.version })
                  }}
                </span>
                <Badge :variant="revisionTone(rev.state)" size="sm">
                  {{ t(`s03.design.revisionState.${rev.state}`) }}
                </Badge>
                <span class="revision-item__meta">
                  {{ rev.created_by_name ?? rev.created_by }}
                </span>
                <span class="revision-item__meta">
                  {{ formatDateTime(rev.created_at) }}
                </span>
                <Badge v-if="rev.source === 'ai'" variant="info" size="sm">
                  {{ t("s03.design.detail.aiSource") }}
                </Badge>
              </button>
            </li>
          </ul>
        </Card>

        <div class="detail-main">
          <Card class="detail-content">
            <template #header>
              {{ t("s03.design.detail.contentTitle") }}
            </template>
            <p
              v-if="revisionSummary(selectedRevision?.content)"
              class="detail-summary"
              data-testid="design-revision-summary"
            >
              {{ revisionSummary(selectedRevision?.content) }}
            </p>
            <pre
              v-if="revisionMarkdown(selectedRevision?.content)"
              class="detail-markdown"
              data-testid="design-revision-markdown"
              >{{ revisionMarkdown(selectedRevision?.content) }}</pre
            >
            <p v-else class="detail-empty">
              {{ t("s03.design.detail.noContent") }}
            </p>
          </Card>

          <Card class="detail-links">
            <template #header>
              {{ t("s03.design.analysis.requirementsTitle") }}
            </template>
            <p v-if="requirements.length === 0" class="detail-empty">
              {{ t("s03.design.analysis.noRequirements") }}
            </p>
            <ul v-else class="link-list">
              <li
                v-for="link in requirements"
                :key="link.requirement_id"
                class="link-row"
              >
                <span class="link-row__code">{{
                  link.requirement_code ?? ""
                }}</span>
                <Badge
                  v-if="link.is_current === false"
                  variant="warning"
                  size="sm"
                >
                  {{ t("s03.design.analysis.supersededRequirement") }}
                </Badge>
                <span class="link-row__meta">
                  {{
                    link.requirement_version
                      ? t("s03.design.analysis.version", {
                          version: link.requirement_version
                        })
                      : ""
                  }}
                </span>
                <Badge
                  v-if="link.requirement_revision_state"
                  variant="neutral"
                  size="sm"
                >
                  {{
                    t(
                      `s03.design.requirementState.${link.requirement_revision_state}`
                    )
                  }}
                </Badge>
              </li>
            </ul>
          </Card>

          <Card class="detail-findings">
            <template #header>
              {{ t("s03.design.detail.findingsTitle") }}
            </template>
            <p v-if="findings.length === 0" class="detail-empty">
              {{ t("s03.design.detail.noFindings") }}
            </p>
            <ul v-else class="finding-list">
              <li
                v-for="item in findings"
                :key="item.id"
                class="finding-row"
                :data-testid="`design-finding-${item.id}`"
              >
                <Badge :variant="severityTone(item.severity)" size="sm">
                  {{ t(`s03.design.severity.${item.severity}`) }}
                </Badge>
                <span class="finding-row__body">{{ item.body }}</span>
                <Badge
                  :variant="item.status === 'open' ? 'warning' : 'neutral'"
                  size="sm"
                >
                  {{ t(`s03.design.findingStatus.${item.status}`) }}
                </Badge>
              </li>
            </ul>
          </Card>
        </div>
      </div>
    </DesignFrame>
  </PageFrame>
</template>

<style scoped>
.detail-layout {
  display: grid;
  grid-template-columns: minmax(220px, 280px) 1fr;
  gap: var(--space-5);
  align-items: start;
}

.detail-main {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.detail-empty {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.revision-list,
.link-list,
.finding-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.revision-item__button {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
  text-align: left;
  background: transparent;
  border: var(--border-1) solid transparent;
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.revision-item__button:hover {
  background: var(--surface-base);
}

.revision-item__button:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.revision-item__button.is-active {
  color: var(--ws-text-primary);
  border-color: var(--ws-accent);
}

.revision-item__version {
  font-variant-numeric: tabular-nums;
  font-weight: var(--weight-semibold);
}

.revision-item__meta {
  color: var(--ws-text-secondary);
}

.detail-summary {
  margin-bottom: var(--space-3);
  font-size: var(--font-size-body);
  color: var(--ws-text-primary);
}

.detail-markdown {
  margin: 0;
  padding: var(--space-4);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-body);
  color: var(--ws-text-primary);
  white-space: pre-wrap;
  background: var(--surface-base);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.link-row,
.finding-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.link-row__code {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-primary);
}

.link-row__meta {
  color: var(--ws-text-secondary);
}

.finding-row__body {
  flex: 1;
  min-width: 200px;
  color: var(--ws-text-primary);
}
</style>
