<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { translateError } from "@/api/base";
import SettingsAppFrame from "./SettingsAppFrame.vue";
import TeamProviderCard from "./TeamProviderCard.vue";
import {
  cardFormOf,
  configurationOf,
  invalidTimeout,
  timeoutOf,
  type LlmProvider
} from "./team-llm-types";
import { TEAM_APP_NAV, TEAM_ICONS } from "./nav";
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

/**
 * 团队设置 — AI 配置（Team LLM Provider Management §13）。
 * Team≡tenant：Provider 是团队资源，仅 owner/admin 可管理（后端守卫；
 * 非管理员在此会收到 403 文案）。生命周期：测试成功≠自动启用；
 * enable 需已配凭证（openai_compatible 本地端点豁免）。
 */
const { t } = useI18n();
const toast = useToast();

const tenantId = ref<string | null>(null);
const providers = ref<LlmProvider[]>([]);
const loading = ref(false);
const saving = ref<string | null>(null);
const testing = ref<string | null>(null);
const testMessages = reactive<Record<string, { ok: boolean; text: string }>>({});

const breadcrumb = [
  { label: t("settings.personal.landing.title"), to: "/settings/personal" },
  { label: t("settings.nav.aiProviders") }
];

// ── 新建 ─────────────────────────────────────────────────────
const showCreate = ref(false);
const creating = ref(false);
const draft = reactive({
  name: "",
  provider_type: "openai_compatible" as LlmProvider["provider_type"],
  endpoint: "",
  model: "",
  api_key: "",
  timeout: "120"
});

const providerTypeOptions = (
  [
    "openai",
    "anthropic",
    "gemini",
    "deepseek",
    "openai_compatible"
  ] as const
).map(type => ({
  value: type,
  label: t(`settings.team.ai.providerTypes.${type}`)
}));

const draftInvalid = computed(
  () => !draft.name.trim() || invalidTimeout({ ...draft, model: "", endpoint: "" })
);

function statusLabel(status: LlmProvider["status"]): string {
  return t(`settings.team.ai.status.${status}`);
}

function applyRow(row: LlmProvider) {
  const index = providers.value.findIndex(item => item.id === row.id);
  if (index >= 0) {
    providers.value[index] = row;
  } else {
    providers.value.push(row);
  }
}

function fail(titleKey: string, error: unknown) {
  toast.toast({
    variant: "danger",
    title: t(titleKey),
    description: translateError(String(error))
  });
}

async function load() {
  loading.value = true;
  try {
    tenantId.value = await resolveTenantId();
    const result = await listLlmProviders(tenantId.value);
    providers.value = result.items ?? [];
  } catch (error) {
    fail("settings.team.ai.loadFailed", error);
  } finally {
    loading.value = false;
  }
}

async function create() {
  if (!tenantId.value || draftInvalid.value) return;
  creating.value = true;
  try {
    const row = await createLlmProvider(tenantId.value, {
      name: draft.name.trim(),
      provider_type: draft.provider_type,
      endpoint: draft.endpoint.trim(),
      configuration: draft.model.trim() ? { model: draft.model.trim() } : {},
      api_key: draft.api_key.trim() || undefined,
      timeout_secs: timeoutOf({ ...draft, model: "", endpoint: "" })
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
      title: t("settings.team.ai.created"),
      description: row.name
    });
  } catch (error) {
    fail("settings.team.ai.saveFailed", error);
  } finally {
    creating.value = false;
  }
}

async function save(
  id: string,
  form: { name: string; endpoint: string; model: string; timeout: string }
) {
  const item = providers.value.find(row => row.id === id);
  if (!tenantId.value || !item || invalidTimeout(form)) return;
  saving.value = id;
  try {
    applyRow(
      await updateLlmProvider(tenantId.value, id, {
        expected_revision: item.revision,
        name: form.name.trim(),
        endpoint: form.endpoint.trim(),
        configuration: configurationOf(form),
        timeout_secs: timeoutOf(form)
      })
    );
    toast.toast({
      variant: "success",
      title: t("settings.team.ai.saved")
    });
  } catch (error) {
    fail("settings.team.ai.saveFailed", error);
  } finally {
    saving.value = null;
  }
}

async function replaceKey(id: string, key: string) {
  const item = providers.value.find(row => row.id === id);
  if (!tenantId.value || !item || !key) return;
  saving.value = id;
  try {
    applyRow(
      await replaceLlmCredential(tenantId.value, id, {
        expected_revision: item.revision,
        api_key: key
      })
    );
    toast.toast({
      variant: "success",
      title: t("settings.team.ai.keyReplaced")
    });
  } catch (error) {
    fail("settings.team.ai.saveFailed", error);
  } finally {
    saving.value = null;
  }
}

async function test(id: string) {
  if (!tenantId.value) return;
  testing.value = id;
  try {
    const result = await testLlmProvider(tenantId.value, id);
    testMessages[id] = {
      ok: result.ok,
      text: result.ok
        ? t("settings.team.ai.testOk", { ms: result.latency_ms })
        : `${t("settings.team.ai.testFailed")}: ${result.message}`
    };
    // 测试可能推进状态（connected/failed）——刷新列表。
    const fresh = await listLlmProviders(tenantId.value);
    providers.value = fresh.items ?? [];
  } catch (error) {
    testMessages[id] = {
      ok: false,
      text: `${t("settings.team.ai.testFailed")}: ${translateError(String(error))}`
    };
  } finally {
    testing.value = null;
  }
}

