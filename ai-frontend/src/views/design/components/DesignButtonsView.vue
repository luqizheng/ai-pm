<script setup lang="ts">
import { Ellipsis, Plus, Trash2 } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import { IconButton } from "@/components/ui/icon-button";
import { Button } from "@/components/ui/button";
import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

/** /design/components/buttons —— Button + IconButton 全变体/尺寸/状态 */
const { t } = useI18n();

const buttonVariants = [
  "primary",
  "secondary",
  "tertiary",
  "ghost",
  "danger"
] as const;

const iconVariants = [
  { variant: "secondary", icon: Ellipsis },
  { variant: "ghost", icon: Plus },
  { variant: "danger", icon: Trash2 }
] as const;

const sizes = ["sm", "md", "lg"] as const;
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">
        {{ t("design.components.buttons.title") }}
      </h1>
      <p class="design-page__subtitle">
        {{ t("design.components.buttons.subtitle") }}
      </p>
    </template>

    <section class="design-page__group">
      <h2>{{ t("design.specimen.variants") }} · Button</h2>
      <div class="design-page__row">
        <Specimen v-for="v in buttonVariants" :key="v" :label="`Variant=${v}`">
          <Button :variant="v">Save changes</Button>
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>{{ t("design.specimen.states") }} · Button (primary)</h2>
      <div class="design-page__row">
        <Specimen label="Default"><Button>Save changes</Button></Specimen>
        <Specimen label="Loading"><Button loading>Saving…</Button></Specimen>
        <Specimen label="Disabled"
          ><Button disabled>Save changes</Button></Specimen
        >
      </div>
      <p class="design-page__hint">
        {{ t("design.specimen.interactionHint") }}
      </p>
    </section>

    <section class="design-page__group">
      <h2>{{ t("design.specimen.sizes") }} · Button / IconButton</h2>
      <div class="design-page__row">
        <Specimen v-for="s in sizes" :key="s" :label="`Size=${s}`">
          <Button :size="s">Save changes</Button>
        </Specimen>
        <Specimen
          v-for="s in sizes"
          :key="`icon-${s}`"
          :label="`IconButton Size=${s}`"
        >
          <IconButton :label="`More ${s}`" :size="s">
            <Ellipsis class="design-page__icon" />
          </IconButton>
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>
        IconButton — {{ t("design.specimen.variants") }} /
        {{ t("design.specimen.states") }}
      </h2>
      <div class="design-page__row">
        <Specimen
          v-for="iv in iconVariants"
          :key="iv.variant"
          :label="`Variant=${iv.variant}`"
        >
          <IconButton :label="`Icon ${iv.variant}`">
            <component :is="iv.icon" class="design-page__icon" />
          </IconButton>
        </Specimen>
        <Specimen label="State=Loading"
          ><IconButton label="Loading" loading
        /></Specimen>
        <Specimen label="State=Disabled">
          <IconButton label="Disabled" disabled>
            <Ellipsis class="design-page__icon" />
          </IconButton>
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

.design-page__hint {
  margin: var(--space-2) 0 0;
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.design-page__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
}
</style>
