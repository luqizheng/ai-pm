<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="DS/Data/Avatar" node=13:34344 date=2026-11-04 -->
<script setup lang="ts">
import { computed } from "vue";
import { AvatarFallback, AvatarImage, AvatarRoot } from "reka-ui";
import { UserRound } from "lucide-vue-next";

interface Props {
  /** 图片地址；为空时直接渲染回退 */
  src?: string;
  /** 图片替代文本 */
  alt?: string;
  /** 用户名称：用于派生首字母回退（取首词/末词首字母，最多两位） */
  name?: string;
  /** 头像的无障碍名称（头像独立出现、旁边没有姓名文本时使用） */
  label?: string;
  /** 尺寸档 —— Figma：sm 28 / md 36 / lg 44（复用控件三档 token） */
  size?: "sm" | "md" | "lg";
  /** 在线状态：右下角 presence 徽标（accent 圆点） */
  online?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  src: undefined,
  alt: undefined,
  name: undefined,
  label: undefined,
  size: "md",
  online: false
});

/**
 * 首字母回退 —— Figma Initials 态：最多两位（如 "AM" = Alex Morgan）；
 * 单词名取首字母，多词名取首词 + 末词首字母。
 */
const initials = computed(() => {
  const words = (props.name ?? props.alt ?? "")
    .trim()
    .split(/\s+/)
    .filter(Boolean);
  if (words.length === 0) return "";
  const first = words[0]?.charAt(0) ?? "";
  const last =
    words.length > 1 ? (words[words.length - 1]?.charAt(0) ?? "") : "";
  return (first + last).toUpperCase();
});

/** 有无障碍名称时，图片本身退化为装饰，避免同一信息被朗读两次 */
const imageAlt = computed(() =>
  props.label ? "" : (props.alt ?? props.name ?? "")
);
</script>

<template>
  <AvatarRoot :class="['ui-avatar', `ui-avatar--${size}`]" :aria-label="label">
    <AvatarImage
      v-if="src"
      class="ui-avatar__image"
      :src="src"
      :alt="imageAlt"
    />
    <AvatarFallback
      class="ui-avatar__fallback"
      :aria-hidden="label ? 'true' : undefined"
    >
      <slot name="fallback">
        <span v-if="initials" class="ui-avatar__initials">{{ initials }}</span>
        <UserRound v-else class="ui-avatar__fallback-icon" aria-hidden="true" />
      </slot>
    </AvatarFallback>
    <!-- 在线徽标 —— Figma Online 态：12×12 elevated 圆 + canvas 2px 描边 + 6px accent 圆点 -->
    <span v-if="online" class="ui-avatar__presence" aria-hidden="true">
      <span class="ui-avatar__presence-dot" />
    </span>
  </AvatarRoot>
</template>

<style scoped>
/* Figma DS/Data/Avatar（13:34344）：圆形 / elevated 底 / border 1px 描边 /
 * 三档 28/36/44（复用 --size-control-sm/md/lg）/ 首字母 10/12/14 Medium 主文字。 */
.ui-avatar {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  box-sizing: border-box;
  width: var(--size-control-md);
  height: var(--size-control-md);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-full);
  overflow: visible;
  background: var(--surface-raised);
  font-family: var(--font-ui);
  user-select: none;
}

.ui-avatar--sm {
  width: var(--size-control-sm);
  height: var(--size-control-sm);
}

.ui-avatar--lg {
  width: var(--size-control-lg);
  height: var(--size-control-lg);
}

.ui-avatar__image {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: inherit;
}

/* 内层裁切：图片与回退面按圆形裁切，presence 徽标在外层不被裁掉 */
.ui-avatar__image,
.ui-avatar__fallback {
  border-radius: var(--radius-full);
}

/* 回退 — elevated 底 / 主文字 / 首字母字号随档位 */
.ui-avatar__fallback {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: var(--surface-raised);
  color: var(--text-primary);
}

.ui-avatar__initials {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-medium);
  letter-spacing: var(--tracking-caption);
}

.ui-avatar--sm .ui-avatar__initials {
  font-size: var(--font-size-nano);
  line-height: 1.2;
}

.ui-avatar--lg .ui-avatar__initials {
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

/* 无首字母时的图形回退（Figma Fallback 态：user-round 16px） */
.ui-avatar__fallback-icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--text-secondary);
}

/* 在线徽标 —— 右下角贴边 */
.ui-avatar__presence {
  position: absolute;
  right: 0;
  bottom: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  width: var(--space-3);
  height: var(--space-3);
  background: var(--surface-raised);
  border: var(--border-2) solid var(--ws-canvas);
  border-radius: var(--radius-full);
}

.ui-avatar__presence-dot {
  width: calc(var(--space-3) - var(--space-1));
  height: calc(var(--space-3) - var(--space-1));
  background: var(--accent);
  border-radius: var(--radius-full);
}
</style>
