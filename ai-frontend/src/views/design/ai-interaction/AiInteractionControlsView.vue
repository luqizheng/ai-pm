<script setup lang="ts">
import { ref } from "vue";

import {
  AcceptRejectControl,
  CheckboxGroupControl,
  DecisionOptionSelector,
  DeferControl,
  DiffViewer,
  EditAcceptControl,
  FormalDecisionSubmit,
  HelpfulControl,
  InlineEditor,
  MultiSelectControl,
  RadioGroupControl,
  RatingControl,
  ReasonInputControl,
  RetryControl,
  RichTextEditorControl,
  SaveDraftControl,
  SelectDropdownControl,
  SingleSelectControl,
  SubmissionStatus,
  TextFeedbackControl
} from "@/components/ui/ai-interaction/controls";
import type {
  DecisionOption,
  QuestionOption
} from "@/components/ui/ai-interaction/types";
import {
  decisionFixture,
  proposalDiffFixture
} from "@/components/ui/ai-interaction/fixtures";
import CatalogPage from "./_shared/CatalogPage.vue";
import DesignFrame from "../design-kit/DesignFrame.vue";
import Specimen from "../design-kit/Specimen.vue";

/**
 * /design/ai-interaction/controls —— 独立控件库页。
 * 五族 20 个控件（+ 复用现有 ConfirmationDialog），逐控件给出 8 种表现态的样板。
 */
const options: QuestionOption[] = [
  { value: "basic", label: "Basic distribution" },
  {
    value: "full",
    label: "Full distribution",
    description: "Density + species"
  },
  { value: "defer", label: "Defer to next iteration" }
];

const checkboxOptions: QuestionOption[] = [
  { value: "forest", label: "Temperate forest" },
  { value: "taiga", label: "Taiga" },
  { value: "rainforest", label: "Rainforest" }
];

const single = ref("basic");
const multi = ref<string[]>(["forest"]);
const checkbox = ref<string[]>(["forest"]);
const radio = ref("basic");
const dropdown = ref("basic");
const textFeedback = ref("");
const reason = ref("");
const reasonInvalid = ref("");
const rating = ref(3);
const helpful = ref<"helpful" | "not-helpful" | null>("helpful");
const inlineText = ref("Generated requirement: world map render pipeline v2.");
const richText = ref("## Acceptance criteria\n\n- First load <= 1.2s");
const editState = ref<"editing" | "dirty" | "saving" | "saved">("dirty");
const rejectSubmitting = ref(false);
const selectedOption = ref("opt-gpu");
const decisionComment = ref("");
const draftDirty = ref(true);
const draftSavedAt = ref("");
const attempts = ref(1);

const decisionOptions: DecisionOption[] = decisionFixture.options;

function onAcceptRejectAccept() {
  rejectSubmitting.value = true;
  window.setTimeout(() => {
    rejectSubmitting.value = false;
  }, 900);
}

function onSaveDraft() {
  draftSavedAt.value = "10:42";
}
</script>

