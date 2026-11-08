<script setup lang="ts">
import { onMounted, ref } from "vue";
import { ShieldCheck } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import FormField from "@/components/ui/form-field/FormField.vue";
import Input from "@/components/ui/input/Input.vue";
import PasswordInput from "@/components/ui/input/PasswordInput.vue";
import {
  AuthHeading,
  AuthLayout,
  AuthPanel,
  AuthTextAction
} from "@/components/auth";
import { AUTH_PATHS } from "@/router/auth";
import { SetupError, fetchSetupStatus, registerFirstAdmin } from "@/api/setup";
import { useAuthStore } from "@/stores/auth";

/**
 * 初始化注册页 —— 空库强制注册首个账号（该账号成为全系统最大管理员）。
 *
 * 进入条件：登录页探测到 setup_required=true 时强制重定向到本页；系统已
 * 初始化后直达本页会被送回登录页（注册通道在服务端已关闭，上游 403）。
 *
 * 成功链路：注册（上游 iam 空表门内建首个用户）→ 自动登录 → 认证进度页 →
 * 团队 onboarding 建团 → 建团者自动升 owner + 激活（唯一租户全权管理员）。
 * 错误分两层：字段级校验（行内）与网关级失败（面板顶部反馈框），
 * 与登录页同约定。凭据组用原生 <form>，Enter 隐式提交与点击等价。
 */
const { t } = useI18n();
const router = useRouter();
const auth = useAuthStore();

const username = ref("");
const nickname = ref("");
const password = ref("");
const confirm = ref("");
const email = ref("");
const submitting = ref(false);
const usernameError = ref<string | undefined>(undefined);
const nicknameError = ref<string | undefined>(undefined);
const passwordError = ref<string | undefined>(undefined);
const confirmError = ref<string | undefined>(undefined);
const emailError = ref<string | undefined>(undefined);
/** 网关级失败（网络 / 服务不可用） */
const registerFailed = ref(false);
/** 系统已初始化（服务端 403：注册通道已关闭） */
const alreadyInitialized = ref(false);
/** 注册成功但自动登录失败 */
const autoSignInFailed = ref(false);

const usernameField = ref<HTMLElement | null>(null);
const nicknameField = ref<HTMLElement | null>(null);
const passwordField = ref<HTMLElement | null>(null);
const confirmField = ref<HTMLElement | null>(null);
const emailField = ref<HTMLElement | null>(null);

/** 邮箱形态校验：与登录页同规则（可选字段，填了才判） */
const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
/** 服务端 CreateUserRequest 契约：口令最少 8 位 */
const PASSWORD_MIN_LENGTH = 8;

onMounted(async () => {
  // 系统已初始化（注册通道关闭）时把误入者送回登录页；探测失败不引导。
  const status = await fetchSetupStatus();
  if (status && !status.setup_required) {
    await router.replace(AUTH_PATHS.login);
  }
});

function validate() {
  usernameError.value = undefined;
  nicknameError.value = undefined;
  passwordError.value = undefined;
  confirmError.value = undefined;
  emailError.value = undefined;

  if (!username.value.trim()) {
    usernameError.value = t("auth.setup.validation.usernameRequired");
  }
  if (!nickname.value.trim()) {
    nicknameError.value = t("auth.setup.validation.nicknameRequired");
  }
  if (!password.value) {
    passwordError.value = t("auth.setup.validation.passwordRequired");
  } else if (password.value.length < PASSWORD_MIN_LENGTH) {
    passwordError.value = t("auth.setup.validation.passwordTooShort");
  }
  if (confirm.value !== password.value) {
    confirmError.value = t("auth.setup.validation.mismatch");
  }
  const trimmedEmail = email.value.trim();
  if (trimmedEmail && !EMAIL_PATTERN.test(trimmedEmail)) {
    emailError.value = t("auth.setup.validation.emailInvalid");
  }

  return (
    !usernameError.value &&
    !nicknameError.value &&
    !passwordError.value &&
    !confirmError.value &&
    !emailError.value
  );
}

/** 聚焦首个无效字段（与登录页同约定） */
function focusField(field: HTMLElement | null) {
  field?.querySelector("input")?.focus();
}

async function onSubmit() {
  registerFailed.value = false;
  alreadyInitialized.value = false;
  autoSignInFailed.value = false;

  if (!validate()) {
    const firstInvalid =
      usernameField.value && usernameError.value
        ? usernameField.value
        : nicknameField.value && nicknameError.value
          ? nicknameField.value
          : passwordField.value && passwordError.value
            ? passwordField.value
            : confirmField.value && confirmError.value
              ? confirmField.value
              : emailField.value;
    focusField(firstInvalid);
    return;
  }

  submitting.value = true;
  try {
    await registerFirstAdmin({
      username: username.value.trim(),
      nickname: nickname.value.trim(),
      password: password.value,
      ...(email.value.trim() ? { email: email.value.trim() } : {})
    });
  } catch (error) {
    if (error instanceof SetupError && error.code === "closed") {
      alreadyInitialized.value = true;
    } else if (error instanceof SetupError && error.code === "duplicate") {
      usernameError.value = t("auth.setup.validation.duplicate");
      focusField(usernameField.value);
    } else {
      registerFailed.value = true;
    }
    return;
  } finally {
    submitting.value = false;
  }

  // 注册即最大管理员：直接登录进入 onboarding（建团 → owner + 激活）。
  try {
    await auth.signIn({
      account: username.value.trim(),
      password: password.value,
      remember: true
    });
    await router.push(AUTH_PATHS.signingIn);
  } catch {
    autoSignInFailed.value = true;
  }
}
</script>

