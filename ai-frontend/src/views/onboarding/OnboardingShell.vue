<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

/**
 * Team Onboarding 共享外壳：应用壳之外（meta.layout="bare"）的定版居中版式。
 * 顶部品牌位 + 步骤指示 + 内容卡片。无 Skip/Later（产品裁决 Q1：
 * 首登必须建团或入团），也不渲染任何工作区导航（设计稿 §5 导航规则）。
 */
const { t } = useI18n();
const route = useRoute();

/** 当前步骤（welcome=0 起）；accept 屏不经此外壳 */
const STEPS: string[] = ["welcome", "create", "invite", "link"];
const currentStep = computed(() =>
  Math.max(0, STEPS.indexOf((route.name as string | undefined) ?? ""))
);
const stepLabels = computed(() => [
  t("team.onboarding.step.welcome"),
  t("team.onboarding.step.create"),
  t("team.onboarding.step.invite"),
  t("team.onboarding.step.link")
]);
</script>

<template>
  <div class="ob-shell">
    <header class="ob-shell__top">
      <span class="ob-shell__brand">PM-AI</span>
      <ol class="ob-shell__steps" aria-hidden="true">
        <li
          v-for="(label, index) in stepLabels"
          :key="label"
          class="ob-shell__step"
          :class="{
            'is-current': index === currentStep,
            'is-done': index < currentStep
          }"
        >
          {{ label }}
        </li>
      </ol>
    </header>

    <main class="ob-shell__main">
      <slot />
    </main>

    <footer class="ob-shell__foot">
      {{ t("team.onboarding.footer") }}
    </footer>
  </div>
</template>

<style scoped>
.ob-shell {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--surface-sunken);
  color: var(--text-primary);
}

.ob-shell__top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-5);
  padding: var(--space-5) var(--space-8);
  border-bottom: 1px solid var(--border-subtle);
  background: var(--surface-base);
}

.ob-shell__brand {
  font-family: var(--font-ui);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
  letter-spacing: 0.02em;
  color: var(--accent);
}

.ob-shell__steps {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ob-shell__step {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.ob-shell__step.is-current {
  color: var(--text-primary);
  font-weight: var(--weight-medium);
}

.ob-shell__step.is-done {
  color: var(--accent);
}

.ob-shell__main {
  flex: 1;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding: var(--space-8) var(--space-5);
  overflow-y: auto;
}

.ob-shell__foot {
  padding: var(--space-4) var(--space-8);
  border-top: 1px solid var(--border-subtle);
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
  text-align: center;
}
</style>
