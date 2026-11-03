<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  cloneMenuConfig,
  listMenuConfigs,
  publishMenuConfig,
  type MenuConfigItem
} from "@/shell/api";
import { prue } from "@/shell/api/prue";
import { useAuthStore } from "@/shell/store/auth";
import { useToast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { DataTable, FormDialog, type TableColumn } from "@/shell/components";

const { t } = useI18n();
const auth = useAuthStore();
const { toast } = useToast();

const rows = ref<MenuConfigItem[]>([]);
const loading = ref(false);
const busyId = ref<string | undefined>(undefined);

const dialogOpen = ref(false);
const submitting = ref(false);
const editingId = ref<string | undefined>(undefined);
const editingEtag = ref<string | undefined>(undefined);
const form = ref({ name: "", description: "", routesJson: "" });

const columns: TableColumn[] = [
  { label: t("system.menu.name"), prop: "name" },
  { label: t("system.menu.version"), prop: "version", numeric: true },
  { label: t("system.menu.status"), prop: "is_active", slot: "status" },
  { label: t("system.user.createdAt"), prop: "created_at", numeric: true }
];

async function load() {
  loading.value = true;
  try {
    const result = await listMenuConfigs();
    rows.value = result.items ?? [];
  } finally {
    loading.value = false;
  }
}

async function onPublish(row: Record<string, unknown>) {
  const id = row.id as string;
  busyId.value = id;
  try {
    await publishMenuConfig(id);
    await load();
  } finally {
    busyId.value = undefined;
  }
}

async function onClone(row: Record<string, unknown>) {
  const id = row.id as string;
  busyId.value = id;
  try {
    await cloneMenuConfig(id);
    await load();
  } finally {
    busyId.value = undefined;
  }
}

function openCreate() {
  editingId.value = undefined;
  form.value = {
    name: "",
    description: "",
    routesJson: JSON.stringify(
      [
        {
          path: "/dashboard",
          name: "Dashboard",
          component: "dashboard/DashboardView",
          // 标题存 i18n key（shell.routes.*），前端按 key 翻译；未登记的 key 回退显示原文
          meta: { title: "shell.routes.dashboard" }
        }
      ],
      null,
      2
    )
  };
  dialogOpen.value = true;
}

/** 编辑草稿：拉取详情回填 routes_json；ETag 取自行数据，提交时带 if_match_etag 乐观锁 */
async function openEdit(row: Record<string, unknown>) {
  const id = row.id as string;
  busyId.value = id;
  try {
    const detail = await prue.configs.get(id);
    editingId.value = id;
    editingEtag.value =
      typeof row.routes_etag === "string"
        ? row.routes_etag
        : detail.routes_etag;
    form.value = {
      name: detail.name,
      description: detail.description ?? "",
      routesJson: JSON.stringify(detail.routes_json ?? [], null, 2)
    };
    dialogOpen.value = true;
  } finally {
    busyId.value = undefined;
  }
}

async function onSubmit() {
  let routes: unknown;
  try {
    routes = JSON.parse(form.value.routesJson);
  } catch {
    toast({ variant: "warning", title: t("system.menu.routesJsonInvalid") });
    return;
  }
  if (!Array.isArray(routes)) {
    toast({ variant: "warning", title: t("system.menu.routesJsonInvalid") });
    return;
  }
  submitting.value = true;
  try {
    if (editingId.value) {
      await prue.configs.update(editingId.value, {
        name: form.value.name,
        description: form.value.description || undefined,
        routes_json: routes,
        if_match_etag: editingEtag.value
      });
      toast({ variant: "success", title: t("system.menu.updateSuccess") });
    } else {
      await prue.configs.create({
        name: form.value.name,
        description: form.value.description || undefined,
        routes_json: routes
      });
      toast({ variant: "success", title: t("system.menu.createSuccess") });
    }
    dialogOpen.value = false;
    await load();
  } catch (error) {
    // 409 ETag 冲突等：保持弹窗打开，错误信息进 toast（error.body.data.current_etag 可取最新值）
    toast({
      variant: "danger",
      title: editingId.value ? t("system.menu.edit") : t("system.menu.create"),
      description: error instanceof Error ? error.message : String(error)
    });
  } finally {
    submitting.value = false;
  }
}

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <header class="ui-page__header">
      <h1 class="ui-page__title">{{ t("system.menu.title") }}</h1>
      <Button v-if="auth.hasRole('admin')" @click="openCreate">
        {{ t("system.menu.create") }}
      </Button>
    </header>

    <DataTable
      :columns="columns"
      :rows="rows"
      :loading="loading"
      :caption="t('system.menu.caption')"
      @update:page="load"
    >
      <template #status="{ row }">
        <Badge :variant="row.is_active ? 'success' : 'neutral'">
          {{ row.is_active ? t("system.menu.active") : t("system.menu.draft") }}
        </Badge>
      </template>
      <template #name="{ row }">
        <span class="ui-page__name">{{ row.name }}</span>
        <span class="ui-page__actions">
          <Button
            v-if="!row.is_active && auth.hasRole('admin')"
            variant="secondary"
            size="sm"
            :loading="busyId === row.id"
            @click="onPublish(row)"
          >
            {{ t("system.menu.publish") }}
          </Button>
          <Button
            v-if="row.status === 'draft' && auth.hasRole('admin')"
            variant="secondary"
            size="sm"
            @click="openEdit(row)"
          >
            {{ t("system.menu.edit") }}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            :loading="busyId === row.id"
            @click="onClone(row)"
          >
            {{ t("system.menu.clone") }}
          </Button>
        </span>
      </template>
    </DataTable>

    <FormDialog
      v-model:open="dialogOpen"
      :title="editingId ? t('system.menu.edit') : t('system.menu.create')"
      :submitting="submitting"
      @submit="onSubmit"
      @cancel="dialogOpen = false"
    >
      <FormField :label="t('system.menu.name')" required>
        <Input v-model="form.name" />
      </FormField>
      <FormField :label="t('system.menu.description')">
        <Input v-model="form.description" />
      </FormField>
      <FormField :label="t('system.menu.routesJson')" required>
        <Textarea v-model="form.routesJson" :rows="12" />
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

.ui-page__name {
  color: var(--text-primary);
}

.ui-page__actions {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  margin-left: var(--space-3);
}
</style>
