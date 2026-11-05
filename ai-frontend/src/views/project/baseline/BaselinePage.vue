<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import PageFrame from "@/components/shell/PageFrame.vue";
import { useScopeStore } from "@/api/scope";
import {
  confirmBaseline,
  createBaselineProposal,
  decideBaselineItem,
  loadBaselineOverview,
  type BaselineItem,
  type BaselineOverview
} from "@/api/baseline";

/**
 * S01-05 · Project Baseline（AI 提案 → 人类逐项确认 → 版本不可变）。
 * 提案条目从 S01-04 已确认知识 + 仓库快照派生；未决条目阻塞确认基线；
 * deferred/investigation 作为 documented uncertainties 随版本保留。
 */
const { t } = useI18n();
const route = useRoute();
const scope = useScopeStore();

const overview = ref<BaselineOverview | null>(null);
const loading = ref(true);
const errorMessage = ref("");
const busy = ref("");
const editingId = ref("");
const editDraft = ref("");

const projectId = computed(() => {
  const q = route.query.project_id;
  if (typeof q === "string" && q) return q;
  return scope.currentProjectId && scope.currentProjectId !== "all"
    ? scope.currentProjectId
    : "";
});

const proposedCount = computed(
  () => overview.value?.items.filter(i => i.state === "proposed").length ?? 0
);
const canConfirm = computed(
  () => Boolean(overview.value?.proposal) && proposedCount.value === 0
);

async function load() {
  loading.value = true;
  errorMessage.value = "";
  try {
    if (!projectId.value) return;
    overview.value = await loadBaselineOverview(projectId.value);
  } catch (error) {
    overview.value = null;
    errorMessage.value = error instanceof Error ? error.message : String(error ?? "");
  } finally {
    loading.value = false;
  }
}

async function withBusy(key: string, fn: () => Promise<unknown>) {
  if (busy.value) return;
  busy.value = key;
  try {
    await fn();
    await load();
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error ?? "");
  } finally {
    busy.value = "";
  }
}

const createProposal = () => withBusy("propose", () => createBaselineProposal(projectId.value));
const confirmAll = () => withBusy("confirm", () => confirmBaseline(projectId.value));
const decide = (item: BaselineItem, action: "confirm" | "reject", content?: string) =>
  withBusy(item.id, () =>
    decideBaselineItem(projectId.value, item.id, {
      action,
      content: action === "confirm" ? content : undefined
    })
  );

