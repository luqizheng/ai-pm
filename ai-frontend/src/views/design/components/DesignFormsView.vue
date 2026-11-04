<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";

import { SearchInput } from "@/components/ui/search-input";
import { Radio } from "@/components/ui/radio";
import { DatePicker } from "@/components/ui/date-picker";
import { Checkbox } from "@/components/ui/checkbox";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { Select } from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

/** /design/components/forms —— 表单族全件（含本轮新同步 Search/Radio/DatePicker） */
const { t } = useI18n();

const radio = ref("a");
const radioModel = ref("b");
const checked = ref(true);
const switched = ref(false);
const searchText = ref("design");
const dateValue = ref("Nov 18, 2026");
const dateOpen = ref(false);
const text = ref("");
const selected = ref("");
const input = ref("");

const selectOptions = [
  { value: "members", label: "Only members" },
  { value: "workspace", label: "Anyone in workspace" },
  { value: "public", label: "Public" }
];
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">
        {{ t("design.components.forms.title") }}
      </h1>
      <p class="design-page__subtitle">
        {{ t("design.components.forms.subtitle") }}
      </p>
    </template>

    <section class="design-page__group">
      <h2>SearchInput (13:32905)</h2>
      <div class="design-page__row">
        <Specimen label="State=Default">
          <SearchInput
            v-model="searchText"
            :shortcut="''"
            style="width: 280px"
          />
        </Specimen>
        <Specimen label="With shortcut">
          <SearchInput
            model-value=""
            placeholder="Search items…"
            style="width: 280px"
          />
        </Specimen>
        <Specimen label="State=Empty">
          <SearchInput
            model-value="design"
            empty-text="No results for “design”. Try another term."
            style="width: 280px"
          />
        </Specimen>
        <Specimen label="State=Loading">
          <SearchInput loading style="width: 280px" />
        </Specimen>
        <Specimen label="State=Disabled">
          <SearchInput disabled style="width: 280px" />
        </Specimen>
        <Specimen
          v-for="s in ['sm', 'md', 'lg'] as const"
          :key="s"
          :label="`Size=${s}`"
        >
          <SearchInput :size="s" placeholder="Search…" style="width: 240px" />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>Radio (13:33076) — Label={{ JSON.stringify(radioModel) }}</h2>
      <div class="design-page__row">
        <Specimen label="State=Default">
          <Radio v-model="radio" label="Only assigned to me" value="a" />
        </Specimen>
        <Specimen label="State=Checked">
          <Radio v-model="radioModel" label="Assigned to my group" value="b" />
        </Specimen>
        <Specimen label="State=Disabled">
          <Radio label="Locked option" value="c" disabled />
        </Specimen>
        <Specimen
          v-for="s in ['sm', 'md', 'lg'] as const"
          :key="s"
          :label="`Size=${s}`"
        >
          <Radio :size="s" label="Size demo" :value="s" :model-value="s" />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>DatePicker (13:33274)</h2>
      <div class="design-page__row">
        <Specimen label="State=Default">
          <DatePicker v-model="dateValue" style="width: 320px" />
        </Specimen>
        <Specimen label="State=Open (click to toggle)">
          <DatePicker
            v-model:open="dateOpen"
            v-model="dateValue"
            style="width: 320px"
          />
        </Specimen>
        <Specimen label="State=Error">
          <DatePicker
            model-value="Nov 18, 2026"
            error="Choose a date after today."
            style="width: 320px"
          />
        </Specimen>
        <Specimen label="State=Disabled">
          <DatePicker disabled style="width: 320px" />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>
        TextInput / Textarea / Select / Checkbox / Switch (already synced)
      </h2>
      <div class="design-page__row">
        <Specimen label="TextInput">
          <Input
            v-model="input"
            placeholder="Enter a name"
            style="width: 280px"
          />
        </Specimen>
        <Specimen label="Textarea">
          <Textarea
            v-model="text"
            placeholder="Add a short description…"
            style="width: 280px"
          />
        </Specimen>
        <Specimen label="Select">
          <Select
            v-model="selected"
            :options="selectOptions"
            placeholder="Choose visibility"
            style="width: 280px"
          />
        </Specimen>
        <Specimen label="Checkbox">
          <Checkbox
            v-model="checked"
            label="Include archived items"
            name="archived"
          />
        </Specimen>
        <Specimen label="Switch">
          <Switch v-model="switched" label="Notifications" />
        </Specimen>
      </div>
    </section>
  </DesignLayout>
</template>

<style scoped>
.design-page__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.design-page__subtitle {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.design-page__group {
  margin: 0 0 var(--space-8);
}

.design-page__group h2 {
  margin: 0 0 var(--space-3);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.design-page__row {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-4);
}
</style>
