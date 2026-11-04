<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { Archive } from "lucide-vue-next";

import { ConfirmationDialog } from "@/components/ui/confirmation-dialog";
import { Dialog } from "@/components/ui/dialog";
import { Popover } from "@/components/ui/popover";
import { DropdownMenu } from "@/components/ui/dropdown-menu";
import { Command } from "@/components/ui/command";
import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

/** /design/components/overlays —— DS/Overlays 族样板（既有件重同步 + 新增 ConfirmationDialog） */
const { t } = useI18n();

const confirmOpen = ref(true);
const dialogOpen = ref(true);
const popoverOpen = ref(true);
const dropdownOpen = ref(true);
const commandOpen = ref(true);
const commandItems = [
  { id: "projects", label: "Go to Projects" },
  { id: "repositories", label: "Go to Repositories" },
  { id: "new", label: "Create project" }
];
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">
        {{ t("design.components.overlays.title") }}
      </h1>
      <p class="design-page__subtitle">
        {{ t("design.components.overlays.subtitle") }}
      </p>
    </template>

    <section class="design-page__group">
      <h2>{{ t("design.specimen.states") }}</h2>
      <p class="design-page__hint">
        {{ t("design.specimen.interactionHint") }}
      </p>
      <div class="design-page__row">
        <button
          type="button"
          class="design-page__trigger"
          @click="confirmOpen = !confirmOpen"
        >
          ConfirmationDialog (open={{ confirmOpen }})
        </button>
        <button
          type="button"
          class="design-page__trigger"
          @click="dialogOpen = !dialogOpen"
        >
          Dialog / Modal (open={{ dialogOpen }})
        </button>
        <button
          type="button"
          class="design-page__trigger"
          @click="popoverOpen = !popoverOpen"
        >
          Popover (open={{ popoverOpen }})
        </button>
        <button
          type="button"
          class="design-page__trigger"
          @click="dropdownOpen = !dropdownOpen"
        >
          DropdownMenu (open={{ dropdownOpen }})
        </button>
        <button
          type="button"
          class="design-page__trigger"
          @click="commandOpen = !commandOpen"
        >
          CommandPalette (open={{ commandOpen }})
        </button>
      </div>
    </section>

    <section class="design-page__group">
      <h2>ConfirmationDialog (13:33877) — Variant=Default</h2>
      <div class="design-page__row">
        <Specimen label="Size=md · State=Default" width="448px">
          <div class="design-page__panel">
            <p class="design-page__panel-title">Archive this item?</p>
            <p class="design-page__panel-body">
              The item will be hidden from active lists. You can restore it
              later.
            </p>
          </div>
        </Specimen>
      </div>
    </section>

    <!-- 活浮层（Teleport 到 body） -->
    <ConfirmationDialog
      v-model:open="confirmOpen"
      title="Archive this item?"
      description="The item will be hidden from active lists. You can restore it later."
      confirm-label="Archive item"
      cancel-label="Cancel"
      checkbox-label="I have reviewed this change."
      :icon="Archive"
      @confirm="confirmOpen = false"
    />
    <Dialog
      v-model:open="dialogOpen"
      title="Modal title"
      description="Modal body copy goes here."
    >
      <p class="design-page__panel-body">Modal content slot.</p>
    </Dialog>
    <Popover title="Workspace context">
      <button type="button" class="design-page__trigger">Open popover</button>
      <template #content>
        <p class="design-page__panel-body">Popover content slot.</p>
      </template>
    </Popover>
    <DropdownMenu
      :items="[
        { label: 'Duplicate', value: 'duplicate' },
        { label: 'Delete', value: 'delete', danger: true }
      ]"
      aria-label="Item actions"
    >
      <button type="button" class="design-page__trigger">Open menu</button>
    </DropdownMenu>
    <Command v-model:open="commandOpen" :items="commandItems" />
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
  align-items: flex-start;
  gap: var(--space-4);
}

.design-page__trigger {
  height: var(--size-control-md);
  padding: 0 var(--space-3);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  background-color: var(--ws-surface);
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-body);
  color: var(--ws-text-primary);
  cursor: pointer;
}

.design-page__trigger:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: 0;
}

.design-page__panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.design-page__panel-title {
  margin: 0;
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.design-page__panel-body {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}
</style>