onMounted(load);
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('s01.baseline.breadcrumb.project') },
      { label: t('s01.baseline.breadcrumb.baseline') }
    ]"
    :title="t('s01.baseline.page.title')"
    :description="t('s01.baseline.page.description')"
  >
    <template #actions>
      <Button
        v-if="!overview?.proposal"
        variant="primary"
        size="sm"
        :loading="busy === 'propose'"
        :disabled="!projectId"
        data-testid="baseline-propose"
        @click="createProposal"
      >
        {{ t("s01.baseline.action.propose") }}
      </Button>
      <Button
        v-else
        variant="primary"
        size="sm"
        :loading="busy === 'confirm'"
        :disabled="!canConfirm"
        data-testid="baseline-confirm"
        @click="confirmAll"
      >
        {{ t("s01.baseline.action.confirmBaseline") }}
      </Button>
    </template>

    <Alert
      v-if="errorMessage"
      variant="danger"
      :title="t('s01.baseline.state.loadFailed')"
    >
      {{ errorMessage }}
    </Alert>

    <div v-else-if="loading" class="bl-page__loading">
      {{ t("s01.baseline.state.loading") }}
    </div>

    <EmptyState
      v-else-if="!projectId"
      :title="t('s01.baseline.state.noProject')"
    />

    <template v-else-if="overview">
      <!-- 当前基线状态 -->
      <section class="bl-page__status" data-testid="baseline-status">
        <template v-if="overview.current_version">
          <Badge variant="info" size="sm">
            {{ t("s01.baseline.status.version", { version: overview.current_version }) }}
          </Badge>
          <span class="bl-page__hint">
            {{
              t("s01.baseline.status.confirmedAt", {
                time: new Date(overview.current_confirmed_at ?? "").toLocaleString()
              })
            }}
          </span>
        </template>
        <template v-else>
          <Badge variant="neutral" size="sm">
            {{ t("s01.baseline.status.none") }}
          </Badge>
        </template>
      </section>

      <!-- 待确认提案（AI 提案 ≠ 已确认事实） -->
      <section
        v-if="overview.proposal"
        class="bl-page__card bl-page__proposal"
        data-testid="baseline-proposal"
      >
        <h2>{{ t("s01.baseline.proposal.title") }}</h2>
        <p class="bl-page__hint">{{ t("s01.baseline.proposal.notice") }}</p>
        <p class="bl-page__hint">
          {{ t("s01.baseline.proposal.pending", { count: proposedCount }) }}
        </p>
        <ul class="bl-page__items">
          <li
            v-for="item in overview.items"
            :key="item.id"
            class="bl-page__item"
            data-testid="baseline-item"
          >
            <div class="bl-page__item-head">
              <Badge variant="neutral" size="sm">
                {{ t(`s01.baseline.category.${item.category}`) }}
              </Badge>
              <Badge
                :variant="item.state === 'confirmed' ? 'info' : item.state === 'rejected' ? 'danger' : 'neutral'"
                size="sm"
              >
                {{ t(`s01.baseline.itemState.${item.state}`) }}
              </Badge>
            </div>
            <p class="bl-page__content">{{ item.content }}</p>
            <div v-if="item.state === 'proposed'" class="bl-page__actions">
              <Button
                variant="primary"
                size="sm"
                :loading="busy === item.id"
                :data-testid="`baseline-confirm-${item.id}`"
                @click="decide(item, 'confirm')"
              >
                {{ t("s01.baseline.action.confirm") }}
              </Button>
              <Button
                variant="secondary"
                size="sm"
                :disabled="Boolean(busy)"
                :data-testid="`baseline-reject-${item.id}`"
                @click="decide(item, 'reject')"
              >
                {{ t("s01.baseline.action.reject") }}
              </Button>
            </div>
          </li>
        </ul>
      </section>

      <!-- 历史版本 -->
      <section class="bl-page__card">
        <h2>{{ t("s01.baseline.history.title") }}</h2>
        <p v-if="!overview.history.length" class="bl-page__hint">
          {{ t("s01.baseline.history.empty") }}
        </p>
        <ul v-else class="bl-page__history">
          <li v-for="entry in overview.history" :key="entry.version">
            <Badge variant="neutral" size="sm">
              {{ t("s01.baseline.status.version", { version: entry.version }) }}
            </Badge>
            <span class="bl-page__hint">
              {{
                t("s01.baseline.status.confirmedAt", {
                  time: new Date(entry.confirmed_at).toLocaleString()
                })
              }}
            </span>
          </li>
        </ul>
      </section>
    </template>
  </PageFrame>
</template>

<style scoped>
.bl-page__loading { padding: var(--space-10); color: var(--ws-text-secondary); }
.bl-page__status { display: flex; align-items: center; gap: var(--space-3); }
.bl-page__hint { margin: 0; color: var(--ws-text-secondary); font-size: var(--font-size-caption); }
.bl-page__card {
  display: flex; flex-direction: column; gap: var(--space-3);
  padding: var(--space-5); background: var(--ws-elevated);
  border: 1px solid var(--ws-border); border-radius: var(--ws-radius-card);
}
.bl-page__proposal { border-color: var(--ws-accent); }
.bl-page__card h2 {
  margin: 0; color: var(--ws-text-primary);
  font-size: var(--ws-font-size-section); font-weight: var(--weight-semibold);
}
.bl-page__items, .bl-page__history {
  display: flex; flex-direction: column; gap: var(--space-3);
  margin: 0; padding: 0; list-style: none;
}
.bl-page__item {
  display: flex; flex-direction: column; gap: var(--space-2);
  padding: var(--space-3); border: 1px solid var(--ws-border);
  border-radius: var(--radius-sm); background: var(--ws-canvas);
}
.bl-page__item-head { display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; }
.bl-page__content { margin: 0; color: var(--ws-text-primary); font-size: var(--font-size-body); }
.bl-page__actions { display: flex; align-items: center; gap: var(--space-2); }
.bl-page__history li { display: flex; align-items: center; gap: var(--space-3); }
</style>
