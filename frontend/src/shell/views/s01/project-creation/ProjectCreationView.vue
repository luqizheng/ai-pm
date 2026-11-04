<script setup lang="ts">
/**
 * S01-02 · 项目创建。
 *
 * UX 依据：`docs/ui/s01/s01-02-project-creation-ux-v1-1.md`
 *
 * 硬语义：
 *   - Project Name 必填；Project Key 由系统建议、人类确认。
 *   - Project Owner 默认当前用户，允许修改；**AI 不得指定 Owner**。
 *   - 可选背景信息（type/domain/outcome/context）不阻止创建。
 *   - 创建由人类主动提交；AI 不代为创建。
 *   - 创建成功后自动进入 Project Initialization Workspace。
 *   - 创建请求带幂等键，网络重试不产生重复 Project。
 */
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/shell/components";
import { useConfirm } from "@/shell/composables/useConfirm";
import { listCustomers } from "@/api/customers";
import { listPrincipals } from "@/api/pm";
import type { Customer, Principal } from "@/api/types";
import {
  createInitializedProject,
  suggestProjectCode,
  type CreateProjectPayload
} from "@/api/initialization";

const { t } = useI18n();
const router = useRouter();
const toast = useToast();
const confirm = useConfirm();

const STEPS = ["name", "owner", "background", "confirm"] as const;
type Step = (typeof STEPS)[number];

const step = ref<Step>("name");
const stepIndex = computed(() => STEPS.indexOf(step.value));

const customers = ref<Customer[]>([]);
const principals = ref<Principal[]>([]);

const form = ref({
  name: "",
  code: "",
  customer_id: "",
  owner_principal_id: "",
  description: "",
  project_type: "",
  business_domain: "",
  expected_outcome: "",
  initial_context: ""
});

/** 幂等键在页面生命周期内保持不变：重试同一请求不产生重复 Project。 */
const idempotencyKey = ref(crypto.randomUUID());

const codeSuggestion = ref<{
  code: string;
  available: boolean;
  conflict_code: string | null;
} | null>(null);
const codeSuggesting = ref(false);
const submitting = ref(false);
const created = ref<{ id: string; code: string; name: string } | null>(null);

/** 名称变化后重新建议 Key（用户手动改过 Key 则不再覆盖）。 */
const codeTouched = ref(false);
let suggestTimer: ReturnType<typeof setTimeout> | null = null;

const nameMissing = computed(() => form.value.name.trim() === "");
const codeMissing = computed(() => form.value.code.trim() === "");
/**
 * 校验提示仅在用户交互过后出现，避免首屏满屏红字。
 * 与上面的 `codeTouched`（是否手动改过 Key）是两件事，勿混。
 */
const nameInvalidated = ref(false);
const codeInvalidated = ref(false);
const codeConflict = computed(
  () => codeSuggestion.value !== null && !codeSuggestion.value.available
);

const customerOptions = computed(() =>
  customers.value.map(item => ({
    value: item.id,
    label:
      item.status === "placeholder"
        ? `${t("s01.initialization.projectCreation.customerUnassigned")} · ${
            item.short_name || item.name
          }`
        : item.short_name || item.name
  }))
);

const ownerOptions = computed(() =>
  principals.value.map(item => ({
    value: item.id,
    label: item.display_name
  }))
);

/** 无客户选择时的哨兵（reka-ui Select 不接受空串 value）。 */
const NO_CUSTOMER = "__none__";
const customerModel = computed({
  get: () => form.value.customer_id || NO_CUSTOMER,
  set: (value: string) => {
    form.value.customer_id = value === NO_CUSTOMER ? "" : value;
  }
});

const customerName = computed(
  () =>
    customers.value.find(item => item.id === form.value.customer_id)?.name ??
    "—"
);

const ownerName = computed(
  () =>
    principals.value.find(item => item.id === form.value.owner_principal_id)
      ?.display_name ?? "—"
);

const canSubmit = computed(
  () => !nameMissing.value && !codeMissing.value && !codeConflict.value
);

async function loadReferences() {
  try {
    const [customerResult, principalResult] = await Promise.all([
      listCustomers({ size: 200 }),
      listPrincipals({ page: 1, size: 200 })
    ]);
    customers.value = customerResult.items ?? [];
    principals.value = principalResult.items ?? [];
  } catch {
    customers.value = [];
    principals.value = [];
  }
}

