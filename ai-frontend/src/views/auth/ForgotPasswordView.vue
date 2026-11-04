<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2546 set="02 — Authentication / Forgot password" date=2026-11-04 -->
<script setup lang="ts">
import { ref } from "vue";
import { ArrowLeft, KeyRound } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import FormField from "@/components/ui/form-field/FormField.vue";
import Input from "@/components/ui/input/Input.vue";
import {
  AuthHeading,
  AuthLayout,
  AuthPanel,
  AuthTextAction
} from "@/components/auth";
import { AUTH_PATHS } from "@/router/auth";
import { useAuthStore } from "@/stores/auth";

/**
 * 忘记口令 —— Figma「02 — Authentication / Forgot password」（5:2546）。
 *
 * 单字段流程：提交后不暴露账号是否存在，统一回「查收邮件」信息态
 * （Figma 状态板口径），并给出「换一个邮箱」出口。
 */
const { t } = useI18n();
const auth = useAuthStore();

/** 邮箱 */
const email = ref("");
/** 提交中 */
const submitting = ref(false);
/** 字段级错误 */
const emailError = ref<string | undefined>(undefined);
/** 已发送（进入成功态） */
const sent = ref(false);

const emailField = ref<HTMLElement | null>(null);

const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

function validate() {
  emailError.value = undefined;
  const trimmed = email.value.trim();
  if (!trimmed) {
    emailError.value = t("auth.forgot.validation.emailRequired");
  } else if (!EMAIL_PATTERN.test(trimmed)) {
    emailError.value = t("auth.forgot.validation.emailInvalid");
  }
  return !emailError.value;
}

async function onSubmit() {
  if (!validate()) {
    emailField.value?.querySelector("input")?.focus();
    return;
  }

  submitting.value = true;
  try {
    await auth.requestPasswordReset(email.value.trim());
    sent.value = true;
  } finally {
    submitting.value = false;
  }
}

function useAnotherEmail() {
  sent.value = false;
  email.value = "";
  emailError.value = undefined;
}
</script>

<template>
  <AuthLayout>
    <AuthPanel>
      <AuthHeading
        :icon="KeyRound"
        :title="t('auth.forgot.title')"
        :subtitle="t('auth.forgot.subtitle')"
      />

      <div class="forgot__form">
        <Alert
          v-if="sent"
          variant="info"
          :title="t('auth.forgot.successTitle')"
        >
          {{ t("auth.forgot.successDetail") }}
        </Alert>

        <template v-if="!sent">
          <div ref="emailField">
            <FormField
              :label="t('auth.forgot.emailLabel')"
              :error="emailError"
              html-for="auth-forgot-email"
            >
              <Input
                id="auth-forgot-email"
                v-model="email"
                type="email"
                :placeholder="t('auth.forgot.emailPlaceholder')"
                :invalid="Boolean(emailError)"
                autocomplete="email"
                @update:model-value="emailError = undefined"
              />
            </FormField>
          </div>

          <Button type="submit" :loading="submitting" @click="onSubmit">
            {{
              submitting ? t("auth.forgot.submitting") : t("auth.forgot.submit")
            }}
          </Button>

          <p class="forgot__hint">{{ t("auth.forgot.hint") }}</p>
        </template>

        <AuthTextAction
          v-if="sent"
          :label="t('auth.forgot.useAnotherEmail')"
          @click="useAnotherEmail"
        />
      </div>

      <AuthTextAction
        :icon="ArrowLeft"
        :label="t('auth.forgot.backToSignIn')"
        :to="AUTH_PATHS.login"
      />
    </AuthPanel>

    <p class="forgot__guidance">{{ t("auth.forgot.guidance") }}</p>
  </AuthLayout>
</template>

<style scoped>
/* 找回流程组 —— Figma：间距 16 */
.forgot__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.forgot__hint {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-secondary);
}

/* 面板外指引 —— 12px 次文字居中 */
.forgot__guidance {
  margin: 0;
  width: var(--ws-auth-panel-width);
  max-width: 100%;
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  text-align: center;
  color: var(--ws-text-secondary);
}
</style>
