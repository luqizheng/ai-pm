<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Ban,
  ClipboardCheck,
  FileQuestion,
  ShieldAlert,
  ShieldCheck
} from "lucide-vue-next";
import type { Component } from "vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import type { RequirementEntry } from "../../requirements/requirement-entry";
import type { BaselineIssueKind, BaselineRecord } from "../baseline-contract";
import { revisionCriteriaOf } from "../fixtures";

/**
 * 准备新基线向导（S02-UI-06 §6 + Deliverables 3/4）。
 *
 * 五步：① 选择含入需求 → ② 审查需求修订 → ③ 检查未解决问题 →
 * ④ 审查基线摘要 → ⑤ 人类确认。
 * 红线：draft / pending_confirmation / conflict / 验收标准不完整的需求
 * 一律锁定并明确标识，不得静默含入；「建立基线」只发出确认请求，
 * 最终落地由页根的显式确认弹窗完成，绝不自动建立基线。
 */
interface Props {
  requirements: RequirementEntry[];
  currentBaseline: BaselineRecord | null;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "cancel"): void;
  (e: "confirm", codes: string[]): void;
}>();

const { t } = useI18n();

const STEP_COUNT = 5;
const step = ref(1);
const pickedCodes = ref<string[]>(
  props.requirements
    .filter(item => item.status === "confirmed")
    .map(item => item.code)
);

const eligible = computed(() =>
  props.requirements.filter(item => item.status === "confirmed")
);

const issueGroups = computed(() => {
  const groups: Array<{ kind: BaselineIssueKind; items: RequirementEntry[] }> =
    [
      { kind: "draft", items: [] },
      { kind: "pending_confirmation", items: [] },
      { kind: "conflict", items: [] },
      { kind: "incomplete_criteria", items: [] }
    ];
  for (const item of props.requirements) {
    if (item.status === "draft") groups[0].items.push(item);
    else if (item.status === "pending_confirmation") groups[1].items.push(item);
    else if (item.status === "conflict") groups[2].items.push(item);
    if (item.acceptance_criteria.length === 0) groups[3].items.push(item);
  }
  return groups;
});

const issueMeta: Record<BaselineIssueKind, { icon: Component; key: string }> = {
  draft: { icon: FileQuestion, key: "issueDraft" },
  pending_confirmation: { icon: ShieldAlert, key: "issuePending" },
  conflict: { icon: ShieldAlert, key: "issueConflict" },
  incomplete_criteria: { icon: Ban, key: "issueIncomplete" }
};

const stepTitles = computed(() => [
  t("pm.requirementBaseline.stepSelect"),
  t("pm.requirementBaseline.stepRevisions"),
  t("pm.requirementBaseline.stepIssues"),
  t("pm.requirementBaseline.stepSummary"),
  t("pm.requirementBaseline.stepConfirm")
]);

const pickedEntries = computed(() =>
  eligible.value.filter(item => pickedCodes.value.includes(item.code))
);

const newVersion = computed(() => (props.currentBaseline?.version ?? 0) + 1);

const functionalCount = computed(
  () => pickedEntries.value.filter(item => item.type === "functional").length
);
const nonFunctionalCount = computed(
  () =>
    pickedEntries.value.filter(item => item.type === "non_functional").length
);

/** 勾选需求的含入修订（confirmed 需求的当前 approved 修订） */
const pickedRevisions = computed(() =>
  pickedEntries.value.map(item => {
    const approved = [...item.revisions]
      .filter(revision => revision.state === "approved")
      .sort((a, b) => b.version - a.version)[0];
    return {
      code: item.code,
      title: item.title,
      version: approved?.version ?? 1,
      state: approved?.state ?? "approved",
      criteria: approved?.criteria ?? item.acceptance_criteria
    };
  })
);

const canFinish = computed(() => pickedCodes.value.length > 0);

function togglePick(code: string) {
  if (pickedCodes.value.includes(code)) {
    pickedCodes.value = pickedCodes.value.filter(item => item !== code);
  } else {
    pickedCodes.value = [...pickedCodes.value, code];
  }
}

function goNext() {
  if (step.value < STEP_COUNT) step.value += 1;
}

function goPrev() {
  if (step.value > 1) step.value -= 1;
}

function onFinish() {
  if (!canFinish.value) return;
  emit("confirm", [...pickedCodes.value]);
}
</script>

