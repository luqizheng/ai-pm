<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowRight, Info } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";
import type { InitStage } from "@/api/initialization-overview";
import InitStatusBadge from "./InitStatusBadge.vue";

/**
 * 初始化阶段卡片。
 *
 * 五要素（设计稿 §3.0.3）：阶段序号 + 标题 / 状态徽标 / 实况 / 权威来源 / 依赖与入口。
 *
 * 纪律：
 *   - 状态徽标始终可见（含「不适用」「尚无法判断」）；
 *   - 权威来源缺失时显示「暂无权威来源」而不是省略；
 *   - 阶段**永不禁用**——六阶段是阅读分组，不是门禁；
 *   - 前端合成的阶段（基线确认）标注来源，避免把推导当成后端事实。
 */

interface Props {
  stage: InitStage;
  /** 依赖阶段的展示名（已本地化） */
  dependencyLabels: string[];
}

const props = defineProps<Props>();

const emit = defineEmits<{ open: [stage: InitStage] }>();

const { t } = useI18n();

const title = computed(() =>
  t(`s01.initialization-overview.${props.stage.titleKey}.title`)
);

const hint = computed(() =>
  t(`s01.initialization-overview.${props.stage.titleKey}.hint`)
);

/** 实况描述：后端 detail 优先；无 detail 时用阶段提示文案兜底 */
const detail = computed(() => props.stage.detail ?? hint.value);

/**
 * 实况描述。
 *
 * 优先级：
 *   1. 基线阶段的合成枚举（conflict / awaiting_cognition / pending_confirmations）；
 *   2. 结构化合成（detailKey + detailParams，走 i18n 模板，可随语言切换）；
 *   3. 后端 detail 原文；
 *   4. 仅在「未开始」时用阶段提示兜底——已开始的事项用提示语会与事实冲突。
 */
const detailLabel = computed(() => {
  const raw = props.stage.detail;
  if (props.stage.id === "baseline") {
    if (raw === "conflict") {
      return t("s01.initialization-overview.stage.baseline.detailConflict");
    }
    if (raw === "awaiting_cognition") {
      return t("s01.initialization-overview.stage.baseline.detailAwaiting");
    }
    if (raw === "pending_confirmations") {
      return t("s01.initialization-overview.stage.baseline.detailPending", {
        count: props.stage.sourceState ?? "0"
      });
    }
  }
  if (props.stage.detailKey) {
    return t(
      `s01.initialization-overview.${props.stage.detailKey}`,
      props.stage.detailParams ?? {}
    );
  }
  if (raw) return raw;
  return props.stage.status === "not_started" ? hint.value : "";
});

const dependencyText = computed(() => {
  if (props.dependencyLabels.length === 0) {
    return t("s01.initialization-overview.stage.independent");
  }
  return t("s01.initialization-overview.stage.dependsOn", {
    stage: props.dependencyLabels.join(" / ")
  });
});
</script>

<template>
  <article
    class="init-stage-card"
    :class="`is-${props.stage.status}`"
    :data-stage="props.stage.id"
    :data-status="props.stage.status"
  >
    <header class="init-stage-card__header">
      <span class="init-stage-card__index" aria-hidden="true">
        {{ props.stage.order }}
      </span>
      <h3 class="init-stage-card__title">{{ title }}</h3>
      <InitStatusBadge :status="props.stage.status" size="sm" />
    </header>

    <p v-if="detailLabel" class="init-stage-card__detail">{{ detailLabel }}</p>

    <p class="init-stage-card__source">
      <span class="init-stage-card__source-label">
        {{ t("s01.initialization-overview.stage.source") }}
      </span>
      <code
        v-if="props.stage.sourceState"
        class="init-stage-card__source-value"
      >
        {{ props.stage.sourceState }}
      </code>
      <span v-else class="init-stage-card__source-empty">
        {{ t("s01.initialization-overview.stage.sourceEmpty") }}
      </span>
    </p>

    <p v-if="props.stage.source === 'derived'" class="init-stage-card__note">
      <Info :size="12" aria-hidden="true" />
      <span>{{ t("s01.initialization-overview.stage.derivedNote") }}</span>
    </p>

    <footer class="init-stage-card__footer">
      <span class="init-stage-card__dependency">{{ dependencyText }}</span>
      <Button
        v-if="props.stage.route"
        variant="ghost"
        size="sm"
        @click="emit('open', props.stage)"
      >
        {{ t("s01.initialization-overview.stage.open") }}
        <ArrowRight :size="14" aria-hidden="true" />
      </Button>
    </footer>
  </article>
</template>

<style scoped>
.init-stage-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
}

.init-stage-card__header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.init-stage-card__index {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  background: var(--ws-elevated);
  border-radius: var(--radius-xs);
}

.init-stage-card__title {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.init-stage-card__detail {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
}

.init-stage-card__source {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin: 0;
}

.init-stage-card__source-label {
  flex: none;
  color: var(--ws-text-disabled);
  font-size: var(--font-size-micro);
}

.init-stage-card__source-value {
  overflow: hidden;
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.init-stage-card__source-empty {
  color: var(--ws-text-disabled);
  font-size: var(--font-size-micro);
}

.init-stage-card__note {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  color: var(--ws-text-disabled);
  font-size: var(--font-size-micro);
}

.init-stage-card__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding-top: var(--space-3);
  border-top: var(--border-1) solid var(--ws-border);
}

/* 依赖文案可换行，入口按钮保持单行 */
.init-stage-card__footer :deep(.ui-button) {
  flex: none;
  white-space: nowrap;
}

.init-stage-card__dependency {
  color: var(--ws-text-disabled);
  font-size: var(--font-size-micro);
}

/* 阻塞态：左侧 2px 轨（颜色只承载紧急度，不承载类型） */
.init-stage-card.is-blocked {
  border-left: var(--size-indicator) solid var(--ws-color-danger);
}
</style>
