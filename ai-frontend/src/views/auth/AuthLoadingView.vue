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
import { useTeamStore } from "@/stores/team";

/**
 * 认证进行中 —— Figma「02 — Authentication / Authentication loading」（5:2688）。
 *
 * 三步进度（凭据已核验 → 建立安全会话 → 打开工作区）：
 * 凭据核验在进入本页前已完成，故第一步直接置为已完成；
 * 页面按节奏推进步骤，推进完毕后解析团队上下文并分流落点：
 *   - onboarding_required=true → /onboarding（首登必须建团或入团，Q1 裁决）；
 *   - 带 ?redirect= → 回跳原目标（路由守卫写入）；
 *   - 默认 → /personal。
 */
const { t } = useI18n();
const router = useRouter();
const teamStore = useTeamStore();

/** 当前进行到的步骤序号（0 起）；凭据核验在本页之前已完成，故从第 2 步开始 */
const stage = ref(1);

/** 步骤节奏（毫秒） */
const STAGE_INTERVAL = 1200;

const timers: ReturnType<typeof setTimeout>[] = [];

/** 分流只执行一次（定时器与组件卸载竞态防护） */
let settled = false;

/** 登录后落点：团队 onboarding 门 + redirect 回跳 + 默认工作区 */
async function settleDestination(): Promise<void> {
  if (settled) return;
  settled = true;
  const redirect = router.currentRoute.value.query.redirect;
  try {
    const context = await teamStore.load(true);
    if (context?.onboarding_required) {
      await router.push("/onboarding");
      return;
    }
  } catch {
    // 团队上下文解析失败不阻断进入工作区（页面级各自处理加载失败）。
  }
  if (typeof redirect === "string" && redirect.startsWith("/")) {
    await router.push(redirect);
    return;
  }
  await router.push("/personal");
}

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
        void settleDestination();
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
