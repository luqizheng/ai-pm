<script setup lang="ts">
import type { Component } from "vue";

import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import { CircleAlert, Inbox, LoaderCircle, SearchX } from "lucide-vue-next";

/**
 * 队列四态（V1.0 §13）：全部处理完成 / 无匹配 / 加载中 / 加载失败。
 *
 * 纯展示件：文案由调用方翻译后经 props 传入（D8）。
 */
export type ReviewEmptyKind = "allDone" | "noMatch" | "loading" | "loadFailed";

interface Props {
  kind: ReviewEmptyKind;
  title: string;
  hint?: string;
  retryLabel?: string;
}

const props = defineProps<Props>();
defineEmits<{ retry: [] }>();

const ICONS: Record<ReviewEmptyKind, Component> = {
  allDone: Inbox,
  noMatch: SearchX,
  loading: LoaderCircle,
  loadFailed: CircleAlert
};
</script>

<template>
  <div class="review-empty" :data-kind="props.kind">
    <EmptyState
      :icon="ICONS[props.kind]"
      :title="props.title"
      :description="props.hint"
      :action-label="
        props.kind === 'loadFailed' ? (props.retryLabel ?? '') : ''
      "
      @action="$emit('retry')"
    />
  </div>
</template>

<style scoped>
.review-empty {
  padding: 32px 16px;
}
</style>