async function changeStatus(
  id: string,
  action: "enable" | "disable" | "archive"
) {
  const item = providers.value.find(row => row.id === id);
  if (!tenantId.value || !item) return;
  saving.value = id;
  try {
    applyRow(
      await setLlmProviderStatus(tenantId.value, id, action, item.revision)
    );
    toast.toast({
      variant: "success",
      title: t(`settings.team.ai.${action}Done`)
    });
  } catch (error) {
    fail("settings.team.ai.saveFailed", error);
  } finally {
    saving.value = null;
  }
}

async function remove(id: string) {
  if (!tenantId.value) return;
  saving.value = id;
  try {
    await deleteLlmProvider(tenantId.value, id);
    providers.value = providers.value.filter(row => row.id !== id);
    toast.toast({
      variant: "success",
      title: t("settings.team.ai.deleteDone")
    });
  } catch (error) {
    fail("settings.team.ai.deleteFailed", error);
  } finally {
    saving.value = null;
  }
}

onMounted(load);
</script>

<template>
  <div class="settings-shell-pad">
    <SettingsAppFrame
      :breadcrumb="breadcrumb"
      :title="t('settings.nav.aiProviders')"
      :description="t('settings.team.ai.desc')"
      :scope-label="t('settings.team.landing.title')"
      :entries="TEAM_APP_NAV"
      :icons="TEAM_ICONS"
    >
      <p class="tp-note">{{ t("settings.team.ai.securityNote") }}</p>

      <div class="tp-toolbar">
        <Button data-test="add-provider" size="sm" @click="showCreate = !showCreate">
          {{ t("settings.team.ai.addProvider") }}
        </Button>
      </div>

      <section v-if="showCreate" class="tp-create" data-test="create-form">
        <div class="tp-create__grid">
          <FormField :label="t('settings.team.ai.fields.name')">
            <Input v-model="draft.name" data-test="create-name" />
          </FormField>
          <FormField :label="t('settings.team.ai.fields.type')">
            <Select
              v-model="draft.provider_type"
              :options="providerTypeOptions"
              data-test="create-type"
            />
          </FormField>
          <FormField :label="t('settings.team.ai.fields.endpoint')">
            <Input v-model="draft.endpoint" placeholder="https://" data-test="create-endpoint" />
          </FormField>
          <FormField :label="t('settings.team.ai.fields.model')">
            <Input v-model="draft.model" data-test="create-model" />
          </FormField>
          <FormField
            :label="t('settings.team.ai.fields.apiKey')"
            :description="t('settings.team.ai.keyMissing')"
          >
            <Input v-model="draft.api_key" type="password" data-test="create-key" />
          </FormField>
          <FormField
            :label="t('settings.team.ai.fields.timeout')"
            :description="t('settings.team.ai.fields.timeoutHint')"
          >
            <Input v-model="draft.timeout" type="number" data-test="create-timeout" />
          </FormField>
        </div>
        <footer class="tp-create__actions">
          <Button
            data-test="create-submit"
            size="sm"
            :loading="creating"
            :disabled="draftInvalid"
            @click="create"
          >
            {{ t("settings.team.ai.create") }}
          </Button>
          <Button size="sm" variant="ghost" @click="showCreate = false">
            {{ t("settings.team.ai.cancel") }}
          </Button>
        </footer>
      </section>

      <p v-if="loading" class="tp-empty">{{ t("settings.team.ai.loading") }}</p>
      <p v-else-if="providers.length === 0" class="tp-empty">
        {{ t("settings.team.ai.empty") }}
      </p>

      <div v-else class="tp-grid" data-test="provider-grid">
        <TeamProviderCard
          v-for="row in providers"
          :key="row.id"
          :provider-id="row.id"
          :label="row.name"
          :provider-type="row.provider_type"
          :status="row.status"
          :status-label="statusLabel(row.status)"
          :has-credential="row.has_credential"
          :credential-hint="row.credential_hint"
          :last-tested-at="row.last_tested_at"
          :form="cardFormOf(row)"
          :saving="saving === row.id"
          :testing="testing === row.id"
          :test-message="testMessages[row.id]?.text ?? null"
          :test-ok="testMessages[row.id]?.ok ?? false"
          @save="form => save(row.id, form)"
          @replace-key="key => replaceKey(row.id, key)"
          @test="test(row.id)"
          @enable="changeStatus(row.id, 'enable')"
          @disable="changeStatus(row.id, 'disable')"
          @archive="changeStatus(row.id, 'archive')"
          @remove="remove(row.id)"
        />
      </div>
    </SettingsAppFrame>
  </div>
</template>

<style scoped>
.settings-shell-pad {
  padding: var(--space-5) var(--space-8);
}

.tp-note {
  margin: 0 0 var(--space-4);
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.tp-toolbar {
  display: flex;
  justify-content: flex-end;
  margin-bottom: var(--space-4);
}

.tp-create {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5);
  margin-bottom: var(--space-5);
  background-color: var(--ws-surface);
  border: var(--border-1) dashed var(--ws-border);
  border-radius: var(--radius-md);
}

.tp-create__grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-4);
}

.tp-create__actions {
  display: flex;
  gap: var(--space-2);
}

.tp-empty {
  margin: 0;
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-secondary);
}

.tp-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-4);
}

@media (max-width: 960px) {
  .tp-create__grid,
  .tp-grid {
    grid-template-columns: 1fr;
  }
}
</style>
