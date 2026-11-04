<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/UserMenu/SignOutConfirmation" node=33:86201 date=2026-11-04 -->
<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  DialogClose,
  DialogContent,
  DialogOverlay,
  DialogPortal,
  DialogRoot
} from "reka-ui";
import { CircleHelp, X } from "lucide-vue-next";

import Button from "@/components/ui/button/Button.vue";
import Checkbox from "@/components/ui/checkbox/Checkbox.vue";

/**
 * 退出登录确认（Figma Global/UserMenu/SignOutConfirmation 33:86201，
 * DS/Overlays/ConfirmationDialog 13:33877）。
 *
 * 头部（circle-help + 标题 + X）/ 正文（说明 + **人工确认勾选**）/
 * 页脚（取消 + 退出登录）。
 * 红线：**确认按钮初始禁用（0.45），必须人工勾选后才可用**——
 * 不做推断退出，操作由人显式发起。
 */
interface Props {
  email?: string;
}

const props = withDefaults(defineProps<Props>(), {
  email: undefined
});

const emit = defineEmits<{
  confirm: [];
}>();

const open = defineModel<boolean>("open", { default: false });

const { t } = useI18n();

const confirmed = ref(false);

watch(open, value => {
  if (!value) confirmed.value = false;
});

function onConfirm() {
  open.value = false;
  emit("confirm");
}
</script>

<template>
  <DialogRoot v-model:open="open">
    <DialogPortal>
      <DialogOverlay class="user-signout__overlay" />
      <DialogContent
        class="user-signout"
        aria-describedby="user-signout-desc"
      >
        <div class="user-signout__header">
          <CircleHelp :size="16" aria-hidden="true" />
          <span class="user-signout__title">{{
            t("global.usermenu.signOutTitle")
          }}</span>
          <DialogClose
            class="user-signout__close"
            :aria-label="t('global.usermenu.cancel')"
          >
            <X :size="16" aria-hidden="true" />
          </DialogClose>
        </div>

        <div class="user-signout__body">
          <p id="user-signout-desc" class="user-signout__desc">
            {{ t("global.usermenu.signOutDesc", { email: props.email ?? "" }) }}
          </p>
          <Checkbox
            v-model="confirmed"
            class="user-signout__confirm"
            :label="t('global.usermenu.signOutConfirm')"
          />
        </div>

        <div class="user-signout__footer">
          <DialogClose as-child>
            <Button variant="secondary">{{ t("global.usermenu.cancel") }}</Button>
          </DialogClose>
          <Button
            variant="secondary"
            :disabled="!confirmed"
            @click="onConfirm"
          >
            {{ t("global.usermenu.signOut") }}
          </Button>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style scoped>
/* 遮罩 28% 透明做进背景色 */
.user-signout__overlay {
  position: fixed;
  inset: 0;
  z-index: 70;
  background: color-mix(in srgb, var(--ws-canvas) 28%, transparent);
}

.user-signout {
  position: fixed;
  top: 15vh;
  left: 50%;
  z-index: 71;
  display: flex;
  flex-direction: column;
  width: var(--size-user-menu-width);
  max-width: calc(100vw - var(--space-8));
  transform: translateX(-50%);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  outline: none;
}

.user-signout__header {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-4);
  color: var(--ws-text-secondary);
  border-bottom: var(--border-1) solid var(--ws-border);
}

.user-signout__title {
  flex: 1;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.user-signout__close {
  display: inline-flex;
  padding: 0;
  color: var(--ws-text-secondary);
  cursor: pointer;
  background: transparent;
  border: none;
}

.user-signout__close:hover {
  color: var(--ws-text-primary);
}

.user-signout__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
}

.user-signout__desc {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.user-signout__footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  padding: var(--space-4);
  border-top: var(--border-1) solid var(--ws-border);
}
</style>
