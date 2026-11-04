<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import Vditor from "vditor";
import "vditor/dist/index.css";
import type { FileUrlResolver } from "./types";
import { hydrateFileRefs } from "./file-ref";
import { VDITOR_CDN } from "./cdn";

interface Props {
  /** Markdown 正文 */
  modelValue?: string;
  /** 图片与附件地址解析端口；不传则正文按原样渲染 */
  resolveUrl?: FileUrlResolver;
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: "",
  resolveUrl: undefined
});

const bodyRef = ref<HTMLDivElement | null>(null);
const html = ref("");

/** 渲染序号，防止切文档时旧结果覆盖新结果 */
let renderToken = 0;

async function render(): Promise<void> {
  const token = ++renderToken;
  const source = props.modelValue ?? "";
  if (!source.trim()) {
    html.value = "";
    return;
  }

  let rendered: string;
  try {
    rendered = await Vditor.md2html(source, {
      cdn: VDITOR_CDN,
      mode: "light"
    });
  } catch {
    rendered = "";
  }
  if (token !== renderToken) return;

  html.value = rendered;
  await nextTick();
  if (token !== renderToken) return;
  await hydrate();
}

async function hydrate(): Promise<void> {
  const resolve = props.resolveUrl;
  const root = bodyRef.value;
  if (!resolve || !root) return;
  await hydrateFileRefs(root, resolve);
}

watch(
  () => [props.modelValue, props.resolveUrl],
  () => {
    void render();
  },
  { immediate: true }
);

watch(
  () => props.resolveUrl,
  () => {
    void hydrate();
  }
);

onBeforeUnmount(() => {
  renderToken += 1;
});
</script>

<template>
  <div ref="bodyRef" class="md-viewer vditor-reset">
    <!-- eslint-disable-next-line vue/no-v-html -->
    <div v-if="html" class="md-viewer__body" v-html="html" />
    <slot v-else name="empty" />
  </div>
</template>

<style scoped>
.md-viewer {
  color: var(--text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  letter-spacing: var(--tracking-body);
}

.md-viewer :deep(h1) {
  font-size: var(--font-size-h2);
  line-height: var(--line-height-h2);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h2);
  margin: var(--space-6) 0 var(--space-3);
}

.md-viewer :deep(h2) {
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  margin: var(--space-6) 0 var(--space-3);
}

.md-viewer :deep(h3),
.md-viewer :deep(h4) {
  font-size: var(--font-size-h4);
  line-height: var(--line-height-h4);
  font-weight: var(--weight-medium);
  margin: var(--space-5) 0 var(--space-2);
}

.md-viewer :deep(p) {
  margin: 0 0 var(--space-3);
}

.md-viewer :deep(a) {
  color: var(--accent);
  text-decoration: underline;
  text-underline-offset: 2px;
}

.md-viewer :deep(a:focus-visible) {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 2px;
  border-radius: var(--radius-xs);
}

.md-viewer :deep(ul),
.md-viewer :deep(ol) {
  margin: 0 0 var(--space-3);
  padding-left: var(--space-6);
}

.md-viewer :deep(li) {
  margin-bottom: var(--space-1);
}

.md-viewer :deep(blockquote) {
  margin: var(--space-4) 0;
  padding-left: var(--space-4);
  color: var(--text-secondary);
  border-left: var(--size-indicator) solid var(--border-strong);
}

.md-viewer :deep(code) {
  font-family: var(--font-mono);
  font-size: var(--font-size-small);
  background: var(--surface-sunken);
  border-radius: var(--radius-xs);
  padding: 0 var(--space-1);
}

.md-viewer :deep(pre) {
  margin: 0 0 var(--space-4);
  padding: var(--space-4);
  overflow-x: auto;
  background: var(--surface-sunken);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.md-viewer :deep(pre code) {
  background: none;
  padding: 0;
}

.md-viewer :deep(img) {
  max-width: 100%;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-subtle);
}

.md-viewer :deep(img[data-file-error]) {
  padding: var(--space-4);
  background: var(--danger-subtle);
  border-color: var(--danger);
}

.md-viewer :deep(table) {
  width: 100%;
  margin: 0 0 var(--space-4);
  border-collapse: collapse;
  border: 1px solid var(--border-default);
}

.md-viewer :deep(th),
.md-viewer :deep(td) {
  padding: var(--space-2) var(--space-3);
  text-align: left;
  border: 1px solid var(--border-subtle);
}

.md-viewer :deep(th) {
  background: var(--surface-base);
  font-weight: var(--weight-medium);
}

.md-viewer :deep(hr) {
  margin: var(--space-6) 0;
  border: none;
  border-top: 1px solid var(--border-subtle);
}

/* 附件卡片：仅对已解析成临时地址的文件引用生效，外链保持行内链接外观 */
.md-viewer :deep(a[data-file-hydrated]) {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  max-width: 100%;
  padding: var(--space-2) var(--space-3);
  margin: var(--space-1) 0;
  text-decoration: none;
  color: var(--text-primary);
  background: var(--surface-base);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  transition:
    background var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard);
}

.md-viewer :deep(a[data-file-hydrated])::before {
  content: "";
  flex: none;
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  background: var(--accent);
  border-radius: var(--radius-xs);
}

.md-viewer :deep(a[data-file-hydrated]:hover) {
  background: var(--surface-hover-veil);
  border-color: var(--border-strong);
}

@media (prefers-reduced-motion: reduce) {
  .md-viewer :deep(a[data-file-hydrated]) {
    transition: none;
  }
}
</style>
