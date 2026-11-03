<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { AlertCircle, Eye, EyeOff } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { FormField } from "@/components/ui/form-field";
import { useAuthStore } from "../../store/auth";
import { onUnauthorized } from "../../api/client";
import { i18n, setLocale, type SupportedLocale } from "../../i18n";

const { t } = useI18n();

const route = useRoute();
const router = useRouter();
const auth = useAuthStore();

const username = ref("");
const password = ref("");
const revealPassword = ref(false);
const submitting = ref(false);
/** 提交级错误（凭证被拒）：字段级问题不写这里，避免同一条文案出现两次 */
const errorMessage = ref("");
const usernameError = ref("");
const passwordError = ref("");

const usernameField = ref<InstanceType<typeof Input> | null>(null);
const passwordField = ref<InstanceType<typeof Input> | null>(null);

const points = computed(() => [
  t("shell.login.pointProjects"),
  t("shell.login.pointApprovals"),
  t("shell.login.pointAi")
]);

/** 语言切换：当前语言 → 另一种语言 */
const currentLocale = computed(
  () => i18n.global.locale.value as SupportedLocale
);
const targetLocale = computed<SupportedLocale>(() =>
  currentLocale.value === "zh-CN" ? "en" : "zh-CN"
);

function toggleLocale() {
  setLocale(targetLocale.value);
}

function focusField(field: typeof usernameField) {
  const element = field.value?.$el as HTMLInputElement | undefined;
  element?.focus();
}

onMounted(() => focusField(usernameField));

// 用户开始修正时立即撤掉旧提示：错误不能粘在已改过的字段上
watch(username, () => {
  usernameError.value = "";
  errorMessage.value = "";
});

watch(password, () => {
  passwordError.value = "";
  errorMessage.value = "";
});

onUnauthorized(() => {
  auth.reset();
});

