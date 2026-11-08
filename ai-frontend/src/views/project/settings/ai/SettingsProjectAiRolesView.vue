<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, ChevronRight, RefreshCw } from "lucide-vue-next";
import { HttpError } from "@digua/http";

import Alert from "@/components/ui/alert/Alert.vue";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";
import Switch from "@/components/ui/switch/Switch.vue";
import {
  listProjectAiRoleVersions,
  listProjectAiRoles,
  setProjectAiRoleEnabled,
  type AiRoleVersion,
  type ProjectAiRole
} from "@/api/aiRoles";
import AiTeamFrame from "./AiTeamFrame.vue";
import { useProjectSettingsFrame } from "../shared";

/**
 * Project Setting · AI Team · Roles 子页（V2 D-1=A）。
 *
 * 对接真实后端（pm-api ai_roles.rs / pm-application ai_roles.rs）：
 * - 列表 = 目录合并视图（5 模板 LEFT JOIN 项目行）：项目行缺失 = enabled false
 *   （懒生效，D-1=A——启用时才建项目行 + 版本 1.0，无需向用户解释落库细节）。
 * - 启停：写守卫 = 租户 owner/admin 或项目 owner。403 → 本页进入只读态
 *   （全部开关禁用 + 提示无权限）；409 AI_ROLE_NOT_DISABLEABLE → 提示
 *   「AI Project Manager 不可禁用」（AT-D12）。操作成功后刷新列表。
 * - 版本历史：懒加载（首次展开才请求 versions 端点），append-only 只读。
 */
const { t } = useI18n();
const settingsFrame = useProjectSettingsFrame();
const projectId = settingsFrame.projectId;

const roles = ref<ProjectAiRole[]>([]);
const loading = ref(true);
const errorMessage = ref("");
const busyRole = ref("");
/** 403 后置为 false：本调用人无写权限，全部开关进禁用态 */
const canManage = ref(true);
const actionError = ref("");

/** 版本历史：按 role_id 缓存；expandedRole = 当前展开行（懒加载） */
const expandedRole = ref("");
const versionsByRole = ref<Record<string, AiRoleVersion[]>>({});
const versionsLoading = ref("");
const versionsError = ref("");

async function load() {
  if (!projectId.value) {
    loading.value = false;
    return;
  }
  loading.value = true;
  errorMessage.value = "";
  try {
    roles.value = await listProjectAiRoles(projectId.value);
  } catch (error) {
    errorMessage.value =
      error instanceof Error ? error.message : String(error ?? "");
  } finally {
    loading.value = false;
  }
}

/** 错误判别：403（无权限）与 409（不可禁用）有专门文案，其余走原文 */
function toggleErrorText(error: unknown): string {
  if (error instanceof HttpError) {
    if (error.status === 403) {
      canManage.value = false;
      return t("aiTeam.roles.forbidden");
    }
    if (
      error.status === 409 ||
      error.code === "AI_ROLE_NOT_DISABLEABLE" ||
      error.message.includes("AI_ROLE_NOT_DISABLEABLE")
    ) {
      return t("aiTeam.roles.undisableable");
    }
  }
  return `${t("aiTeam.roles.enableFailed")}: ${
    error instanceof Error ? error.message : String(error ?? "")
  }`;
}

async function toggleRole(row: ProjectAiRole, next: boolean) {
  if (!projectId.value || busyRole.value) return;
  busyRole.value = row.role_id;
  actionError.value = "";
  try {
    await setProjectAiRoleEnabled(projectId.value, row.role_id, next);
    await load();
  } catch (error) {
    actionError.value = toggleErrorText(error);
    // 回读服务端真相（受控开关未翻转，刷新保证目录与后端一致）
    await load();
  } finally {
    busyRole.value = "";
  }
}

async function toggleHistory(row: ProjectAiRole) {
  if (expandedRole.value === row.role_id) {
    expandedRole.value = "";
    return;
  }
  expandedRole.value = row.role_id;
  versionsError.value = "";
  if (versionsByRole.value[row.role_id]) return;
  if (!projectId.value) return;
  versionsLoading.value = row.role_id;
  try {
    versionsByRole.value[row.role_id] = await listProjectAiRoleVersions(
      projectId.value,
      row.role_id
    );
  } catch (error) {
    versionsError.value = `${t("aiTeam.roles.historyLoadFailed")}: ${
      error instanceof Error ? error.message : String(error ?? "")
    }`;
  } finally {
    versionsLoading.value = "";
  }
}

const expandedVersions = computed(() =>
  expandedRole.value ? (versionsByRole.value[expandedRole.value] ?? null) : null
);

function formatTime(value: string | null): string {
  return value ? new Date(value).toLocaleString() : "—";
}

onMounted(load);
</script>

