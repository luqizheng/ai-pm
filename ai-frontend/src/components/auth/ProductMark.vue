<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2491 set="02 — Authentication / Workspace header" date=2026-11-04 -->
<script setup lang="ts">
/**
 * 品牌标识章 —— 认证页页头左上角的产品标识。
 *
 * Figma「02 — Authentication」九帧共用：28×28 章底（radius 6，accent 着色面
 * #242238）+ 18×16 字形（accent 描边 1.6）。
 *
 * 字形内联而非外链图片：描边走 currentColor，浅色主题下章底与字形保持品牌一致
 * （Figma 浅色稿：章底仍为深色，字形仍为 #8B83F6）。
 */
</script>

<template>
  <span class="product-mark" aria-hidden="true">
    <svg
      class="product-mark__glyph"
      width="18"
      height="16"
      viewBox="0 0 21 17"
      fill="none"
      focusable="false"
    >
      <path
        d="M6.95341 2.29304L1.13438 8.5L6.95341 14.707M13.9362 2.29304L19.7553 8.5L13.9362 14.707M12.7724 0.224054L8.11721 16.7759"
        stroke-width="1.65519"
        stroke="currentColor"
      />
    </svg>
  </span>
</template>

<style scoped>
.product-mark {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  border-radius: var(--radius-sm);
  background: var(--ws-brand-mark-surface);
  color: var(--ws-brand-mark-foreground);
}

.product-mark__glyph {
  display: block;
}
</style>