<template>
  <section class="rbw" :aria-label="t('pm.requirementBaseline.wizardTitle')">
    <div class="rbw__head">
      <h3 class="rbw__title">{{ t("pm.requirementBaseline.wizardTitle") }}</h3>
      <span class="rbw__step-indicator">
        {{
          t("pm.requirementBaseline.wizardStep", {
            current: step,
            total: STEP_COUNT
          })
        }}
        · {{ stepTitles[step - 1] }}
      </span>
    </div>

    <ol class="rbw__steps" role="list">
      <li
        v-for="(title, index) in stepTitles"
        :key="title"
        class="rbw__step"
        :class="{
          'rbw__step--done': index + 1 < step,
          'rbw__step--current': index + 1 === step
        }"
        :aria-current="index + 1 === step ? 'step' : undefined"
      >
        <span class="rbw__step-num">{{ index + 1 }}</span>
        <span class="rbw__step-title">{{ title }}</span>
      </li>
    </ol>

    <!-- Step 1：选择含入需求 -->
    <div v-if="step === 1" class="rbw__panel">
      <ul class="rbw__pick-list">
        <li v-for="item in eligible" :key="item.id">
          <label class="rbw__pick">
            <input
              type="checkbox"
              :checked="pickedCodes.includes(item.code)"
              class="rbw__checkbox"
              @change="togglePick(item.code)"
            />
            <span class="rbw__pick-code">{{ item.code }}</span>
            <span class="rbw__pick-title">{{ item.title }}</span>
          </label>
        </li>
      </ul>
      <div class="rbw__locked">
        <div class="rbw__locked-head">
          <ShieldAlert class="rbw__locked-icon" aria-hidden="true" />
          <span>{{ t("pm.requirementBaseline.lockedTitle") }}</span>
        </div>
        <ul class="rbw__locked-list">
          <li
            v-for="group in issueGroups.filter(group => group.items.length > 0)"
            :key="group.kind"
            class="rbw__locked-group"
          >
            <Badge variant="danger" size="sm" outline>
              {{ t(`pm.requirementBaseline.${issueMeta[group.kind].key}`) }}
            </Badge>
            <span
              v-for="item in group.items"
              :key="`${group.kind}-${item.id}`"
              class="rbw__locked-item"
            >
              {{ item.code }} {{ item.title }}
            </span>
          </li>
          <li
            v-for="group in issueGroups.filter(
              group => group.items.length === 0
            )"
            :key="`empty-${group.kind}`"
            class="rbw__locked-group"
          >
            <Badge variant="neutral" size="sm">
              {{ t(`pm.requirementBaseline.${issueMeta[group.kind].key}`) }}
            </Badge>
            <span class="rbw__locked-item rbw__locked-item--none">
              {{ t("pm.requirementBaseline.issueNone") }}
            </span>
          </li>
        </ul>
        <p class="rbw__locked-note">
          {{ t("pm.requirementBaseline.lockedNote") }}
        </p>
      </div>
    </div>

    <!-- Step 2：审查需求修订 -->
    <div v-else-if="step === 2" class="rbw__panel">
      <ul v-if="pickedRevisions.length > 0" class="rbw__revisions">
        <li
          v-for="item in pickedRevisions"
          :key="item.code"
          class="rbw__revision"
        >
          <div class="rbw__revision-head">
            <span class="rbw__pick-code">{{ item.code }}</span>
            <span class="rbw__revision-title">{{ item.title }}</span>
            <Badge variant="success" size="sm">
              {{
                t("pm.requirements.revisionVersion", {
                  version: item.version
                })
              }}
            </Badge>
          </div>
          <ol class="rbw__revision-criteria">
            <li
              v-for="(statement, index) in item.criteria.map(c => c.statement)"
              :key="index"
            >
              {{ statement }}
            </li>
          </ol>
        </li>
      </ul>
      <p v-else class="rbw__empty">
        {{ t("pm.requirementBaseline.wizardNoPick") }}
      </p>
    </div>

    <!-- Step 3：检查未解决问题 -->
    <div v-else-if="step === 3" class="rbw__panel">
      <ul class="rbw__issues">
        <li v-for="group in issueGroups" :key="group.kind" class="rbw__issue">
          <component
            :is="issueMeta[group.kind].icon"
            class="rbw__issue-icon"
            aria-hidden="true"
          />
          <Badge
            :variant="group.items.length > 0 ? 'warning' : 'neutral'"
            size="sm"
            :outline="group.items.length === 0"
          >
            {{ t(`pm.requirementBaseline.${issueMeta[group.kind].key}`) }}
          </Badge>
          <span class="rbw__issue-count">
            {{
              t("pm.requirementBaseline.issueCount", {
                count: group.items.length
              })
            }}
          </span>
          <span class="rbw__issue-items">
            <template v-if="group.items.length > 0">
              <span
                v-for="item in group.items"
                :key="item.id"
                class="rbw__issue-item"
              >
                {{ item.code }}
              </span>
            </template>
            <span v-else class="rbw__issue-none">
              {{ t("pm.requirementBaseline.issueNone") }}
            </span>
          </span>
        </li>
      </ul>
      <p class="rbw__issue-note">{{ t("pm.requirementBaseline.issueNote") }}</p>
    </div>

    <!-- Step 4：审查基线摘要 -->
    <div v-else-if="step === 4" class="rbw__panel">
      <dl class="rbw__summary">
        <div>
          <dt>{{ t("pm.requirementBaseline.overviewVersion") }}</dt>
          <dd class="rbw__mono">
            {{
              t("pm.requirementBaseline.baselineVersion", {
                version: newVersion
              })
            }}
          </dd>
        </div>
        <div>
          <dt>{{ t("pm.requirementBaseline.overviewCount") }}</dt>
          <dd>{{ pickedEntries.length }}</dd>
        </div>
        <div>
          <dt>{{ t("pm.requirements.typeFunctional") }}</dt>
          <dd>{{ functionalCount }}</dd>
        </div>
        <div>
          <dt>{{ t("pm.requirements.typeNonFunctional") }}</dt>
          <dd>{{ nonFunctionalCount }}</dd>
        </div>
      </dl>
      <ul class="rbw__summary-list">
        <li v-for="item in pickedEntries" :key="item.id">
          <span class="rbw__pick-code">{{ item.code }}</span>
          <span>{{ item.title }}</span>
        </li>
      </ul>
    </div>

    <!-- Step 5：人类确认 -->
    <div v-else class="rbw__panel">
      <div class="rbw__confirm">
        <ClipboardCheck class="rbw__confirm-icon" aria-hidden="true" />
        <div class="rbw__confirm-body">
          <p class="rbw__confirm-text">
            {{
              t("pm.requirementBaseline.wizardConfirmText", {
                version: newVersion,
                count: pickedEntries.length
              })
            }}
          </p>
          <p class="rbw__confirm-note">
            {{ t("pm.requirementBaseline.wizardConfirmNote") }}
          </p>
        </div>
      </div>
    </div>

    <div class="rbw__footer">
      <Button v-if="step > 1" variant="secondary" size="sm" @click="goPrev">
        {{ t("pm.requirementBaseline.wizardPrev") }}
      </Button>
      <Button
        v-if="step < STEP_COUNT"
        variant="primary"
        size="sm"
        @click="goNext"
      >
        {{ t("pm.requirementBaseline.wizardNext") }}
      </Button>
      <Button
        v-else
        variant="primary"
        size="sm"
        :disabled="!canFinish"
        @click="onFinish"
      >
        <ShieldCheck aria-hidden="true" />
        {{ t("pm.requirementBaseline.wizardEstablish") }}
      </Button>
      <Button variant="ghost" size="sm" @click="emit('cancel')">
        {{ t("common.cancel") }}
      </Button>
    </div>
  </section>
