<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import Card from "@/components/ui/card/Card.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import { listImports, startImport } from "@/api/imports";
import { listRepositoriesV2 } from "@/api/initialization";
import {
  describeImportError,
  findActiveImport,
  hasRetryableImport,
  makeIdempotencyKey
} from "@/lib/import-analysis";
import type { ImportListItem } from "@/api/types";
import type {
  CommandCenterInitData,
  InitItemView
} from "../command-center-data";

/**
 * 项目初始化进度卡（S01-01 Initialization Overview）。
 *
 * 数据 = `/command-center` 的 `initialization_progress` 块（后端早已返回，
 * 此前前端未映射、未渲染，导致 S01-01 缺页）。
 *
 * 关键语义：`completion = not_determinable` 时**不得宣称已完成**——
 * 无项目类型/初始化要求配置就无法判定，页面显示「无法判定」而不是绿灯。
 *
 * S3（2026-11-13）：「代码分析」事项本体带仓库设置跳转链接，旁边挂动作
 * 按钮（接入 listImports 状态，可发起 / 重发）。七态逐项状态展示语义不变：
 * StatusIndicator / Badge / statusLabel / detail 全部沿用原渲染。
 */
const props = withDefaults(
  defineProps<{
    /** 卡标题 */
    title: string;
    /** 进度概览文案（调用方翻译，如「必需 2 / 3」） */
    progressLabel: string;
    /** 各完成度的判定文案（调用方翻译） */
    completionLabels: {
      complete: string;
      incomplete: string;
      notDeterminable: string;
    };
    /** 必需事项标记文案 */
    requiredLabel: string;
    /** 数据（null = 后端未提供该块，整卡不渲染） */
    data: CommandCenterInitData | null;
    /** 当前项目 id；空串 = 无项目上下文，分析动作整段隐藏（scope=__all__） */
    projectId?: string;
  }>(),
  { projectId: "" }
);

const { t } = useI18n();
const translate = (key: string, params?: Record<string, unknown>): string =>
  t(key, params ?? {}) as string;

const hasData = computed(() => props.data !== null);

const completionLabel = computed(() => {
  if (!props.data) return "";
  switch (props.data.completion) {
    case "complete":
      return props.completionLabels.complete;
    case "incomplete":
      return props.completionLabels.incomplete;
    default:
      // not_determinable 与任何未知值都按「无法判定」处理——不猜完成。
      return props.completionLabels.notDeterminable;
  }
});

const completionTone = computed<"success" | "warning" | "neutral">(() => {
  if (!props.data) return "neutral";
  switch (props.data.completion) {
    case "complete":
      return "success";
    case "incomplete":
      return "warning";
    default:
      return "neutral";
  }
});

const items = computed<InitItemView[]>(() => props.data?.items ?? []);

/** 收起/展开：初始跟随后端判定（§4.6 收缩规则=初始化完成后自动折叠），
 *  用户手动切换后以用户为准；收起不得隐藏待人工事项（waiting_for_human
 *  会让后端 expanded=true，天然不进收起态）。 */
const userExpanded = ref<boolean | null>(null);
const isExpanded = computed(() => {
  if (userExpanded.value !== null) return userExpanded.value;
  return props.data?.expanded ?? true;
});
const waitingHumanCount = computed(
  () => items.value.filter(item => item.tone === "warning").length
);

/* ── S3 代码分析动作（单源判定见 lib/import-analysis） ── */
const importItems = ref<ImportListItem[]>([]);
const analysisError = ref("");
const analysisBusy = ref(false);
/** 防双击：同一轮只发一次，成功/失败后清除以便下次重发。 */
const analysisKey = ref("");

const activeImport = computed(() => findActiveImport(importItems.value));
const retryableImport = computed(
  () => hasRetryableImport(importItems.value) && !activeImport.value
);
/** 全部批次已应用/驳回且无进行中 → 不再提供冷启动动作（409 由设置页兜底）。 */
const isApplied = computed(
  () =>
    !activeImport.value &&
    importItems.value.length > 0 &&
    importItems.value.every(
      item => item.status === "applied" || item.status === "rejected"
    )
);
const showAnalysisAction = computed(
  () => !!props.projectId && !isApplied.value
);
const analysisActionLabel = computed(() => {
  if (activeImport.value) {
    return t("s01.command-center.init.analysis.running");
  }
  return retryableImport.value
    ? t("s01.command-center.init.analysis.retry")
    : t("s01.command-center.init.analysis.start");
});
const analysisActionDisabled = computed(
  () => !!activeImport.value || analysisBusy.value || !!analysisKey.value
);
const analysisPath = computed(() =>
  props.projectId
    ? {
        path: "/project/settings/repositories",
        query: { project_id: props.projectId }
      }
    : "/project/settings/repositories"
);

async function refreshImports() {
  if (!props.projectId) {
    importItems.value = [];
    return;
  }
  try {
    const page = await listImports(props.projectId);
    importItems.value = page.items;
  } catch {
    // 导入状态读失败不打断初始化卡（七态展示仍以 /command-center 为准）。
    importItems.value = [];
  }
}

