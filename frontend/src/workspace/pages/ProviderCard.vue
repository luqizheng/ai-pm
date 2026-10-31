<script setup lang="ts">
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  invalidTimeout,
  type ProviderAction,
  type ProviderForm,
  type ProviderItem
} from "./models-types";

const props = defineProps<{
  item: ProviderItem;
  saving: boolean;
  testing: boolean;
}>();

const emit = defineEmits<{
  save: [action: ProviderAction];
  test: [provider: ProviderItem["data"]["provider"]];
  "clear-key": [action: ProviderAction];
}>();

const { t } = useI18n();

/** 卡片内表单是本地副本：prop 只读，保存时把整份表单回传给页面。 */
const form = reactive<ProviderForm>({ ...props.item.form });

watch(
  () => props.item,
  item => Object.assign(form, item.form)
);

function action(): ProviderAction {
  return {
    provider: props.item.data.provider,
    label: props.item.label,
    form: { ...form }
  };
}

function testMessage(): string {
  const result = props.item.testResult;
  if (!result) return "";
  if (result.ok) {
    return t("workspace.pages.models.testOk", { ms: result.latency_ms });
  }
  return `${t("workspace.pages.models.testFailed")}: ${result.message}`;
}
</script>

<template>
  <section
    class="ws-card ws-model-card"
    data-test="provider-card"
    :data-provider="item.data.provider"
  >
    <header class="ws-model-head">
      <div class="ws-model-title">
        <span class="ws-model-name">{{ item.label }}</span>
        <span class="ws-model-slug">{{ item.data.provider }}</span>
      </div>
      <Badge :variant="item.data.enabled ? 'success' : 'neutral'">
        {{
          item.data.enabled
            ? t("workspace.pages.models.enabled")
            : t("workspace.pages.models.disabled")
        }}
      </Badge>
    </header>

    <FormField :label="t('workspace.pages.models.fields.baseUrl')">
      <Input v-model="form.base_url" placeholder="https://" />
    </FormField>

    <FormField :label="t('workspace.pages.models.fields.model')">
      <Input v-model="form.model" />
    </FormField>

    <FormField
      :label="t('workspace.pages.models.fields.apiKey')"
      :description="
        item.data.has_api_key
          ? t('workspace.pages.models.keyConfigured', {
              hint: item.data.api_key_hint ?? ''
            })
          : t('workspace.pages.models.keyMissing')
      "
    >
      <Input
        v-model="form.api_key"
        type="password"
        :placeholder="t('workspace.pages.models.keyPlaceholder')"
      />
    </FormField>

    <div class="ws-model-row">
      <div class="ws-model-field ws-model-field--narrow">
        <FormField :label="t('workspace.pages.models.fields.timeout')">
          <Input
            v-model="form.timeout"
            type="number"
            :invalid="invalidTimeout(form)"
          />
        </FormField>
      </div>
      <div class="ws-model-toggle">
        <Switch
          v-model="form.enabled"
          hide-label
          :aria-label="t('workspace.pages.models.enableLabel')"
          :state-label="{
            on: t('workspace.pages.models.enabled'),
            off: t('workspace.pages.models.disabled')
          }"
        />
        <span class="ws-model-label">
          {{ t("workspace.pages.models.enableLabel") }}
        </span>
      </div>
    </div>

    <p v-if="invalidTimeout(form)" class="ws-model-test is-bad">
      {{ t("workspace.pages.models.timeoutInvalid") }}
    </p>

    <footer class="ws-model-actions">
      <Button
        data-test="save"
        size="sm"
        :loading="saving"
        :disabled="invalidTimeout(form)"
        @click="emit('save', action())"
      >
        {{ t("workspace.pages.models.save") }}
      </Button>
      <Button
        data-test="test"
        size="sm"
        variant="secondary"
        :loading="testing"
        @click="emit('test', item.data.provider)"
      >
        {{ t("workspace.pages.models.test") }}
      </Button>
      <Button
        v-if="item.data.has_api_key"
        data-test="clear-key"
        size="sm"
        variant="ghost"
        :loading="saving"
        @click="emit('clear-key', action())"
      >
        {{ t("workspace.pages.models.clearKey") }}
      </Button>
    </footer>

    <p class="ws-model-hint">{{ t("workspace.pages.models.testUsesSaved") }}</p>
    <p
      v-if="item.testResult"
      class="ws-model-test"
      :class="{
        'is-ok': item.testResult.ok,
        'is-bad': !item.testResult.ok
      }"
      data-test="test-result"
    >
      {{ testMessage() }}
    </p>
  </section>
</template>

<style scoped>
.ws-model-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
}

.ws-model-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.ws-model-title {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.ws-model-name {
  font-size: var(--ws-font-size-section);
  font-weight: 600;
  color: var(--ws-text-primary);
}

.ws-model-slug {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-model-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
}

.ws-model-field--narrow {
  max-width: 140px;
}

.ws-model-label {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-model-hint {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-model-row {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
}

.ws-model-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-bottom: 8px;
}

.ws-model-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.ws-model-test {
  margin: 0;
  font-size: var(--ws-font-size-caption);
}

.ws-model-test.is-ok {
  color: var(--ws-color-success-text);
}

.ws-model-test.is-bad {
  color: var(--ws-color-danger-text);
}
</style>
