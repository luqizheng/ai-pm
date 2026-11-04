<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2688 set="02 — Authentication / Authentication loading" date=2026-11-04 -->
<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import { LoaderCircle } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import {
  AuthHeading,
  AuthLayout,
  AuthPanel,
  AuthProgressSteps,
  AuthTextAction,
  type AuthStep
} from "@/components/auth";
import { AUTH_PATHS } from "@/router/auth";

/**
 * 认证进行中 —— Figma「02 — Authentication / Authentication loading」（5:2688）。
 *
 * 三步进度（凭据已核验 → 建立安全会话 → 打开工作区）：
 * 凭据核验在进入本页前已完成，故第一步直接置为已完成；
 * 页面按节奏推进步骤，推进完毕后进入工作区。
 */
const { t } = useI18n();
const router = useRouter();

/** 当前进行到的步骤序号（0 起）；凭据核验在本页之前已完成，故从第 2 步开始 */
const stage = ref(1);

/** 步骤节奏（毫秒） */
const STAGE_INTERVAL = 1200;

const timers: ReturnType<typeof setTimeout>[] = [];

const steps = computed<AuthStep[]>(() => {
  const labels = [
    t("auth.loading.stepCredentials"),
    t("auth.loading.stepSession"),
    t("auth.loading.stepWorkspace")
  ];
  return labels.map((label, index) => ({
    id: String(index),
    label,
    state:
      index < stage.value
        ? "done"
        : index === stage.value
          ? "current"
          : "pending"
  }));
});

/** 进度比例 —— 已推进到的步骤数 / 总步骤数 */
const progress = computed(() => (stage.value + 1) / steps.value.length);

function schedule() {
  if (stage.value >= steps.value.length - 1) {
    timers.push(
      setTimeout(() => {
        void router.push("/personal");
      }, STAGE_INTERVAL)
    );
    return;
  }
  timers.push(
    setTimeout(() => {
      stage.value += 1;
      schedule();
    }, STAGE_INTERVAL)
  );
}

function onCancel() {
  void router.push(AUTH_PATHS.login);
}

schedule();

onBeforeUnmount(() => {
  for (const timer of timers) {
    clearTimeout(timer);
  }
  timers.length = 0;
});
</script>

<template>
  <AuthLayout>
    <AuthPanel>
      <AuthHeading
        :icon="LoaderCircle"
        spin
        :title="t('auth.loading.title')"
        :subtitle="t('auth.loading.subtitle')"
      />

      <AuthProgressSteps :steps="steps" :progress="progress" />

      <p class="loading__hint">{{ t("auth.loading.hint") }}</p>

      <AuthTextAction :label="t('auth.loading.cancel')" @click="onCancel" />
    </AuthPanel>
  </AuthLayout>
</template>

<style scoped>
.loading__hint {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-secondary);
}
</style>
