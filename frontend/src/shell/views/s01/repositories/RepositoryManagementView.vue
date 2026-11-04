<script setup lang="ts">
/**
 * S01-03 · 仓库管理（v2 语义）。
 *
 * UX 依据：`docs/ui/s01/s01-03-repository-management-ux-v1-1.md`
 *
 * 硬语义：
 *   - 一个 Project 可关联多个 Repository；Primary 可为空，最多一个。
 *   - **不自动**把第一个接入的仓库提升为 Primary。
 *   - Default Branch / Selected Branch / Selected Commit / HEAD 四者严格区分。
 *   - Branch HEAD 变化**只提示**，由人类确认才更新采用版本。
 *   - Disconnect 是逻辑断开，历史 Run / 交付 / 基线引用全部保留。
 *   - Re-verify（核验身份与版本）与 Re-scan（重新分析代码）不是同一操作。
 */
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { useToast } from "@/components/ui/toast";
import { PageHeader } from "@/shell/components";
import { useConfirm } from "@/shell/composables/useConfirm";
import { formatDateTime } from "@/api/format";
import {
  disconnectRepository,
  listGitProviders,
  listRepositoriesV2,
  restoreRepository,
  setPrimaryRepository,
  updateRepositoryVersion,
  verifyRepositoryV2,
  type RepositoryV2Payload
} from "@/api/initialization";
import type {
  GitProviderAuthorization,
  RepositoryV2,
  RepositoryVerificationOutcome
} from "@/api/types/initialization";

const { t } = useI18n();
const route = useRoute();
const toast = useToast();
const confirm = useConfirm();

const projectId = computed(() => String(route.params.projectId ?? ""));

const rows = ref<RepositoryV2[]>([]);
const providers = ref<GitProviderAuthorization[]>([]);
const loading = ref(false);
const busyId = ref<string | null>(null);

const ROLES = ["primary", "supporting", "infrastructure", "test"] as const;
const PROVIDERS = ["github", "gitlab", "other"] as const;

const roleOptions = computed(() =>
  ROLES.map(value => ({
    value,
    label: t(`s01.initialization.repositories.role.${value}`)
  }))
);

const providerOptions = computed(() =>
  PROVIDERS.map(value => ({
    value,
    label: t(
      `s01.initialization.repositories.provider${value === "gitlab" ? "GitLab" : value === "github" ? "GitHub" : "Other"}`
    )
  }))
);

const primaryCount = computed(
  () => rows.value.filter(row => row.role === "primary").length
);

/** S01-03 §3.2：无 Primary 是合法状态，只提示不阻断。 */
const missingPrimary = computed(
  () => rows.value.length > 0 && primaryCount.value === 0
);

/** §6.2：HEAD 与采用版本不一致 —— 只提示，不自动更新。 */
const drifted = computed(() =>
  rows.value.filter(
    row =>
      row.selected_commit !== null &&
      row.head_commit_sha !== null &&
      row.selected_commit !== row.head_commit_sha
  )
);

function connectionVariant(
  value: string
): "success" | "danger" | "warning" | "neutral" {
  if (value === "connected") return "success";
  if (value === "failed") return "danger";
  if (value === "pending") return "warning";
  return "neutral";
}

function verificationVariant(
  value: string
): "success" | "danger" | "warning" | "neutral" {
  if (value === "passed") return "success";
  if (value === "failed") return "danger";
  if (value === "warning") return "warning";
  return "neutral";
}

function failure(error: unknown) {
  toast.toast({
    variant: "danger",
    title: t("s01.initialization.common.operationFailed"),
    description: String(error)
  });
}

async function load() {
  loading.value = true;
  try {
    const result = await listRepositoriesV2(projectId.value);
    rows.value = result.items ?? [];
  } catch (error) {
    failure(error);
  } finally {
    loading.value = false;
  }
}

async function loadProviders() {
  try {
    providers.value = await listGitProviders(projectId.value);
  } catch {
    providers.value = [];
  }
}

