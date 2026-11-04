<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2767 set="02 — Authentication / Unauthorized access" date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { ArrowLeft, ShieldAlert } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import Button from "@/components/ui/button/Button.vue";
import {
  AuthDetailCard,
  AuthDetailRow,
  AuthHeading,
  AuthLayout,
  AuthPanel,
  AuthTextAction
} from "@/components/auth";
import { AUTH_PATHS } from "@/router/auth";
import { useAuthStore } from "@/stores/auth";

/**
 * 无权访问（403）—— Figma「02 — Authentication / Unauthorized access」（5:2767）。
 *
 * 已登录但目标工作区无权限：回显「当前身份 / 目标工作区」，
 * 给出换账号与返回工作区两条出口，并标注 403 状态码便于对客沟通。
 */
const { t } = useI18n();
const router = useRouter();
const auth = useAuthStore();

/** 演示账号（无会话上下文时的回显账号） */
const DEMO_ACCOUNT = "alex.morgan@acme.dev";
/** 演示目标工作区 */
const DEMO_WORKSPACE = "Acme Research";

/** 当前登录账号 */
const account = computed(() => auth.account ?? DEMO_ACCOUNT);
/** 目标工作区 */
const workspace = computed(() => auth.organization ?? DEMO_WORKSPACE);

function onSwitchAccount() {
  auth.signOut();
  void router.push(AUTH_PATHS.login);
}

function onBackToWorkspaces() {
  void router.push("/personal");
}
</script>

<template>
  <AuthLayout>
    <AuthPanel>
      <AuthHeading
        :icon="ShieldAlert"
        :title="t('auth.unauthorized.title')"
        :subtitle="t('auth.unauthorized.subtitle')"
      />

      <AuthDetailCard padding="md" gap="md">
        <AuthDetailRow
          :label="t('auth.unauthorized.signedInAs')"
          :value="account"
        />
        <AuthDetailRow
          :label="t('auth.unauthorized.requestedWorkspace')"
          :value="workspace"
        />
      </AuthDetailCard>

      <p class="unauthorized__description">
        {{ t("auth.unauthorized.description") }}
      </p>

      <div class="unauthorized__actions">
        <Button type="button" @click="onSwitchAccount">
          {{ t("auth.unauthorized.switchAccount") }}
        </Button>
        <Button
          variant="secondary"
          surface="elevated"
          @click="onBackToWorkspaces"
        >
          <ArrowLeft class="unauthorized__action-icon" aria-hidden="true" />
          {{ t("auth.unauthorized.backToWorkspaces") }}
        </Button>
      </div>

      <div class="unauthorized__reference">
        <span class="unauthorized__code">
          {{ t("auth.unauthorized.reference") }}
        </span>
        <AuthTextAction :label="t('auth.unauthorized.contactSupport')" />
      </div>
    </AuthPanel>
  </AuthLayout>
</template>

<style scoped>
.unauthorized__description {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-secondary);
}

/* 动作组 —— Figma：间距 12 */
.unauthorized__actions {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.unauthorized__action-icon {
  width: var(--space-3);
  height: var(--space-3);
}

/* 状态码行 —— 左右分布，状态码走等宽字体 */
.unauthorized__reference {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.unauthorized__code {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}
</style>
