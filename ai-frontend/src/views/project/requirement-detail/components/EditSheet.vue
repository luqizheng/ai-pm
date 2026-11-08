<script setup lang="ts">
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import Button from "@/components/ui/button/Button.vue";
import Input from "@/components/ui/input/Input.vue";
import Sheet from "@/components/ui/sheet/Sheet.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import type { RequirementEntry } from "../../requirements/requirement-entry";

/**
 * 需求编辑抽屉（S02-UI-02 §5 Edit requirement，2026-11-08 接真语义）。
 *
 * 后端契约（S02-I1）：仅支持正文修订——title/type/priority 创建后不可改，
 * 规格/验收标准（S02-I5/AI 编排）无后端承载 → 相关字段不展示伪输入。
 * 保存由页根落地：最新 draft 则更新之，否则起草新修订——确认仍走显式
 * 提交+确认流程，本抽屉绝不改需求状态。
 */
interface Props {
  open: boolean;
  entry: RequirementEntry | null;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
  (e: "save", payload: { markdown: string; summary: string }): void;
}>();

const { t } = useI18n();

const form = reactive({
  summary: "",
  markdown: ""
});

watch(
  () => [props.open, props.entry?.id] as const,
  ([open]) => {
    if (!open || !props.entry) return;
    form.summary = "";
    form.markdown = props.entry.body_markdown ?? props.entry.description;
  },
  { immediate: true }
);

function onSave() {
  if (!props.entry) return;
  emit("save", {
    markdown: form.markdown.trim(),
    summary: form.summary.trim()
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
          <Input :model-value="props.entry.title" size="sm" disabled />
          <span class="reqd-edit__hint">{{
            t("pm.requirements.editImmutableHint")
          }}</span>
        </label>
      </fieldset>

      <fieldset class="reqd-edit__group">
        <legend class="reqd-edit__group-title">
          {{ t("pm.requirements.editBody") }}
        </legend>
        <label class="reqd-edit__field">
          <span class="reqd-edit__label">{{
            t("pm.requirements.editSummary")
          }}</span>
          <Input
            v-model="form.summary"
            size="sm"
            :placeholder="t('pm.requirements.editSummaryPlaceholder')"
          />
        </label>
        <label class="reqd-edit__field">
          <Textarea v-model="form.markdown" size="sm" :rows="14" required />
          <span class="reqd-edit__hint">{{
            t("pm.requirements.editBodyHint")
          }}</span>
        </label>
      </fieldset>

      <p class="reqd-edit__note">
        {{ t("pm.requirementDetail.editSheetNote") }}
      </p>

      <div class="reqd-edit__footer">
        <Button
          variant="primary"
          size="sm"
          type="button"
          :disabled="form.markdown.trim().length === 0"
          @click="onSave"
        >
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
