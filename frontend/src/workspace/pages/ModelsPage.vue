<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import type { SelectOption } from "@/components/ui/select";
import { translateError } from "@/shell/api";
import PageHeader from "./PageHeader.vue";
import ProviderCard from "./ProviderCard.vue";
import {
  invalidTimeout,
  PROVIDER_TYPES,
  timeoutOf,
  type ProviderAction,
  type ProviderForm,
  type ProviderItem
} from "./models-types";
import {
  createLlmProvider,
  deleteLlmProvider,
  listLlmProviders,
  replaceLlmCredential,
  resolveTenantId,
  setLlmProviderStatus,
  testLlmProvider,
  updateLlmProvider
} from "@/api/llm";
import type { LlmProvider, LlmProviderType } from "@/api/llm";

const { t } = useI18n();
const toast = useToast();

const items = ref<ProviderItem[]>([]);
const loading = ref(false);
const saving = ref<string | null>(null);
const testing = ref<string | null>(null);
const tenantId = ref<string | null>(null);

// ── 新建表单（内联卡片）────────────────────────────────────
const showCreate = ref(false);
const creating = ref(false);
const draft = reactive<ProviderForm>({
  name: "",
  provider_type: "openai_compatible",
  endpoint: "",
  model: "",
  api_key: "",
  timeout: "120"
});

function buildForm(row: LlmProvider): ProviderForm {
  return {
    name: row.name,
    provider_type: row.provider_type,
    endpoint: row.endpoint,
    model: String(row.configuration?.model ?? ""),
    api_key: "",
    timeout: String(row.timeout_secs)
  };
}

function buildItem(row: LlmProvider): ProviderItem {
  return {
    data: row,
    label: row.name,
    form: buildForm(row)
  };
}

function applyRow(row: LlmProvider) {
  const index = items.value.findIndex(item => item.data.id === row.id);
  const next = buildItem(row);
  if (index >= 0) {
    // 重建时保留卡片上的即时测试结果（test 之后的刷新会走这里）。
    next.testResult = items.value[index]?.testResult;
    items.value[index] = next;
  } else {
    items.value.push(next);
  }
}

async function load() {
  loading.value = true;
  try {
    tenantId.value = await resolveTenantId();
    const result = await listLlmProviders(tenantId.value);
    items.value = (result.items ?? []).map(buildItem);
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("workspace.pages.models.loadFailed"),
      description: translateError(String(error))
    });
  } finally {
    loading.value = false;
  }
}

const providerTypeOptions: SelectOption[] = PROVIDER_TYPES.map(type => ({
  value: type,
  label: t(`workspace.pages.models.providerTypes.${type}`)
}));

function configurationOf(form: ProviderForm) {
  const model = form.model.trim();
  return model ? { model } : {};
}

async function create() {
  if (!tenantId.value) return;
  if (!draft.name.trim() || invalidTimeout(draft)) return;
  creating.value = true;
  try {
    const row = await createLlmProvider(tenantId.value, {
      name: draft.name.trim(),
      provider_type: draft.provider_type,
      endpoint: draft.endpoint.trim(),
      configuration: configurationOf(draft),
      api_key: draft.api_key.trim() || undefined,
      timeout_secs: timeoutOf(draft)
    });
    applyRow(row);
    showCreate.value = false;
    Object.assign(draft, {
      name: "",
      provider_type: "openai_compatible",
      endpoint: "",
      model: "",
      api_key: "",
      timeout: "120"
    });
    toast.toast({
      variant: "success",
      title: t("workspace.pages.models.created"),
      description: row.name
    });
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("workspace.pages.models.saveFailed"),
      description: translateError(String(error))
    });
  } finally {
    creating.value = false;
  }
}

async function save(action: ProviderAction) {
  if (!tenantId.value) return;
  const form = action.form;
  if (invalidTimeout(form)) return;
  saving.value = action.providerId;
  try {
    const updated = await updateLlmProvider(tenantId.value, action.providerId, {
      expected_revision: action.revision,
      name: form.name.trim(),
      provider_type: form.provider_type,
      endpoint: form.endpoint.trim(),
      configuration: configurationOf(form),
      timeout_secs: timeoutOf(form)
    });
    applyRow(updated);
    toast.toast({
      variant: "success",
      title: t("workspace.pages.models.saved"),
      description: action.label
    });
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("workspace.pages.models.saveFailed"),
      description: translateError(String(error))
    });
  } finally {
    saving.value = null;
  }
}

async function replaceKey(action: ProviderAction) {
  if (!tenantId.value) return;
  const key = action.form.api_key.trim();
  if (!key) return;
  saving.value = action.providerId;
  try {
    const updated = await replaceLlmCredential(tenantId.value, action.providerId, {
      expected_revision: action.revision,
      api_key: key
    });
    applyRow(updated);
    toast.toast({
      variant: "success",
      title: t("workspace.pages.models.keyReplaced"),
      description: action.label
    });
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("workspace.pages.models.saveFailed"),
      description: translateError(String(error))
    });
  } finally {
    saving.value = null;
  }
}

async function changeStatus(providerId: string, action: "enable" | "disable" | "archive") {
  if (!tenantId.value) return;
  const item = items.value.find(entry => entry.data.id === providerId);
  if (!item) return;
  saving.value = providerId;
  try {
    const updated = await setLlmProviderStatus(
      tenantId.value,
      providerId,
      action,
      item.data.revision
    );
    applyRow(updated);
    toast.toast({
      variant: "success",
      title: t(`workspace.pages.models.${action}Done`),
      description: item.label
    });
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("workspace.pages.models.saveFailed"),
      description: translateError(String(error))
    });
  } finally {
    saving.value = null;
  }
}

