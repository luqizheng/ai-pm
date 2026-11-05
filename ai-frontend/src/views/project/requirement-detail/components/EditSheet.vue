<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import Button from "@/components/ui/button/Button.vue";
import Input from "@/components/ui/input/Input.vue";
import Select from "@/components/ui/select/Select.vue";
import Sheet from "@/components/ui/sheet/Sheet.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import type {
  RequirementEntry,
  RequirementPriority,
  RequirementType
} from "../../requirements/requirement-entry";

/**
 * 需求编辑抽屉（S02-UI-02 §5 Edit requirement）。
 *
 * 覆盖基本信息（标题/描述/类型/优先级/验收标准）与需求规格四节
 * （业务理由 / 预期结果 / 范围 / 明确排除）。保存由页根落地：
 * 仅生成新的 draft 修订，不改需求状态——确认仍走显式确认流程。
 */
interface Props {
  open: boolean;
  entry: RequirementEntry | null;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
  (
    e: "save",
    payload: {
      title: string;
      description: string;
      type: RequirementType;
      priority: RequirementPriority;
      criteria: string[];
      specification: {
        business_rationale: string;
        expected_outcome: string;
        scope: string;
        explicit_exclusions: string;
      };
    }
  ): void;
}>();

const { t } = useI18n();

const form = reactive({
  title: "",
  description: "",
  type: "functional" as RequirementType,
  priority: "medium" as RequirementPriority,
  criteriaText: "",
  rationale: "",
  outcome: "",
  scope: "",
  exclusions: ""
});

watch(
  () => [props.open, props.entry?.id] as const,
  ([open]) => {
    if (!open || !props.entry) return;
    form.title = props.entry.title;
    form.description = props.entry.description;
    form.type = props.entry.type;
    form.priority = props.entry.priority;
    form.criteriaText = props.entry.acceptance_criteria
      .map(item => item.statement)
      .join("\n");
    form.rationale = props.entry.specification.business_rationale;
    form.outcome = props.entry.specification.expected_outcome;
    form.scope = props.entry.specification.scope;
    form.exclusions = props.entry.specification.explicit_exclusions;
  },
  { immediate: true }
);

const typeOptions = computed(() => [
  { value: "functional", label: t("pm.requirements.typeFunctional") },
  { value: "non_functional", label: t("pm.requirements.typeNonFunctional") }
]);

const priorityOptions = computed(() => [
  { value: "high", label: t("pm.requirements.priorityHigh") },
  { value: "medium", label: t("pm.requirements.priorityMedium") },
  { value: "low", label: t("pm.requirements.priorityLow") }
]);

function onSave() {
  if (!props.entry) return;
  emit("save", {
    title: form.title.trim() || props.entry.title,
    description: form.description.trim(),
    type: form.type,
    priority: form.priority,
    criteria: form.criteriaText
      .split("\n")
      .map(line => line.trim())
      .filter(Boolean),
    specification: {
      business_rationale: form.rationale.trim(),
      expected_outcome: form.outcome.trim(),
      scope: form.scope.trim(),
      explicit_exclusions: form.exclusions.trim()
    }
  });
}
</script>

<template>
  <Sheet
    :open="props.open"
    :title="t('pm.requirementDetail.editSheetTitle')"
    :description="props.entry ? `${props.entry.code} ${props.entry.title}` : ''"
    side="right"
    @update:open="emit('update:open', $event)"
  >
    <form v-if="props.entry" class="reqd-edit" @submit.prevent="onSave">
      <fieldset class="reqd-edit__group">
        <legend class="reqd-edit__group-title">
          {{ t("pm.requirementDetail.editBasicTitle") }}
        </legend>
        <label class="reqd-edit__field">
          <span class="reqd-edit__label">{{
            t("pm.requirements.editTitleField")
          }}</span>
          <Input v-model="form.title" size="sm" required />
        </label>
        <div class="reqd-edit__row">
          <label class="reqd-edit__field">
            <span class="reqd-edit__label">{{
              t("pm.requirements.detailType")
            }}</span>
            <Select v-model="form.type" :options="typeOptions" size="sm" />
          </label>
          <label class="reqd-edit__field">
            <span class="reqd-edit__label">{{
              t("pm.requirements.detailPriority")
            }}</span>
            <Select
              v-model="form.priority"
              :options="priorityOptions"
              size="sm"
            />
          </label>
        </div>
        <label class="reqd-edit__field">
          <span class="reqd-edit__label">{{
            t("pm.requirements.editDescription")
          }}</span>
          <Textarea v-model="form.description" size="sm" :rows="3" />
        </label>
        <label class="reqd-edit__field">
          <span class="reqd-edit__label">{{
            t("pm.requirements.editCriteria")
          }}</span>
          <Textarea v-model="form.criteriaText" size="sm" :rows="4" />
          <span class="reqd-edit__hint">{{
            t("pm.requirements.editCriteriaHint")
          }}</span>
        </label>
      </fieldset>

      <fieldset class="reqd-edit__group">
        <legend class="reqd-edit__group-title">
          {{ t("pm.requirementDetail.specTitle") }}
        </legend>
        <label class="reqd-edit__field">
          <span class="reqd-edit__label">{{
            t("pm.requirementDetail.specRationale")
          }}</span>
          <Textarea v-model="form.rationale" size="sm" :rows="2" />
        </label>
        <label class="reqd-edit__field">
          <span class="reqd-edit__label">{{
            t("pm.requirementDetail.specOutcome")
          }}</span>
          <Textarea v-model="form.outcome" size="sm" :rows="2" />
        </label>
        <label class="reqd-edit__field">
          <span class="reqd-edit__label">{{
            t("pm.requirementDetail.specScope")
          }}</span>
          <Textarea v-model="form.scope" size="sm" :rows="2" />
        </label>
        <label class="reqd-edit__field">
          <span class="reqd-edit__label">{{
            t("pm.requirementDetail.specExclusions")
          }}</span>
          <Textarea v-model="form.exclusions" size="sm" :rows="2" />
        </label>
      </fieldset>

      <p class="reqd-edit__note">
        {{ t("pm.requirementDetail.editSheetNote") }}
      </p>

      <div class="reqd-edit__footer">
        <Button variant="primary" size="sm" type="submit">
          {{ t("pm.requirements.editSave") }}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          type="button"
          @click="emit('update:open', false)"
        >
          {{ t("pm.requirements.editCancel") }}
        </Button>
      </div>
    </form>
  </Sheet>
</template>

<style scoped>
.reqd-edit {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.reqd-edit__group {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  border: 0;
}

.reqd-edit__group-title {
  padding: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.reqd-edit__field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.reqd-edit__row {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
}

.reqd-edit__label {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.reqd-edit__hint {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.reqd-edit__note {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.reqd-edit__footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  padding-top: var(--space-3);
  border-top: var(--hairline) solid var(--border-default);
}
</style>
