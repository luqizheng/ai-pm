<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2638 set="02 — Authentication / Invitation acceptance" date=2026-11-04 -->
<script setup lang="ts">
import { ref } from "vue";
import { ArrowRight, MailOpen } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import Alert from "@/components/ui/alert/Alert.vue";
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
 * 接受邀请 —— Figma「02 — Authentication / Invitation acceptance」（5:2638）。
 *
 * 邀请落地页：先回显「谁邀请了你 / 去哪个工作区 / 以什么角色 / 何时过期」，
 * 再由本人确认加入；不用静默建号。
 */
const { t } = useI18n();
const router = useRouter();
const auth = useAuthStore();

/** 演示邀请上下文（真实场景来自邀请票据） */
const INVITATION = {
  inviter: "Maya Chen",
  workspace: "Acme Research",
  workspaceSlug: "acme-research",
  email: "alex.morgan@acme.dev",
  initials: "AR"
};

/** 提交中 */
const submitting = ref(false);
/** 邀请失效 */
const invalid = ref(false);

async function onAccept() {
  invalid.value = false;
  submitting.value = true;
  try {
    await auth.acceptInvitation();
    await router.push(AUTH_PATHS.signingIn);
  } catch {
    invalid.value = true;
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <AuthLayout>
    <AuthPanel>
      <AuthHeading
        :icon="MailOpen"
        :title="t('auth.invitation.title')"
        :subtitle="
          t('auth.invitation.subtitle', {
            inviter: INVITATION.inviter,
            workspace: INVITATION.workspace
          })
        "
      />

      <Alert
        v-if="invalid"
        variant="danger"
        :title="t('auth.invitation.errorTitle')"
      >
        {{ t("auth.invitation.errorDetail") }}
      </Alert>

      <AuthDetailCard padding="md" gap="md">
        <div class="invitation__workspace">
          <span class="invitation__avatar" aria-hidden="true">
            {{ INVITATION.initials }}
          </span>
          <span class="invitation__workspace-text">
            <span class="invitation__workspace-name">
              {{ INVITATION.workspace }}
            </span>
            <span class="invitation__workspace-slug">
              {{ INVITATION.workspaceSlug }} ·
              {{ t("auth.chrome.product") }}
            </span>
          </span>
        </div>

        <AuthDetailRow
          :label="t('auth.invitation.invitedEmailLabel')"
          :value="INVITATION.email"
        />
        <AuthDetailRow
          :label="t('auth.invitation.roleLabel')"
          :value="t('auth.invitation.role')"
        />
        <AuthDetailRow
          :label="t('auth.invitation.expiresLabel')"
          :value="t('auth.invitation.expiresAt')"
        />
      </AuthDetailCard>

      <div class="invitation__actions">
        <Button type="submit" :loading="submitting" @click="onAccept">
          {{
            submitting
              ? t("auth.invitation.submitting")
              : t("auth.invitation.submit")
          }}
          <ArrowRight class="invitation__action-icon" aria-hidden="true" />
        </Button>
        <p class="invitation__note">
          {{ t("auth.invitation.note", { account: INVITATION.email }) }}
        </p>
      </div>

      <div class="invitation__switch">
        <span class="invitation__switch-label">
          {{ t("auth.invitation.notYourAccount") }}
        </span>
        <AuthTextAction
          :label="t('auth.invitation.useAnotherAccount')"
          :to="AUTH_PATHS.login"
        />
      </div>
    </AuthPanel>
  </AuthLayout>
</template>

<style scoped>
/* 工作区标识行 —— Figma：32×32 头像块 + 名称 / 标识两行 */
.invitation__workspace {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.invitation__avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  width: var(--ws-auth-indicator-size);
  height: var(--ws-auth-indicator-size);
  border-radius: var(--radius-xs);
  background: var(--ws-elevated);
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.invitation__workspace-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.invitation__workspace-name {
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  color: var(--ws-text-primary);
}

.invitation__workspace-slug {
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}

/* 动作组 —— Figma：间距 12 */
.invitation__actions {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.invitation__action-icon {
  width: var(--space-3);
  height: var(--space-3);
}

.invitation__note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}

/* 换账号行 —— 左右分布 */
.invitation__switch {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.invitation__switch-label {
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-secondary);
}
</style>