async function onSubmit() {
  if (submitting.value) return;
  errorMessage.value = "";

  usernameError.value = username.value.trim()
    ? ""
    : t("shell.login.usernameRequired");
  passwordError.value = password.value ? "" : t("shell.login.passwordRequired");

  if (usernameError.value) {
    focusField(usernameField);
    return;
  }

  if (passwordError.value) {
    focusField(passwordField);
    return;
  }

  submitting.value = true;
  try {
    await auth.signIn(username.value.trim(), password.value);
    const redirect = (route.query.redirect as string | undefined) ?? "/";
    await router.replace(redirect);
  } catch (error) {
    // 服务端错误码只留给控制台：它对使用者没有意义，还会泄露内部约定
    console.error("[login] signIn failed:", error);
    errorMessage.value = t("shell.login.failed");
    // 只落焦点、不整段选中：选中态在深色下是一块突兀的高亮，且会盖住密码点
    focusField(passwordField);
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <main class="ui-login">
    <section class="ui-login__brand">
      <div class="ui-login__brand-top">
        <span class="ui-login__mark" aria-hidden="true" />
        <span class="ui-login__wordmark">{{ t("shell.login.title") }}</span>
        <button
          type="button"
          class="ui-login__locale"
          :aria-label="
            t('shell.locale.switchTo', {
              language: t(`shell.locale.${targetLocale}`)
            })
          "
          @click="toggleLocale"
        >
          {{ t(`shell.locale.${targetLocale}`) }}
        </button>
      </div>

      <div class="ui-login__brand-body">
        <p class="ui-login__tagline">{{ t("shell.login.tagline") }}</p>
        <ul class="ui-login__points">
          <li v-for="point in points" :key="point" class="ui-login__point">
            {{ point }}
          </li>
        </ul>
      </div>

      <p class="ui-login__console">{{ t("shell.login.consoleLabel") }}</p>
    </section>

    <section class="ui-login__panel" aria-labelledby="login-heading">
      <div class="ui-login__form-wrap">
        <header class="ui-login__head">
          <h1 id="login-heading" class="ui-login__title">
            {{ t("shell.login.heading") }}
          </h1>
          <p class="ui-login__subtitle">{{ t("shell.login.subtitle") }}</p>
        </header>

        <form class="ui-login__form" novalidate @submit.prevent="onSubmit">
          <p v-if="errorMessage" class="ui-login__error" role="alert">
            <AlertCircle class="ui-login__error-icon" aria-hidden="true" />
            <span>{{ errorMessage }}</span>
          </p>

          <FormField
            :label="t('shell.login.username')"
            html-for="login-username"
            :error="usernameError || undefined"
            required
          >
            <Input
              id="login-username"
              ref="usernameField"
              v-model="username"
              type="text"
              name="username"
              :placeholder="t('shell.login.usernamePlaceholder')"
              autocomplete="username"
              autocapitalize="none"
              spellcheck="false"
              :invalid="Boolean(usernameError)"
            />
          </FormField>

          <FormField
            :label="t('shell.login.password')"
            html-for="login-password"
            :error="passwordError || undefined"
            required
          >
            <div class="ui-login__password">
              <Input
                id="login-password"
                ref="passwordField"
                v-model="password"
                class="ui-login__password-input"
                :type="revealPassword ? 'text' : 'password'"
                name="password"
                :placeholder="t('shell.login.passwordPlaceholder')"
                autocomplete="current-password"
                :invalid="Boolean(passwordError)"
              />
              <button
                type="button"
                class="ui-login__reveal"
                :aria-label="
                  revealPassword
                    ? t('shell.login.passwordHide')
                    : t('shell.login.passwordShow')
                "
                :aria-pressed="revealPassword"
                @click="revealPassword = !revealPassword"
              >
                <EyeOff v-if="revealPassword" aria-hidden="true" />
                <Eye v-else aria-hidden="true" />
              </button>
            </div>
          </FormField>

          <Button
            type="submit"
            size="lg"
            class="ui-login__submit"
            :loading="submitting"
          >
            {{ t("shell.login.submit") }}
          </Button>
        </form>

        <p class="ui-login__note">{{ t("shell.login.securityNote") }}</p>
      </div>
    </section>
  </main>
</template>

<style scoped>
/* 登录页 — PRISM 品牌第一门面
 *
 * 主题：跟随 app 级换轨（styles/workspace/tokens.css 在 :root 覆盖同名 PRISM 变量，
 *       深色为默认）。因此这里只用会被换轨的语义 token，不写死明暗假设：
 *       --surface-sunken / --ws-sidebar / --text-* / --accent 在两种主题下各自成立。
 *
 * 左：品牌面板（比画布略抬一级，等同应用侧栏的层级）+ 一条主干上的三条锚点
 * 右：画布上的表单，直接落在页面上，不做「卡片套卡片」
 * 窄屏：品牌面板收成顶部条，只留标记与产品名
 */
.ui-login {
  display: grid;
  grid-template-columns: minmax(var(--size-login-panel), 42%) 1fr;
  min-height: 100vh;
  min-height: 100dvh;
  background: var(--surface-sunken);
}

/* ── 品牌面板 ───────────────────────────────────────────────────────── */

.ui-login__brand {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: var(--space-12);
  padding: var(--space-12);
  background: var(--ws-sidebar);
  border-right: var(--border-1) solid var(--border-default);
}

.ui-login__brand-top {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

/* 棱镜几何标记：唯一的实心强调色块（换轨后 accent-gradient 为 none，用实色） */
.ui-login__mark {
  flex: none;
  width: var(--size-login-mark);
  height: var(--size-login-mark);
  background: var(--accent);
  clip-path: polygon(50% 0%, 100% 100%, 0% 100%);
}

.ui-login__wordmark {
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-primary);
}

/* 语言切换：与 wordmark 同排的轻量按钮 */
.ui-login__locale {
  margin-left: auto;
  padding: var(--space-1) var(--space-3);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-full);
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-secondary);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-login__locale:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-login__locale:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

.ui-login__brand-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-8);
}

