<script setup lang="ts">
/**
 * VizStaleBar —— 过期提示条（stale 状态的显式告知）。
 *
 * 依据 18 号 UX 规格 §6.2：用户必须明确看到「此图形所依据的业务数据已经发生变化，
 * 当前展示的是历史版本」，并可查看旧版本 / 查看变化的来源 / 请求重新生成。
 * **Stale 不自动重渲染**（ADR-VIZ-006）——只提示，由用户决定。
 */
import { useI18n } from "vue-i18n";
import { AlertTriangle, RefreshCw } from "lucide-vue-next";

interface Props {
  /** 是否展示。 */
  visible: boolean;
  /** 已变化的来源数量（可空——尚未逐项比对时不要假装知道）。 */
  changedCount?: number;
  /** 请求重新生成（触发新一轮 AI 生成，不覆盖已发布版本）。 */
  regenerating?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  changedCount: undefined,
  regenerating: false
});

const { t } = useI18n();

const emit = defineEmits<{ regenerate: [] }>();
</script>

<template>
  <div
    v-if="props.visible"
    class="viz-stale"
    role="status"
    :data-testid="'viz-stale-bar'"
  >
    <AlertTriangle :size="14" aria-hidden="true" />
    <span class="viz-stale__text">{{ t("viz.artifact.staleHint") }}</span>
    <span v-if="props.changedCount !== undefined" class="viz-stale__count">
      {{ t("viz.stale.changed", { count: props.changedCount }) }}
    </span>
    <button
      type="button"
      class="viz-stale__action"
      :disabled="props.regenerating"
      :data-testid="'viz-stale-regenerate'"
      @click="emit('regenerate')"
    >
      <RefreshCw :size="12" aria-hidden="true" />
      <span>{{ t("viz.stale.regenerate") }}</span>
    </button>
  </div>
</template>

<style scoped>
.viz-stale {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
  padding: var(--space-2) var(--space-3);
  font-size: var(--font-size-micro);
  color: var(--warning-strong);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
}

.viz-stale__text {
  flex: 1;
  min-width: 0;
}

.viz-stale__count {
  color: var(--text-tertiary);
}

.viz-stale__action {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  height: var(--size-control-md);
  padding: 0 var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-primary);
  background: transparent;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.viz-stale__action:disabled {
  opacity: var(--opacity-disabled-control);
  cursor: not-allowed;
}
</style>
