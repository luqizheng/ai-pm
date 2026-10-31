<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { FormField } from "@/components/ui/form-field";
import {
  MarkdownEditor,
  MarkdownViewer,
  type AttachmentUploader,
  type FileUrlResolver
} from "@/components/ui/markdown-editor";
import {
  resolveAttachmentUrl,
  uploadAttachment
} from "@/shell/api/attachments";

/**
 * 业务侧的 Markdown / 富文本字段（公共组件，新增录入场景一律用它）。
 *
 * 与 `components/ui/markdown-editor` 的分工：
 * - 底座组件只认端口（uploader / resolveUrl），不碰 shell 层，便于替换存储后端；
 * - 本组件把端口默认接到 `file-services`，调用方只给一个 `v-model` 即可，
 *   不需要知道预签名、multipart、引用约定这些细节。
 *
 * 正文以 Markdown 字符串保存（`v-model`），附件以 `/v1/file/{id}` 引用，
 * 渲染时才换临时地址——正文里不会出现会过期的链接。
 */

interface Props {
  /** Markdown 正文（v-model） */
  modelValue?: string;
  /** edit = 编辑态；view = 只读详情（附件渲染为可下载卡片） */
  mode?: "edit" | "view";
  /** 字段名（传入已翻译文案） */
  label?: string;
  /** 必填：显示装饰性星号 */
  required?: boolean;
  /** 说明文案；与 error 同时存在时以 error 为准 */
  description?: string;
  /** 错误文案；非空即进入错误态 */
  error?: string;
  /** 空内容提示语，缺省取 i18n 文案 */
  placeholder?: string;
  /** 编辑器总高，数字按 px 处理 */
  height?: number | string;
  /** 编辑器最小高度，数字按 px 处理 */
  minHeight?: number;
  /** 可上传类型，同 input accept */
  accept?: string;
  /** 单文件上限（字节），缺省为文件服务的 multipart 上限 */
  maxSize?: number;
  /** 禁止编辑（仍展示正文） */
  disabled?: boolean;
  /** 覆盖默认上传实现；仅测试或特殊存储场景需要 */
  uploader?: AttachmentUploader;
  /** 覆盖默认地址解析实现；仅测试或特殊存储场景需要 */
  resolveUrl?: FileUrlResolver;
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: "",
  mode: "edit",
  label: undefined,
  required: false,
  description: undefined,
  error: undefined,
  placeholder: undefined,
  height: undefined,
  minHeight: undefined,
  accept: undefined,
  maxSize: undefined,
  disabled: false,
  uploader: undefined,
  resolveUrl: undefined
});

const emit = defineEmits<{
  "update:modelValue": [value: string];
  "upload-error": [message: string];
}>();

const { t } = useI18n();

/** file-services multipart 的单文件上限，超过要改走预签名分片 */
const DEFAULT_MAX_SIZE = 100 * 1024 * 1024;

const editorRef = ref<InstanceType<typeof MarkdownEditor> | null>(null);

const isView = computed(() => props.mode === "view");
const effectiveUploader = computed(() => props.uploader ?? uploadAttachment);
const effectiveResolver = computed(
  () => props.resolveUrl ?? resolveAttachmentUrl
);
const effectiveMaxSize = computed(() => props.maxSize ?? DEFAULT_MAX_SIZE);
const effectivePlaceholder = computed(
  () => props.placeholder ?? t("markdown.field.placeholder")
);

defineExpose({
  /** 取当前 Markdown 正文 */
  getValue: (): string => editorRef.value?.getValue() ?? props.modelValue,
  /** 覆盖正文并清空撤销栈 */
  setValue: (value: string): void => editorRef.value?.setValue(value),
  /** 聚焦编辑器 */
  focus: (): void => editorRef.value?.focus(),
  /** 是否仍有上传在进行 */
  isUploading: (): boolean => editorRef.value?.isUploading() ?? false
});
</script>

<template>
  <FormField
    class="md-field"
    :label="props.label"
    :required="props.required"
    :description="props.description"
    :error="props.error"
  >
    <MarkdownViewer
      v-if="isView"
      :model-value="props.modelValue"
      :resolve-url="effectiveResolver"
    >
      <template #empty>
        <p class="md-field__empty">{{ t("markdown.field.empty") }}</p>
      </template>
    </MarkdownViewer>

    <MarkdownEditor
      v-else
      ref="editorRef"
      :model-value="props.modelValue"
      :placeholder="effectivePlaceholder"
      :height="props.height"
      :min-height="props.minHeight"
      :readonly="props.disabled"
      :accept="props.accept"
      :max-size="effectiveMaxSize"
      :uploader="effectiveUploader"
      :resolve-url="effectiveResolver"
      @update:model-value="emit('update:modelValue', $event)"
      @upload-error="emit('upload-error', $event)"
    />
  </FormField>
</template>

<style scoped>
.md-field {
  width: 100%;
  min-width: 0;
}

.md-field__empty {
  margin: 0;
  padding: var(--space-6) 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-tertiary);
}
</style>
