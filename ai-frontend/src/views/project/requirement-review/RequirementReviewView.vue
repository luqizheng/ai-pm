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
  getRequirementReviewQueue,
  revisionMarkdown,
  type ReviewProposal
} from "@/api/requirement-read";
import { useDesignProject } from "@/views/project/design/use-design-project";

/**
 * 需求评审与确认页（S02-UI-03，接真）。
 *
 * 数据 = GET /projects/{id}/requirement-review-queue：**待确认修订**
 * （state=pending_confirmation）+ 前一正式版本（供并排比较）。
 *
 * 关键语义：评审的对象是**修订**，不是需求——需求是稳定身份，评审的是
 * 它的一版正文；确认后该修订成为当前正式版本。
 *
 * 诚实缺位：AI 分析段、修订级来源证据、逐块 diff——后端无对应读面
 * （来源是需求级关联，无修订级证据投影）。本页只呈现真实的修订正文、
 * 前一版本号与元数据，不做伪对比。
 */
const { t } = useI18n();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const proposals = ref<ReviewProposal[]>([]);
const selectedId = ref("");
const loading = ref(true);
const loadError = ref(false);

async function load() {
  if (!hasProject.value) {
    proposals.value = [];
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    const queue = await getRequirementReviewQueue(projectId.value);
    proposals.value = queue?.proposals ?? [];
    if (!proposals.value.some(item => item.revision_id === selectedId.value)) {
      selectedId.value = proposals.value[0]?.revision_id ?? "";
    }
  } catch {
    loadError.value = true;
    proposals.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(projectId, load);

const selected = computed(
  () =>
    proposals.value.find(item => item.revision_id === selectedId.value) ?? null
);

function openRequirement(requirementId: string) {
  void router.push(`/project/requirements/${requirementId}`);
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('pm.requirements.pageTitle') }]"
    :title="t('pm.requirementReview.pageTitle')"
    :description="t('pm.requirementReview.pageDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="rv-refresh"
        @click="load"
      >
        <RefreshCw :size="14" />
        {{ t("pm.requirementReview.refresh") }}
      </Button>
    </template>

    <Alert
      v-if="loadError"
      variant="danger"
      size="sm"
      :title="t('pm.requirementReview.errorTitle')"
    />

    <EmptyState
      v-if="!hasProject"
      :title="t('pm.requirementReview.noProjectTitle')"
      :description="t('pm.requirementReview.noProjectHint')"
    />

    <EmptyState
      v-else-if="!loading && proposals.length === 0"
      :title="t('pm.requirementReview.emptyTitle')"
      :description="t('pm.requirementReview.emptyHint')"
    />

    <div v-else class="rv-layout">
      <Card class="rv-queue" data-testid="rv-queue">
        <template #header>
          {{
            t("pm.requirementReview.queueTitle", { count: proposals.length })
          }}
        </template>
        <ul class="rv-list">
          <li v-for="item in proposals" :key="item.revision_id">
            <button
              type="button"
              class="rv-item"
              :class="{ 'is-active': item.revision_id === selectedId }"
              :aria-pressed="item.revision_id === selectedId"
              :data-testid="`rv-item-${item.requirement_code ?? item.version}`"
              @click="selectedId = item.revision_id"
            >
              <span class="rv-item__code">{{ item.requirement_code }}</span>
              <span class="rv-item__title">{{ item.requirement_title }}</span>
              <Badge variant="warning" size="sm">
                {{ t("pm.requirementReview.pendingBadge") }}
              </Badge>
            </button>
          </li>
        </ul>
      </Card>

      <Card v-if="selected" class="rv-detail" data-testid="rv-detail">
        <template #header>
          <div class="rv-detail__head">
            <span>{{ selected.requirement_code }}</span>
            <span class="rv-detail__title">{{
              selected.requirement_title
            }}</span>
            <Button
              variant="ghost"
              size="sm"
              :data-testid="`rv-open-${selected.requirement_code}`"
              @click="openRequirement(selected.requirement_id)"
            >
              {{ t("pm.requirementReview.openRequirement") }}
            </Button>
          </div>
        </template>

        <dl class="rv-meta">
          <div class="rv-meta__row">
            <dt>{{ t("pm.requirementReview.metaVersion") }}</dt>
            <dd>
              {{
                t("pm.requirementReview.versionBadge", {
                  version: selected.version
                })
              }}
            </dd>
          </div>
          <div class="rv-meta__row">
            <dt>{{ t("pm.requirementReview.metaBase") }}</dt>
            <dd>
              {{
                selected.base_version
                  ? t("pm.requirementReview.versionBadge", {
                      version: selected.base_version
                    })
                  : t("pm.requirementReview.noBase")
              }}
            </dd>
          </div>
          <div class="rv-meta__row">
            <dt>{{ t("pm.requirementReview.metaSubmitted") }}</dt>
            <dd>{{ selected.submitted_at ?? selected.created_at }}</dd>
          </div>
          <div class="rv-meta__row">
            <dt>{{ t("pm.requirementReview.metaAuthor") }}</dt>
            <dd>{{ selected.created_by_name ?? selected.created_by }}</dd>
          </div>
          <div class="rv-meta__row">
            <dt>{{ t("pm.requirementReview.metaSources") }}</dt>
            <dd>{{ selected.source_count }}</dd>
          </div>
          <div class="rv-meta__row">
            <dt>{{ t("pm.requirementReview.metaHash") }}</dt>
            <dd class="rv-meta__mono">
              {{ selected.content_hash.slice(0, 16) }}
            </dd>
          </div>
        </dl>

        <section class="rv-body">
          <h3>{{ t("pm.requirementReview.bodyTitle") }}</h3>
          <pre
            v-if="revisionMarkdown(selected.content)"
            class="rv-markdown"
            data-testid="rv-markdown"
            >{{ revisionMarkdown(selected.content) }}</pre
          >
          <p v-else class="rv-empty">
            {{ t("pm.requirementReview.noBody") }}
          </p>
        </section>
      </Card>
    </div>
  </PageFrame>
</template>

<style scoped>
.rv-layout {
  display: grid;
  grid-template-columns: minmax(240px, 340px) 1fr;
  gap: var(--space-5);
  align-items: start;
}

.rv-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rv-item {
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

.rv-item:hover {
  background: var(--surface-base);
}

.rv-item:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.rv-item.is-active {
  color: var(--ws-text-primary);
  border-color: var(--ws-accent);
}

.rv-item__code {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-primary);
}

.rv-item__title {
  flex: 1;
  min-width: 100px;
}

.rv-detail__head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-3);
}

.rv-detail__title {
  flex: 1;
  min-width: 160px;
}

.rv-meta {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0 0 var(--space-4);
}

.rv-meta__row {
  display: grid;
  grid-template-columns: minmax(100px, 160px) 1fr;
  gap: var(--space-3);
  font-size: var(--font-size-caption);
}

.rv-meta__row dt {
  color: var(--ws-text-secondary);
}

.rv-meta__row dd {
  margin: 0;
  color: var(--ws-text-primary);
}

.rv-meta__mono {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}

.rv-body h3 {
  margin-bottom: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-secondary);
}

.rv-markdown {
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

.rv-empty {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