async function verify(row: RepositoryV2) {
  busyId.value = row.id;
  try {
    const outcome: RepositoryVerificationOutcome = await verifyRepositoryV2(
      row.id
    );
    if (outcome.head_changed) {
      toast.toast({
        variant: "warning",
        title: t("s01.initialization.repositories.headChanged"),
        description: t("s01.initialization.repositories.headChangedHint", {
          current: outcome.selected_commit ?? "—",
          latest: outcome.head_commit_sha ?? "—"
        })
      });
    } else {
      toast.toast({
        variant: "success",
        title: t("s01.initialization.repositories.verified")
      });
    }
    await load();
  } catch (error) {
    failure(error);
  } finally {
    busyId.value = null;
  }
}

async function setPrimary(row: RepositoryV2) {
  busyId.value = row.id;
  try {
    await setPrimaryRepository(row.id);
    toast.toast({
      variant: "success",
      title: t("s01.initialization.repositories.primarySet")
    });
    await load();
  } catch (error) {
    failure(error);
  } finally {
    busyId.value = null;
  }
}

/** §6.2：人类确认后才更新采用版本；不动正式基线。 */
async function updateVersion(row: RepositoryV2) {
  const branch = window.prompt(
    t("s01.initialization.repositories.selectedBranch"),
    row.selected_branch ?? row.default_branch ?? ""
  );
  if (!branch) return;
  const commit = window.prompt(
    t("s01.initialization.repositories.selectedCommit"),
    row.head_commit_sha ?? ""
  );
  if (!commit) return;
  const reason = window.prompt(
    t("s01.initialization.repositories.updateVersionReason")
  );
  if (reason === null) return;

  busyId.value = row.id;
  try {
    await updateRepositoryVersion(row.id, {
      selected_branch: branch,
      selected_commit: commit,
      reason
    });
    toast.toast({
      variant: "success",
      title: t("s01.initialization.repositories.versionUpdated")
    });
    await load();
  } catch (error) {
    failure(error);
  } finally {
    busyId.value = null;
  }
}

async function disconnect(row: RepositoryV2) {
  const ok = await confirm.confirm({
    title: t("s01.initialization.repositories.disconnectTitle"),
    description: t("s01.initialization.repositories.disconnectConfirm", {
      name: row.name
    }),
    confirmLabel: t("s01.initialization.repositories.disconnect"),
    danger: true
  });
  if (!ok) return;
  busyId.value = row.id;
  try {
    await disconnectRepository(row.id);
    toast.toast({
      variant: "success",
      title: t("s01.initialization.repositories.disconnected")
    });
    await load();
  } catch (error) {
    failure(error);
  } finally {
    busyId.value = null;
  }
}

async function restore(row: RepositoryV2) {
  const ok = await confirm.confirm({
    title: t("s01.initialization.repositories.restore"),
    description: t("s01.initialization.repositories.restoreConfirm", {
      name: row.name
    }),
    confirmLabel: t("s01.initialization.repositories.restore")
  });
  if (!ok) return;
  busyId.value = row.id;
  try {
    await restoreRepository(row.id);
    toast.toast({
      variant: "success",
      title: t("s01.initialization.repositories.restored")
    });
    await load();
  } catch (error) {
    failure(error);
  } finally {
    busyId.value = null;
  }
}

onMounted(async () => {
  await Promise.all([load(), loadProviders()]);
});
</script>

