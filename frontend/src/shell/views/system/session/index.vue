<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { iam } from "@/shell/api/iam";
import type { SessionInfo } from "iam-services";
import { Badge } from "@/components/ui/badge";
import {
  DataTable,
  UserLabel,
  type QueryField,
  type TableColumn
} from "@/shell/components";

const { t } = useI18n();

const sessions = ref<SessionInfo[]>([]);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);
const keyword = ref("");

const columns: TableColumn[] = [
  { label: t("system.session.user"), prop: "user_id", slot: "user" },
  { label: t("system.session.ip"), prop: "client_ip" },
  { label: t("system.session.current"), prop: "is_current", slot: "current" },
  { label: t("system.session.expiresAt"), prop: "expires_at", numeric: true }
];

const query: QueryField[] = [
  { prop: "keyword", placeholder: t("system.session.searchPlaceholder") }
];

/** 后端返回当前用户全部会话（裸数组），分页与过滤在客户端做 */
const rows = computed<Record<string, unknown>[]>(() => {
  const kw = keyword.value.trim().toLowerCase();
  const filtered = kw
    ? sessions.value.filter(
        s =>
          s.client_ip.toLowerCase().includes(kw) ||
          (s.user_agent ?? "").toLowerCase().includes(kw)
      )
    : sessions.value;
  const start = (page.value - 1) * pageSize;
  return filtered.slice(start, start + pageSize) as unknown as Record<
    string,
    unknown
  >[];
});

const total = computed(() => sessions.value.length);

/** 表格插槽里的行是弱类型，取 userId 时只认字符串 */
function userIdOf(row: Record<string, unknown>): string | undefined {
  return typeof row.user_id === "string" ? row.user_id : undefined;
}

async function load() {
  loading.value = true;
  try {
    sessions.value = await iam.auth.listSessions();
  } finally {
    loading.value = false;
  }
}

function onSearch(params: Record<string, string>) {
  keyword.value = params.keyword ?? "";
  page.value = 1;
}

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <header class="ui-page__header">
      <h1 class="ui-page__title">{{ t("system.session.title") }}</h1>
    </header>

    <DataTable
      :columns="columns"
      :rows="rows"
      :query="query"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('system.session.caption')"
      @search="onSearch"
      @update:page="value => (page = value)"
    >
      <template #user="{ row }">
        <UserLabel :user-id="userIdOf(row)" />
      </template>

      <template #current="{ row }">
        <Badge :variant="row.is_current ? 'accent' : 'neutral'">
          {{
            row.is_current
              ? t("system.session.currentSession")
              : t("system.session.otherSession")
          }}
        </Badge>
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
