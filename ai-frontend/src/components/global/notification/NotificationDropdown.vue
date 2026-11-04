<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/Notification/Dropdown" node=33:87790 date=2026-11-04 -->
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger
} from "reka-ui";
import { SearchX } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import Skeleton from "@/components/ui/skeleton/Skeleton.vue";
import { useNotificationsStore } from "@/stores/notifications";
import NotificationBell from "./NotificationBell.vue";
import NotificationRecord from "./NotificationRecord.vue";
import {
  NOTIFICATION_CATEGORIES,
  type NotificationCategory,
  type NotificationRecord as NotificationRecordType
} from "./types";

/**
 * 通知下拉（Figma Global/Notification/Dropdown 33:87790，铃铛锚定浮层）。
 *
 * 五态：Open（All/Unread 页签 + 分类分组记录）/ Unread view / Empty
 * （「已全部处理」空态）/ Loading（骨架行）/ Error（danger 反馈框 + 重试）。
 * 设计口径：**锚定的事件流，不是聊天**；头部「全部已读」+ 底部「查看全部通知」
 * 跳转通知中心。数据与顶栏未读计数同源（stores/notifications）。
 */
interface Props {
  disabled?: boolean;
}

defineProps<Props>();

const emit = defineEmits<{
  /** 打开一条通知的关联对象（target 路由） */
  open: [target: string];
  /** 打开完整通知中心 */
  "view-all": [];
}>();

const { t } = useI18n();

const store = useNotificationsStore();

const open = ref(false);
const filter = ref<"all" | "unread">("all");

watch(open, value => {
  if (value && store.status === "idle") {
    store.load();
  }
  if (!value) {
    filter.value = "all";
  }
});

const filtered = computed(() =>
  filter.value === "unread"
    ? store.records.filter(record => !record.read)
    : store.records
);

/** 按分类分组（空类不展示，顺序固定） */
const grouped = computed(() => {
  const groups: {
    category: NotificationCategory;
    items: NotificationRecordType[];
  }[] = [];
  for (const category of NOTIFICATION_CATEGORIES) {
    const items = filtered.value.filter(item => item.category === category);
    if (items.length > 0) {
      groups.push({ category, items });
    }
  }
  return groups;
});

const showEmpty = computed(
  () => store.status === "ready" && filtered.value.length === 0
);

function onToggleRead(record: NotificationRecordType) {
  if (record.read) {
    store.markUnread(record.id);
  } else {
    store.markRead(record.id);
  }
}

/** 打开对象：发出目标路由并关闭浮层 */
function onOpen(record: NotificationRecordType) {
  open.value = false;
  emit("open", record.target);
}
</script>

