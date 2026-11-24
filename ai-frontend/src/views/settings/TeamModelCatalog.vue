<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { translateError } from "@/api/base";
import {
  createLlmModel,
  LLM_CAPABILITIES,
  LLM_ROLE_KEYS,
  listLlmAuthorizations,
  listLlmModels,
  listLlmTeamPolicies,
  setLlmTeamPolicies,
  updateLlmModel,
  upsertLlmAuthorizations,
  type LlmModelRow,
  type LlmPolicyRow,
  type LlmProvider
} from "@/api/llm";

/**
 * 团队设置 — 模型目录 / 授权 / 策略（R3 / B01-04）。
 *
 * 后端三面（`/v1/pm/teams/{id}/llm-models`、`llm-model-authorizations`、
 * `llm-model-policies`）在此首次接入前端。激活链 = Provider 启用 →
 * 登记（enabled）模型行 → team_wide 授权 → general 策略——四者齐备后
 * worker 选模才具备数据条件（`/ai-readiness` ready=true）。
 *
 * 不伪造：模型行必须人工登记（真实后端行），不提供静态模型清单；
 * 授权撤销只报告影响面（后端语义），不自动改写策略。
 */
const { t } = useI18n();
const toast = useToast();

const props = defineProps<{
  tenantId: string;
  providers: LlmProvider[];
}>();

const models = ref<LlmModelRow[]>([]);
const policies = ref<LlmPolicyRow[]>([]);
const loading = ref(false);
const acting = ref<string | null>(null);

// ── 新增模型行表单 ──────────────────────────────────────────────
const enabledProviders = computed(() =>
  props.providers.filter(row => row.status === "enabled")
);
const draft = reactive({
  provider_id: "",
  model_id: "",
  display_name: "",
  capabilities: ["text_generation"] as string[],
  context_window: ""
});
const draftInvalid = computed(
  () => !draft.provider_id || !draft.model_id.trim()
);

// ── 策略编辑草稿：role_key → model_id（哨兵 = 继承 general）────────
// reka-ui SelectItem 不接受空字符串 value——「继承」用哨兵表示，保存时归 null。
const INHERIT_SENTINEL = "__inherit__";
const policyDraft = reactive<Record<string, string>>({});

async function load() {
  loading.value = true;
  try {
    const [modelResult, policyResult, authResult] = await Promise.all([
      listLlmModels(props.tenantId),
      listLlmTeamPolicies(props.tenantId),
      listLlmAuthorizations(props.tenantId)
    ]);
    models.value = modelResult.items ?? [];
    policies.value = policyResult.items ?? [];
    for (const roleKey of LLM_ROLE_KEYS) {
      const row = policies.value.find(item => item.role_key === roleKey);
      policyDraft[roleKey] = row?.model_id ?? INHERIT_SENTINEL;
    }
    // 授权态真源 = 后端清单（只回 team_wide 行）。
    authorizationState.value = Object.fromEntries(
      (authResult.items ?? []).map(row => [row.model_id, row.authorization])
    );
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("settings.team.ai.catalog.loadFailed"),
      description: translateError(String(error))
    });
  } finally {
    loading.value = false;
  }
}

function modelOf(modelId: string): LlmModelRow | undefined {
  return models.value.find(row => row.id === modelId);
}
void modelOf;

/** 授权态真源：后端清单回填（model_id → state）；缺失 = 未授权（解析拒绝）。 */
const authorizationState = ref<Record<string, string>>({});

async function reloadAuthorizationsQuietly() {
  try {
    const authResult = await listLlmAuthorizations(props.tenantId);
    authorizationState.value = Object.fromEntries(
      (authResult.items ?? []).map(row => [row.model_id, row.authorization])
    );
  } catch {
    /* 静默：列表仍可用 */
  }
}

async function addModel() {
  if (draftInvalid.value) return;
  acting.value = "create";
  try {
    const row = await createLlmModel(props.tenantId, {
      provider_id: draft.provider_id,
      model_id: draft.model_id.trim(),
      display_name: draft.display_name.trim() || draft.model_id.trim(),
      capabilities: draft.capabilities,
      context_window: draft.context_window.trim()
        ? Number(draft.context_window)
        : undefined
    });
    models.value = [row, ...models.value];
    draft.model_id = "";
    draft.display_name = "";
    draft.context_window = "";
    toast.toast({
      variant: "success",
      title: t("settings.team.ai.catalog.modelAdded"),
      description: row.display_name
    });
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("settings.team.ai.catalog.saveFailed"),
      description: translateError(String(error))
    });
  } finally {
    acting.value = null;
  }
}

