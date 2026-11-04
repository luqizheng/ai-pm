<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Feedback/Skeleton" node=13:34281 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  width?: string;
  height?: string;
  /**
   * 占位线色档 —— Figma 双色：strong = border 色（首行/标题），
   * base = elevated 色（次行/内容块）。
   */
  tone?: "base" | "strong";
}

const props = withDefaults(defineProps<Props>(), {
  width: undefined,
  height: undefined,
  tone: "base"
});

const style = computed(() => ({
  width: props.width ?? "100%",
  height: props.height ?? "1em"
}));

const classes = computed(() => cn("ui-skeleton", `ui-skeleton--${props.tone}`));
</script>

<template>
  <div class="ui-skeleton" :class="classes" :style="style" aria-hidden="true" />
</template>

<style scoped>
/* Figma DS/Feedback/Skeleton（13:34281）：占位块 radius 4 / 双色
 * （强线 --skeleton-base-strong=border 色、弱线 --skeleton-base=elevated 色）；
 * shimmer 扫光保留为加载动效（reduced-motion 静止为静态色块）。 */
.ui-skeleton {
  border-radius: var(--radius-xs);
  background-color: var(--skeleton-base);
  background-image: linear-gradient(
    90deg,
    transparent,
    var(--skeleton-shimmer),
    transparent
  );
  background-size: 200% 100%;
  animation: ui-skeleton-shimmer var(--duration-shimmer) linear infinite;
}

.ui-skeleton--strong {
  background-color: var(--skeleton-base-strong);
}

/* 减少动效 —— 骨架屏静止为静态色块 */
@media (prefers-reduced-motion: reduce) {
  .ui-skeleton {
    animation: none;
    background-image: none;
  }
}

@keyframes ui-skeleton-shimmer {
  from {
    background-position: 200% 0;
  }
  to {
    background-position: -200% 0;
  }
}
</style>