async function requestSuggestion() {
  const name = form.value.name.trim();
  if (!name) {
    codeSuggestion.value = null;
    return;
  }
  codeSuggesting.value = true;
  try {
    const result = await suggestProjectCode(name);
    codeSuggestion.value = result;
    if (!codeTouched.value) {
      form.value.code = result.code;
    }
  } catch {
    // 建议失败不阻断人工填写（UX：AI 不可用不影响人工创建）。
    codeSuggestion.value = null;
  } finally {
    codeSuggesting.value = false;
  }
}

watch(
  () => form.value.name,
  () => {
    if (suggestTimer) clearTimeout(suggestTimer);
    suggestTimer = setTimeout(() => {
      void requestSuggestion();
    }, 300);
  }
);

function onCodeInput(value: string) {
  form.value.code = value;
  codeTouched.value = true;
  codeInvalidated.value = true;
}

function regenerateCode() {
  codeTouched.value = false;
  void requestSuggestion();
}

function goTo(target: Step) {
  step.value = target;
}

function next() {
  if (step.value === "name" && (nameMissing.value || codeMissing.value)) {
    return;
  }
  const index = stepIndex.value;
  if (index < STEPS.length - 1) {
    step.value = STEPS[index + 1];
  }
}

function prev() {
  const index = stepIndex.value;
  if (index > 0) {
    step.value = STEPS[index - 1];
  }
}

/** UX §5.2：提交前必须让用户明确知道即将创建什么。 */
async function submit() {
  if (!canSubmit.value) return;
  const ok = await confirm.confirm({
    title: t("s01.initialization.projectCreation.confirmTitle"),
    description: t("s01.initialization.projectCreation.confirmHint"),
    confirmLabel: t("s01.initialization.projectCreation.submit")
  });
  if (!ok) return;

  const payload: CreateProjectPayload = {
    code: form.value.code.trim(),
    name: form.value.name.trim(),
    idempotency_key: idempotencyKey.value
  };
  if (form.value.description.trim()) {
    payload.description = form.value.description.trim();
  }
  if (form.value.customer_id) {
    payload.customer_id = form.value.customer_id;
  }
  if (form.value.owner_principal_id) {
    payload.owner_principal_id = form.value.owner_principal_id;
  }
  if (form.value.project_type.trim()) {
    payload.project_type = form.value.project_type.trim();
  }
  if (form.value.business_domain.trim()) {
    payload.business_domain = form.value.business_domain.trim();
  }
  if (form.value.expected_outcome.trim()) {
    payload.expected_outcome = form.value.expected_outcome.trim();
  }
  if (form.value.initial_context.trim()) {
    payload.initial_context = form.value.initial_context.trim();
  }

  submitting.value = true;
  try {
    const result = await createInitializedProject(payload);
    created.value = { id: result.id, code: result.code, name: result.name };
    toast.toast({
      variant: "success",
      title: t("s01.initialization.projectCreation.created")
    });
    step.value = "confirm";
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("s01.initialization.projectCreation.createFailed"),
      description: String(error)
    });
  } finally {
    submitting.value = false;
  }
}

onMounted(async () => {
  await loadReferences();
  // Owner 默认当前登录用户：principal 列表首项即当前用户上下文。
  if (principals.value.length > 0 && !form.value.owner_principal_id) {
    form.value.owner_principal_id = principals.value[0].id;
  }
});
</script>