async function toggleModel(row: LlmModelRow) {
  acting.value = row.id;
  try {
    const fresh = await updateLlmModel(props.tenantId, row.id, {
      expected_revision: row.revision,
      enabled: !row.enabled
    });
    models.value = models.value.map(item => (item.id === row.id ? fresh : item));
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("settings.team.ai.catalog.saveFailed"),
      description: translateError(String(error))
    });
  } finally {
    acting.value = null;
  }
}

async function grant(row: LlmModelRow, action: "grant" | "revoke") {
  acting.value = `${row.id}:${action}`;
  try {
    const result = await upsertLlmAuthorizations(props.tenantId, [
      { model_id: row.id, action }
    ]);
    // 撤销影响面提示（后端语义：只报告不自动改写）。
    const affected = result.items?.[0];
    const stillReferenced =
      action === "revoke" &&
      affected &&
      affected.team_policy_refs + affected.project_override_refs > 0;
    toast.toast({
      variant: "success",
      title: t(`settings.team.ai.catalog.${action}Done`),
      description: stillReferenced
        ? t("settings.team.ai.catalog.revokeImpact", {
            count:
              (affected?.team_policy_refs ?? 0) +
              (affected?.project_override_refs ?? 0)
          })
        : undefined
    });
    await reloadAuthorizationsQuietly();
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("settings.team.ai.catalog.saveFailed"),
      description: translateError(String(error))
    });
  } finally {
    acting.value = null;
  }
}

async function savePolicies() {
  acting.value = "policies";
  try {
    const items = LLM_ROLE_KEYS.map(roleKey => ({
      role_key: roleKey,
      model_id:
        policyDraft[roleKey] === INHERIT_SENTINEL
          ? null
          : policyDraft[roleKey] || null
    }));
    const result = await setLlmTeamPolicies(props.tenantId, items);
    policies.value = result.items ?? [];
    toast.toast({
      variant: "success",
      title: t("settings.team.ai.catalog.policiesSaved")
    });
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("settings.team.ai.catalog.saveFailed"),
      description: translateError(String(error))
    });
  } finally {
    acting.value = null;
  }
}

const generalSet = computed(() =>
  Boolean(policies.value.find(row => row.role_key === "general")?.model_id)
);

const modelOptions = computed(() =>
  models.value
    .filter(row => row.enabled)
    .map(row => ({
      value: row.id,
      label: `${row.display_name}（${row.provider_name}）`
    }))
);

const capabilityOptions = LLM_CAPABILITIES.map(capability => ({
  value: capability,
  label: t(`settings.team.ai.catalog.capabilities.${capability}`)
}));

const providerOptions = computed(() =>
  enabledProviders.value.map(row => ({ value: row.id, label: row.name }))
);

onMounted(load);
</script>