/** 一键发起 / 重发：取项目全部 verified 仓库 ids（无 requirement count 端点）。 */
async function runAnalysis() {
  if (!props.projectId || analysisActionDisabled.value) return;
  const key = makeIdempotencyKey();
  analysisKey.value = key;
  analysisBusy.value = true;
  analysisError.value = "";
  try {
    const page = await listRepositoriesV2(props.projectId);
    const ids = page.items
      .filter(repo => repo.status === "verified")
      .map(repo => repo.id);
    if (ids.length === 0) {
      analysisError.value = t("s01.command-center.init.analysis.noVerified");
      return;
    }
    await startImport(props.projectId, ids, key);
    await refreshImports();
  } catch (error) {
    analysisError.value = describeImportError(translate, error);
  } finally {
    analysisKey.value = "";
    analysisBusy.value = false;
  }
}

onMounted(() => void refreshImports());
watch(
  () => props.projectId,
  () => void refreshImports()
);
</script>

<template>
  <Card
    v-if="hasData && data"
    class="init-card"
    data-testid="cc-initialization"
  >
    <template #header>
      <div class="init-card__head">
        <span>{{ title }}</span>
        <Badge :variant="completionTone" size="sm">
          {{ completionLabel }}
        </Badge>
        <span class="init-card__progress">{{ progressLabel }}</span>
      </div>
    </template>

    <!-- 收起态：只显示必需进度摘要 + 展开入口（初始化完成后自动折叠） -->
    <div
      v-if="!isExpanded"
      class="init-collapsed"
      data-testid="cc-init-collapsed"
    >
      <span class="init-collapsed__summary">{{
        t("s01.command-center.init.collapsedSummary", {
          done: data?.requiredDone ?? 0,
          total: data?.requiredTotal ?? 0
        })
      }}</span>
      <Badge v-if="waitingHumanCount > 0" variant="warning" size="sm">
        {{
          t("s01.command-center.init.waitingBadge", {
            count: waitingHumanCount
          })
        }}
      </Badge>
      <Button
        variant="ghost"
        size="sm"
        data-testid="cc-init-expand"
        @click="userExpanded = true"
      >
        {{ t("s01.command-center.init.expand") }}
      </Button>
    </div>
    <template v-else>
      <Button
        variant="ghost"
        size="sm"
        class="init-collapse-toggle"
        data-testid="cc-init-collapse"
        @click="userExpanded = false"
      >
        {{ t("s01.command-center.init.collapse") }}
      </Button>
    <ul class="init-list">
      <li
        v-for="item in items"
        :key="item.key"
        class="init-row"
        :data-testid="`cc-init-${item.key}`"
      >
        <StatusIndicator
          :status="
            item.tone === 'success'
              ? 'success'
              : item.tone === 'danger'
                ? 'danger'
                : item.tone === 'info'
                  ? 'info'
                  : 'neutral'
          "
          size="sm"
        />
        <RouterLink
          v-if="item.link"
          class="init-row__title init-row__link"
          :to="item.link"
          :aria-label="t('s01.command-center.init.openStep')"
        >
          {{ item.title }}
        </RouterLink>
        <span v-else class="init-row__title">{{ item.title }}</span>
        <Badge v-if="item.required" variant="neutral" size="sm" outline>
          {{ requiredLabel }}
        </Badge>
        <span class="init-row__status">{{ item.statusLabel }}</span>
        <span v-if="item.detail" class="init-row__detail">{{
          item.detail
        }}</span>
        <Button
          v-if="item.key === 'code_analysis' && showAnalysisAction"
          variant="ghost"
          size="sm"
          :disabled="analysisActionDisabled"
          data-testid="cc-init-analysis-action"
          @click="runAnalysis"
        >
          {{ analysisActionLabel }}
        </Button>
        <span
          v-if="item.key === 'code_analysis' && analysisError"
          class="init-row__detail"
          data-testid="cc-init-analysis-error"
        >
          {{ analysisError }}
        </span>
      </li>
    </ul>
    </template>
  </Card>
</template>

<style scoped>
.init-collapsed {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.init-collapsed__summary {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-secondary);
}

.init-collapse-toggle {
  align-self: flex-end;
}

.init-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.init-card__head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-3);
}

.init-card__progress {
  margin-left: auto;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
  color: var(--ws-text-secondary);
}

.init-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.init-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
}

.init-row__title {
  flex: 1;
  min-width: 140px;
  color: var(--ws-text-primary);
}

/* 事项本体跳转：仍用主文字色，hover/focus 才换 accent，避免误读为 AI 建议 */
.init-row__link {
  color: var(--ws-text-primary);
  text-decoration: none;
}

.init-row__link:hover,
.init-row__link:focus-visible {
  color: var(--ws-accent);
  text-decoration: underline;
}

.init-row__status {
  color: var(--ws-text-secondary);
}

.init-row__detail {
  flex-basis: 100%;
  color: var(--ws-text-secondary);
}
</style>