<template>
  <section class="ui-page">
    <PageHeader
      :title="t('s01.initialization.projectCreation.pageTitle')"
      :subtitle="t('s01.initialization.projectCreation.subtitle')"
    />

    <ol class="steps">
      <li
        v-for="(item, index) in STEPS"
        :key="item"
        class="steps__item"
        :class="{
          'is-active': index === stepIndex,
          'is-done': index < stepIndex
        }"
      >
        <button type="button" class="steps__btn" @click="goTo(item)">
          <span class="steps__index">{{ index + 1 }}</span>
          <span class="steps__label">
            {{
              t(
                `s01.initialization.projectCreation.step${item[0].toUpperCase()}${item.slice(1)}`
              )
            }}
          </span>
        </button>
      </li>
    </ol>

    <!-- 步骤一：项目信息 -->
    <section v-if="step === 'name'" class="ui-card">
      <div class="field">
        <p class="field__label">
          {{ t("s01.initialization.projectCreation.name") }}
        </p>
        <Input
          v-model="form.name"
          :placeholder="t('s01.initialization.projectCreation.namePlaceholder')"
          :invalid="nameInvalidated && nameMissing"
          @update:model-value="nameInvalidated = true"
        />
        <p v-if="nameInvalidated && nameMissing" class="field__hint">
          {{ t("s01.initialization.projectCreation.nameRequired") }}
        </p>
      </div>

      <div class="field">
        <p class="field__label">
          {{ t("s01.initialization.projectCreation.code") }}
        </p>
        <div class="field-row">
          <Input
            :model-value="form.code"
            :placeholder="
              t('s01.initialization.projectCreation.codePlaceholder')
            "
            :invalid="codeConflict"
            @update:model-value="onCodeInput"
          />
          <Button
            size="sm"
            variant="secondary"
            :loading="codeSuggesting"
            @click="regenerateCode"
          >
            {{ t("s01.initialization.projectCreation.codeRegenerate") }}
          </Button>
        </div>
        <p v-if="codeInvalidated && codeMissing" class="field__hint">
          {{ t("s01.initialization.projectCreation.codeRequired") }}
        </p>
        <p v-else-if="codeConflict" class="field__hint">
          {{ t("s01.initialization.projectCreation.codeConflict") }}
        </p>
        <p
          v-else-if="codeSuggestion?.available"
          class="field__hint field__hint--ok"
        >
          {{ t("s01.initialization.projectCreation.codeAvailable") }}
        </p>
        <p v-else class="field__hint field__hint--muted">
          {{ t("s01.initialization.projectCreation.codeHint") }}
        </p>
      </div>

      <div class="field">
        <p class="field__label">
          {{ t("s01.initialization.projectCreation.description") }}
        </p>
        <Textarea v-model="form.description" :rows="3" />
      </div>
    </section>

    <!-- 步骤二：客户与负责人 -->
    <section v-else-if="step === 'owner'" class="ui-card">
      <div class="field">
        <p class="field__label">
          {{ t("s01.initialization.projectCreation.customer") }}
        </p>
        <Select
          v-model="customerModel"
          :options="[
            {
              value: NO_CUSTOMER,
              label: t('s01.initialization.projectCreation.customerPlaceholder')
            },
            ...customerOptions
          ]"
          :aria-label="t('s01.initialization.projectCreation.customer')"
        />
      </div>

      <div class="field">
        <p class="field__label">
          {{ t("s01.initialization.projectCreation.owner") }}
        </p>
        <Select
          v-model="form.owner_principal_id"
          :options="ownerOptions"
          :aria-label="t('s01.initialization.projectCreation.owner')"
        />
        <p class="field__hint field__hint--muted">
          {{ t("s01.initialization.projectCreation.ownerHint") }}
        </p>
      </div>
    </section>

    <!-- 步骤三：项目背景（全部可选） -->
    <section v-else-if="step === 'background'" class="ui-card">
      <p class="field__hint field__hint--muted">
        {{ t("s01.initialization.projectCreation.backgroundHint") }}
      </p>

      <div class="field-row">
        <div class="field">
          <p class="field__label">
            {{ t("s01.initialization.projectCreation.projectType") }}
          </p>
          <Input
            v-model="form.project_type"
            :placeholder="
              t('s01.initialization.projectCreation.projectTypePlaceholder')
            "
          />
        </div>
        <div class="field">
          <p class="field__label">
            {{ t("s01.initialization.projectCreation.businessDomain") }}
          </p>
          <Input
            v-model="form.business_domain"
            :placeholder="
              t('s01.initialization.projectCreation.businessDomainPlaceholder')
            "
          />
        </div>
      </div>

      <div class="field">
        <p class="field__label">
          {{ t("s01.initialization.projectCreation.expectedOutcome") }}
        </p>
        <Textarea
          v-model="form.expected_outcome"
          :rows="2"
          :placeholder="
            t('s01.initialization.projectCreation.expectedOutcomePlaceholder')
          "
        />
      </div>

      <div class="field">
        <p class="field__label">
          {{ t("s01.initialization.projectCreation.initialContext") }}
        </p>
        <Textarea
          v-model="form.initial_context"
          :rows="3"
          :placeholder="
            t('s01.initialization.projectCreation.initialContextPlaceholder')
          "
        />
      </div>
    </section>

    <!-- 步骤四：确认 -->
    <section v-else class="ui-card">
      <template v-if="created">
        <h2 class="ui-card__title">
          {{ t("s01.initialization.projectCreation.created") }}
        </h2>
        <dl class="summary">
          <div class="summary__row">
            <dt>{{ t("s01.initialization.projectCreation.confirmName") }}</dt>
            <dd>{{ created.name }}</dd>
          </div>
          <div class="summary__row">
            <dt>{{ t("s01.initialization.projectCreation.confirmCode") }}</dt>
            <dd>
              <code>{{ created.code }}</code>
            </dd>
          </div>
        </dl>
        <p class="field__hint field__hint--muted">
          {{ t("s01.initialization.projectCreation.nextStep") }}
        </p>
        <Button @click="router.push(`/projects/${created.id}/initialization`)">
          {{ t("s01.initialization.projectCreation.goToWorkspace") }}
        </Button>
      </template>

      <template v-else>
        <h2 class="ui-card__title">
          {{ t("s01.initialization.projectCreation.confirmTitle") }}
        </h2>
        <p class="field__hint field__hint--muted">
          {{ t("s01.initialization.projectCreation.confirmHint") }}
        </p>
        <dl class="summary">
          <div class="summary__row">
            <dt>{{ t("s01.initialization.projectCreation.confirmName") }}</dt>
            <dd>{{ form.name }}</dd>
          </div>
          <div class="summary__row">
            <dt>{{ t("s01.initialization.projectCreation.confirmCode") }}</dt>
            <dd>
              <code>{{ form.code }}</code>
            </dd>
          </div>
          <div class="summary__row">
            <dt>{{ t("s01.initialization.projectCreation.confirmOwner") }}</dt>
            <dd>{{ ownerName }}</dd>
          </div>
          <div class="summary__row">
            <dt>
              {{ t("s01.initialization.projectCreation.confirmCustomer") }}
            </dt>
            <dd>{{ customerName }}</dd>
          </div>
          <div v-if="form.description" class="summary__row">
            <dt>{{ t("s01.initialization.projectCreation.description") }}</dt>
            <dd>{{ form.description }}</dd>
          </div>
        </dl>
      </template>
    </section>

    <footer class="actions">
      <Button variant="secondary" :disabled="stepIndex === 0" @click="prev">
        {{ t("s01.initialization.common.cancel") }}
      </Button>
      <Button
        v-if="stepIndex < STEPS.length - 1"
        :disabled="step === 'name' && !canSubmit"
        @click="next"
      >
        {{ t("s01.initialization.common.confirm") }}
      </Button>
      <Button
        v-else-if="!created"
        :loading="submitting"
        :disabled="!canSubmit"
        @click="submit"
      >
        {{ t("s01.initialization.projectCreation.submit") }}
      </Button>
    </footer>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  max-width: 960px;
}