</template>

<style scoped>
.rbw {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rbw__head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rbw__title {
  margin: 0;
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.rbw__step-indicator {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rbw__steps {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
  margin: 0;
  padding: 0;
  list-style: none;
}

.rbw__step {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-2);
  border-radius: var(--radius-full);
}

.rbw__step--done {
  color: var(--text-secondary);
}

.rbw__step--current {
  background: var(--ws-color-accent-surface);
}

.rbw__step-num {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-badge-sm);
  height: var(--size-badge-sm);
  border-radius: var(--radius-full);
  background: var(--surface-raised);
  font-size: var(--font-size-nano);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.rbw__step--current .rbw__step-num {
  background: var(--accent);
  color: var(--accent-contrast-text);
}

.rbw__step-title {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rbw__step--current .rbw__step-title {
  color: var(--accent);
  font-weight: var(--weight-medium);
}

.rbw__panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-width: 0;
}

.rbw__pick-list {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  overflow: hidden;
}

.rbw__pick {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: var(--size-nav-item);
  padding: 0 var(--space-3);
  cursor: pointer;
}

.rbw__pick-list li + li .rbw__pick {
  border-top: var(--hairline) solid var(--border-default);
}

.rbw__checkbox {
  accent-color: var(--accent);
}

.rbw__checkbox:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.rbw__pick-code {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.rbw__pick-title {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rbw__locked {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.rbw__locked-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.rbw__locked-icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--warning-strong);
}

.rbw__locked-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rbw__locked-group {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rbw__locked-item {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rbw__locked-item--none {
  color: var(--text-disabled);
}

.rbw__locked-note {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rbw__revisions {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rbw__revision {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: var(--space-3);
}

.rbw__revision-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rbw__revision-title {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rbw__revision-criteria {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding-left: var(--space-5);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.rbw__empty {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-disabled);
}

.rbw__issues {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rbw__issue {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: var(--space-2) var(--space-3);
}

.rbw__issue-icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--text-secondary);
}

.rbw__issue-count {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.rbw__issue-items {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.rbw__issue-item {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-primary);
}

.rbw__issue-none {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-disabled);
}

.rbw__issue-note {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rbw__summary {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-3);
  margin: 0;
}

.rbw__summary dt {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rbw__summary dd {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.rbw__mono {
  font-family: var(--font-mono);
}

.rbw__summary-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rbw__summary-list li {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
}

.rbw__confirm {
  display: flex;
  gap: var(--space-3);
  border: var(--border-1) solid var(--accent);
  border-radius: var(--radius-sm);
  background: var(--ws-color-accent-surface);
  padding: var(--space-3);
}

.rbw__confirm-icon {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
  flex: none;
  color: var(--accent);
}

.rbw__confirm-text {
  margin: 0;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-primary);
}

.rbw__confirm-note {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.rbw__footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  padding-top: var(--space-3);
  border-top: var(--hairline) solid var(--border-default);
}
</style>