<template>
  <CatalogPage
    title-key="design.aiInteraction.controls.title"
    subtitle-key="design.aiInteraction.controls.subtitle"
  >
    <DesignFrame
      label="Selection — 5 controls"
      note="Radio / Checkbox semantics; default, selected, disabled and validation-error states."
      width="full"
    >
      <div class="controls-grid">
        <Specimen label="SingleSelect / selected">
          <SingleSelectControl
            v-model="single"
            label="First-release scope"
            :options="options"
          />
        </Specimen>
        <Specimen label="SingleSelect / invalid">
          <SingleSelectControl
            v-model="single"
            label="First-release scope"
            :options="options"
            invalid
            error-message="Select one option"
            required
          />
        </Specimen>
        <Specimen label="MultiSelect / selected">
          <MultiSelectControl
            v-model="multi"
            label="Biomes"
            :options="checkboxOptions"
          />
        </Specimen>
        <Specimen label="CheckboxGroup / disabled">
          <CheckboxGroupControl
            v-model="checkbox"
            label="Biomes"
            :options="checkboxOptions"
            disabled
          />
        </Specimen>
        <Specimen label="RadioGroup / horizontal">
          <RadioGroupControl
            v-model="radio"
            label="Scope"
            :options="options"
            orientation="horizontal"
          />
        </Specimen>
        <Specimen label="SelectDropdown / help">
          <SelectDropdownControl
            v-model="dropdown"
            label="Scope"
            :options="options"
            help-text="Used to derive the acceptance range."
          />
        </Specimen>
      </div>
    </DesignFrame>

    <DesignFrame
      label="Feedback — 4 controls"
      note="Text feedback, rejection reason, rating, helpful vote; error and disabled states."
      width="full"
    >
      <div class="controls-grid">
        <Specimen label="TextFeedback / default">
          <TextFeedbackControl
            v-model="textFeedback"
            label="Additional context"
            placeholder="Optional"
            :max-length="240"
          />
        </Specimen>
        <Specimen label="ReasonInput / required + invalid">
          <ReasonInputControl
            v-model="reasonInvalid"
            label="Rejection reason"
            placeholder="Why are you rejecting?"
            required
            invalid
            error-message="A reason is required"
          />
        </Specimen>
        <Specimen label="ReasonInput / filled">
          <ReasonInputControl
            v-model="reason"
            label="Rejection reason"
            placeholder="Why are you rejecting?"
          />
        </Specimen>
        <Specimen label="Rating / 3 of 5">
          <RatingControl v-model="rating" :max="5" />
        </Specimen>
        <Specimen label="Helpful / selected">
          <HelpfulControl v-model="helpful" />
        </Specimen>
        <Specimen label="Helpful / disabled">
          <HelpfulControl :model-value="null" disabled />
        </Specimen>
      </div>
    </DesignFrame>

    <DesignFrame
      label="Content editing — 4 controls"
      note="Inline editing, rich text, unified diff, edit accept; diff uses semantic add/remove colors."
      width="full"
    >
      <div class="controls-grid">
        <Specimen label="InlineEditor / read">
          <InlineEditor v-model="inlineText" :editing="false" />
        </Specimen>
        <Specimen label="InlineEditor / editing">
          <InlineEditor v-model="inlineText" :editing="true" />
        </Specimen>
        <Specimen label="EditAccept / dirty">
          <EditAcceptControl :state="editState" />
        </Specimen>
        <Specimen label="EditAccept / saved">
          <EditAcceptControl state="saved" />
        </Specimen>
      </div>
      <div class="controls-grid controls-grid--single">
        <Specimen label="DiffViewer / unified diff">
          <DiffViewer :diff="proposalDiffFixture" />
        </Specimen>
        <Specimen label="RichTextEditor / markdown">
          <RichTextEditorControl v-model="richText" :min-height="140" />
        </Specimen>
      </div>
    </DesignFrame>

    <DesignFrame
      label="Decision — 3 controls (+ ConfirmationDialog)"
      note="Accept/reject, option selection, formal decision submit; the confirmation dialog reuses the existing component."
      width="full"
    >
      <div class="controls-grid controls-grid--single">
        <Specimen label="AcceptReject / default">
          <AcceptRejectControl
            :submitting="rejectSubmitting"
            @accept="onAcceptRejectAccept"
          />
        </Specimen>
        <Specimen label="DecisionOptionSelector / AI recommended">
          <DecisionOptionSelector
            v-model="selectedOption"
            :options="decisionOptions"
          />
        </Specimen>
        <Specimen label="FormalDecisionSubmit / require confirm">
          <FormalDecisionSubmit
            v-model:comment="decisionComment"
            :selected-option="selectedOption"
            require-confirm
          />
        </Specimen>
      </div>
    </DesignFrame>

    <DesignFrame
      label="Workflow — 4 controls"
      note="Defer, save draft, retry, submission status; submitting / success / error states."
      width="full"
    >
      <div class="controls-grid">
        <Specimen label="Defer">
          <DeferControl />
        </Specimen>
        <Specimen label="SaveDraft / dirty">
          <SaveDraftControl :dirty="draftDirty" @save="onSaveDraft" />
        </Specimen>
        <Specimen label="SaveDraft / saved">
          <SaveDraftControl :dirty="false" :saved-at="draftSavedAt" />
        </Specimen>
        <Specimen label="Retry / attempt 2">
          <RetryControl :attempts="attempts" :max-attempts="3" />
        </Specimen>
      </div>
      <div class="controls-grid controls-grid--single">
        <Specimen label="SubmissionStatus / submitting">
          <SubmissionStatus phase="submitting" />
        </Specimen>
        <Specimen label="SubmissionStatus / success">
          <SubmissionStatus
            phase="success"
            :result="{ recorded: true, reference: 'INT-2041' }"
          />
        </Specimen>
        <Specimen label="SubmissionStatus / error">
          <SubmissionStatus
            phase="error"
            error-message="Submission failed, please retry"
          />
        </Specimen>
      </div>
      <p class="controls-state-hint">
        Hover and focus states are live: tab through the controls for the focus
        ring, hover rows for the veil, and pick options to see the selected
        state. Disabled, validation-error, submitting and submitted states are
        shown above.
      </p>
    </DesignFrame>
  </CatalogPage>
</template>

<style scoped>
.controls-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: var(--space-4);
  width: 100%;
}

.controls-grid + .controls-grid {
  margin-top: var(--space-4);
}

.controls-grid--single {
  grid-template-columns: minmax(0, 1fr);
}

.controls-state-hint {
  margin: var(--space-4) 0 0;
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
  line-height: var(--line-height-micro);
}
</style>
