<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2488 set="02 — Authentication / Login" date=2026-11-04 -->
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Building2 } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import Checkbox from "@/components/ui/checkbox/Checkbox.vue";
import FormField from "@/components/ui/form-field/FormField.vue";
import Input from "@/components/ui/input/Input.vue";
import PasswordInput from "@/components/ui/input/PasswordInput.vue";
import {
  AuthHeading,
  AuthLayout,
  AuthPanel,
  AuthProviderDivider,
  AuthTextAction
} from "@/components/auth";
import { AUTH_PATHS } from "@/router/auth";
import { fetchSetupStatus } from "@/api/setup";
import { useAuthStore } from "@/stores/auth";

/**
 * 登录页 —— Figma「02 — Authentication / Login」（5:2488）。
 *
 * 无导航外壳、无营销横幅：面板 400px 承载「凭据 + 身份提供方」两组，
 * 面板外一行账号指引。错误分两层：
 *   - 字段级校验（行内，紧贴字段）；
 *   - 网关级失败（面板顶部反馈框，Figma 的「Unable to sign in」）。
 *
 * 凭据组用原生 <form>：输入框内按 Enter 隐式提交与按钮点击同走
 * submit 通道（@submit.prevent 拦整页刷新），键盘提交与点击提交等价。
 */
const { t } = useI18n();
const router = useRouter();
const auth = useAuthStore();

/** 邮箱或用户名 */
const account = ref(auth.rememberedAccount ?? "");
/** 口令 */
const password = ref("");
/** 记住我 */
const remember = ref(Boolean(auth.rememberedAccount));
/** 提交中 */
const submitting = ref(false);
/** 字段级错误 */
const accountError = ref<string | undefined>(undefined);
const passwordError = ref<string | undefined>(undefined);
/** 网关级失败（凭据错误） */
const signInFailed = ref(false);
/** SSO 未开通提示 */
const ssoNotice = ref(false);

const accountField = ref<HTMLElement | null>(null);
const passwordField = ref<HTMLElement | null>(null);

/** 邮箱形态校验：含 @ 时才按邮箱规则判定（也允许纯用户名登录） */
const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

onMounted(async () => {
  // 空库强制注册：系统尚未初始化（无任何用户）时登录无从谈起，
  // 直接引导到首个管理员注册页；探测失败（网关不可达）不引导。
  const status = await fetchSetupStatus();
  if (status?.setup_required) {
    await router.replace(AUTH_PATHS.setup);
  }
});

function validate() {
  accountError.value = undefined;
  passwordError.value = undefined;

  const trimmed = account.value.trim();
  if (!trimmed) {
    accountError.value = t("auth.login.validation.accountRequired");
  } else if (trimmed.includes("@") && !EMAIL_PATTERN.test(trimmed)) {
    accountError.value = t("auth.login.validation.accountInvalid");
  }

  if (!password.value) {
    passwordError.value = t("auth.login.validation.passwordRequired");
  }

  return !accountError.value && !passwordError.value;
}

/** 聚焦首个无效字段（Figma「Focus the first invalid field after submit」） */
function focusField(field: HTMLElement | null) {
  field?.querySelector("input")?.focus();
}

async function onSubmit() {
  signInFailed.value = false;
  ssoNotice.value = false;

  if (!validate()) {
    if (accountError.value) {
      focusField(accountField.value);
    } else if (passwordError.value) {
      focusField(passwordField.value);
    }
    return;
  }

  submitting.value = true;
  try {
    await auth.signIn({
      account: account.value.trim(),
      password: password.value,
      remember: remember.value
    });
    await router.push(AUTH_PATHS.signingIn);
  } catch {
    signInFailed.value = true;
  } finally {
    submitting.value = false;
  }
}

function onSso() {
  ssoNotice.value = true;
}
</script>

<template>
  <AuthLayout>
    <AuthPanel>
      <AuthHeading
        :title="t('auth.login.title')"
        :subtitle="t('auth.login.subtitle')"
      />

      <form class="login__credentials" novalidate @submit.prevent="onSubmit">
        <Alert
          v-if="signInFailed"
          variant="danger"
          :title="t('auth.login.errorTitle')"
        >
          {{ t("auth.login.errorDetail") }}
        </Alert>

        <div ref="accountField">
          <FormField
            :label="t('auth.login.accountLabel')"
            :error="accountError"
            html-for="auth-login-account"
          >
            <Input
              id="auth-login-account"
              v-model="account"
              :placeholder="t('auth.login.accountPlaceholder')"
              :invalid="Boolean(accountError)"
              autocomplete="username"
              @update:model-value="accountError = undefined"
            />
          </FormField>
        </div>

        <div ref="passwordField">
          <FormField
            :label="t('auth.login.passwordLabel')"
            :error="passwordError"
            html-for="auth-login-password"
          >
            <PasswordInput
              id="auth-login-password"
              v-model="password"
              :placeholder="t('auth.login.passwordPlaceholder')"
              :invalid="Boolean(passwordError)"
              autocomplete="current-password"
              @update:model-value="passwordError = undefined"
            />
          </FormField>
        </div>

        <div class="login__options">
          <Checkbox
            v-model="remember"
            :label="t('auth.login.rememberMe')"
            name="remember"
          />
          <AuthTextAction
            :label="t('auth.login.forgotPassword')"
            :to="AUTH_PATHS.forgotPassword"
          />
        </div>

        <Button type="submit" :loading="submitting">
          {{ submitting ? t("auth.login.submitting") : t("auth.login.submit") }}
        </Button>
      </form>

      <div class="login__providers">
        <AuthProviderDivider :label="t('auth.login.divider')" />
        <Button variant="secondary" @click="onSso">
          <Building2 class="login__provider-icon" aria-hidden="true" />
          {{ t("auth.login.sso") }}
        </Button>
        <p class="login__provider-hint">{{ t("auth.login.ssoHint") }}</p>
        <Alert v-if="ssoNotice" variant="info">
          {{ t("auth.login.ssoUnavailable") }}
        </Alert>
      </div>
    </AuthPanel>

    <p class="login__guidance">{{ t("auth.login.guidance") }}</p>
  </AuthLayout>
</template>

<style scoped>
/* 凭据组 —— Figma：纵向间距 16 */
.login__credentials {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

/* 记住我 / 忘记口令 —— 左右分布 */
.login__options {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

/* 身份提供方组 —— Figma：间距 12 */
.login__providers {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.login__provider-icon {
  width: var(--space-3);
  height: var(--space-3);
}

.login__provider-hint {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  text-align: center;
  color: var(--ws-text-secondary);
}

/* 面板外账号指引 —— 12px 次文字居中 */
.login__guidance {
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
