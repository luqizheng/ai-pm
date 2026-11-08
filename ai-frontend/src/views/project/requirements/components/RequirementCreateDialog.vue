<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";

import Button from "@/components/ui/button/Button.vue";
import Dialog from "@/components/ui/dialog/Dialog.vue";
import Input from "@/components/ui/input/Input.vue";
import Select from "@/components/ui/select/Select.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";
import type {
  RequirementCreatePayload,
  RequirementType
} from "../requirement-entry";

/**
 * 新建需求对话框（S02-UI-01 §4 Add 动作的接真实现）。
 *
 * 后端契约（S02-I1 CreateRequirementRequest）：code/title 必填，title/type/
 * priority 创建后不可改 → 必须在创建时就让人类显式给定。code 由页根按现有
 * 序列预生成（项目内唯一，撞号由后端 409 兜底），此处展示为只读。
 */

interface Props {
  open: boolean;
  /** 预生成的下一个需求编码（REQ-NNN） */
  nextCode: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
  (e: "create", payload: RequirementCreatePayload): void;
}>();

const { t } = useI18n();

const form = reactive({
  title: "",
  type: "functional" as RequirementType,
  priority: "normal" as RequirementCreatePayload["priority"],
  markdown: ""
});

watch(
  () => props.open,
  open => {
    if (open) {
      form.title = "";
      form.type = "functional";
      form.priority = "normal";
      form.markdown = "";
    }
  }
);

const typeOptions = computed(() => [
  { value: "functional", label: t("pm.requirements.typeFunctional") },
  {
    value: "non_functional",
    label: t("pm.requirements.typeNonFunctional")
  }
]);

const priorityOptions = computed(() => [
  { value: "high", label: t("pm.requirements.priorityHigh") },
  { value: "normal", label: t("pm.requirements.priorityNormal") },
  { value: "low", label: t("pm.requirements.priorityLow") }
]);

const canSubmit = computed(
  () => form.title.trim().length > 0 && props.nextCode.length > 0
);

function onCreate() {
  if (!canSubmit.value) return;
  emit("create", {
    code: props.nextCode,
    title: form.title.trim(),
    requirement_type: form.type,
    priority: form.priority,
    markdown: form.markdown.trim() || undefined
  });
}
</script>

<template>
  <Dialog
    :open="props.open"
    :title="t('pm.requirements.createTitle')"
    :description="t('pm.requirements.createDesc')"
    size="md"
    @update:open="emit('update:open', $event)"
  >
    <form class="req-create" @submit.prevent="onCreate">
      <div class="req-create__field-row">
        <label class="req-create__field">
          <span class="req-create__label">{{
            t("pm.requirements.createCode")
          }}</span>
          <Input :model-value="props.nextCode" size="sm" disabled />
        </label>
        <label class="req-create__field">
          <span class="req-create__label">{{
            t("pm.requirements.createPriority")
          }}</span>
          <Select
            v-model="form.priority"
            :options="priorityOptions"
            size="sm"
          />
        </label>
      </div>
      <label class="req-create__field">
        <span class="req-create__label">{{
          t("pm.requirements.createTitleField")
        }}</span>
        <Input v-model="form.title" size="sm" required />
      </label>
      <label class="req-create__field">
        <span class="req-create__label">{{
          t("pm.requirements.createType")
        }}</span>
        <Select v-model="form.type" :options="typeOptions" size="sm" />
      </label>
      <label class="req-create__field">
        <span class="req-create__label">{{
          t("pm.requirements.createBody")
        }}</span>
        <Textarea
          v-model="form.markdown"
          size="sm"
          :rows="6"
          :placeholder="t('pm.requirements.createBodyPlaceholder')"
        />
      </label>
    </form>
    <template #footer>
      <Button variant="ghost" size="sm" @click="emit('update:open', false)">
        {{ t("common.cancel") }}
      </Button>
      <Button
        variant="primary"
        size="sm"
        :disabled="!canSubmit"
        @click="onCreate"
      >
        {{ t("pm.requirements.createSubmit") }}
      </Button>
    </template>
  </Dialog>
</template>

<style scoped>
.req-create {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.req-create__field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.req-create__field-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-3);
}

.req-create__label {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}
</style>