<template>
  <AiTeamFrame :description="t('aiTeam.roles.description')">
    <template #actions>
      <Button
        variant="ghost"
        size="sm"
        :disabled="loading"
        data-testid="ai-roles-refresh"
        @click="load"
      >
        <RefreshCw :size="14" aria-hidden="true" />
        {{ t("aiTeam.roles.refresh") }}
      </Button>
    </template>

    <div class="ai-roles">
      <Alert v-if="errorMessage" variant="danger" data-testid="ai-roles-error">
        {{ t("aiTeam.roles.loadFailed") }}: {{ errorMessage }}
      </Alert>

      <!-- 无项目上下文：不发请求，指引从总控台进入（scope 哨兵不做项目 id） -->
      <EmptyState
        v-if="!projectId"
        :title="t('aiTeam.noProject.title')"
        :description="t('aiTeam.noProject.hint')"
        data-testid="ai-roles-no-project"
      />

      <template v-else>
        <Alert
          v-if="actionError"
          variant="warning"
          data-testid="ai-roles-action-error"
        >
          {{ actionError }}
        </Alert>

        <EmptyState
          v-if="!loading && roles.length === 0"
          :title="
            errorMessage
              ? t('aiTeam.roles.loadFailed')
              : t('aiTeam.roles.empty')
          "
          data-testid="ai-roles-empty"
        />

        <section
          v-else
          class="ai-roles__table"
          data-testid="ai-roles-list"
          :aria-busy="loading"
        >
          <span class="ai-roles__head" aria-hidden="true">
            <span>{{ t("aiTeam.roles.column.role") }}</span>
            <span>{{ t("aiTeam.roles.column.category") }}</span>
            <span>{{ t("aiTeam.roles.column.status") }}</span>
            <span>{{ t("aiTeam.roles.column.version") }}</span>
            <span>{{ t("aiTeam.roles.column.updatedAt") }}</span>
            <span>{{ t("aiTeam.roles.column.history") }}</span>
          </span>

          <template v-for="row in roles" :key="row.role_id">
            <div class="ai-roles__item">
              <div
                class="ai-roles__row"
                :data-testid="`ai-role-row-${row.role_key}`"
              >
                <span class="ai-roles__name">
                  <span class="ai-roles__name-main">{{ row.name }}</span>
                  <span class="ai-roles__name-key">{{ row.role_key }}</span>
                </span>
                <span>
                  <Badge variant="neutral" size="sm">
                    {{ t(`aiTeam.roles.category.${row.category}`) }}
                  </Badge>
                </span>
                <span>
                  <Switch
                    :model-value="row.enabled"
                    size="sm"
                    :disabled="!canManage || busyRole === row.role_id"
                    :label="t('aiTeam.roles.column.status')"
                    hide-label
                    :state-label="{
                      on: t('aiTeam.roles.statusOn'),
                      off: t('aiTeam.roles.statusOff')
                    }"
                    :data-testid="`ai-role-toggle-${row.role_key}`"
                    @update:model-value="next => toggleRole(row, next)"
                  />
                </span>
                <span class="ai-roles__cell">
                  {{ row.current_version?.version ?? "—" }}
                </span>
                <span class="ai-roles__cell">
                  {{ formatTime(row.current_version?.created_at ?? null) }}
                </span>
                <span>
                  <Button
                    variant="ghost"
                    size="sm"
                    :data-testid="`ai-role-history-${row.role_key}`"
                    @click="toggleHistory(row)"
                  >
                    <component
                      :is="
                        expandedRole === row.role_id
                          ? ChevronDown
                          : ChevronRight
                      "
                      :size="14"
                      aria-hidden="true"
                    />
                    {{
                      expandedRole === row.role_id
                        ? t("aiTeam.roles.action.collapseHistory")
                        : t("aiTeam.roles.action.expandHistory")
                    }}
                  </Button>
                </span>
              </div>

              <!-- 版本历史（懒加载展开行） -->
              <div
                v-if="expandedRole === row.role_id"
                class="ai-roles__history"
                :data-testid="`ai-role-history-panel-${row.role_key}`"
              >
                <p
                  v-if="versionsLoading === row.role_id"
                  class="ai-roles__history-note"
                >
                  …
                </p>
                <Alert v-else-if="versionsError" variant="danger" size="sm">
                  {{ versionsError }}
                </Alert>
                <p
                  v-else-if="!expandedVersions || expandedVersions.length === 0"
                  class="ai-roles__history-note"
                >
                  {{ t("aiTeam.roles.historyEmpty") }}
                </p>
                <ol v-else class="ai-roles__history-list">
                  <li
                    v-for="version in expandedVersions"
                    :key="version.id"
                    class="ai-roles__history-item"
                  >
                    <span class="ai-roles__history-version">{{
                      version.version
                    }}</span>
                    <span>{{ formatTime(version.created_at) }}</span>
                  </li>
                </ol>
              </div>
            </div>
          </template>
        </section>
      </template>
    </div>
  </AiTeamFrame>
</template>

<style scoped>
.ai-roles {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

/* 列宽固定 + 横向滚动兜底（设置主列窄于表格时不挤压角色名） */
.ai-roles__table {
  display: flex;
  flex-direction: column;
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  overflow-x: auto;
}

.ai-roles__item {
  border-bottom: var(--border-1) solid var(--ws-border);
}

.ai-roles__item:last-child {
  border-bottom: none;
}

.ai-roles__head,
.ai-roles__row {
  display: grid;
  grid-template-columns: minmax(180px, 1fr) 104px 88px 104px 170px 150px;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-4);
  min-width: 720px;
}

.ai-roles__head {
  height: var(--size-row);
  border-bottom: var(--border-1) solid var(--ws-border);
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-roles__row {
  min-height: var(--size-row);
}

.ai-roles__name {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  padding: var(--space-2) 0;
}

.ai-roles__name-main {
  color: var(--ws-text-primary);
  font-weight: var(--weight-semibold);
}

.ai-roles__name-key {
  color: var(--ws-text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-roles__cell {
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

/* 版本历史展开行：跨全宽的凹底列表 */
.ai-roles__history {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0 var(--space-4) var(--space-3);
  padding: var(--space-3);
  background: var(--surface-sunken);
  border-radius: var(--radius-sm);
}

.ai-roles__history-note {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-roles__history-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ai-roles__history-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  color: var(--ws-text-primary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

.ai-roles__history-version {
  font-family: var(--font-mono);
}
</style>
