<!-- @figma file=hNlGqLEMQpOVCZ0o5u3DkT node=5:2489 set="02 — Authentication / Workspace header + footer" date=2026-11-04 -->
<script setup lang="ts">
import { LockKeyhole } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import ProductMark from "./ProductMark.vue";

/**
 * 认证页外壳 —— Figma「02 — Authentication」九帧共用的页面骨架。
 *
 * 三段结构（每帧一致）：
 *   页头 64px（产品标识章 + 产品名 + 分隔线 + 当前区段 / 右侧安全提示）
 *   内容区（垂直居中，padding 48）
 *   页脚 64px（版本号 / 支持链接）
 *
 * 认证页不套用应用壳（WorkspaceShell）：登录类页面是无导航的全屏页。
 */
const { t } = useI18n();
</script>

<template>
  <div class="auth-layout">
    <header class="auth-layout__header">
      <div class="auth-layout__identity">
        <ProductMark />
        <span class="auth-layout__product">{{ t("auth.chrome.product") }}</span>
        <span class="auth-layout__divider" aria-hidden="true" />
        <span class="auth-layout__section">{{ t("auth.chrome.section") }}</span>
      </div>

      <p class="auth-layout__status">
        <LockKeyhole class="auth-layout__status-icon" aria-hidden="true" />
        <span>{{ t("auth.chrome.secure") }}</span>
      </p>
    </header>

    <main class="auth-layout__content">
      <slot />
    </main>

    <footer class="auth-layout__footer">
      <p class="auth-layout__version">{{ t("auth.chrome.version") }}</p>
      <nav
        class="auth-layout__links"
        :aria-label="t('auth.chrome.supportLinks')"
      >
        <button type="button" class="auth-layout__link">
          {{ t("auth.chrome.help") }}
        </button>
        <button type="button" class="auth-layout__link">
          {{ t("auth.chrome.privacy") }}
        </button>
        <button type="button" class="auth-layout__link">
          {{ t("auth.chrome.terms") }}
        </button>
      </nav>
    </footer>
  </div>
</template>

<style scoped>
/* Figma「02 — Authentication」九帧页面骨架：
 * 画布底 --ws-canvas / 页头 --ws-sidebar + 1px 下边框 / 页脚无边框；
 * 页头页脚同构：左右分布 + 32px 内边距 + 64px 行高。 */
.auth-layout {
  display: flex;
  flex-direction: column;
  min-height: 100dvh;
  background: var(--ws-canvas);
  color: var(--ws-text-primary);
  font-family: var(--font-ui);
  font-size: var(--ws-font-size-body);
}

.auth-layout__header,
.auth-layout__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  flex: none;
  height: var(--ws-auth-header-height);
  padding: 0 var(--space-8);
}

.auth-layout__header {
  background: var(--ws-sidebar);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.auth-layout__identity {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-width: 0;
}

.auth-layout__product {
  font-size: var(--font-size-h4);
  font-weight: var(--weight-semibold);
  line-height: var(--line-height-h4);
  color: var(--ws-text-primary);
}

.auth-layout__divider {
  flex: none;
  width: var(--border-1);
  height: var(--space-4);
  background: var(--ws-border);
}

.auth-layout__section {
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

/* 右侧安全提示 —— Figma：锁形图标 12px + IBM Plex Mono 11px */
.auth-layout__status {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.auth-layout__status-icon {
  width: var(--space-3);
  height: var(--space-3);
}

/* 内容区 —— 垂直居中，padding 48 */
.auth-layout__content {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-5);
  padding: var(--space-12);
}

.auth-layout__version {
  margin: 0;
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.auth-layout__links {
  display: flex;
  align-items: center;
  gap: var(--space-6);
}

/* 支持链接 —— 12px 次文字；焦点走 accent 描边 */
.auth-layout__link {
  padding: 0;
  border: 0;
  background: transparent;
  font-family: inherit;
  font-size: var(--ws-font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-secondary);
  cursor: pointer;
}

.auth-layout__link:hover {
  color: var(--ws-text-primary);
}

.auth-layout__link:focus-visible {
  outline: var(--size-focus-ring) solid var(--ws-accent);
  outline-offset: var(--space-1);
  border-radius: var(--radius-xs);
}
</style>