<template>
  <PopoverRoot v-model:open="open">
    <!-- asChild：触发器即铃铛真实按钮（含文本未读徽标） -->
    <PopoverTrigger as-child>
      <NotificationBell
        :unread-count="store.unreadCount"
        :active="open"
        :disabled="disabled"
      />
    </PopoverTrigger>

    <PopoverPortal>
      <PopoverContent
        class="notification-dropdown"
        side="bottom"
        align="end"
        :side-offset="4"
      >
        <div class="notification-dropdown__heading">
          <span class="notification-dropdown__title">{{
            t("global.notification.label")
          }}</span>
          <Button variant="ghost" size="sm" @click="store.markAllRead()">
            {{ t("global.notification.markAllRead") }}
          </Button>
        </div>

        <!-- Loading：骨架行 -->
        <template v-if="store.status === 'loading'">
          <div
            v-for="n in 2"
            :key="n"
            class="notification-dropdown__skeleton-row"
            aria-hidden="true"
          >
            <Skeleton
              class="notification-dropdown__skeleton-line"
              tone="strong"
            />
            <Skeleton
              class="notification-dropdown__skeleton-line"
              tone="base"
            />
          </div>
        </template>

        <!-- Error：反馈框 + 重试 -->
        <template v-else-if="store.status === 'error'">
          <Alert variant="danger" :title="t('global.notification.errorTitle')">
            {{ t("global.notification.errorDesc") }}
          </Alert>
          <div>
            <Button variant="secondary" size="sm" @click="store.load()">
              {{ t("global.notification.retry") }}
            </Button>
          </div>
        </template>

        <!-- Ready：All / Unread 页签 + 分类分组记录 -->
        <template v-else-if="store.status === 'ready'">
          <div class="notification-dropdown__tabs" role="tablist">
            <button
              type="button"
              role="tab"
              class="notification-dropdown__tab"
              :class="{ 'is-active': filter === 'all' }"
              :aria-selected="filter === 'all'"
              @click="filter = 'all'"
            >
              {{ t("global.notification.filterAll") }}
            </button>
            <button
              type="button"
              role="tab"
              class="notification-dropdown__tab"
              :class="{ 'is-active': filter === 'unread' }"
              :aria-selected="filter === 'unread'"
              @click="filter = 'unread'"
            >
              {{
                t("global.notification.filterUnread", {
                  count: store.unreadCount
                })
              }}
            </button>
          </div>

          <div v-if="showEmpty" class="notification-dropdown__empty">
            <SearchX :size="24" aria-hidden="true" />
            <p class="notification-dropdown__empty-title">
              {{ t("global.notification.emptyTitle") }}
            </p>
            <p class="notification-dropdown__empty-desc">
              {{ t("global.notification.emptyDesc") }}
            </p>
          </div>

          <div v-else class="notification-dropdown__records">
            <div
              v-for="group in grouped"
              :key="group.category"
              class="notification-dropdown__group"
            >
              <p class="notification-dropdown__group-label">
                {{ t(`global.notification.category.${group.category}`) }}
              </p>
              <NotificationRecord
                v-for="record in group.items"
                :key="record.id"
                :record="record"
                @open="onOpen"
                @toggle-read="onToggleRead"
              />
            </div>
          </div>

          <div class="notification-dropdown__footer">
            <Button
              variant="secondary"
              size="sm"
              @click="
                open = false;
                emit('view-all');
              "
            >
              {{ t("global.notification.viewAll") }}
            </Button>
          </div>
        </template>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<style scoped>
/* reka-ui 浮层经 Portal 挂到 body，样式经 :deep 落在内容根 */
:deep(.notification-dropdown),
.notification-dropdown {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  width: var(--size-notification-dropdown-width);
  max-height: 60vh;
  overflow-y: auto;
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
  z-index: 50;
}

.notification-dropdown__heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.notification-dropdown__title {
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.notification-dropdown__tabs {
  display: flex;
  gap: var(--space-2);
  border-bottom: var(--border-1) solid var(--ws-border);
}

/* 页签：h36 / 底部 2px accent 指示（DS/Navigation/Tab 口径） */
.notification-dropdown__tab {
  height: var(--ws-row-height);
  padding: 0 var(--space-3);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-body);
  cursor: pointer;
  background: transparent;
  border: none;
  border-bottom: var(--border-2) solid transparent;
}

.notification-dropdown__tab:hover {
  color: var(--ws-text-primary);
}

.notification-dropdown__tab.is-active {
  color: var(--ws-text-primary);
  border-bottom-color: var(--ws-accent);
}

.notification-dropdown__tab:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.notification-dropdown__records {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.notification-dropdown__group {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.notification-dropdown__group-label {
  margin: 0;
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

/* 空态（You're all caught up） */
.notification-dropdown__empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-6);
  color: var(--ws-text-secondary);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.notification-dropdown__empty-title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
}

.notification-dropdown__empty-desc {
  margin: 0;
  font-size: var(--font-size-micro);
  text-align: center;
}

.notification-dropdown__footer {
  display: flex;
  justify-content: flex-start;
}

.notification-dropdown__skeleton-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  height: var(--ws-row-height);
  padding: 0 var(--space-2);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.notification-dropdown__skeleton-line {
  height: var(--space-2);
}

.notification-dropdown__skeleton-line:first-child {
  width: 45%;
}

.notification-dropdown__skeleton-line:last-child {
  width: 30%;
}
</style>
