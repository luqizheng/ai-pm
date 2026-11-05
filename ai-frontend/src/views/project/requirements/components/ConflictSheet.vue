<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Sheet from "@/components/ui/sheet/Sheet.vue";
import type { RequirementEntry } from "../requirement-entry";
import { conflictResolutionOptions, sortAnalysis } from "../fixtures";
import type { ConflictResolutionOption } from "../fixtures";

/**
 * 冲突解决抽屉（S02-UI-01 §11 状态 6：冲突态）。
 *
 * 列出该需求的冲突类分析发现，人工选择处理方式后显式应用：
 * 应用只是把需求带回修订流程（draft），冲突解决本身不等于确认正式需求。
 */
interface Props {
  open: boolean;
  entry: RequirementEntry | null;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
  (e: "apply", option: ConflictResolutionOption): void;
}>();

const { t } = useI18n();

const selected = ref<ConflictResolutionOption>("scope_limit");

watch(
  () => [props.open, props.entry?.id] as const,
  ([open]) => {
    if (open) selected.value = "scope_limit";
  },
  { immediate: true }
);

const conflicts = computed(() =>
  props.entry
    ? sortAnalysis(props.entry.analysis).filter(
        finding => finding.kind === "conflict"
      )
    : []
);

const optionLabels = computed<Record<ConflictResolutionOption, string>>(() => ({
  scope_limit: t("pm.requirements.conflictOptionScope"),
  revise_draft: t("pm.requirements.conflictOptionRevise"),
  defer: t("pm.requirements.conflictOptionDefer")
}));

const optionHints = computed<Record<ConflictResolutionOption, string>>(() => ({
  scope_limit: t("pm.requirements.conflictHintScope"),
  revise_draft: t("pm.requirements.conflictHintRevise"),
  defer: t("pm.requirements.conflictHintDefer")
}));

function onApply() {
  emit("apply", selected.value);
}
</script>

<template>
  <Sheet
    :open="props.open"
    :title="t('pm.requirements.conflictTitle')"
    :description="props.entry ? `${props.entry.code} ${props.entry.title}` : ''"
    side="right"
    @update:open="emit('update:open', $event)"
  >
    <div v-if="props.entry" class="req-conflict">
      <ul class="req-conflict__findings">
        <li
          v-for="finding in conflicts"
          :key="finding.id"
          class="req-conflict__finding"
        >
          <div class="req-conflict__finding-head">
            <Badge variant="danger" size="sm">
              {{ t("pm.requirements.analysisConflict") }}
            </Badge>
            <Badge variant="neutral" size="sm">
              {{
                t(
                  `pm.requirements.severity${finding.severity.charAt(0).toUpperCase()}${finding.severity.slice(1)}`
                )
              }}
            </Badge>
          </div>
          <p class="req-conflict__finding-body">{{ finding.body }}</p>
          <span v-if="finding.related" class="req-conflict__finding-related">
            {{
              t("pm.requirements.analysisRelated", { target: finding.related })
            }}
          </span>
        </li>
      </ul>

      <fieldset class="req-conflict__options">
        <legend class="req-conflict__options-title">
          {{ t("pm.requirements.conflictChoose") }}
        </legend>
        <label
          v-for="option in conflictResolutionOptions"
          :key="option"
          class="req-conflict__option"
          :class="{ 'req-conflict__option--selected': selected === option }"
        >
          <input
            v-model="selected"
            type="radio"
            name="conflict-resolution"
            :value="option"
            class="req-conflict__radio"
          />
          <span class="req-conflict__option-body">
            <span class="req-conflict__option-label">{{
              optionLabels[option]
            }}</span>
            <span class="req-conflict__option-hint">{{
              optionHints[option]
            }}</span>
          </span>
        </label>
      </fieldset>

      <p class="req-conflict__note">{{ t("pm.requirements.conflictNote") }}</p>

      <div class="req-conflict__footer">
        <Button variant="primary" size="sm" @click="onApply">
          {{ t("pm.requirements.conflictApply") }}
        </Button>
        <Button variant="ghost" size="sm" @click="emit('update:open', false)">
          {{ t("pm.requirements.historyClose") }}
        </Button>
      </div>
    </div>
  </Sheet>
</template>

<style scoped>
.req-conflict {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.req-conflict__findings {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.req-conflict__finding {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: var(--space-3);
}

.req-conflict__finding-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.req-conflict__finding-body {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.req-conflict__finding-related {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.req-conflict__options {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  border: 0;
}

.req-conflict__options-title {
  padding: 0;
  margin-bottom: var(--space-1);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.req-conflict__option {
  display: flex;
  gap: var(--space-2);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: var(--space-2) var(--space-3);
  cursor: pointer;
  transition: border-color var(--duration-fast) var(--ease-standard);
}

.req-conflict__option:hover {
  border-color: var(--border-hover-strong);
}

.req-conflict__option--selected {
  border-color: var(--accent);
}

.req-conflict__option:focus-within {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.req-conflict__radio {
  margin-top: 2px;
  accent-color: var(--accent);
}

.req-conflict__option-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.req-conflict__option-label {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.req-conflict__option-hint {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.req-conflict__note {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.req-conflict__footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}
</style>
