<script setup lang="ts">
import Badge from "@/components/ui/badge/Badge.vue";
import IconButton from "@/components/ui/icon-button/IconButton.vue";
import { X } from "lucide-vue-next";
import { onBeforeUnmount, onMounted } from "vue";

/**
 * 审核详情 Drawer（R03）：720px 覆盖式面板，锚定主工作区右侧，
 * 覆盖主内容区 + 右上下文栏，保留左导航与 AI Chat 常驻列。
 *
 * 层级：高于项目主内容（z 60），低于全局模态确认框（ConfirmationDialog 100）。
 * 非模态（无遮罩）——队列与 AI Chat 在审核期间保持可用（V1.0 §2.3）。
 *
 * 纯展示件：文案经 props 传入，业务上下文与决策区由插槽注入（D8）。
 */
interface Props {
  open: boolean;
  title: string;
  kindLabel: string;
  stateLabel: string;
  timeLabel: string;
  hint: string;
  closeLabel: string;
  /** Drawer 右侧让位（AI Chat 开启时 = var(--ws-chat-width)，关闭时 = 0）。 */
  rightInset: string;
  busy?: boolean;
}

const props = withDefaults(defineProps<Props>(), { busy: false });
const emit = defineEmits<{ close: [] }>();

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && props.open) emit("close");
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <aside
      v-if="open"
      class="review-drawer"
      role="dialog"
      aria-modal="false"
      :aria-busy="busy"
      :data-busy="busy"
    >
      <header class="review-drawer__head">
        <div class="review-drawer__heading">
          <p class="review-drawer__title">{{ title }}</p>
          <div class="review-drawer__meta">
            <Badge variant="accent" size="sm" outline>{{ kindLabel }}</Badge>
            <Badge variant="neutral" size="sm">{{ stateLabel }}</Badge>
            <span class="review-drawer__time">{{ timeLabel }}</span>
          </div>
        </div>
        <IconButton
          class="review-drawer__close"
          variant="secondary"
          size="sm"
          :label="closeLabel"
          :disabled="busy"
          @click="emit('close')"
        >
          <X :size="16" aria-hidden="true" />
        </IconButton>
      </header>

      <p v-if="hint" class="review-drawer__hint">{{ hint }}</p>

      <div class="review-drawer__body">
        <slot name="context" />
        <slot name="decision" />
      </div>
    </aside>
  </Teleport>
</template>

<style scoped>
.review-drawer {
  position: fixed;
  top: 52px; /* 顶栏高度，Drawer 从顶栏下方开始 */
  bottom: 0;
  left: var(--ws-sidebar-current);
  right: v-bind("props.rightInset");
  z-index: 60;
  display: flex;
  flex-direction: column;
  max-width: 90vw;
  width: min(720px, 100%);
  background: var(--surface-raised);
  border-left: 1px solid var(--border-default);
  box-shadow: var(--shadow-md);
}

.review-drawer__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 20px 12px;
  border-bottom: 1px solid var(--border-subtle);
}

.review-drawer__title {
  margin: 0;
  color: var(--text-primary);
  font-size: 16px;
  font-weight: 600;
  line-height: 1.4;
}

.review-drawer__meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
}

.review-drawer__time {
  color: var(--text-secondary);
  font-size: 12px;
}

.review-drawer__hint {
  margin: 0;
  padding: 10px 20px;
  color: var(--text-secondary);
  font-size: 12px;
  border-bottom: 1px solid var(--border-subtle);
}

.review-drawer__body {
  flex: 1;
  overflow-y: auto;
  padding: 16px 20px 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
</style>