async function remove(providerId: string) {
  if (!tenantId.value) return;
  const item = items.value.find(entry => entry.data.id === providerId);
  if (!item) return;
  saving.value = providerId;
  try {
    await deleteLlmProvider(tenantId.value, providerId);
    items.value = items.value.filter(entry => entry.data.id !== providerId);
    toast.toast({
      variant: "success",
      title: t("workspace.pages.models.deleteDone"),
      description: item.label
    });
  } catch (error) {
    // 409 = 仍有调用记录（执行记录永久保留，DEC-LLM-004）。
    toast.toast({
      variant: "danger",
      title: t("workspace.pages.models.deleteFailed"),
      description: translateError(String(error))
    });
  } finally {
    saving.value = null;
  }
}

async function test(providerId: string) {
  if (!tenantId.value) return;
  const item = items.value.find(entry => entry.data.id === providerId);
  if (!item) return;
  testing.value = providerId;
  try {
    item.testResult = await testLlmProvider(tenantId.value, providerId);
    // 测试可能推进状态（connected/failed）——重新拉取该实例。
    const result = await listLlmProviders(tenantId.value);
    const fresh = (result.items ?? []).find(row => row.id === providerId);
    if (fresh) applyRow(fresh);
  } catch (error) {
    item.testResult = {
      ok: false,
      latency_ms: 0,
      message: translateError(String(error))
    };
  } finally {
    testing.value = null;
  }
}

onMounted(load);
</script>

<template>
  <div class="ws-page">
    <PageHeader
      :title="t('workspace.pages.models.title')"
      :subtitle="t('workspace.pages.models.subtitle')"
    />
    <p class="ws-models-note">{{ t("workspace.pages.models.securityNote") }}</p>

    <div class="ws-models-toolbar">
      <Button data-test="add-provider" size="sm" @click="showCreate = !showCreate">
        {{ t("workspace.pages.models.addProvider") }}
      </Button>
    </div>

    <section v-if="showCreate" class="ws-card ws-model-card" data-test="create-form">
      <FormField :label="t('workspace.pages.models.fields.name')">
        <Input v-model="draft.name" data-test="create-name" />
      </FormField>
      <FormField :label="t('workspace.pages.models.fields.type')">
        <Select
          v-model="draft.provider_type"
          :options="providerTypeOptions"
          data-test="create-type"
        />
      </FormField>
      <FormField :label="t('workspace.pages.models.fields.endpoint')">
        <Input v-model="draft.endpoint" placeholder="https://" data-test="create-endpoint" />
      </FormField>
      <FormField :label="t('workspace.pages.models.fields.model')">
        <Input v-model="draft.model" data-test="create-model" />
      </FormField>
      <FormField
        :label="t('workspace.pages.models.fields.apiKey')"
        :description="t('workspace.pages.models.keyMissing')"
      >
        <Input v-model="draft.api_key" type="password" data-test="create-key" />
      </FormField>
      <div class="ws-model-row">
        <div class="ws-model-field ws-model-field--narrow">
          <FormField :label="t('workspace.pages.models.fields.timeout')">
            <Input v-model="draft.timeout" type="number" :invalid="invalidTimeout(draft)" />
          </FormField>
        </div>
      </div>
      <footer class="ws-model-actions">
        <Button
          data-test="create-submit"
          size="sm"
          :loading="creating"
          :disabled="!draft.name.trim() || invalidTimeout(draft)"
          @click="create"
        >
          {{ t("workspace.pages.models.create") }}
        </Button>
        <Button size="sm" variant="ghost" @click="showCreate = false">
          {{ t("workspace.pages.models.cancel") }}
        </Button>
      </footer>
    </section>

    <p v-if="loading" class="ws-models-empty">
      {{ t("workspace.pages.models.loading") }}
    </p>
    <p v-else-if="items.length === 0" class="ws-models-empty">
      {{ t("workspace.pages.models.empty") }}
    </p>

    <div v-else class="ws-models-grid" data-test="provider-grid">
      <ProviderCard
        v-for="item in items"
        :key="item.data.id"
        :item="item"
        :saving="saving === item.data.id"
        :testing="testing === item.data.id"
        @save="save"
        @test="test"
        @replace-key="replaceKey"
        @enable="id => changeStatus(id, 'enable')"
        @disable="id => changeStatus(id, 'disable')"
        @archive="id => changeStatus(id, 'archive')"
        @remove="remove"
      />
    </div>
  </div>
</template>

<style scoped>
.ws-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px 32px;
  max-width: 1120px;
}

.ws-models-note {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.ws-models-toolbar {
  display: flex;
  justify-content: flex-end;
}

.ws-models-empty {
  margin: 0;
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-secondary);
}

.ws-models-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

@media (max-width: 900px) {
  .ws-models-grid {
    grid-template-columns: 1fr;
  }
}

.ws-card {
  border: 1px solid var(--ws-border-color, var(--ws-text-secondary));
  border-radius: 8px;
  background: var(--ws-surface-primary, transparent);
}

.ws-model-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
}

.ws-model-row {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
}

.ws-model-field--narrow {
  max-width: 140px;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.ws-model-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}
</style>
