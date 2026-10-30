<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { useToast } from "@/components/ui/toast";
import {
  DataTable,
  FormDialog,
  PageHeader,
  type TableColumn
} from "@/shell/components";
import ScopeSelect from "@/shell/views/pm/components/ScopeSelect.vue";
import { useUserDirectory } from "@/shell/store/user-directory";
import { translateError } from "@/shell/api";
import { useScopeStore } from "@/api/scope";
import { statusVariant } from "@/api/status";
import { formatDateTime } from "@/api/format";
import { decideApproval, listApprovals } from "@/api/pm";
import type { ApprovalRequest } from "@/api/types";

const { t } = useI18n();
const toast = useToast();
const directory = useUserDirectory();
const scope = useScopeStore();

const columns: TableColumn[] = [
  { label: t("pm.columns.proposal"), prop: "proposal_id" },
  { label: t("pm.columns.status"), prop: "status", slot: "status" },
  { label: t("pm.columns.requiredRole"), prop: "required_role" },
  { label: t("pm.columns.decidedBy"), prop: "decided_by", slot: "decidedBy" },
  { label: t("pm.columns.decisionReason"), prop: "decision_reason" },
  { label: t("pm.columns.actions"), prop: "_actions", slot: "actions" },
  {
    label: t("pm.columns.createdAt"),
    prop: "created_at",
    slot: "createdAt",
    numeric: true
  }
];

const rows = ref<ApprovalRequest[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);

/** 决策弹框状态：null = 关闭 */
const deciding = ref<ApprovalRequest | null>(null);
const decisionKind = ref<"approved" | "rejected">("approved");
const decisionReason = ref("");
const submitting = ref(false);

const dialogOpen = computed(() => deciding.value !== null);
const reasonMissing = computed(
  () => decisionReason.value.trim() === "" && deciding.value !== null
);

/** 待处理的排最前，其余按申请时间倒序 */
const sortedRows = computed(() =>
  [...rows.value].sort((a, b) => {
    if ((a.status === "pending") !== (b.status === "pending")) {
      return a.status === "pending" ? -1 : 1;
    }
    return b.created_at.localeCompare(a.created_at);
  })
);

function statusText(value: unknown): string {
  const key = typeof value === "string" ? value : String(value ?? "");
  return t(`pm.status.${key}`, key);
}

function displayName(id: unknown): string {
  if (typeof id !== "string" || id === "") return "—";
  return directory.names[id] ?? id;
}

/** 壳 DataTable 插槽 row 是 Record<string, unknown>，按列约定收窄 */
function asApproval(row: Record<string, unknown>): ApprovalRequest {
  return row as unknown as ApprovalRequest;
}

async function load() {
  loading.value = true;
  try {
    await directory.ensureLoaded();
    const result = await listApprovals({
      project_id: scope.projectIdParam,
      page: page.value,
      size: pageSize
    });
    rows.value = result.items ?? [];
    total.value = result.total ?? 0;
  } catch (error) {
    toast.toast({
      variant: "danger",
      title: t("pm.loadFailed"),
      description: String(error)
    });
  } finally {
    loading.value = false;
  }
}

function openDecide(row: ApprovalRequest, kind: "approved" | "rejected") {
  deciding.value = row;
  decisionKind.value = kind;
  decisionReason.value = "";
}

async function submitDecide() {
  const target = deciding.value;
  if (!target || decisionReason.value.trim() === "") return;
  submitting.value = true;
  try {
    await decideApproval(target.id, {
      decision: decisionKind.value,
      reason: decisionReason.value.trim()
    });
    toast.toast({
      variant: "success",
      title: t("pm.approvals.decided")
    });
    deciding.value = null;
    await load();
  } catch (error) {
    // HttpError.message 已被壳翻译（409 等），原样展示
    toast.toast({
      variant: "danger",
      title: t("pm.approvals.decideFailed"),
      description: translateError(String(error))
    });
  } finally {
    submitting.value = false;
  }
}

watch(
  () => scope.currentProjectId,
  () => {
    page.value = 1;
    load();
  }
);

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <PageHeader
      :title="t('pm.approvals.pageTitle')"
      :subtitle="t('pm.approvals.subtitle')"
    >
      <template #actions>
        <ScopeSelect />
      </template>
    </PageHeader>

    <DataTable
      :columns="columns"
      :rows="sortedRows"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('pm.approvals.pageTitle')"
      :empty-text="t('pm.approvals.emptyHint')"
      @update:page="
        value => {
          page = value;
          load();
        }
      "
    >
      <template #status="{ row }">
        <Badge :variant="statusVariant(row.status)">{{
          statusText(row.status)
        }}</Badge>
      </template>
      <template #decidedBy="{ row }">{{
        displayName(row.decided_by)
      }}</template>
      <template #actions="{ row }">
        <span v-if="asApproval(row).status !== 'pending'" class="decided"
          >—</span
        >
        <span v-else class="actions">
          <Button size="sm" @click="openDecide(asApproval(row), 'approved')">
            {{ t("pm.approvals.approve") }}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            @click="openDecide(asApproval(row), 'rejected')"
          >
            {{ t("pm.approvals.reject") }}
          </Button>
        </span>
      </template>
      <template #createdAt="{ row }">{{
        formatDateTime(row.created_at)
      }}</template>
    </DataTable>

    <FormDialog
      v-model:open="dialogOpen"
      :title="
        decisionKind === 'approved'
          ? t('pm.approvals.approveTitle')
          : t('pm.approvals.rejectTitle')
      "
      :submitting="submitting"
      :submit-label="t('common.confirm')"
      @update:open="
        value => {
          if (!value) deciding = null;
        }
      "
      @submit="submitDecide"
    >
      <p v-if="deciding" class="dialog-proposal">
        {{ t("pm.columns.proposal") }}:
        <code>{{ deciding.proposal_id }}</code>
      </p>
      <Textarea
        v-model="decisionReason"
        :placeholder="t('pm.approvals.reasonPlaceholder')"
        :rows="4"
      />
      <p v-if="reasonMissing" class="dialog-hint">
        {{ t("pm.approvals.reasonRequired") }}
      </p>
    </FormDialog>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.actions {
  display: inline-flex;
  gap: var(--space-2);
}

.decided {
  color: var(--text-tertiary);
}

.dialog-proposal {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.dialog-hint {
  margin: 0;
  font-size: var(--font-size-caption);
  color: var(--danger-strong);
}
</style>
