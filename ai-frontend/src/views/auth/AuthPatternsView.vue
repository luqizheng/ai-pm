<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2810 set="02 — Authentication / Patterns and states" date=2026-11-04 -->
<script setup lang="ts">
import { CircleCheck } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import FormField from "@/components/ui/form-field/FormField.vue";
import Input from "@/components/ui/input/Input.vue";
import PasswordInput from "@/components/ui/input/PasswordInput.vue";

/**
 * 认证状态参照板 —— Figma「02 — Authentication / Patterns and states」（5:2810）。
 *
 * 把九帧里散落的**状态**集中成一页可对照的规范：
 *   1. 字段与校验 —— 默认 / 聚焦 / 错误 / 口令可见 / 口令隐藏 / 禁用；
 *   2. 动作与交互态 —— 默认 / 悬浮 / 键盘聚焦 / 按压 / 禁用 / 载入中；
 *   3. 反馈与键盘可达 —— 错误 / 信息 / 过期 / 成功反馈 + 键盘约定。
 *
 * 静态参照页（不做交互原型），故用强制类把 hover / focus / active 态固定下来。
 */
const { t } = useI18n();
</script>

<template>
  <div class="patterns">
    <header class="patterns__heading">
      <div class="patterns__identity">
        <p class="patterns__eyebrow">{{ t("auth.patterns.eyebrow") }}</p>
        <h1 class="patterns__title">{{ t("auth.patterns.title") }}</h1>
        <p class="patterns__description">
          {{ t("auth.patterns.description") }}
        </p>
      </div>
      <p class="patterns__version">{{ t("auth.patterns.version") }}</p>
    </header>

    <div class="patterns__groups">
      <!-- 1. 字段与校验 -->
      <section class="patterns__group">
        <div class="patterns__group-heading">
          <h2 class="patterns__group-title">
            {{ t("auth.patterns.fields.title") }}
          </h2>
          <p class="patterns__group-description">
            {{ t("auth.patterns.fields.description") }}
          </p>
        </div>

        <FormField :label="t('auth.patterns.fields.defaultLabel')">
          <Input :placeholder="t('auth.patterns.fields.placeholder')" />
        </FormField>

        <div class="patterns__field">
          <FormField :label="t('auth.patterns.fields.focusLabel')">
            <div class="patterns__input--focus">
              <Input :model-value="t('auth.patterns.fields.placeholder')" />
            </div>
          </FormField>
          <p class="patterns__hint">
            {{ t("auth.patterns.fields.focusHint") }}
          </p>
        </div>

        <FormField
          :label="t('auth.patterns.fields.invalidLabel')"
          :error="t('auth.patterns.fields.invalidHint')"
        >
          <Input model-value="alex@" invalid />
        </FormField>

        <div class="patterns__field">
          <FormField :label="t('auth.patterns.fields.visibleLabel')">
            <PasswordInput
              :model-value="t('auth.patterns.fields.visibleValue')"
              default-visible
            />
          </FormField>
          <p class="patterns__hint">
            {{ t("auth.patterns.fields.visibleHint") }}
          </p>
        </div>

        <FormField :label="t('auth.patterns.fields.hiddenLabel')">
          <PasswordInput
            :model-value="t('auth.patterns.fields.visibleValue')"
          />
        </FormField>

        <FormField :label="t('auth.patterns.fields.disabledLabel')">
          <Input model-value="alex.morgan@acme.dev" disabled />
        </FormField>
      </section>

      <!-- 2. 动作与交互态 -->
      <section class="patterns__group">
        <div class="patterns__group-heading">
          <h2 class="patterns__group-title">
            {{ t("auth.patterns.actions.title") }}
          </h2>
          <p class="patterns__group-description">
            {{ t("auth.patterns.actions.description") }}
          </p>
        </div>

        <div class="patterns__state">
          <p class="patterns__state-label">
            {{ t("auth.patterns.actions.default") }}
          </p>
          <Button>{{ t("auth.patterns.actions.label") }}</Button>
        </div>

        <div class="patterns__state">
          <p class="patterns__state-label">
            {{ t("auth.patterns.actions.hover") }}
          </p>
          <div class="patterns__button--hover">
            <Button>{{ t("auth.patterns.actions.label") }}</Button>
          </div>
        </div>

        <div class="patterns__state">
          <p class="patterns__state-label">
            {{ t("auth.patterns.actions.focus") }}
          </p>
          <div class="patterns__button--focus">
            <Button>{{ t("auth.patterns.actions.label") }}</Button>
          </div>
        </div>

        <div class="patterns__state">
          <p class="patterns__state-label">
            {{ t("auth.patterns.actions.active") }}
          </p>
          <div class="patterns__button--active">
            <Button>{{ t("auth.patterns.actions.label") }}</Button>
          </div>
        </div>

        <div class="patterns__state">
          <p class="patterns__state-label">
            {{ t("auth.patterns.actions.disabled") }}
          </p>
          <Button disabled>{{ t("auth.patterns.actions.label") }}</Button>
        </div>

        <div class="patterns__state">
          <p class="patterns__state-label">
            {{ t("auth.patterns.actions.loading") }}
          </p>
          <Button loading>
            {{ t("auth.patterns.actions.loadingLabel") }}
          </Button>
        </div>

        <p class="patterns__hint">{{ t("auth.patterns.actions.note") }}</p>
      </section>

      <!-- 3. 反馈与键盘可达 -->
      <section class="patterns__group">
        <div class="patterns__group-heading">
          <h2 class="patterns__group-title">
            {{ t("auth.patterns.feedback.title") }}
          </h2>
          <p class="patterns__group-description">
            {{ t("auth.patterns.feedback.description") }}
          </p>
        </div>

        <Alert variant="danger" :title="t('auth.patterns.feedback.errorTitle')">
          {{ t("auth.patterns.feedback.errorDetail") }}
        </Alert>

        <Alert variant="info" :title="t('auth.patterns.feedback.infoTitle')">
          {{ t("auth.patterns.feedback.infoDetail") }}
        </Alert>

        <Alert
          variant="danger"
          :title="t('auth.patterns.feedback.expiredTitle')"
        >
          {{ t("auth.patterns.feedback.expiredDetail") }}
        </Alert>

        <p class="patterns__success">
          <CircleCheck class="patterns__success-icon" aria-hidden="true" />
          <span>{{ t("auth.patterns.feedback.success") }}</span>
        </p>

        <hr class="patterns__divider" />

        <div class="patterns__keyboard">
          <h3 class="patterns__keyboard-title">
            {{ t("auth.patterns.feedback.keyboardTitle") }}
          </h3>
          <div class="patterns__keyboard-row">
            <span class="patterns__key">
              {{ t("auth.patterns.feedback.keyTab") }}
            </span>
            <span class="patterns__key-desc">
              {{ t("auth.patterns.feedback.keyTabDesc") }}
            </span>
          </div>
          <div class="patterns__keyboard-row">
            <span class="patterns__key">
              {{ t("auth.patterns.feedback.keyEnter") }}
            </span>
            <span class="patterns__key-desc">
              {{ t("auth.patterns.feedback.keyEnterDesc") }}
            </span>
          </div>
          <div class="patterns__keyboard-row">
            <span class="patterns__key">
              {{ t("auth.patterns.feedback.keySpace") }}
            </span>
            <span class="patterns__key-desc">
              {{ t("auth.patterns.feedback.keySpaceDesc") }}
            </span>
          </div>
          <p class="patterns__hint">{{ t("auth.patterns.feedback.note") }}</p>
        </div>
      </section>
    </div>

    <footer class="patterns__summary">
      <span>{{ t("auth.patterns.summary.metrics") }}</span>
      <span>{{ t("auth.patterns.summary.accent") }}</span>
    </footer>
  </div>
