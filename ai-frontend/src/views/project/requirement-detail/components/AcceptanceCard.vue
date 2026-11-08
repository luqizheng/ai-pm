<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Plus, Trash2 } from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Input from "@/components/ui/input/Input.vue";
import Select from "@/components/ui/select/Select.vue";
import type {
  AcceptanceCriterion,
  CriterionVerdict,
  VerificationKind
} from "../../requirements/requirement-entry";

/**
 * 验收标准卡（S02-UI-02 §3C，2026-11-08 S02-I5 接真）。
 *
 * 真实读模型：承载词表徽标（Q-13）+ 需求级裁决徽标（Q-14）。
 * 治理动作（新增/移除/裁决）由页根落地 REST——后端闸 = 真人 + Lead OR
 * 显式 Approver（0040），非授权调用 403 由页根错误态呈现；本卡不预判
 * 权限（授权权威在后端）。
 */
interface Props {
  criteria: AcceptanceCriterion[];
  /** 命令在途（禁用动作按钮） */
  acting?: boolean;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (
    e: "add",
    payload: { statement: string; verification_kind: VerificationKind }
  ): void;
  (e: "remove", criterionId: string): void;
  (
    e: "verdict",
    payload: { criterionId: string; verdict: CriterionVerdict }
  ): void;
}>();

const { t } = useI18n();

const addOpen = ref(false);
const form = reactive({
  statement: "",
  kind: "manual" as VerificationKind
});

const kindOptions = computed(() => [
  { value: "manual", label: t("pm.requirements.critKindManual") },
  {
    value: "automated_test",
    label: t("pm.requirements.critKindAutomatedTest")
  },
  {
    value: "artifact_exists",
    label: t("pm.requirements.critKindArtifactExists")
  },
  { value: "review", label: t("pm.requirements.critKindReview") },
  { value: "performance", label: t("pm.requirements.critKindPerformance") },
  { value: "security", label: t("pm.requirements.critKindSecurity") }
]);

const criterionKindLabel = (kind: string) =>
  (
    Object.fromEntries(
      kindOptions.value.map(o => [o.value, o.label])
    ) as Record<string, string>
  )[kind] ?? kind;

const criterionVerdictLabel = (verdict: string) =>
  ({
    pending: t("pm.requirements.critVerdictPending"),
    passed: t("pm.requirements.critVerdictPassed"),
    failed: t("pm.requirements.critVerdictFailed")
  })[verdict] ?? verdict;

const canAdd = computed(
  () => props.acting !== true && form.statement.trim().length > 0
);

function onAdd() {
  if (!canAdd.value) return;
  emit("add", {
    statement: form.statement.trim(),
    verification_kind: form.kind
  });
  form.statement = "";
  addOpen.value = false;
}
</script>

<template>
  <section
    class="reqd-card"
    :aria-label="t('pm.requirementDetail.acceptTitle')"
  >
    <h3 class="reqd-card__title">
      {{ t("pm.requirementDetail.acceptTitle") }}
    </h3>
    <p class="reqd-accept__governance">
      {{ t("pm.requirements.critGovernanceNote") }}
    </p>

    <ol v-if="criteria.length > 0" class="reqd-accept__list">
      <li
        v-for="criterion in criteria"
        :key="criterion.id"
        class="reqd-accept__item"
      >
        <span class="reqd-accept__index" aria-hidden="true" />
        <span class="reqd-accept__statement">{{ criterion.statement }}</span>
        <Badge
          v-if="criterion.verification_kind"
          variant="neutral"
          size="sm"
          outline
        >
          {{ criterionKindLabel(criterion.verification_kind) }}
        </Badge>
        <Badge
          v-if="criterion.verdict && criterion.verdict !== 'pending'"
          :variant="criterion.verdict === 'passed' ? 'success' : 'danger'"
          size="sm"
        >
          {{ criterionVerdictLabel(criterion.verdict) }}
        </Badge>
        <span
          v-if="criterion.verdict_note"
          class="reqd-accept__note"
          :title="criterion.verdict_note"
          >{{ criterion.verdict_note }}</span
        >
        <span class="reqd-accept__actions">
          <Button
            variant="ghost"
            size="sm"
            :disabled="acting"
            :aria-label="t('pm.requirements.critVerdictPass')"
            @click="
              emit('verdict', { criterionId: criterion.id, verdict: 'passed' })
            "
          >
            {{ t("pm.requirements.critVerdictPass") }}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            :disabled="acting"
            :aria-label="t('pm.requirements.critVerdictFail')"
            @click="
              emit('verdict', { criterionId: criterion.id, verdict: 'failed' })
            "
          >
            {{ t("pm.requirements.critVerdictFail") }}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            :disabled="acting"
            :aria-label="t('pm.requirements.critRemove')"
            @click="emit('remove', criterion.id)"
          >
            <Trash2 aria-hidden="true" />
          </Button>
        </span>
      </li>
    </ol>
    <p v-else class="reqd-accept__empty">
      {{ t("pm.requirements.criteriaEmpty") }}
    </p>

    <div v-if="addOpen" class="reqd-accept__form">
      <Input
        v-model="form.statement"
        size="sm"
        :placeholder="t('pm.requirements.critAddPlaceholder')"
        @keyup.enter="onAdd"
      />
      <Select v-model="form.kind" :options="kindOptions" size="sm" />
      <Button variant="primary" size="sm" :disabled="!canAdd" @click="onAdd">
        {{ t("pm.requirements.critAdd") }}
      </Button>
      <Button variant="ghost" size="sm" @click="addOpen = false">
        {{ t("common.cancel") }}
      </Button>
    </div>
    <Button
      v-else
      variant="secondary"
      size="sm"
      class="reqd-accept__add-toggle"
      :disabled="acting"
      @click="addOpen = true"
    >
      <Plus aria-hidden="true" />
      {{ t("pm.requirements.critAddToggle") }}
    </Button>
  </section>
</template>

<style scoped>
.reqd-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.reqd-card__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.reqd-accept__governance {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.reqd-accept__list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
  counter-reset: accept;
}

.reqd-accept__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: var(--space-2) var(--space-3);
  counter-increment: accept;
}

.reqd-accept__index {
  flex: none;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.reqd-accept__index::before {
  content: "AC-" counter(accept);
}

.reqd-accept__statement {
  min-width: 0;
  word-break: break-word;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.reqd-accept__note {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 16rem;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.reqd-accept__actions {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  margin-left: auto;
  flex: none;
}

.reqd-accept__form {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.reqd-accept__form > .ui-input,
.reqd-accept__form > *:first-child {
  flex: 1 1 16rem;
}

.reqd-accept__add-toggle {
  align-self: flex-start;
}

.reqd-accept__empty {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-disabled);
}
</style>
