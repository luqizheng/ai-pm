<script setup lang="ts">
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import {
  invalidTimeout,
  statusBadgeVariant,
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
  test: [providerId: string];
  "replace-key": [action: ProviderAction];
  enable: [providerId: string];
  disable: [providerId: string];
  archive: [providerId: string];
  remove: [providerId: string];
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
    providerId: props.item.data.id,
    label: props.item.label,
    revision: props.item.data.revision,
    form: { ...form }
  };
}

function statusLabel(status: typeof props.item.data.status): string {
  return t(`workspace.pages.models.status.${status}`);
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
    :data-provider="item.data.id"
  >
    <header class="ws-model-head">
      <div class="ws-model-title">
        <span class="ws-model-name">{{ item.label }}</span>
        <span class="ws-model-slug">{{ item.data.provider_type }}</span>
      </div>
      <Badge :variant="statusBadgeVariant(item.data.status)">
        {{ statusLabel(item.data.status) }}
      </Badge>
    </header>

    <FormField :label="t('workspace.pages.models.fields.name')">
      <Input v-model="form.name" />
    </FormField>

    <FormField :label="t('workspace.pages.models.fields.endpoint')">
      <Input v-model="form.endpoint" placeholder="https://" />
    </FormField>

    <FormField :label="t('workspace.pages.models.fields.model')">
      <Input v-model="form.model" />
    </FormField>

    <FormField
      :label="t('workspace.pages.models.fields.apiKey')"
      :description="
        item.data.has_credential
          ? t('workspace.pages.models.keyConfigured', {
              hint: item.data.credential_hint ?? ''
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
      <span class="ws-model-label">
        {{ t("workspace.pages.models.lastTested", {
          time: item.data.last_tested_at
            ? new Date(item.data.last_tested_at).toLocaleString()
            : t("workspace.pages.models.neverTested")
        }) }}
      </span>
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
        v-if="form.api_key.trim()"
        data-test="replace-key"
        size="sm"
        variant="secondary"
        :loading="saving"
        :disabled="invalidTimeout(form)"
        @click="emit('replace-key', action())"
      >
        {{ t("workspace.pages.models.replaceKey") }}
      </Button>
      <Button
        data-test="test"
        size="sm"
        variant="secondary"
        :loading="testing"
        @click="emit('test', item.data.id)"
      >
        {{ t("workspace.pages.models.test") }}
      </Button>
      <Button
        v-if="item.data.status !== 'enabled' && item.data.status !== 'archived'"
        data-test="enable"
        size="sm"
        variant="secondary"
        :loading="saving"
        @click="emit('enable', item.data.id)"
      >
        {{ t("workspace.pages.models.enable") }}
      </Button>
      <Button
        v-if="item.data.status === 'enabled'"
        data-test="disable"
        size="sm"
        variant="secondary"
        :loading="saving"
        @click="emit('disable', item.data.id)"
      >
        {{ t("workspace.pages.models.disable") }}
      </Button>
      <Button
        v-if="item.data.status !== 'archived'"
        data-test="archive"
        size="sm"
        variant="ghost"
        :loading="saving"
        @click="emit('archive', item.data.id)"
      >
        {{ t("workspace.pages.models.archive") }}
      </Button>
      <Button
        v-if="item.data.status === 'archived'"
        data-test="delete"
        size="sm"
        variant="ghost"
        :loading="saving"
        @click="emit('remove', item.data.id)"
      >
        {{ t("workspace.pages.models.delete") }}
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
