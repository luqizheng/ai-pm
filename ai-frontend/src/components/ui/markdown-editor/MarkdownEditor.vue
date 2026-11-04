<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import Vditor from "vditor";
import "vditor/dist/index.css";
import type { AttachmentUploader, FileUrlResolver } from "./types";
import { hydrateFileRefs } from "./file-ref";
import { VDITOR_CDN } from "./cdn";
import { createUploadHandler, type UploadMessages } from "./upload-handler";

interface Props {
  /** Markdown 正文（v-model） */
  modelValue?: string;
  /** 空内容时的提示语 */
  placeholder?: string;
  /** 编辑器总高，数字按 px 处理 */
  height?: number | string;
  /** 编辑器最小高度，数字按 px 处理 */
  minHeight?: number;
  /** 仅查看，不进入编辑态 */
  readonly?: boolean;
  /** 上传端口；不传则工具栏上传与粘贴上传不可用 */
  uploader?: AttachmentUploader;
  /** 图片与附件地址解析端口；不传则正文按原样渲染 */
  resolveUrl?: FileUrlResolver;
  /** 可上传类型，同 input accept */
  accept?: string;
  /** 单文件上限（字节） */
  maxSize?: number;
  /** 工具栏最小集，默认 PRISM 富文本工具条 */
  toolbar?: string[];
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: "",
  placeholder: undefined,
  height: undefined,
  minHeight: undefined,
  readonly: false,
  uploader: undefined,
  resolveUrl: undefined,
  accept: undefined,
  maxSize: undefined,
  toolbar: undefined
});

const emit = defineEmits<{
  "update:modelValue": [value: string];
  ready: [instance: Vditor];
  "upload-error": [message: string];
}>();

const { t, locale } = useI18n();

const hostRef = ref<HTMLDivElement | null>(null);
/** Vditor 实例内部持有大量可变引用，交给 shallowRef，避免被 Vue 代理包裹 */
const instance = shallowRef<Vditor | null>(null);

const DEFAULT_TOOLBAR = [
  "headings",
  "bold",
  "italic",
  "strike",
  "link",
  "|",
  "list",
  "ordered-list",
  "check",
  "quote",
  "|",
  "code",
  "inline-code",
  "table",
  "line",
  "|",
  "upload",
  "undo",
  "redo"
];

function buildMessages(): UploadMessages {
  return {
    tooLarge: (name, limit) => t("markdown.upload.tooLarge", { name, limit }),
    rejected: name => t("markdown.upload.rejected", { name }),
    failed: name => t("markdown.upload.failed", { name })
  };
}

/** 图片与附件插入后，把引用换成临时地址 */
let hydrateTimer: ReturnType<typeof setTimeout> | null = null;
function scheduleHydrate(): void {
  const resolve = props.resolveUrl;
  if (!resolve) return;
  if (hydrateTimer) clearTimeout(hydrateTimer);
  hydrateTimer = setTimeout(() => {
    hydrateTimer = null;
    const root = hostRef.value?.querySelector<HTMLElement>(".vditor");
    if (!root) return;
    void hydrateFileRefs(root, resolve);
  }, 200);
}

function createInstance(): void {
  const host = hostRef.value;
  if (!host) return;

  const uploadHandler = props.uploader
    ? createUploadHandler({
        uploader: props.uploader,
        insert: markdown => instance.value?.insertValue(markdown, true),
        messages: buildMessages(),
        accept: props.accept,
        maxSize: props.maxSize
      })
    : undefined;

  instance.value = new Vditor(host, {
    mode: "wysiwyg",
    value: props.modelValue,
    height: props.height,
    minHeight: props.minHeight,
    placeholder: props.placeholder,
    lang: locale.value.startsWith("zh") ? "zh_CN" : "en_US",
    // 解析引擎与主题从同源加载，不使用默认的 unpkg 地址
    cdn: VDITOR_CDN,
    // 默认会往 localStorage 写草稿，与外部 v-model 抢真源，必须关闭
    cache: { enable: false },
    counter: { enable: false },
    outline: { enable: false, position: "left" },
    toolbar: props.toolbar ?? DEFAULT_TOOLBAR,
    // 内置高亮依赖外部 CDN 资源，内网环境下不可靠，交由样式层统一处理
    preview: { hljs: { enable: false } },
    upload: {
      accept: props.accept ?? "",
      multiple: true,
      handler: uploadHandler
    },
    input: value => {
      emit("update:modelValue", value);
      scheduleHydrate();
    },
    after: () => {
      emit("ready", instance.value as Vditor);
      scheduleHydrate();
    }
  });
}

onMounted(() => {
  createInstance();
});

onBeforeUnmount(() => {
  if (hydrateTimer) clearTimeout(hydrateTimer);
  instance.value?.destroy();
  instance.value = null;
});

