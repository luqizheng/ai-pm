<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { notification } from "@/shell/api/notification";
import type { Announcement, ScopeType } from "notification-services";
import { useAuthStore } from "@/shell/store/auth";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { useToast } from "@/components/ui/toast";
import {
  DataTable,
  FormDialog,
  type QueryField,
  type TableColumn
} from "@/shell/components";

type Row = Announcement;

const { t } = useI18n();
const auth = useAuthStore();
const { toast } = useToast();

const rows = ref<Row[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);
const keyword = ref("");

const dialogOpen = ref(false);
const submitting = ref(false);
const form = ref({
  title: "",
  body: "",
  publish_at: "",
  pinned: false,
  popover: false,
  scopeType: "role" as ScopeType,
  scopeId: ""
});

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

function openCreate() {
  form.value = {
    title: "",
    body: "",
    publish_at: "",
    pinned: false,
    popover: false,
    scopeType: "role",
    scopeId: ""
  };
  dialogOpen.value = true;
}

async function onSubmit() {
  submitting.value = true;
  try {
    await notification.announcements.create({
      title: form.value.title,
      body: form.value.body,
      // 服务端要求 RFC3339；datetime-local 无时区，统一转 UTC
      publish_at: form.value.publish_at
        ? new Date(form.value.publish_at).toISOString()
        : new Date().toISOString(),
      pinned: form.value.pinned,
      popover: form.value.popover,
      recipients: form.value.scopeId
        ? [{ scope_type: form.value.scopeType, scope_id: form.value.scopeId }]
        : []
    });
    dialogOpen.value = false;
    await load();
  } catch (error) {
    // 服务端校验失败（如 recipients 为空 / 非管理员）保持弹窗打开并提示
    toast({
      variant: "danger",
      title: t("system.notification.create"),
      description: error instanceof Error ? error.message : String(error)
    });
  } finally {
    submitting.value = false;
  }
}

async function onPublish(row: Record<string, unknown>) {
  await notification.announcements.publish(row.id as number);
  await load();
}

async function onRevoke(row: Record<string, unknown>) {
  await notification.announcements.revoke(row.id as number);
  await load();
}

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <header class="ui-page__header">
      <h1 class="ui-page__title">{{ t("system.notification.title") }}</h1>
      <!-- 后端公告写操作仅限管理员（403）；上游权限码目录暂无 NTF_ANNOUNCEMENT_*，先以角色门禁 -->
      <Button v-if="auth.hasRole('admin')" @click="openCreate">
        {{ t("system.notification.create") }}
      </Button>
    </header>

    <DataTable
      :columns="columns"
      :rows="rows"
      :query="query"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('system.notification.caption')"
      @search="onSearch"
      @update:page="value => (page = value)"
    >
      <template #status="{ row }">
        <Badge :variant="statusVariant(row.status as string)">
          {{ statusLabel(row.status as string) }}
        </Badge>
      </template>
      <template #actions="{ row }">
        <div class="ui-announcement__actions">
          <Button
            v-if="row.status === 'draft' && auth.hasRole('admin')"
            @click="onPublish(row)"
          >
            {{ t("system.notification.publish") }}
          </Button>
          <Button
            v-if="row.status === 'published' && auth.hasRole('admin')"
            @click="onRevoke(row)"
          >
            {{ t("system.notification.revoke") }}
          </Button>
        </div>
      </template>
    </DataTable>

    <FormDialog
      v-model:open="dialogOpen"
      :title="t('system.notification.create')"
      :submitting="submitting"
      @submit="onSubmit"
      @cancel="dialogOpen = false"
    >
      <FormField :label="t('system.notification.announcementTitle')" required>
        <Input v-model="form.title" />
      </FormField>
      <FormField :label="t('system.notification.announcementBody')">
        <Textarea v-model="form.body" :rows="6" />
      </FormField>
      <FormField :label="t('system.audit.createdAt')">
        <Input v-model="form.publish_at" type="datetime-local" />
      </FormField>
      <FormField :label="t('system.notification.pinned')">
        <Checkbox v-model="form.pinned" />
      </FormField>
      <FormField :label="t('system.notification.popover')">
        <Checkbox v-model="form.popover" />
      </FormField>
      <FormField :label="t('system.notification.scopeType')">
        <Select
          v-model="form.scopeType"
          :options="[
            { label: t('system.notification.scopeTypeRole'), value: 'role' },
            { label: t('system.notification.scopeTypeUser'), value: 'user' },
            { label: t('system.notification.scopeTypeDept'), value: 'dept' }
          ]"
        />
      </FormField>
      <FormField :label="t('system.notification.scopeId')" required>
        <Input v-model="form.scopeId" />
      </FormField>
    </FormDialog>
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

.ui-announcement__actions {
  display: flex;
  gap: var(--space-2);
  justify-content: flex-end;
}
</style>
