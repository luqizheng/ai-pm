<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/UserMenu/ProfilePopover" node=33:86124 date=2026-11-04 -->
<script setup lang="ts">
import { Link } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import Button from "@/components/ui/button/Button.vue";

/**
 * 用户资料浮层卡片（Figma Global/UserMenu/ProfilePopover 33:86124）。
 *
 * elevated 卡片：标题行（姓名 13 SB + 关闭）+ 多行身份信息（邮箱/组织/角色）+
 * 「我的资料」动作（link 图标）。
 * 交付为展示组件（个人资料悬停卡的落点待定），账户菜单走 AccountMenu。
 */
interface Props {
  name: string;
  lines: string[];
}

const props = defineProps<Props>();

const emit = defineEmits<{
  close: [];
  profile: [];
}>();

const { t } = useI18n();
</script>

<template>
  <section class="user-profile-popover" :aria-label="props.name">
    <div class="user-profile-popover__heading">
      <span class="user-profile-popover__name">{{ props.name }}</span>
      <button
        type="button"
        class="user-profile-popover__close"
        :aria-label="t('global.usermenu.cancel')"
        @click="emit('close')"
      >
        ×
      </button>
    </div>
    <p v-for="line in props.lines" :key="line" class="user-profile-popover__line">
      {{ line }}
    </p>
    <div>
      <Button variant="secondary" surface="elevated" @click="emit('profile')">
        <Link :size="16" aria-hidden="true" />
        {{ t("global.usermenu.myProfile") }}
      </Button>
    </div>
  </section>
</template>

<style scoped>
.user-profile-popover {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  width: var(--size-user-menu-width);
  padding: var(--space-4);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.user-profile-popover__heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.user-profile-popover__name {
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  line-height: var(--line-height-body);
}

.user-profile-popover__close {
  padding: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body-strong);
  line-height: 1;
  cursor: pointer;
  background: transparent;
  border: none;
}

.user-profile-popover__close:hover {
  color: var(--ws-text-primary);
}

.user-profile-popover__line {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  white-space: pre-line;
}
</style>
