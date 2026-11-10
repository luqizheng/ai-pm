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
import { getAdr, type AdrDetail } from "@/api/designs";
import DesignFrame from "../DesignFrame.vue";
import { adrTone, formatDateTime } from "../design-contract";

/**
 * S03-04 架构决策详情（决策内容 / 版本链 / 影响关系）。
 *
 * 数据 = GET /v1/pm/adrs/{id}（ADR + 全部修订 + 关联设计/需求 + 审批）。
 *
 * ADR 内容形状（content jsonb）：decision_question / context / options /
 * chosen_option / rationale / consequences——决策记录不可变：已提交的版本
 * 内容冻结，决策改变时新建版本并显式替代（§3.3 / §4）。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();

const adrId = computed(() => String(route.params.adrId ?? ""));
const detail = ref<AdrDetail | null>(null);
const selectedRevisionId = ref("");
const loading = ref(true);
const loadError = ref(false);

async function load() {
  if (!adrId.value) return;
  loading.value = true;
  loadError.value = false;
  try {
    detail.value = await getAdr(adrId.value);
    const revisions = detail.value.revisions;
    const current = detail.value.adr.current_revision_id;
    selectedRevisionId.value =
      revisions.find(rev => rev.id === current)?.id ?? revisions[0]?.id ?? "";
  } catch {
    loadError.value = true;
    detail.value = null;
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(adrId, load);

const selectedRevision = computed(
  () =>
    detail.value?.revisions.find(rev => rev.id === selectedRevisionId.value) ??
    null
);

const content = computed(() => {
  const raw = selectedRevision.value?.content;
  if (!raw || typeof raw !== "object") return {} as Record<string, unknown>;
  return raw as Record<string, unknown>;
});

function text(key: string): string {
  const value = content.value[key];
  return typeof value === "string" ? value : "";
}

function options(): string[] {
  const value = content.value.options;
  return Array.isArray(value) ? value.filter(v => typeof v === "string") : [];
}

function goBack() {
  void router.push("/project/design/adrs");
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[
      { label: t('s03.design.module') },
      { label: t('s03.design.page.adrsTitle'), to: '/project/design/adrs' }
    ]"
    :title="detail?.adr.title ?? t('s03.design.page.adrDetailTitle')"
    :description="detail?.adr.code ?? t('s03.design.page.adrDetailDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="adr-detail-back"
        @click="goBack"
      >
        <ArrowLeft :size="14" />
        {{ t("s03.design.common.back") }}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        data-testid="adr-detail-refresh"
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
        v-else-if="!loading && !detail"
        :title="t('s03.design.adrs.notFoundTitle')"
        :description="t('s03.design.adrs.notFoundHint')"
      />

      <div v-else-if="detail" class="adr-detail">
        <Card class="adr-meta">
          <template #header>{{ t("s03.design.adrs.metaTitle") }}</template>
          <dl class="meta-list">
            <div class="meta-row">
              <dt>{{ t("s03.design.adrs.colStatus") }}</dt>
              <dd>
                <Badge :variant="adrTone(detail.adr.status)" size="sm">
                  {{ t(`s03.design.adrStatus.${detail.adr.status}`) }}
                </Badge>
              </dd>
            </div>
            <div class="meta-row">
              <dt>{{ t("s03.design.adrs.colBlocking") }}</dt>
              <dd>
                <Badge
                  v-if="detail.adr.blockers_implementation"
                  variant="danger"
                  size="sm"
                >
                  {{ t("s03.design.adrs.blocking") }}
                </Badge>
                <span v-else class="meta-muted">
                  {{ t("s03.design.adrs.notBlocking") }}
                </span>
              </dd>
            </div>
            <div class="meta-row">
              <dt>{{ t("s03.design.adrs.colUpdated") }}</dt>
              <dd class="meta-muted">
                {{ formatDateTime(detail.adr.updated_at) }}
              </dd>
            </div>
            <div v-if="detail.adr.superseded_by_adr_id" class="meta-row">
              <dt>{{ t("s03.design.adrs.supersededBy") }}</dt>
              <dd class="meta-muted">{{ detail.adr.superseded_by_adr_id }}</dd>
            </div>
          </dl>
        </Card>

        <Card class="adr-content">
          <template #header>
            <div class="adr-content__head">
              <span>{{ t("s03.design.adrs.contentTitle") }}</span>
              <nav
                v-if="detail.revisions.length > 0"
                class="adr-revisions"
                :aria-label="t('s03.design.detail.revisionsTitle')"
              >
                <button
                  v-for="rev in detail.revisions"
                  :key="rev.id"
                  type="button"
                  class="adr-revisions__item"
                  :class="{ 'is-active': rev.id === selectedRevisionId }"
                  :aria-pressed="rev.id === selectedRevisionId"
                  :data-testid="`adr-revision-${rev.version}`"
                  @click="selectedRevisionId = rev.id"
                >
                  {{
                    t("s03.design.analysis.version", { version: rev.version })
                  }}
                </button>
              </nav>
            </div>
          </template>

          <dl class="decision-list">
            <div class="decision-row">
              <dt>{{ t("s03.design.adrs.decisionQuestion") }}</dt>
              <dd>
                {{ text("decision_question") || t("s03.design.common.none") }}
              </dd>
            </div>
            <div class="decision-row">
              <dt>{{ t("s03.design.adrs.context") }}</dt>
              <dd>{{ text("context") || t("s03.design.common.none") }}</dd>
            </div>
            <div class="decision-row">
              <dt>{{ t("s03.design.adrs.options") }}</dt>
              <dd>
                <ul v-if="options().length > 0" class="option-list">
                  <li
                    v-for="option in options()"
                    :key="option"
                    class="option-row"
                  >
                    {{ option }}
                  </li>
                </ul>
                <span v-else class="meta-muted">{{
                  t("s03.design.common.none")
                }}</span>
              </dd>
            </div>
            <div class="decision-row">
              <dt>{{ t("s03.design.adrs.chosenOption") }}</dt>
              <dd>
                {{ text("chosen_option") || t("s03.design.common.none") }}
              </dd>
            </div>
            <div class="decision-row">
              <dt>{{ t("s03.design.adrs.rationale") }}</dt>
              <dd>{{ text("rationale") || t("s03.design.common.none") }}</dd>
            </div>
            <div class="decision-row">
              <dt>{{ t("s03.design.adrs.consequences") }}</dt>
              <dd>{{ text("consequences") || t("s03.design.common.none") }}</dd>
            </div>
          </dl>
        </Card>

        <Card class="adr-links">
          <template #header>{{ t("s03.design.adrs.impactTitle") }}</template>
          <p
            v-if="
              detail.design_links.length === 0 &&
              detail.requirement_links.length === 0
            "
            class="meta-muted"
          >
            {{ t("s03.design.adrs.noImpact") }}
          </p>
          <ul v-else class="link-list">
            <li
              v-for="link in detail.design_links"
              :key="link.adr_revision_id + link.design_id"
              class="link-row"
            >
              <span class="link-row__code">{{ link.design_code ?? "" }}</span>
              <span class="link-row__meta">
                {{
                  link.design_revision_version
                    ? t("s03.design.analysis.version", {
                        version: link.design_revision_version
                      })
                    : ""
                }}
              </span>
            </li>
            <li
              v-for="link in detail.requirement_links"
              :key="link.adr_revision_id + link.requirement_id"
              class="link-row"
            >
              <span class="link-row__code">{{
                link.requirement_code ?? ""
              }}</span>
              <span class="link-row__meta">
                {{
                  link.requirement_revision_version
                    ? t("s03.design.analysis.version", {
                        version: link.requirement_revision_version
                      })
                    : ""
                }}
              </span>
            </li>
          </ul>
        </Card>
      </div>
    </DesignFrame>
  </PageFrame>
</template>

<style scoped>
.adr-detail {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.meta-list,
.decision-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.meta-row,
.decision-row {
  display: grid;
  grid-template-columns: minmax(120px, 200px) 1fr;
  gap: var(--space-3);
  font-size: var(--font-size-caption);
}

.meta-row dt,
.decision-row dt {
  color: var(--ws-text-secondary);
}

.meta-row dd,
.decision-row dd {
  margin: 0;
  color: var(--ws-text-primary);
}

.meta-muted {
  color: var(--ws-text-secondary);
}

.adr-content__head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.adr-revisions {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.adr-revisions__item {
  padding: var(--space-1) var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-secondary);
  background: transparent;
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.adr-revisions__item:hover {
  color: var(--ws-text-primary);
}

.adr-revisions__item:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.adr-revisions__item.is-active {
  color: var(--ws-text-primary);
  border-color: var(--ws-accent);
}

.option-list,
.link-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.option-row {
  font-size: var(--font-size-caption);
  color: var(--ws-text-primary);
}

.link-row {
  display: flex;
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
</style>
