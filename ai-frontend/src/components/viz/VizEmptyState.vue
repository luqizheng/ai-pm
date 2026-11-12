<script setup lang="ts">
/**
 * VizEmptyState —— 图形区空态。
 *
 * 纪律（AGENTS.md 规则 4 + 18 号 UX 规格）：**不得用演示数据填充空态**——
 * 没有图形就说没有，并给出真实可走的下一步（去对话里要求生成 / 选一个已有槽位）。
 */
import { useI18n } from "vue-i18n";
import { Network } from "lucide-vue-next";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";

interface Props {
  /** 区分「加载失败」与「确实没有图形」——失败态要给出重试入口。 */
  failed?: boolean;
  /** 重试（加载失败时）。 */
  onRetry?: () => void;
}

const props = withDefaults(defineProps<Props>(), {
  failed: false,
  onRetry: undefined
});

const { t } = useI18n();
</script>

<template>
  <EmptyState
    size="sm"
    :icon="Network"
    :title="props.failed ? t('viz.tab.loadFailed') : t('viz.tab.empty')"
    :description="
      props.failed ? t('viz.tab.loadFailedHint') : t('viz.tab.emptyHint')
    "
    :action-label="props.failed ? t('viz.tab.retry') : ''"
    @action="props.onRetry?.()"
  />
</template>
