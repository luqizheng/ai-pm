<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { notification } from "@/shell/api/notification";
import type { Announcement } from "notification-services";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DataTable,
  type QueryField,
  type TableColumn
} from "@/shell/components";

type Row = Announcement;

const { t } = useI18n();

const rows = ref<Row[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);
const keyword = ref("");

const columns: TableColumn[] = [
  { label: t("system.notification.announcementTitle"), prop: "title" },
  { label: t("system.notification.status"), prop: "status", slot: "status" },
  { label: t("system.audit.createdAt"), prop: "publish_at", numeric: true },
  { label: "", prop: "actions", slot: "actions" }
];

const query: QueryField[] = [
  { prop: "keyword", placeholder: t("system.notification.searchPlaceholder") }
];

const statusVariant = (status: string) => {
  if (status === "published") return "success";
  if (status === "revoked") return "danger";
  return "neutral";
};

const statusLabel = (status: string) => {
  if (status === "published") return t("system.notification.published");
  if (status === "revoked") return t("system.notification.revoked");
  return t("system.notification.draft");
};

async function load() {
  loading.value = true;
  try {
    // 后端返回当前用户可见公告的裸数组（无分页信封）
    const list = await notification.announcements.listMine();
    rows.value = list ?? [];
    total.value = rows.value.length;
  } finally {
    loading.value = false;
  }
}

function onSearch(params: Record<string, string>) {
  keyword.value = params.keyword ?? "";
  page.value = 1;
  load();
}

async function onMarkRead(row: Record<string, unknown>) {
  await notification.announcements.markRead(row.id as number);
  await load();
}

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <header class="ui-page__header">
      <h1 class="ui-page__title">{{ t("system.notification.inbox.title") }}</h1>
    </header>

    <DataTable
      :columns="columns"
      :rows="rows"
      :query="query"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('system.notification.inbox.caption')"
      @search="onSearch"
      @update:page="value => (page = value)"
    >
      <template #status="{ row }">
        <Badge :variant="statusVariant(row.status as string)">
          {{ statusLabel(row.status as string) }}
        </Badge>
      </template>
      <template #actions="{ row }">
        <Button size="sm" variant="ghost" @click="onMarkRead(row)">
          {{ t("system.notification.inbox.markRead") }}
        </Button>
      </template>
    </DataTable>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.ui-page__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.ui-page__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h2);
  line-height: var(--line-height-h2);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h2);
  color: var(--text-primary);
}
</style>
