<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2584 set="02 — Authentication / Password reset" date=2026-11-04 -->
<script setup lang="ts">
import { computed, ref } from "vue";
import { ArrowLeft, LockKeyhole } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import FormField from "@/components/ui/form-field/FormField.vue";
import PasswordInput from "@/components/ui/input/PasswordInput.vue";
import {
  AuthHeading,
  AuthLayout,
  AuthPanel,
  AuthRequirementList,
  AuthTextAction,
  type AuthRequirement
} from "@/components/auth";
import { AUTH_PATHS } from "@/router/auth";
import { useAuthStore } from "@/stores/auth";

/**
 * 重置口令 —— Figma「02 — Authentication / Password reset」（5:2584）。
 *
 * 三段状态：填写中 / 提交成功 / 票据过期。
 * 口令要求清单随输入实时勾选（Figma 三条：长度 12、大小写 + 数字、特殊字符）。
 */
const { t } = useI18n();
const route = useRoute();
const auth = useAuthStore();

/** 演示账号（无会话上下文时的回显账号） */
const DEMO_ACCOUNT = "alex.morgan@acme.dev";

/** 新口令 */
const password = ref("");
/** 确认口令 */
const confirmPassword = ref("");
/** 提交中 */
const submitting = ref(false);
/** 字段级错误 */
const passwordError = ref<string | undefined>(undefined);
const confirmError = ref<string | undefined>(undefined);
/** 提交成功 */
const done = ref(false);
/** 票据过期 */
const expired = ref(route.query.token === "expired");

const passwordField = ref<HTMLElement | null>(null);
const confirmField = ref<HTMLElement | null>(null);

/** 账号回显：优先取会话账号 */
const accountLabel = computed(() => auth.account ?? DEMO_ACCOUNT);

/** 口令要求（实时） */
const requirements = computed<AuthRequirement[]>(() => [
  {
    id: "length",
    label: t("auth.reset.requirementLength"),
    met: password.value.length >= 12
  },
  {
    id: "mix",
    label: t("auth.reset.requirementMix"),
    met:
      /[a-z]/.test(password.value) &&
      /[A-Z]/.test(password.value) &&
      /\d/.test(password.value)
  },
  {
    id: "symbol",
    label: t("auth.reset.requirementSymbol"),
    met: /[^A-Za-z0-9]/.test(password.value)
  }
]);

/** 全部要求已满足 */
const requirementsMet = computed(() =>
  requirements.value.every(item => item.met)
);

/** 两个口令一致（仅在都非空时判断） */
const passwordsMatch = computed(
  () => password.value.length > 0 && password.value === confirmPassword.value
);

function validate() {
  passwordError.value = undefined;
  confirmError.value = undefined;

  if (!password.value) {
    passwordError.value = t("auth.reset.validation.passwordRequired");
  }
  if (!confirmPassword.value) {
    confirmError.value = t("auth.reset.validation.mismatch");
  } else if (password.value !== confirmPassword.value) {
    confirmError.value = t("auth.reset.validation.mismatch");
  }

  return !passwordError.value && !confirmError.value;
}

async function onSubmit() {
  expired.value = false;
  if (!validate()) {
    if (passwordError.value) {
      passwordField.value?.querySelector("input")?.focus();
    } else if (confirmError.value) {
      confirmField.value?.querySelector("input")?.focus();
    }
    return;
  }

  submitting.value = true;
  try {
    const token =
      typeof route.query.token === "string" ? route.query.token : undefined;
    await auth.resetPassword(password.value, token);
    done.value = true;
  } catch {
    expired.value = true;
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <AuthLayout>
    <AuthPanel>
      <AuthHeading
        :icon="LockKeyhole"
        :title="t('auth.reset.title')"
        :subtitle="t('auth.reset.subtitle')"
      />

      <div class="reset__form">
        <Alert
          v-if="expired"
          variant="danger"
          :title="t('auth.reset.expiredTitle')"
        >
          {{ t("auth.reset.expiredDetail") }}
        </Alert>

        <Alert
          v-if="done"
          variant="success"
          :title="t('auth.reset.successTitle')"
        >
          {{ t("auth.reset.successDetail") }}
        </Alert>

        <template v-if="!done">
          <p class="reset__account">
            {{ t("auth.reset.account", { account: accountLabel }) }}
          </p>

          <div ref="passwordField">
            <FormField
              :label="t('auth.reset.newPasswordLabel')"
              :error="passwordError"
              html-for="auth-reset-password"
            >
              <PasswordInput
                id="auth-reset-password"
                v-model="password"
                :invalid="Boolean(passwordError)"
                autocomplete="new-password"
                @update:model-value="passwordError = undefined"
              />
            </FormField>
          </div>

          <AuthRequirementList :items="requirements" />

          <div ref="confirmField">
            <FormField
              :label="t('auth.reset.confirmLabel')"
              :error="confirmError"
              html-for="auth-reset-confirm"
            >
              <PasswordInput
                id="auth-reset-confirm"
                v-model="confirmPassword"
                :invalid="Boolean(confirmError)"
                autocomplete="new-password"
                @update:model-value="confirmError = undefined"
              />
            </FormField>
          </div>

          <p v-if="passwordsMatch" class="reset__match">
            {{ t("auth.reset.matchHint") }}
          </p>

          <Button
            type="submit"
            :loading="submitting"
            :disabled="!requirementsMet"
            @click="onSubmit"
          >
            {{
              submitting ? t("auth.reset.submitting") : t("auth.reset.submit")
            }}
          </Button>

          <p class="reset__hint">{{ t("auth.reset.hint") }}</p>
        </template>
      </div>

      <AuthTextAction
        :icon="ArrowLeft"
        :label="t('auth.reset.backToSignIn')"
        :to="AUTH_PATHS.login"
      />
    </AuthPanel>
  </AuthLayout>
</template>

<style scoped>
/* 重置流程组 —— Figma：间距 16 */
.reset__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

/* 账号回显 —— 12px 次文字 */
.reset__account {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-secondary);
}

/* 「口令一致」提示 —— 11px 次文字 */
.reset__match {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}

.reset__hint {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}
</style>
