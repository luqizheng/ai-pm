<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2728 set="02 — Authentication / Session expired" date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { Timer } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import {
  AuthDetailCard,
  AuthHeading,
  AuthLayout,
  AuthPanel,
  AuthTextAction
} from "@/components/auth";
import { AUTH_PATHS } from "@/router/auth";
import { useAuthStore } from "@/stores/auth";

/**
 * 会话过期 —— Figma「02 — Authentication / Session expired」（5:2728）。
 *
 * 说明三件事：为什么被登出、上次的账号是谁、回到哪里。
 * 「换个账号」直接回登录页，不带旧账号。
 */
const { t } = useI18n();
const router = useRouter();
const auth = useAuthStore();

/** 演示账号（无会话上下文时的回显账号） */
const DEMO_ACCOUNT = "alex.morgan@acme.dev";
/** 演示组织（无会话上下文时的回显组织） */
const DEMO_ORGANIZATION = "Acme Research";

/** 上次登录的账号 */
const account = computed(() => auth.account ?? DEMO_ACCOUNT);
/** 上次登录的组织 */
const organization = computed(() => auth.organization ?? DEMO_ORGANIZATION);

function onSignInAgain() {
  auth.signOut();
  void router.push(AUTH_PATHS.login);
}
</script>

<template>
  <AuthLayout>
    <AuthPanel>
      <AuthHeading
        :icon="Timer"
        :title="t('auth.sessionExpired.title')"
        :subtitle="t('auth.sessionExpired.subtitle')"
      />

      <AuthDetailCard padding="sm" gap="sm">
        <span class="session__account">{{ account }}</span>
        <span class="session__organization">{{ organization }}</span>
      </AuthDetailCard>

      <Alert variant="info" :title="t('auth.sessionExpired.infoTitle')">
        {{ t("auth.sessionExpired.infoDetail") }}
      </Alert>

      <div class="session__actions">
        <Button type="button" @click="onSignInAgain">
          {{ t("auth.sessionExpired.signInAgain") }}
        </Button>
        <div class="session__alternative">
          <AuthTextAction
            :label="t('auth.sessionExpired.differentAccount')"
            :to="AUTH_PATHS.login"
          />
        </div>
      </div>
    </AuthPanel>
  </AuthLayout>
</template>

<style scoped>
/* 上次账号 —— 12px Medium 主文字 + 11px 次文字 */
.session__account {
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--ws-text-primary);
}

.session__organization {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}

/* 恢复动作组 —— Figma：间距 16 */
.session__actions {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.session__alternative {
  display: flex;
  justify-content: center;
}
</style>
