<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2638 set="02 — Authentication / Invitation acceptance" date=2026-11-04 -->
<script setup lang="ts">
import { computed, ref } from "vue";
import { MailOpen } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import FormField from "@/components/ui/form-field/FormField.vue";
import PasswordInput from "@/components/ui/input/PasswordInput.vue";
import {
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
 * 邀请链接携带一次性 token（`?token=`）。用户在本页设置口令后，
 * 由 iam-service 消费邀请票据并直接返回登录态。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const auth = useAuthStore();

/** 邀请票据 */
const token = computed(() =>
  typeof route.query.token === "string" ? route.query.token : ""
);

/** 新口令 */
const password = ref("");
/** 确认口令 */
const confirm = ref("");
/** 字段级错误 */
const passwordError = ref<string | undefined>(undefined);
const confirmError = ref<string | undefined>(undefined);
/** 提交中 */
const submitting = ref(false);
/** 邀请失效（无 token 或服务端拒绝） */
const invalid = ref(!token.value);

function validate() {
  passwordError.value = undefined;
  confirmError.value = undefined;

  if (!password.value) {
    passwordError.value = t("auth.invitation.validation.passwordRequired");
  } else if (password.value.length < 8) {
    passwordError.value = t("auth.invitation.validation.passwordTooShort");
  }

  if (!confirm.value || confirm.value !== password.value) {
    confirmError.value = t("auth.invitation.validation.mismatch");
  }

  return !passwordError.value && !confirmError.value;
}

async function onAccept() {
  invalid.value = !token.value;
  if (!validate() || !token.value) {
    return;
  }

  submitting.value = true;
  try {
    await auth.acceptInvitation({
      token: token.value,
      password: password.value
    });
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
        :subtitle="t('auth.invitation.subtitle')"
      />

      <Alert
        v-if="invalid"
        variant="danger"
        :title="t('auth.invitation.errorTitle')"
      >
        {{ t("auth.invitation.errorDetail") }}
      </Alert>

      <div class="invitation__form">
        <FormField
          :label="t('auth.invitation.passwordLabel')"
          :error="passwordError"
          html-for="auth-invitation-password"
        >
          <PasswordInput
            id="auth-invitation-password"
            v-model="password"
            :placeholder="t('auth.invitation.passwordPlaceholder')"
            :invalid="Boolean(passwordError)"
            autocomplete="new-password"
            @update:model-value="passwordError = undefined"
          />
        </FormField>

        <FormField
          :label="t('auth.invitation.confirmLabel')"
          :error="confirmError"
          html-for="auth-invitation-confirm"
        >
          <PasswordInput
            id="auth-invitation-confirm"
            v-model="confirm"
            :placeholder="t('auth.invitation.passwordPlaceholder')"
            :invalid="Boolean(confirmError)"
            autocomplete="new-password"
            @update:model-value="confirmError = undefined"
          />
        </FormField>
      </div>

      <div class="invitation__actions">
        <Button type="submit" :loading="submitting" @click="onAccept">
          {{
            submitting
              ? t("auth.invitation.submitting")
              : t("auth.invitation.submit")
          }}
        </Button>
        <p class="invitation__note">{{ t("auth.invitation.note") }}</p>
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
.invitation__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

/* 动作组 —— Figma：间距 12 */
.invitation__actions {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
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