.ui-login__tagline {
  max-width: 12em;
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h2);
  line-height: var(--line-height-h2);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h2);
  color: var(--text-primary);
  text-wrap: balance;
}

/* 三条锚点挂在一条主干上 —— 正是「一条主干」的图形化，不是装饰线 */
.ui-login__points {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  margin: 0;
  padding: 0 0 0 var(--space-6);
  list-style: none;
}

.ui-login__points::before {
  content: "";
  position: absolute;
  top: var(--space-2);
  bottom: var(--space-2);
  left: 0;
  width: var(--hairline);
  background: var(--border-default);
}

.ui-login__point {
  position: relative;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-secondary);
}

.ui-login__point::before {
  content: "";
  position: absolute;
  top: calc((var(--line-height-small) - var(--space-1)) / 2);
  left: calc(var(--space-6) * -1 - var(--space-1) / 2 + var(--hairline) / 2);
  width: var(--space-1);
  height: var(--space-1);
  background: var(--accent);
  border-radius: var(--radius-full);
}

.ui-login__console {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  letter-spacing: var(--tracking-caption);
  color: var(--text-tertiary);
}

/* ── 表单面板 ───────────────────────────────────────────────────────── */

.ui-login__panel {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-12) var(--space-8);
}

.ui-login__form-wrap {
  display: flex;
  flex-direction: column;
  gap: var(--space-8);
  width: 100%;
  max-width: var(--size-login-form);
}

.ui-login__head {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.ui-login__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h2);
  line-height: var(--line-height-h2);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h2);
  color: var(--text-primary);
}

.ui-login__subtitle {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-tertiary);
}

.ui-login__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

/* 提交级错误：色块 + 图标，比一行红字更容易被看见 */
.ui-login__error {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  margin: 0;
  padding: var(--space-3);
  background: var(--danger-subtle);
  border-radius: var(--radius-sm);
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--danger-strong);
}

.ui-login__error-icon {
  flex: none;
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
  margin-top: calc((var(--line-height-small) - var(--size-icon-sm)) / 2);
}

.ui-login__password {
  position: relative;
}

.ui-login__password-input {
  padding-right: calc(var(--size-control-lg) + var(--space-2));
}

/* 密码可见性：命中区与输入框同高，图标不挤压文字 */
.ui-login__reveal {
  position: absolute;
  top: 0;
  right: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-lg);
  height: 100%;
  padding: 0;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--text-tertiary);
  cursor: pointer;
  transition: color var(--duration-fast) var(--ease-decelerate);
}

.ui-login__reveal:hover {
  color: var(--text-primary);
}

.ui-login__reveal:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: calc(var(--size-focus-ring) * -1);
}

.ui-login__reveal svg {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
}

.ui-login__submit {
  width: 100%;
  margin-top: var(--space-2);
}

.ui-login__note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  letter-spacing: var(--tracking-caption);
  color: var(--text-tertiary);
}

/* ── 窄屏：品牌面板收成顶部条，触摸目标抬到 44px ──────────────────── */

@media (max-width: 900px) {
  .ui-login {
    grid-template-columns: 1fr;
    grid-template-rows: auto 1fr;
  }

  .ui-login__brand {
    flex-direction: row;
    align-items: center;
    justify-content: flex-start;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-6);
  }

  .ui-login__brand-body,
  .ui-login__console {
    display: none;
  }

  .ui-login__panel {
    padding: var(--space-10) var(--space-6) var(--space-16);
  }

  .ui-login__panel :deep(.ui-input) {
    height: var(--size-row);
  }

  .ui-login__password-input {
    padding-right: calc(var(--size-row) + var(--space-2));
  }

  .ui-login__reveal {
    width: var(--size-row);
  }
}
</style>
