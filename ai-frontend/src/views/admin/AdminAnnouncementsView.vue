<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { HttpCore, type Announcement } from "notification-services";

import {
  createAnnouncement,
  publishAnnouncement,
  revokeAnnouncement
} from "@/api/notifications";
import { getAccessToken, getStoredUser } from "@/api/base/token-storage";
import PageFrame from "@/components/shell/PageFrame.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Checkbox from "@/components/ui/checkbox/Checkbox.vue";
import FormField from "@/components/ui/form-field/FormField.vue";
import Input from "@/components/ui/input/Input.vue";
import Table from "@/components/ui/table/Table.vue";
import TableBody from "@/components/ui/table/TableBody.vue";
import TableCell from "@/components/ui/table/TableCell.vue";
import TableHead from "@/components/ui/table/TableHead.vue";
import TableHeader from "@/components/ui/table/TableHeader.vue";
import TableRow from "@/components/ui/table/TableRow.vue";
import Textarea from "@/components/ui/textarea/Textarea.vue";

/**
 * 公告管理（管理员）：列表 / 创建并发布 / 撤销。
 *
 * 数据源为 notification-service 的租户级公告列表；
 * 创建时按收件人 user id 投递（角色/部门选择待成员选择器落地后接入）。
 */
const { t } = useI18n();

const http = new HttpCore({
  baseUrl: import.meta.env.VITE_IAM_BASE_URL ?? "/gw",
  getToken: () => getAccessToken(),
  fetch: fetch.bind(globalThis)
});

const items = ref<Announcement[]>([]);
const loading = ref(false);
const failed = ref(false);

const title = ref("");
const body = ref("");
const recipients = ref(getStoredUser()?.id ?? "");
const pinned = ref(false);
const popover = ref(true);

const STATUS_VARIANT: Record<string, "accent" | "success" | "warning"> = {
  published: "success",
  scheduled: "warning",
  draft: "accent",
  revoked: "warning"
};

async function load() {
  loading.value = true;
  failed.value = false;
  try {
    items.value = await http.get<Announcement[]>(
      "/v1/notification/announcements/manage",
      { auth: true }
    );
  } catch {
    failed.value = true;
  } finally {
    loading.value = false;
  }
}

async function onCreate() {
  const ids = recipients.value
    .split(",")
    .map(id => id.trim())
    .filter(Boolean);
  if (!title.value.trim() || !body.value.trim() || ids.length === 0) {
    failed.value = true;
    return;
  }

  loading.value = true;
  failed.value = false;
  try {
    const { id } = await createAnnouncement({
      title: title.value.trim(),
      body: body.value.trim(),
      publish_at: new Date().toISOString(),
      pinned: pinned.value,
      popover: popover.value,
      recipients: ids.map(id => ({ scope_type: "user", scope_id: id }))
    });
    await publishAnnouncement(id);
    title.value = "";
    body.value = "";
    await load();
  } catch {
    failed.value = true;
  } finally {
    loading.value = false;
  }
}

async function onPublish(id: number) {
  try {
    await publishAnnouncement(id);
    await load();
  } catch {
    failed.value = true;
  }
}

async function onRevoke(id: number) {
  try {
    await revokeAnnouncement(id);
    await load();
  } catch {
    failed.value = true;
  }
}

function statusLabel(status: string) {
  return t(`shell.adminAnnouncements.status.${status}`);
}

function formatTime(value: string) {
  return new Date(value).toLocaleString();
}

onMounted(load);
</script>

<template>
  <PageFrame
    :title="t('shell.adminAnnouncements.title')"
    :description="t('shell.adminAnnouncements.description')"
  >
    <template #actions>
      <Button variant="secondary" size="sm" :loading="loading" @click="load">
        {{ t("shell.adminAnnouncements.actions.refresh") }}
      </Button>
    </template>

    <section class="admin-announcements__form">
      <h2 class="admin-announcements__heading">
        {{ t("shell.adminAnnouncements.createTitle") }}
      </h2>

      <FormField
        :label="t('shell.adminAnnouncements.fields.title')"
        html-for="announcement-title"
      >
        <Input
          id="announcement-title"
          v-model="title"
          :placeholder="t('shell.adminAnnouncements.fields.title')"
        />
      </FormField>

      <FormField
        :label="t('shell.adminAnnouncements.fields.body')"
        html-for="announcement-body"
      >
        <Textarea
          id="announcement-body"
          v-model="body"
          :placeholder="t('shell.adminAnnouncements.fields.body')"
        />
      </FormField>

      <FormField
        :label="t('shell.adminAnnouncements.fields.recipients')"
        :helper="t('shell.adminAnnouncements.fields.recipientsHelper')"
        html-for="announcement-recipients"
      >
        <Input
          id="announcement-recipients"
          v-model="recipients"
          :placeholder="t('shell.adminAnnouncements.fields.recipients')"
        />
      </FormField>

      <div class="admin-announcements__flags">
        <label class="admin-announcements__flag">
          <Checkbox v-model="pinned" />
          {{ t("shell.adminAnnouncements.fields.pinned") }}
        </label>
        <label class="admin-announcements__flag">
          <Checkbox v-model="popover" />
          {{ t("shell.adminAnnouncements.fields.popover") }}
        </label>
      </div>

      <Button variant="primary" size="sm" :loading="loading" @click="onCreate">
        {{ t("shell.adminAnnouncements.actions.create") }}
      </Button>
    </section>

    <p v-if="failed" class="admin-announcements__error">
      {{ t("shell.adminAnnouncements.failed") }}
    </p>

    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{{
            t("shell.adminAnnouncements.columns.title")
          }}</TableHead>
          <TableHead>{{
            t("shell.adminAnnouncements.columns.status")
          }}</TableHead>
          <TableHead>{{
            t("shell.adminAnnouncements.columns.publishAt")
          }}</TableHead>
          <TableHead>{{
            t("shell.adminAnnouncements.columns.actions")
          }}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="item in items" :key="item.id">
          <TableCell>{{ item.title }}</TableCell>
          <TableCell>
            <Badge :variant="STATUS_VARIANT[item.status] ?? 'accent'" size="sm">
              {{ statusLabel(item.status) }}
            </Badge>
          </TableCell>
          <TableCell>{{ formatTime(item.publish_at) }}</TableCell>
          <TableCell>
            <div class="admin-announcements__row-actions">
              <Button
                v-if="item.status !== 'published' && item.status !== 'revoked'"
                variant="secondary"
                size="sm"
                @click="onPublish(item.id)"
              >
                {{ t("shell.adminAnnouncements.actions.publish") }}
              </Button>
              <Button
                v-if="item.status !== 'revoked'"
                variant="danger"
                size="sm"
                @click="onRevoke(item.id)"
              >
                {{ t("shell.adminAnnouncements.actions.revoke") }}
              </Button>
            </div>
          </TableCell>
        </TableRow>
        <TableRow v-if="items.length === 0">
          <TableCell>{{ t("shell.adminAnnouncements.empty") }}</TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </PageFrame>
</template>

<style scoped>
.admin-announcements__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-4);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
}

.admin-announcements__heading {
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.admin-announcements__flags {
  display: flex;
  gap: var(--space-4);
}

.admin-announcements__flag {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.admin-announcements__error {
  margin: 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-color-danger-text);
}

.admin-announcements__row-actions {
  display: flex;
  gap: var(--space-2);
}
</style>
