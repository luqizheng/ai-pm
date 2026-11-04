<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/UserMenu/AccountMenu" node=33:87793 date=2026-11-04 -->
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger
} from "reka-ui";

import Avatar from "@/components/ui/avatar/Avatar.vue";
import UserMenuButton from "./UserMenuButton.vue";
import UserMenuAppearance from "./UserMenuAppearance.vue";
import UserMenuItem from "./UserMenuItem.vue";
import { currentTheme } from "@/lib/theme";

/**
 * 账户菜单（Figma Global/UserMenu/AccountMenu 33:87793，448 面板）。
 *
 * 身份块（Avatar 36 + 姓名 15 SB + 邮箱 + 组织/项目角色行）+ 分隔线 +
 * 七项操作 + 退出登录（Danger）。
 * 「外观」项在面板内切换到 Appearance 子面板（返回项回主菜单）；
 * 其余项经 select(id) 交调用方路由；sign-out / shortcuts 专属事件。
 * 组织行遵循 Shell V1.1：仅上下文展示，**不含租户管理入口**。
 */
interface Props {
  name: string;
  email: string;
  initials?: string;
  tenant: string;
  projectRole: string;
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  initials: undefined,
  disabled: false
});

const emit = defineEmits<{
  /** 普通菜单项（my-profile / account-settings / security / personal-preferences / help-documentation） */
  select: [id: string];
  shortcuts: [];
  "sign-out": [];
}>();

const { t } = useI18n();

const open = ref(false);
/** 面板视图：主菜单 / 外观子面板 */
const view = ref<"menu" | "appearance">("menu");
const theme = ref<"dark" | "light">(currentTheme());

watch(open, value => {
  if (!value) view.value = "menu";
});

const menuItems = computed(() => [
  { id: "my-profile", label: t("global.usermenu.myProfile") },
  { id: "account-settings", label: t("global.usermenu.accountSettings") },
  { id: "security", label: t("global.usermenu.security") },
  {
    id: "personal-preferences",
    label: t("global.usermenu.personalPreferences")
  }
]);

function onItem(id: string) {
  open.value = false;
  emit("select", id);
}

function onShortcuts() {
  open.value = false;
  emit("shortcuts");
}

function onSignOut() {
  open.value = false;
  emit("sign-out");
}
</script>

<template>
  <PopoverRoot v-model:open="open">
    <PopoverTrigger as-child>
      <UserMenuButton
        :name="props.name"
        :initials="props.initials"
        :active="open"
        :disabled="props.disabled"
      />
    </PopoverTrigger>

    <PopoverPortal>
      <PopoverContent
        class="user-menu"
        side="bottom"
        align="end"
        :side-offset="4"
      >
        <!-- 主菜单 -->
        <template v-if="view === 'menu'">
          <div class="user-menu__identity">
            <Avatar :name="props.name" :label="props.name">
              <template v-if="props.initials" #fallback>
                <span class="user-menu__initials">{{ props.initials }}</span>
              </template>
            </Avatar>
            <div class="user-menu__who">
              <p class="user-menu__name">{{ props.name }}</p>
              <p class="user-menu__email">{{ props.email }}</p>
            </div>
          </div>
          <p class="user-menu__context">
            {{ t("global.usermenu.tenantLabel", { tenant: props.tenant }) }}
          </p>
          <p class="user-menu__context">{{ props.projectRole }}</p>

          <div class="user-menu__divider" aria-hidden="true" />

          <nav
            class="user-menu__items"
            role="menu"
            :aria-label="t('global.usermenu.label')"
          >
            <UserMenuItem
              v-for="item in menuItems"
              :key="item.id"
              :label="item.label"
              @click="onItem(item.id)"
            />
            <UserMenuItem
              :label="t('global.usermenu.appearance')"
              @click="view = 'appearance'"
            />
            <UserMenuItem
              :label="t('global.usermenu.keyboardShortcuts')"
              @click="onShortcuts"
            />
            <UserMenuItem
              :label="t('global.usermenu.helpDocumentation')"
              @click="onItem('help-documentation')"
            />
            <UserMenuItem
              :label="t('global.usermenu.signOut')"
              variant="danger"
              @click="onSignOut"
            />
          </nav>
        </template>

        <!-- 外观子面板（返回项回主菜单；Figma 未画返回，实现补齐） -->
        <template v-else>
          <UserMenuItem
            :label="t('global.usermenu.back')"
            @click="view = 'menu'"
          />
          <UserMenuAppearance v-model="theme" />
        </template>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<style scoped>
:deep(.user-menu),
.user-menu {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  width: var(--size-user-menu-width);
  max-height: 70vh;
  overflow-y: auto;
  padding: var(--space-2);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  z-index: 50;
}

.user-menu__identity {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.user-menu__initials {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--weight-medium);
}

.user-menu__who {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.user-menu__name {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.user-menu__email {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.user-menu__context {
  margin: 0;
  padding: 0 var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.user-menu__divider {
  height: var(--hairline);
  background: var(--ws-border);
}

.user-menu__items {
  display: flex;
  flex-direction: column;
}
</style>
