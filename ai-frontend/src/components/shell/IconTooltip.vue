<script setup lang="ts">
import {
  TooltipContent,
  TooltipPortal,
  TooltipProvider,
  TooltipRoot,
  TooltipTrigger
} from "reka-ui";

/**
 * 图标按钮的提示浮层（壳内专用）。
 *
 * 与 ui/tooltip 的差别：这里用 reka-ui 的 `asChild`——
 * **触发器就是调用方自己的按钮**（不额外生成 button 元素）。
 *
 * 原因：共享的 Tooltip 组件的触发器固定是 `<button>`，
 * 若 slot 里再放一个按钮/链接，就形成嵌套交互元素
 * （无效 HTML，且可访问名称重复朗读，如「通知 通知」）。
 * 壳内的导航项是链接、工具条按钮是按钮，都不能被包在 button 里。
 *
 * 视觉与 ui/tooltip 保持同一套 token。
 */

interface Props {
  /** 浮层文案（由调用方注入 i18n，组件内不硬编码中文） */
  content: string;
  /** 浮层相对触发器的位置 */
  side?: "top" | "right" | "bottom" | "left";
  /** 浮层对齐方式 */
  align?: "start" | "center" | "end";
  /** 悬停延迟（毫秒） */
  delayDuration?: number;
}

withDefaults(defineProps<Props>(), {
  side: "top",
  align: "center",
  delayDuration: 300
});
</script>

<template>
  <TooltipProvider :delay-duration="delayDuration">
    <TooltipRoot>
      <!-- asChild：触发器即 slot 内的真实元素（按钮或链接） -->
      <TooltipTrigger as-child>
        <slot />
      </TooltipTrigger>

      <TooltipPortal>
        <TooltipContent
          class="shell-tooltip__content"
          :side="side"
          :align="align"
          :side-offset="8"
        >
          <span class="shell-tooltip__label">{{ content }}</span>
        </TooltipContent>
      </TooltipPortal>
    </TooltipRoot>
  </TooltipProvider>
</template>

<style scoped>
:deep(.shell-tooltip__content) {
  display: inline-flex;
  align-items: center;
  height: var(--size-control-md);
  max-width: calc(var(--space-12) * 5);
  padding: 0 var(--space-3);
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.shell-tooltip__label {
  min-width: 0;
}
</style>
