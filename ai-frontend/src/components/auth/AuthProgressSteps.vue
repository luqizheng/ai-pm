<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2706 set="02 — Authentication / Authentication progress" date=2026-11-04 -->
<script setup lang="ts">
import { CircleCheck, LoaderCircle } from "lucide-vue-next";

/**
 * 认证进度 —— 登录中页的进度条 + 步骤清单。
 *
 * Figma：进度条 3px（轨道 --ws-border / 指示 accent，radius 4）；
 * 步骤行 = 16px 图标 + 12px 文案，三态：
 *   done —— 对勾（次文字色）/ current —— 转圈（主文字色）/ pending —— 空心圆（描边色）
 */
export type AuthStepState = "done" | "current" | "pending";

export interface AuthStep {
  /** 稳定标识 */
  id: string;
  /** 步骤文案 */
  label: string;
  /** 步骤状态 */
  state: AuthStepState;
}

interface Props {
  /** 步骤清单 */
  steps: AuthStep[];
  /** 进度比例（0–1），渲染为进度条指示宽度 */
  progress?: number;
}

const props = withDefaults(defineProps<Props>(), {
  progress: 0
});

/** 进度比例收敛到 0–1 */
function clamp(value: number) {
  if (!Number.isFinite(value)) return 0;
  return Math.min(1, Math.max(0, value));
}
</script>

<template>
  <div class="auth-progress">
    <div
      class="auth-progress__track"
      role="progressbar"
      :aria-valuenow="Math.round(clamp(props.progress) * 100)"
      aria-valuemin="0"
      aria-valuemax="100"
    >
      <span
        class="auth-progress__indicator"
        :style="{ width: `${clamp(props.progress) * 100}%` }"
      />
    </div>

    <ul class="auth-progress__steps">
      <li
        v-for="step in steps"
        :key="step.id"
        class="auth-progress__step"
        :data-state="step.state"
        :aria-current="step.state === 'current' ? 'step' : undefined"
      >
        <CircleCheck
          v-if="step.state === 'done'"
          class="auth-progress__icon"
          aria-hidden="true"
        />
        <LoaderCircle
          v-else-if="step.state === 'current'"
          class="auth-progress__icon auth-progress__icon--spinning"
          aria-hidden="true"
        />
        <span v-else class="auth-progress__dot" aria-hidden="true" />
        <span class="auth-progress__label">{{ step.label }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.auth-progress {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

/* 进度条 —— 3px 轨道 + accent 指示 */
.auth-progress__track {
  width: 100%;
  height: var(--size-indicator);
  border-radius: var(--radius-xs);
  background: var(--ws-border);
  overflow: hidden;
}

.auth-progress__indicator {
  display: block;
  height: 100%;
  border-radius: var(--radius-xs);
  background: var(--ws-accent);
  transition: width var(--duration-medium) var(--ease-decelerate);
}

.auth-progress__steps {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.auth-progress__step {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.auth-progress__icon {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--ws-text-secondary);
}

.auth-progress__icon--spinning {
  color: var(--ws-text-primary);
  animation: auth-progress-spin var(--duration-spinner) linear infinite;
}

/* 未开始步骤 —— 空心圆（描边色） */
.auth-progress__dot {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-full);
}

.auth-progress__label {
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-secondary);
}

.auth-progress__step[data-state="current"] .auth-progress__label {
  color: var(--ws-text-primary);
}

@keyframes auth-progress-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .auth-progress__icon--spinning {
    animation: none;
  }
}
</style>