// 外部改写正文时同步进编辑器；值等价时跳过，避免打断输入
watch(
  () => props.modelValue,
  value => {
    const current = instance.value;
    if (!current) return;
    if (current.getValue() === value) return;
    current.setValue(value ?? "");
    scheduleHydrate();
  }
);

watch(
  () => props.readonly,
  value => {
    const current = instance.value;
    if (!current) return;
    if (value) current.disabled();
    else current.enable();
  }
);

defineExpose({
  /** 取当前 Markdown 正文 */
  getValue: (): string => instance.value?.getValue() ?? "",
  /** 覆盖正文并清空撤销栈 */
  setValue: (value: string): void => instance.value?.setValue(value, true),
  /** 聚焦编辑器 */
  focus: (): void => instance.value?.focus(),
  /** 是否仍有上传在进行 */
  isUploading: (): boolean => instance.value?.isUploading() ?? false
});
</script>

<template>
  <div class="md-editor" :class="{ 'md-editor--readonly': props.readonly }">
    <div ref="hostRef" class="md-editor__host" />
  </div>
</template>

<style scoped>
.md-editor {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  background: var(--surface-raised);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  transition:
    border-color var(--duration-fast) var(--ease-standard),
    box-shadow var(--duration-fast) var(--ease-standard);
}

.md-editor:focus-within {
  border-color: var(--accent);
  box-shadow: 0 0 0 var(--size-input-halo) var(--accent-wash);
}

.md-editor__host {
  display: flex;
  flex: 1;
  min-height: 0;
}

.md-editor :deep(.vditor) {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  background: var(--surface-raised);
  border: none;
  border-radius: 0;
  font-family: var(--font-ui);
}

.md-editor :deep(.vditor-toolbar) {
  padding: 0 var(--space-2);
  background: var(--surface-base);
  border-bottom: 1px solid var(--border-subtle);
}

.md-editor :deep(.vditor-toolbar__item button) {
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
}

.md-editor :deep(.vditor-toolbar__item button:hover) {
  background: var(--surface-hover-veil);
}

.md-editor :deep(.vditor-toolbar__item button:focus-visible) {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: 1px;
}

.md-editor :deep(.vditor-content) {
  flex: 1;
  min-height: 0;
}

.md-editor :deep(.vditor-reset) {
  padding: var(--space-5) var(--space-6);
  color: var(--text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  letter-spacing: var(--tracking-body);
}

.md-editor :deep(.vditor-reset h1),
.md-editor :deep(.vditor-reset h2),
.md-editor :deep(.vditor-reset h3),
.md-editor :deep(.vditor-reset h4) {
  color: var(--text-primary);
  letter-spacing: var(--tracking-h3);
}

.md-editor :deep(.vditor-reset h1) {
  font-size: var(--font-size-h2);
  line-height: var(--line-height-h2);
  font-weight: var(--weight-semibold);
}

.md-editor :deep(.vditor-reset h2) {
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
}

.md-editor :deep(.vditor-reset h3) {
  font-size: var(--font-size-h4);
  line-height: var(--line-height-h4);
  font-weight: var(--weight-semibold);
}

.md-editor :deep(.vditor-reset a) {
  color: var(--accent);
  text-decoration: underline;
  text-underline-offset: 2px;
}

.md-editor :deep(.vditor-reset blockquote) {
  margin: var(--space-4) 0;
  padding-left: var(--space-4);
  color: var(--text-secondary);
  border-left: var(--size-indicator) solid var(--border-strong);
}

.md-editor :deep(.vditor-reset code) {
  font-family: var(--font-mono);
  font-size: var(--font-size-small);
  background: var(--surface-sunken);
  border-radius: var(--radius-xs);
  padding: 0 var(--space-1);
}

.md-editor :deep(.vditor-reset pre code) {
  display: block;
  padding: var(--space-4);
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-subtle);
}

.md-editor :deep(.vditor-reset img) {
  max-width: 100%;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-subtle);
}

.md-editor :deep(.vditor-reset img[data-file-error]) {
  padding: var(--space-4);
  background: var(--danger-subtle);
  border-color: var(--danger);
}

.md-editor :deep(.vditor-reset table) {
  border-color: var(--border-default);
}

.md-editor :deep(.vditor-reset td),
.md-editor :deep(.vditor-reset th) {
  border-color: var(--border-subtle);
  padding: var(--space-2) var(--space-3);
}

.md-editor :deep(.vditor-reset th) {
  background: var(--surface-base);
  font-weight: var(--weight-medium);
}

.md-editor :deep(.vditor-panel) {
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
  background: var(--surface-overlay);
}

.md-editor :deep(.vditor-tip) {
  border-radius: var(--radius-sm);
  box-shadow: var(--shadow-sm);
}

.md-editor--readonly :deep(.vditor-toolbar) {
  display: none;
}

@media (prefers-reduced-motion: reduce) {
  .md-editor {
    transition: none;
  }
}
</style>