<template>
  <AuthLayout>
    <AuthPanel>
      <AuthHeading
        :title="t('auth.setup.title')"
        :subtitle="t('auth.setup.subtitle')"
      />

      <form class="setup__form" novalidate @submit.prevent="onSubmit">
        <Alert
          v-if="alreadyInitialized"
          variant="warning"
          :title="t('auth.setup.closedTitle')"
        >
          {{ t("auth.setup.closedDetail") }}
          <AuthTextAction
            :label="t('auth.setup.backToSignIn')"
            :to="AUTH_PATHS.login"
          />
        </Alert>

        <Alert
          v-if="autoSignInFailed"
          variant="info"
          :title="t('auth.setup.autoSignInFailedTitle')"
        >
          {{ t("auth.setup.autoSignInFailedDetail") }}
          <AuthTextAction
            :label="t('auth.setup.backToSignIn')"
            :to="AUTH_PATHS.login"
          />
        </Alert>

        <Alert
          v-if="registerFailed"
          variant="danger"
          :title="t('auth.setup.errorTitle')"
        >
          {{ t("auth.setup.errorDetail") }}
        </Alert>

        <div ref="usernameField">
          <FormField
            :label="t('auth.setup.usernameLabel')"
            :error="usernameError"
            html-for="auth-setup-username"
          >
            <Input
              id="auth-setup-username"
              v-model="username"
              :placeholder="t('auth.setup.usernamePlaceholder')"
              :invalid="Boolean(usernameError)"
              autocomplete="username"
              @update:model-value="usernameError = undefined"
            />
          </FormField>
        </div>

        <div ref="nicknameField">
          <FormField
            :label="t('auth.setup.nicknameLabel')"
            :error="nicknameError"
            html-for="auth-setup-nickname"
          >
            <Input
              id="auth-setup-nickname"
              v-model="nickname"
              :placeholder="t('auth.setup.nicknamePlaceholder')"
              :invalid="Boolean(nicknameError)"
              autocomplete="name"
              @update:model-value="nicknameError = undefined"
            />
          </FormField>
        </div>

        <div ref="passwordField">
          <FormField
            :label="t('auth.setup.passwordLabel')"
            :error="passwordError"
            html-for="auth-setup-password"
          >
            <PasswordInput
              id="auth-setup-password"
              v-model="password"
              :placeholder="t('auth.setup.passwordPlaceholder')"
              :invalid="Boolean(passwordError)"
              autocomplete="new-password"
              @update:model-value="passwordError = undefined"
            />
          </FormField>
        </div>

        <div ref="confirmField">
          <FormField
            :label="t('auth.setup.confirmLabel')"
            :error="confirmError"
            html-for="auth-setup-confirm"
          >
            <PasswordInput
              id="auth-setup-confirm"
              v-model="confirm"
              :placeholder="t('auth.setup.passwordPlaceholder')"
              :invalid="Boolean(confirmError)"
              autocomplete="new-password"
              @update:model-value="confirmError = undefined"
            />
          </FormField>
        </div>

        <div ref="emailField">
          <FormField
            :label="t('auth.setup.emailLabel')"
            :error="emailError"
            html-for="auth-setup-email"
          >
            <Input
              id="auth-setup-email"
              v-model="email"
              type="email"
              :placeholder="t('auth.setup.emailPlaceholder')"
              :invalid="Boolean(emailError)"
              autocomplete="email"
              @update:model-value="emailError = undefined"
            />
          </FormField>
        </div>

        <Button type="submit" :loading="submitting">
          {{ submitting ? t("auth.setup.submitting") : t("auth.setup.submit") }}
        </Button>
      </form>

      <p class="setup__guidance">
        <ShieldCheck class="setup__guidance-icon" aria-hidden="true" />
        {{ t("auth.setup.guidance") }}
      </p>
    </AuthPanel>
  </AuthLayout>
</template>

<style scoped>
/* 表单组 —— 与登录页凭据组同纵向节奏（16） */
.setup__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

/* 面板外安全指引 */
.setup__guidance {
  margin: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-secondary);
}

.setup__guidance-icon {
  flex-shrink: 0;
  width: var(--space-3);
  height: var(--space-3);
  color: var(--ws-accent);
}
</style>
