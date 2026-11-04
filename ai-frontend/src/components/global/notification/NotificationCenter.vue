<!-- @figma-sync file=hNlGqLEMQpOVCZ0o5u3DkT set="Global/Notification/Center" node=33:87792 date=2026-11-04 -->
<script setup lang="ts">
import { computed, onMounted } from "vue";
import { SearchX } from "lucide-vue-next";
import { useI18n } from "vue-i18n";

import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import Checkbox from "@/components/ui/checkbox/Checkbox.vue";
import Select from "@/components/ui/select/Select.vue";
import Skeleton from "@/components/ui/skeleton/Skeleton.vue";
import { useNotificationsStore } from "@/stores/notifications";
import NotificationRecord from "./NotificationRecord.vue";
import {
  NOTIFICATION_CATEGORIES,
  type NotificationCategory,
  type NotificationRecord as NotificationRecordType
} from "./types";

/**
 * 完整通知中心（Figma Global/Notification/Center 33:87792）。
 *
 * 头部「全部已读」+ 分类下拉 / 仅看未读 双过滤 + 记录数行 +
 * 记录列表（选中态 = accent 着色面 + accent 描边）。
 * Loading/Empty/Error 与下拉同构（同一 store 状态）。
 */
const emit = defineEmits<{
  open: [record: NotificationRecordType];
}>();

const { t } = useI18n();

const store = useNotificationsStore();

const selectedId = defineModel<string | null>("selectedId", { default: null });

/** 分类过滤（"all" = 全部分类） */
const category = defineModel<string>("category", { default: "all" });
const unreadOnly = defineModel<boolean>("unreadOnly", { default: false });

const categoryOptions = computed(() => [
  { value: "all", label: t("global.notification.categoryAll") },
  ...NOTIFICATION_CATEGORIES.map(type => ({
    value: type,
    label: t(`global.notification.category.${type}`)
  }))
]);

const filtered = computed(() =>
  store.records.filter(
    record =>
      (category.value === "all" || record.category === category.value) &&
      (!unreadOnly.value || !record.read)
  )
);

const countLine = computed(() => {
  const scope =
    category.value === "all"
      ? t("global.notification.categoryAll")
      : t(`global.notification.category.${category.value}`);
  return t("global.notification.countLine", {
    scope,
    records: filtered.value.length,
    unread: filtered.value.filter(record => !record.read).length
  });
});

onMounted(() => {
  if (store.status === "idle") store.load();
});

function onToggleRead(record: NotificationRecordType) {
  if (record.read) {
    store.markUnread(record.id);
  } else {
    store.markRead(record.id);
  }
}

function onSelect(record: NotificationRecordType) {
  selectedId.value = record.id;
}
</script>

<template>
  <section
    class="notification-center"
    :aria-label="t('global.notification.label')"
  >
    <div class="notification-center__heading">
      <h2 class="notification-center__title">
        {{ t("global.notification.label") }}
      </h2>
      <Button variant="secondary" size="sm" @click="store.markAllRead()">
        {{ t("global.notification.markAllRead") }}
      </Button>
    </div>

    <!-- Loading -->
    <template v-if="store.status === 'loading'">
      <div
        v-for="n in 2"
        :key="n"
        class="notification-center__skeleton-row"
        aria-hidden="true"
      >
        <Skeleton class="notification-center__skeleton-line" tone="strong" />
        <Skeleton class="notification-center__skeleton-line" tone="base" />
      </div>
    </template>

    <!-- Error -->
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

    <!-- Ready -->
    <template v-else>
      <div class="notification-center__filters">
        <div class="notification-center__filter">
          <label
            class="notification-center__filter-label"
            for="notification-category"
            >{{ t("global.notification.categoryLabel") }}</label
          >
          <Select
            id="notification-category"
            v-model="category"
            :options="categoryOptions"
            :aria-label="t('global.notification.categoryLabel')"
            size="sm"
          />
          <p class="notification-center__filter-hint">
            {{ t("global.notification.categoryHint") }}
          </p>
        </div>
        <Checkbox
          v-model="unreadOnly"
          :label="t('global.notification.unreadOnly')"
        />
      </div>

      <p class="notification-center__count">{{ countLine }}</p>

      <div v-if="filtered.length === 0" class="notification-center__empty">
        <SearchX :size="24" aria-hidden="true" />
        <p class="notification-center__empty-title">
          {{ t("global.notification.emptyTitle") }}
        </p>
        <p class="notification-center__empty-desc">
          {{ t("global.notification.emptyDesc") }}
        </p>
      </div>

      <div v-else class="notification-center__records">
        <NotificationRecord
          v-for="record in filtered"
          :key="record.id"
          :record="record"
          :selected="record.id === selectedId"
          @select="onSelect"
          @open="emit('open', $event)"
          @toggle-read="onToggleRead"
        />
      </div>
    </template>
  </section>
</template>

<style scoped>
.notification-center {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  background: var(--ws-surface);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.notification-center__heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.notification-center__title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body-strong);
  font-weight: var(--weight-semibold);
}

.notification-center__filters {
  display: flex;
  align-items: flex-start;
  gap: var(--space-4);
}

.notification-center__filter {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  width: var(--size-notification-filter-width);
}

.notification-center__filter-label {
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
}

.notification-center__filter-hint {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-micro);
}

.notification-center__count {
  margin: 0;
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
}

.notification-center__records {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.notification-center__empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-6);
  color: var(--ws-text-secondary);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.notification-center__empty-title {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
}

.notification-center__empty-desc {
  margin: 0;
  font-size: var(--font-size-micro);
  text-align: center;
}

.notification-center__skeleton-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  height: var(--ws-row-height);
  padding: 0 var(--space-2);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--radius-sm);
}

.notification-center__skeleton-line {
  height: var(--space-2);
}

.notification-center__skeleton-line:first-child {
  width: 40%;
}

.notification-center__skeleton-line:last-child {
  width: 25%;
}
</style>