<template>
  <section class="catalog" data-test="model-catalog">
    <h3 class="catalog__title">{{ t("settings.team.ai.catalog.title") }}</h3>
    <p class="catalog__hint">{{ t("settings.team.ai.catalog.hint") }}</p>

    <p v-if="loading" class="catalog__empty">
      {{ t("settings.team.ai.catalog.loading") }}
    </p>
    <template v-else>
      <!-- ① 新增模型行 -->
      <div class="catalog__add" data-test="catalog-add-form">
        <Select
          v-model="draft.provider_id"
          :options="providerOptions"
          :placeholder="t('settings.team.ai.catalog.providerPlaceholder')"
          data-test="catalog-add-provider"
        />
        <Input
          v-model="draft.model_id"
          :placeholder="t('settings.team.ai.catalog.modelIdPlaceholder')"
          data-test="catalog-add-model-id"
        />
        <Input
          v-model="draft.display_name"
          :placeholder="t('settings.team.ai.catalog.displayNamePlaceholder')"
          data-test="catalog-add-display-name"
        />
        <Input
          v-model="draft.context_window"
          type="number"
          :placeholder="t('settings.team.ai.catalog.contextWindowPlaceholder')"
          data-test="catalog-add-context-window"
        />
        <Button
          size="sm"
          :loading="acting === 'create'"
          :disabled="draftInvalid"
          data-test="catalog-add-submit"
          @click="addModel"
        >
          {{ t("settings.team.ai.catalog.addModel") }}
        </Button>
      </div>
      <div class="catalog__caps">
        <label
          v-for="capability in capabilityOptions"
          :key="capability.value"
          class="catalog__cap"
        >
          <input
            v-model="draft.capabilities"
            type="checkbox"
            :value="capability.value"
          />
          {{ capability.label }}
        </label>
      </div>

      <!-- ② 目录行 -->
      <p v-if="models.length === 0" class="catalog__empty">
        {{ t("settings.team.ai.catalog.empty") }}
      </p>
      <table v-else class="catalog__table" data-test="catalog-table">
        <thead>
          <tr>
            <th>{{ t("settings.team.ai.catalog.colModel") }}</th>
            <th>{{ t("settings.team.ai.catalog.colProvider") }}</th>
            <th>{{ t("settings.team.ai.catalog.colCapabilities") }}</th>
            <th>{{ t("settings.team.ai.catalog.colEnabled") }}</th>
            <th>{{ t("settings.team.ai.catalog.colAuthorization") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in models" :key="row.id" :data-test="`catalog-row-${row.model_id}`">
            <td>
              {{ row.display_name }}
              <code class="catalog__model-id">{{ row.model_id }}</code>
            </td>
            <td>{{ row.provider_name }}</td>
            <td class="catalog__caps-cell">
              <span v-for="capability in row.capabilities" :key="capability">
                {{ t(`settings.team.ai.catalog.capabilities.${capability}`) }}
              </span>
            </td>
            <td>
              <label class="catalog__toggle">
                <input
                  type="checkbox"
                  :checked="row.enabled"
                  :disabled="acting === row.id"
                  :data-test="`catalog-toggle-${row.model_id}`"
                  @change="toggleModel(row)"
                />
                {{ row.enabled ? t("settings.team.ai.catalog.on") : t("settings.team.ai.catalog.off") }}
              </label>
            </td>
            <td>
              <Button
                v-if="!authorizationState[row.id]"
                size="sm"
                variant="secondary"
                :disabled="acting === `${row.id}:grant` || !row.enabled"
                :data-test="`catalog-grant-${row.model_id}`"
                @click="grant(row, 'grant')"
              >
                {{ t("settings.team.ai.catalog.grant") }}
              </Button>
              <Button
                v-else
                size="sm"
                variant="ghost"
                :disabled="acting === `${row.id}:revoke`"
                :data-test="`catalog-revoke-${row.model_id}`"
                @click="grant(row, 'revoke')"
              >
                {{ t("settings.team.ai.catalog.revoke") }}
              </Button>
            </td>
          </tr>
        </tbody>
      </table>

      <!-- ③ Role 策略 -->
      <div class="catalog__policies" data-test="catalog-policies">
        <h4 class="catalog__subtitle">
          {{ t("settings.team.ai.catalog.policiesTitle") }}
        </h4>
        <p
          v-if="!generalSet"
          class="catalog__warning"
          data-test="catalog-general-missing"
        >
          {{ t("settings.team.ai.catalog.generalMissing") }}
        </p>
        <div
          v-for="roleKey in LLM_ROLE_KEYS"
          :key="roleKey"
          class="catalog__policy-row"
        >
          <span class="catalog__policy-role">
            {{ t(`settings.team.ai.catalog.roles.${roleKey}`) }}
          </span>
          <Select
            v-model="policyDraft[roleKey]"
            :options="[
              {
                value: INHERIT_SENTINEL,
                label: t('settings.team.ai.catalog.inheritGeneral')
              },
              ...modelOptions
            ]"
            :data-test="`catalog-policy-${roleKey}`"
          />
        </div>
        <Button
          size="sm"
          :loading="acting === 'policies'"
          data-test="catalog-policies-save"
          @click="savePolicies"
        >
          {{ t("settings.team.ai.catalog.savePolicies") }}
        </Button>
      </div>
    </template>
  </section>
</template>

<style scoped>
.catalog {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-5);
  margin-top: var(--space-5);
  background-color: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-md);
}

.catalog__title {
  margin: 0;
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-semibold);
}

.catalog__subtitle {
  margin: 0;
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-medium);
}

.catalog__hint,
.catalog__empty {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.catalog__warning {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-color-warning-text);
}

.catalog__add {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  align-items: center;
}

.catalog__caps {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
}

.catalog__cap {
  display: flex;
  gap: var(--space-1);
  align-items: center;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.catalog__table {
  width: 100%;
  font-size: var(--ws-font-size-caption);
  border-collapse: collapse;
}

.catalog__table th,
.catalog__table td {
  padding: var(--space-2);
  text-align: left;
  border-bottom: var(--border-1) solid var(--ws-border);
}

.catalog__model-id {
  margin-left: var(--space-2);
  font-family: var(--font-mono);
  color: var(--ws-text-secondary);
}

.catalog__caps-cell {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.catalog__toggle {
  display: flex;
  gap: var(--space-1);
  align-items: center;
}

.catalog__policies {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.catalog__policy-row {
  display: grid;
  grid-template-columns: 160px minmax(0, 320px);
  gap: var(--space-2);
  align-items: center;
}

.catalog__policy-role {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}
</style>
