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
import { listDesignAnalysis, type DesignAnalysisRow } from "@/api/designs";
import DesignFrame from "../DesignFrame.vue";
import { revisionTone, requirementStateTone } from "../design-contract";
import { useDesignProject } from "../use-design-project";

/**
 * S03-01/02 分析页（docs/ui/s03 页面结构 §8「Analysis」）。
 *
 * 数据 = pm-api REST（GET /v1/pm/projects/{id}/design-analysis）：
 *   - **需求覆盖**：设计 ↔ 需求按修订锚定（§3.4 第一条）；`is_current=false`
 *     表示需求已被新版本取代而设计仍引用旧版 → 标为过期，影响分析重点；
 *   - **影响关系**：本设计被哪些 ADR 引用（ADR ↔ 设计版本，§3.4 第二条）；
 *   - **未闭评审问题**数（design.review_finding status='open'）。
 *
 * 诚实缺位：代码资产分析 / 数据影响 / API 影响（§3.1 技术分析字段）目前
 * 落在设计修订正文的 technical_analysis jsonb 里，无结构化读面——本页只读
 * 结构化可查的部分，不把自由文本包装成结构化指标。
 */
const { t } = useI18n();
const router = useRouter();
const { projectId, hasProject } = useDesignProject();

const rows = ref<DesignAnalysisRow[]>([]);
const loading = ref(true);
const loadError = ref(false);

async function load() {
  if (!hasProject.value) {
    rows.value = [];
    loading.value = false;
    return;
  }
  loading.value = true;
  loadError.value = false;
  try {
    rows.value = await listDesignAnalysis(projectId.value);
  } catch {
    loadError.value = true;
    rows.value = [];
  } finally {
    loading.value = false;
  }
}

onMounted(load);
watch(projectId, load);

const staleRows = computed(() =>
  rows.value.filter(row => row.stale_requirement_count > 0)
);

function openDesign(designId: string) {
  void router.push(`/project/design/technical-designs/${designId}`);
}
</script>

<template>
  <PageFrame
    :breadcrumbs="[{ label: t('s03.design.module') }]"
    :title="t('s03.design.page.analysisTitle')"
    :description="t('s03.design.page.analysisDesc')"
  >
    <template #actions>
      <Button
        variant="secondary"
        size="sm"
        data-testid="design-analysis-refresh"
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

      <Alert
        v-else-if="staleRows.length > 0"
        variant="warning"
        size="sm"
        :title="t('s03.design.analysis.staleTitle')"
      >
        {{ t("s03.design.analysis.staleHint", { count: staleRows.length }) }}
      </Alert>

      <EmptyState
        v-if="!hasProject"
        :title="t('s03.design.common.noProjectTitle')"
        :description="t('s03.design.common.noProjectHint')"
      />

      <EmptyState
        v-else-if="!loading && rows.length === 0"
        :title="t('s03.design.analysis.emptyTitle')"
        :description="t('s03.design.analysis.emptyHint')"
      />

      <div v-else class="analysis-list" data-testid="design-analysis-list">
        <Card
          v-for="row in rows"
          :key="row.design_id"
          class="analysis-row"
          :data-testid="`analysis-row-${row.code}`"
        >
          <template #header>
            <div class="analysis-row__head">
              <span class="analysis-row__code">{{ row.code }}</span>
              <span class="analysis-row__title">{{ row.title }}</span>
              <Badge :variant="revisionTone(row.current_state)" size="sm">
                {{
                  row.current_state
                    ? t(`s03.design.revisionState.${row.current_state}`)
                    : t("s03.design.common.noRevision")
                }}
              </Badge>
              <Badge
                v-if="row.stale_requirement_count > 0"
                variant="warning"
                size="sm"
              >
                {{
                  t("s03.design.analysis.staleBadge", {
                    count: row.stale_requirement_count
                  })
                }}
              </Badge>
              <Button
                variant="ghost"
                size="sm"
                :data-testid="`analysis-open-${row.code}`"
                @click="openDesign(row.design_id)"
              >
                {{ t("s03.design.common.openDesign") }}
              </Button>
            </div>
          </template>

          <div class="analysis-row__body">
            <section class="analysis-block">
              <h4 class="analysis-block__title">
                {{ t("s03.design.analysis.requirementsTitle") }}
              </h4>
              <p
                v-if="row.requirements.length === 0"
                class="analysis-block__empty"
              >
                {{ t("s03.design.analysis.noRequirements") }}
              </p>
              <ul v-else class="analysis-links">
                <li
                  v-for="link in row.requirements"
                  :key="link.requirement_id"
                  class="analysis-link"
                >
                  <span class="analysis-link__code">
                    {{
                      link.requirement_code ?? t("s03.design.common.unknown")
                    }}
                  </span>
                  <span class="analysis-link__title">
                    {{ link.requirement_title ?? "" }}
                  </span>
                  <Badge
                    :variant="requirementStateTone(link.requirement_state)"
                    size="sm"
                  >
                    {{
                      link.requirement_state
                        ? t(
                            `s03.design.requirementState.${link.requirement_state}`
                          )
                        : t("s03.design.common.unknown")
                    }}
                  </Badge>
                  <span class="analysis-link__version">
                    {{
                      link.requirement_version
                        ? t("s03.design.analysis.version", {
                            version: link.requirement_version
                          })
                        : ""
                    }}
                  </span>
                  <Badge
                    v-if="link.is_current === false"
                    variant="warning"
                    size="sm"
                  >
                    {{ t("s03.design.analysis.supersededRequirement") }}
                  </Badge>
                </li>
              </ul>
            </section>

            <section class="analysis-block">
              <h4 class="analysis-block__title">
                {{ t("s03.design.analysis.adrsTitle") }}
              </h4>
              <p v-if="row.adrs.length === 0" class="analysis-block__empty">
                {{ t("s03.design.analysis.noAdrs") }}
              </p>
              <ul v-else class="analysis-links">
                <li
                  v-for="link in row.adrs"
                  :key="link.adr_id"
                  class="analysis-link"
                >
                  <span class="analysis-link__code">
                    {{ link.adr_code ?? t("s03.design.common.unknown") }}
                  </span>
                  <span class="analysis-link__title">
                    {{ link.adr_title ?? "" }}
                  </span>
                  <span class="analysis-link__version">
                    {{
                      link.adr_status
                        ? t(`s03.design.adrStatus.${link.adr_status}`)
                        : ""
                    }}
                  </span>
                </li>
              </ul>
            </section>
          </div>
        </Card>
      </div>
    </DesignFrame>
  </PageFrame>
</template>

<style scoped>
.analysis-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.analysis-row__head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-3);
}

.analysis-row__code {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-secondary);
}

.analysis-row__title {
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.analysis-row__body {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
  gap: var(--space-5);
}

.analysis-block {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.analysis-block__title {
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-secondary);
}

.analysis-block__empty {
  font-size: var(--font-size-caption);
  color: var(--ws-text-secondary);
}

.analysis-links {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.analysis-link {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
}

.analysis-link__code {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-primary);
}

.analysis-link__title {
  color: var(--ws-text-secondary);
}

.analysis-link__version {
  color: var(--ws-text-secondary);
}
</style>
