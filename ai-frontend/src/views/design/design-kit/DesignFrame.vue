<script setup lang="ts">
/**
 * DesignFrame —— 目录页里的「Figma 画框」统一容器。
 *
 * 标签 = 画框名（对应交付清单 01–16），note = 规格说明；
 * 内容为真实组件渲染，不做静态贴图。
 */
withDefaults(
  defineProps<{
    /** 画框标注，例如 03 — Recommendation / Pending */
    label?: string;
    /** 画框下的规格说明 */
    note?: string;
    /** 画框宽度档 */
    width?: "narrow" | "default" | "wide" | "full";
  }>(),
  { label: "", note: "", width: "default" }
);
</script>

<template>
  <figure class="design-frame">
    <figcaption v-if="label" class="design-frame__label">
      {{ label }}
    </figcaption>
    <div class="design-frame__stage" :class="`is-${width}`">
      <slot />
    </div>
    <p v-if="note" class="design-frame__note">{{ note }}</p>
    <figcaption v-if="$slots.caption" class="design-frame__caption">
      <slot name="caption" />
    </figcaption>
  </figure>
</template>

<style scoped>
.design-frame {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0 0 var(--space-8);
}

.design-frame__label {
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
  letter-spacing: var(--tracking-caption);
  text-transform: uppercase;
}

.design-frame__stage {
  width: 100%;
  padding: var(--space-5);
  background: var(--ws-canvas);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.design-frame__stage.is-narrow {
  max-width: var(--size-ai-card-compact-width);
}

.design-frame__stage.is-wide {
  max-width: 960px;
}

.design-frame__stage.is-full {
  max-width: none;
}

.design-frame__note {
  margin: 0;
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
  line-height: var(--line-height-micro);
}

.design-frame__caption {
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-disabled);
}
</style>