</template>

<style scoped>
/* 参照板 —— Figma：画布底 + 32px 内边距 + 24px 间距 */
.patterns {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  min-height: 100dvh;
  padding: var(--space-8);
  background: var(--ws-canvas);
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-body);
}

.patterns__heading {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: var(--space-6);
}

.patterns__identity {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  max-width: 800px;
}

.patterns__eyebrow {
  margin: 0;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-accent);
}

.patterns__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h1);
}

.patterns__description {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--ws-text-secondary);
}

.patterns__version {
  margin: 0;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

/* 三组状态卡 */
.patterns__groups {
  display: flex;
  align-items: stretch;
  gap: var(--space-6);
}

.patterns__group {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  flex: 1;
  padding: var(--space-5);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.patterns__group-heading {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.patterns__group-title {
  margin: 0;
  font-size: var(--font-size-h4);
  line-height: var(--line-height-h4);
  font-weight: var(--weight-semibold);
}

.patterns__group-description {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-secondary);
}

/* 字段条目（含提示行） */
.patterns__field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.patterns__hint {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}

/* 强制态：聚焦（2px accent 描边） */
.patterns__input--focus :deep(.ui-input) {
  border-color: var(--ws-accent);
  box-shadow: inset 0 0 0 var(--border-1) var(--ws-accent);
}

/* 动作态条目 */
.patterns__state {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.patterns__state-label {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}

/* 强制态：悬浮 / 按压 / 键盘聚焦 */
.patterns__button--hover :deep(.ui-button) {
  background-color: var(--ws-accent-hover);
}

.patterns__button--active :deep(.ui-button) {
  background-color: var(--ws-accent-pressed);
}

.patterns__button--focus :deep(.ui-button) {
  outline: var(--size-focus-ring) solid var(--ws-text-primary);
  outline-offset: 0;
}

/* 成功反馈 —— 图标 + 文字一行 */
.patterns__success {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-primary);
}

.patterns__success-icon {
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--ws-color-success);
}

.patterns__divider {
  width: 100%;
  height: var(--border-1);
  margin: 0;
  border: 0;
  background: var(--ws-border);
}

/* 键盘约定 */
.patterns__keyboard {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.patterns__keyboard-title {
  margin: 0;
  font-size: var(--font-size-caption);
  font-weight: var(--weight-semibold);
}

.patterns__keyboard-row {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.patterns__key {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-primary);
}

.patterns__key-desc {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--ws-text-secondary);
}

/* 底注 */
.patterns__summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}
</style>
