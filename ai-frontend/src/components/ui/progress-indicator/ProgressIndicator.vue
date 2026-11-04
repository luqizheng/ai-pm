<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Feedback/ProgressIndicator" node=13:34246 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

/**
 * ProgressIndicator —— Figma DS/Feedback/ProgressIndicator（13:34246）。
 * 轨道（surface 底）+ 指示条（variant 语义色）+ 可选标签；
 * indeterminate 态走来回动画，reduced-motion 下静止。
 * 登录页进度条口径：3px 轨道 / accent 指示条（Authentication loading）。
 */

export type ProgressIndicatorSize = "sm" | "md" | "lg";
export type ProgressIndicatorVariant =
  | "default"
  | "accent"
  | "success"
  | "warning"
  | "danger";

interface Props {
  value?: number;
  max?: number;
  variant?: ProgressIndicatorVariant;
  size?: ProgressIndicatorSize;
  label?: string;
  indeterminate?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  value: 0,
  max: 100,
  variant: "accent",
  size: "sm",
  label: "",
  indeterminate: false
});

const percent = computed(() => {
  if (props.max <= 0) return 0;
  const ratio = (props.value / props.max) * 100;
  return Math.min(100, Math.max(0, Math.round(ratio)));
});

const hasLabel = computed(() => props.label.length > 0);
</script>

<template>
  <div
    :class="
      cn('ui-progress', `ui-progress--${size}`, `ui-progress--${variant}`, {
        'is-indeterminate': indeterminate
      })
    "
    role="progressbar"
    :aria-valuenow="indeterminate ? undefined : percent"
    :aria-valuemin="0"
    :aria-valuemax="100"
    :aria-busy="indeterminate ? 'true' : undefined"
  >
    <div class="ui-progress__track">
      <div
        class="ui-progress__bar"
        :style="indeterminate ? undefined : { width: `${percent}%` }"
      />
    </div>
    <span v-if="hasLabel" class="ui-progress__label">{{ label }}</span>
  </div>
</template>

<style scoped>
.ui-progress {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ui-progress__track {
  width: 100%;
  overflow: hidden;
  border-radius: var(--radius-full);
  background-color: var(--surface-raised);
}

.ui-progress--sm .ui-progress__track {
  height: var(--size-progress-track-sm);
}

.ui-progress--md .ui-progress__track {
  height: var(--size-progress-track);
}

.ui-progress--lg .ui-progress__track {
  height: var(--size-progress-track-lg);
}

.ui-progress__bar {
  height: 100%;
  border-radius: var(--radius-full);
  background-color: var(--accent);
  transition: width var(--duration-medium) var(--ease-standard);
}

/* 变体映射到语义色 */
.ui-progress--default .ui-progress__bar {
  background-color: var(--text-secondary);
}

.ui-progress--success .ui-progress__bar {
  background-color: var(--ws-color-success);
}

.ui-progress--warning .ui-progress__bar {
  background-color: var(--ws-color-warning);
}

.ui-progress--danger .ui-progress__bar {
  background-color: var(--ws-color-danger);
}

/* indeterminate：来回扫动 */
.ui-progress.is-indeterminate .ui-progress__bar {
  width: 40%;
  animation: ui-progress-sweep var(--duration-slow) var(--ease-standard)
    infinite alternate;
}

@keyframes ui-progress-sweep {
  from {
    margin-left: 0;
  }

  to {
    margin-left: 60%;
  }
}

.ui-progress__label {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

@media (prefers-reduced-motion: reduce) {
  .ui-progress.is-indeterminate .ui-progress__bar {
    animation: none;
  }

  .ui-progress__bar {
    transition: none;
  }
}
</style>