<template>
  <section class="ui-page">
    <PageHeader
      :title="t('s01.initialization.repositories.pageTitle')"
      :subtitle="t('s01.initialization.repositories.subtitle')"
    />

    <Alert v-if="missingPrimary" variant="info">
      {{ t("s01.initialization.repositories.primaryHint") }}
    </Alert>

    <section v-if="drifted.length > 0" class="ui-card ui-card--warn">
      <h2 class="ui-card__title">
        {{ t("s01.initialization.repositories.headChanged") }}
      </h2>
      <p class="ui-card__hint">
        {{
          t("s01.initialization.repositories.headChangedHint", {
            current: drifted[0].selected_commit ?? "—",
            latest: drifted[0].head_commit_sha ?? "—"
          })
        }}
      </p>
      <ul class="drift-list">
        <li v-for="row in drifted" :key="row.id" class="drift-list__item">
          <span>{{ row.name }}</span>
          <code>{{ row.selected_commit }}</code>
          <span class="drift-list__arrow">→</span>
          <code>{{ row.head_commit_sha }}</code>
          <Button
            size="sm"
            variant="secondary"
            :disabled="busyId === row.id"
            @click="updateVersion(row)"
          >
            {{ t("s01.initialization.repositories.updateVersion") }}
          </Button>
        </li>
      </ul>
    </section>

    <section class="ui-card">
      <header class="ui-card__head">
        <div class="ui-card__text">
          <h2 class="ui-card__title">
            {{ t("s01.initialization.repositories.providers") }}
          </h2>
          <p class="ui-card__hint">
            {{ t("s01.initialization.repositories.providersHint") }}
          </p>
        </div>
      </header>
      <p v-if="providers.length === 0" class="ui-card__empty">
        {{ t("s01.initialization.repositories.noProvider") }}
      </p>
      <ul v-else class="provider-list">
        <li
          v-for="provider in providers"
          :key="provider.provider"
          class="provider-list__item"
        >
          <span class="provider-list__name">
            {{
              t(
                `s01.initialization.repositories.provider${provider.provider === "gitlab" ? "GitLab" : provider.provider === "github" ? "GitHub" : "Other"}`
              )
            }}
          </span>
          <Badge
            :variant="
              provider.status === 'active'
                ? 'success'
                : provider.status === 'expired'
                  ? 'warning'
                  : 'neutral'
            "
          >
            {{
              provider.status === "active"
                ? t("s01.initialization.repositories.providerActive")
                : provider.status === "expired"
                  ? t("s01.initialization.repositories.providerExpired")
                  : t("s01.initialization.repositories.providerRevoked")
            }}
          </Badge>
          <span class="provider-list__scopes">
            {{ provider.scopes.join(" · ") || "—" }}
          </span>
          <Button size="sm" variant="ghost">
            {{
              provider.status === "active"
                ? t("s01.initialization.repositories.providerRevoke")
                : t("s01.initialization.repositories.providerReauthorize")
            }}
          </Button>
        </li>
      </ul>
    </section>

    <section class="ui-card">
      <p v-if="loading" class="ui-card__empty">
        {{ t("s01.initialization.common.loadFailed") }}
      </p>
      <p v-else-if="rows.length === 0" class="ui-card__empty">
        {{ t("s01.initialization.repositories.emptyHint") }}
      </p>
      <ul v-else class="repo-list">
        <li v-for="row in rows" :key="row.id" class="repo">
          <header class="repo__head">
            <div class="repo__title">
              <span class="repo__name">{{ row.name }}</span>
              <Badge :variant="row.role === 'primary' ? 'info' : 'neutral'">
                {{ t(`s01.initialization.repositories.role.${row.role}`) }}
              </Badge>
              <Badge :variant="connectionVariant(row.connection_status)">
                {{
                  t(
                    `s01.initialization.repositories.connectionStatus.${row.connection_status}`
                  )
                }}
              </Badge>
              <Badge :variant="verificationVariant(row.verification_status)">
                {{
                  t(
                    `s01.initialization.repositories.verificationStatus.${row.verification_status}`
                  )
                }}
              </Badge>
            </div>
            <div class="repo__actions">
              <Button
                size="sm"
                variant="ghost"
                :disabled="busyId === row.id"
                @click="verify(row)"
              >
                {{ t("s01.initialization.repositories.verify") }}
              </Button>
              <Button
                v-if="row.role !== 'primary'"
                size="sm"
                variant="ghost"
                :disabled="busyId === row.id"
                @click="setPrimary(row)"
              >
                {{ t("s01.initialization.repositories.setPrimary") }}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                :disabled="busyId === row.id"
                @click="updateVersion(row)"
              >
                {{ t("s01.initialization.repositories.updateVersion") }}
              </Button>
              <Button
                v-if="row.connection_status !== 'disconnected'"
                size="sm"
                variant="ghost"
                :disabled="busyId === row.id"
                @click="disconnect(row)"
              >
                {{ t("s01.initialization.repositories.disconnect") }}
              </Button>
              <Button
                v-else
                size="sm"
                variant="ghost"
                :disabled="busyId === row.id"
                @click="restore(row)"
              >
                {{ t("s01.initialization.repositories.restore") }}
              </Button>
            </div>
          </header>

          <dl class="repo__facts">
            <div class="repo__fact">
              <dt>{{ t("s01.initialization.repositories.defaultBranch") }}</dt>
              <dd>{{ row.default_branch ?? "—" }}</dd>
            </div>
            <div class="repo__fact">
              <dt>{{ t("s01.initialization.repositories.selectedBranch") }}</dt>
              <dd>{{ row.selected_branch ?? "—" }}</dd>
            </div>
            <div class="repo__fact">
              <dt>{{ t("s01.initialization.repositories.selectedCommit") }}</dt>
              <dd>
                <code v-if="row.selected_commit">{{
                  row.selected_commit
                }}</code>
                <span v-else class="repo__muted">
                  {{ t("s01.initialization.repositories.selectedCommitEmpty") }}
                </span>
              </dd>
            </div>
            <div class="repo__fact">
              <dt>{{ t("s01.initialization.repositories.headCommit") }}</dt>
              <dd>
                <code>{{ row.head_commit_sha ?? "—" }}</code>
              </dd>
            </div>
            <div class="repo__fact">
              <dt>{{ t("s01.initialization.repositories.url") }}</dt>
              <dd>
                <code class="repo__url">{{ row.url }}</code>
              </dd>
            </div>
            <div class="repo__fact">
              <dt>
                {{ t("s01.initialization.repositories.verificationChecks") }}
              </dt>
              <dd>
                <span v-if="row.last_verified_at">
                  {{ formatDateTime(row.last_verified_at) }}
                </span>
                <span v-else class="repo__muted">
                  {{ t("s01.initialization.repositories.verificationPending") }}
                </span>
              </dd>
            </div>
            <div class="repo__fact">
              <dt>{{ t("s01.initialization.repositories.rescan") }}</dt>
              <dd>
                <span v-if="row.last_scanned_at">
                  {{ formatDateTime(row.last_scanned_at) }}
                </span>
                <span v-else class="repo__muted">
                  {{ t("s01.initialization.common.empty") }}
                </span>
              </dd>
            </div>
          </dl>
        </li>
      </ul>
    </section>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.ui-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
}

.ui-card--warn {
  border-color: var(--warning);
}

.ui-card__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
}

.ui-card__title {
  margin: 0;
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ui-card__hint {
  margin: var(--space-1) 0 0;
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.ui-card__empty {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--text-tertiary);
}

.drift-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.drift-list__item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-small);
}

.drift-list__arrow {
  color: var(--text-tertiary);
}

.provider-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.provider-list__item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  font-size: var(--font-size-small);
}

.provider-list__name {
  min-width: 120px;
  color: var(--text-primary);
}

.provider-list__scopes {
  flex: 1;
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  color: var(--text-secondary);
}

.repo-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  margin: 0;
  padding: 0;
  list-style: none;
}

.repo {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
}

.repo__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
}

.repo__title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.repo__name {
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.repo__actions {
  display: flex;
  gap: var(--space-1);
  flex: none;
  flex-wrap: wrap;
}

.repo__facts {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: var(--space-3);
  margin: 0;
}

.repo__fact {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.repo__fact dt {
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.repo__fact dd {
  margin: 0;
  font-size: var(--font-size-small);
  color: var(--text-primary);
}

.repo__muted {
  color: var(--text-tertiary);
}

.repo__url {
  word-break: break-all;
}
</style>