.steps {
  display: flex;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.steps__item {
  flex: 1;
}

.steps__btn {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
  color: var(--text-secondary);
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.steps__item.is-active .steps__btn {
  border-color: var(--accent);
  color: var(--text-primary);
}

.steps__item.is-done .steps__btn {
  color: var(--text-primary);
}

.steps__index {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: var(--radius-full);
  background: var(--surface-sunken);
  font-size: var(--font-size-caption);
}

.steps__item.is-active .steps__index {
  background: var(--accent);
  color: var(--text-inverse);
}

.ui-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
}

.ui-card__title {
  margin: 0;
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  flex: 1;
}

.field-row {
  display: flex;
  gap: var(--space-3);
}

/* 输入框与操作按钮并排：按钮保持单行，输入框吃掉剩余宽度 */
.field-row > :first-child {
  flex: 1;
  min-width: 0;
}

.field-row > :last-child {
  flex: none;
  white-space: nowrap;
}

.field__label {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.field__hint {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--danger-strong);
}

.field__hint--muted {
  color: var(--text-tertiary);
}

.field__hint--ok {
  color: var(--success-strong);
}

.summary {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
}

.summary__row {
  display: flex;
  gap: var(--space-3);
  font-size: var(--font-size-small);
}

.summary__row dt {
  min-width: 120px;
  color: var(--text-secondary);
}

.summary__row dd {
  margin: 0;
  color: var(--text-primary);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-3);
}
</style>
