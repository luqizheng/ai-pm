<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import {
  ArrowUpRight,
  Box,
  FileText,
  History,
  PanelRightClose,
  Pin,
  Sparkles
} from "lucide-vue-next";

import Badge from "@/components/ui/badge/Badge.vue";
import IconTooltip from "./IconTooltip.vue";
import { useAuthStore } from "@/stores/auth";
import { useShellStore } from "@/stores/shell";
import { useAssistantStore, type AssistantTab } from "@/stores/assistant";
import type { RelatedLink } from "./demo";

/**
 * 右上下文栏（AI Collaboration V1.0 / DR-C3 + DR-C8 第 ③ 步，288px）。
 *
 * **四个一级 Tab**（Context / Sources / Results / Actions）+ History Drawer——
 * History 不是第五个主 Tab，走二级入口的 Drawer；Drawer 覆盖本栏视觉区，
 * **不改四栏宽度**。
 *
 * 数据来源（**同源，不另起 fetch**）：
 *   - Context：壳 store 的 `assistantContext`（上下文单一真源）+ 当前路由对象；
 *   - Sources：**真实注入记账**（后端 `metadata.injection`，经会话机制写入
 *     assistant store）——没有注入数据就显示未注入，**不冒充**注入面；
 *   - Results / Actions / History：会话派生视图（assistant store），
 *     由 `useAssistantConversation` 作为唯一写者发布。
 *
 * 当前项目取自 store：切换项目时随之更新；切换过程中不显示旧项目数据
 * （Figma「07」No stale project data is displayed）。
 */

const { t } = useI18n();
const route = useRoute();
const shell = useShellStore();
const auth = useAuthStore();
const assistant = useAssistantStore();

/** mock 预览（本地 dev / 单测）：展示设计稿样本；发布构建走真实仓库上下文 */
const mockPreview = import.meta.env.VITE_USE_MOCK === "1";

interface DemoContext {
  organization: string;
  repository: {
    visibility: string;
    defaultBranch: string;
    role: string;
    owner: string;
  };
  links: RelatedLink[];
}

const demo = ref<DemoContext | null>(null);

onMounted(async () => {
  if (!mockPreview) return;
  const module = await import("./demo");
  demo.value = {
    organization: module.demoProject.organization,
    repository: {
      visibility: module.demoRepository.visibility,
      defaultBranch: module.demoRepository.defaultBranch,
      role: module.demoRepository.role,
      owner: module.demoRepository.owner
    },
    links: module.demoRelatedLinks
  };
});

const organization = computed(
  () => demo.value?.organization ?? auth.organization ?? ""
);

const metadata = computed(() => {
  if (demo.value) {
    return [
      { key: "visibility", value: demo.value.repository.visibility },
      { key: "defaultBranch", value: demo.value.repository.defaultBranch },
      { key: "yourRole", value: demo.value.repository.role },
      { key: "owner", value: demo.value.repository.owner }
    ];
  }
  // 真实态：仓库元数据取自单一真源（shell.assistantContext）
  const context = shell.assistantContext;
  const rows: Array<{ key: string; value: string }> = [];
  if (context.defaultBranch) {
    rows.push({ key: "defaultBranch", value: context.defaultBranch });
  }
  if (context.repositoryRole) {
    rows.push({ key: "yourRole", value: context.repositoryRole });
  }
  return rows;
});

const relatedLinks = computed<RelatedLink[]>(() => demo.value?.links ?? []);

/** 当前项目（切换中为 null → 显示切换态而非旧项目） */
const currentProject = computed(() => shell.currentProject);

/** 助手上下文单一真源（Batch 1）：注入源清单与指令栏/会话页同源。 */
const assistantContext = computed(() => shell.assistantContext);

/**
 * 助手可用上下文的作用域串。
 *
 * 与指令栏共用 shell store 的 `assistantContext`（整合计划 Batch 1 单一
 * 真源）；切换中不回落旧项目名（Figma「07」No stale project data is
 * displayed）。
 */
const commandScope = computed(() => {
  const context = shell.assistantContext;
  const name = context.projectName ?? t("shell.app.project.switching");
  const branch = demo.value?.repository.defaultBranch || context.defaultBranch;
  return branch ? `${name} / Repository / ${branch}` : name;
});

/** 四个一级 Tab（DR-C3）。 */
const TABS: Array<{ id: AssistantTab; labelKey: string }> = [
  { id: "context", labelKey: "shell.app.context.tabs.context" },
  { id: "sources", labelKey: "shell.app.context.tabs.sources" },
  { id: "results", labelKey: "shell.app.context.tabs.results" },
  { id: "actions", labelKey: "shell.app.context.tabs.actions" }
];

const activeTab = computed(() => assistant.activeTab);

/**
 * 已附加对象（归属轴 Attached）——**权威读面**（GET context-items，B5）。
 *
 * 线程锚定（项目 / 仓库）+ 对象级条目两段合成：对象条目来自
 * `assistant.contextItems`（含版本引用与双轴状态），项目 / 仓库来自
 * `assistantContext`（AI 当前确实带着的线程作用域）。未锚定如实显示。
 */
const attachedObjects = computed(() => {
  const context = assistantContext.value;
  const items: Array<{
    key: string;
    label: string;
    version?: string;
    lifecycle?: string;
  }> = [];
  if (context.projectName) {
    items.push({
      key: "project",
      label: context.projectName,
      version: context.projectId ?? undefined
    });
  }
  if (context.repositoryName) {
    items.push({
      key: "repository",
      label: context.repositoryName,
      version: context.defaultBranch ?? undefined
    });
  }
  for (const item of assistant.contextItems) {
    if (item.attachment !== "attached") continue;
    items.push({
      key: item.id,
      label: item.object_code ?? item.object_id,
      version:
        item.version != null
          ? `v${item.version}`
          : item.row_version != null
            ? `row ${item.row_version}`
            : undefined,
      lifecycle: item.lifecycle
    });
  }
  return items;
});

/**
 * 可用对象（归属轴 Available）——当前页面的业务对象。
 *
 * 由当前路由推导（类型 + id）；对象级上下文模型未落地（G3/G4），此处**只
 * 呈现当前页面对象**，不列全项目对象。切换页面即变，**不自动 Attached**。
 */
const availableObject = computed(() => {
  const path = route.path;
  const segments = path.split("/").filter(Boolean);
  const last = segments.at(-1) ?? "";
  const isDetail = /^[0-9a-f-]{8,}$/i.test(last) || /^\d+$/.test(last);
  if (!isDetail) return null;
  const typeKey = path.includes("/design/")
    ? "design"
    : path.includes("/requirements/")
      ? "requirement"
      : path.includes("/tasks/") || path.includes("/execution/")
        ? "task"
        : "object";
  return {
    typeKey,
    typeLabel: t(`shell.app.context.objectTypes.${typeKey}`),
    id: last
  };
});

/** 注入记账（真实；无则 null —— 未收到 AI 回复即未注入）。 */
const injection = computed(() => assistant.injection);

/** 派生结果 / 动作（会话机制发布）。 */
const results = computed(() => assistant.results);
const actions = computed(() => assistant.actions);

/** History Drawer（五类 + 全部）。 */
const HISTORY_CATEGORIES = [
  "all",
  "conversation",
  "ai_action",
  "context_change",
  "approval",
  "execution"
] as const;

const historyCategories = computed(() =>
  HISTORY_CATEGORIES.map(id => ({
    id,
    labelKey: `shell.app.context.historyCategories.category.${id}`
  }))
);

const historyEvents = computed(() =>
  [...assistant.filteredHistory].sort((a, b) =>
    b.createdAt.localeCompare(a.createdAt)
  )
);
</script>

<template>
  <aside class="shell-context" :aria-label="t('shell.app.context.title')">
    <!-- 头部：标题 + History 入口 + pin + 关闭 -->
    <div class="shell-context__header">
      <h2 class="shell-context__title">{{ t("shell.app.context.title") }}</h2>
      <IconTooltip :content="t('shell.app.context.history')" side="bottom">
        <button
          type="button"
          class="shell-context__icon-button"
          :aria-label="t('shell.app.context.history')"
          @click="assistant.openHistory()"
        >
          <History :size="16" aria-hidden="true" />
        </button>
      </IconTooltip>
      <IconTooltip :content="t('shell.app.context.pin')" side="bottom">
        <button
          type="button"
          class="shell-context__icon-button is-pinned"
          :aria-label="t('shell.app.context.pin')"
        >
          <Pin :size="16" aria-hidden="true" />
        </button>
      </IconTooltip>
      <IconTooltip :content="t('shell.app.context.close')" side="bottom">
        <button
          type="button"
          class="shell-context__icon-button"
          :aria-label="t('shell.app.context.close')"
          @click="shell.toggleContext()"
        >
          <PanelRightClose :size="16" aria-hidden="true" />
        </button>
      </IconTooltip>
    </div>

    <!-- 四个一级 Tab（切换不重排壳） -->
    <div class="shell-context__tabs" role="tablist">
      <button
        v-for="tab in TABS"
        :key="tab.id"
        type="button"
        role="tab"
        class="shell-context__tab"
        :class="{ 'is-active': activeTab === tab.id }"
        :aria-selected="activeTab === tab.id"
        @click="assistant.setActiveTab(tab.id)"
      >
        {{ t(tab.labelKey) }}
      </button>
    </div>

    <div class="shell-context__body">
      <!-- ── Context：当前项目 / 仓库 / 关联 / 已附加 / 可用对象 ── -->
      <template v-if="activeTab === 'context'">
        <section class="shell-context__section">
          <p class="shell-context__eyebrow">
            {{ t("shell.app.context.currentProject") }}
          </p>
          <div class="shell-context__identity">
            <span class="shell-context__project-icon" aria-hidden="true">
              <Box :size="16" />
            </span>
            <span class="shell-context__identity-text">
              <span class="shell-context__project-name">{{
                currentProject?.name ?? t("shell.app.project.switching")
              }}</span>
              <span v-if="organization" class="shell-context__project-org">
                {{ organization }}
              </span>
            </span>
          </div>
          <p class="shell-context__description">
            {{
              currentProject?.description ?? t("shell.app.project.switching")
            }}
          </p>
        </section>

        <hr class="shell-context__divider" />

        <section class="shell-context__section">
          <h3 class="shell-context__heading">
            {{ t("shell.app.context.repositoryDetails") }}
          </h3>
          <dl class="shell-context__meta">
            <div
              v-for="row in metadata"
              :key="row.key"
              class="shell-context__meta-row"
            >
              <dt class="shell-context__meta-label">
                {{ t(`shell.app.context.fields.${row.key}`) }}
              </dt>
              <dd class="shell-context__meta-value">{{ row.value }}</dd>
            </div>
          </dl>
        </section>

        <hr class="shell-context__divider" />

        <!-- 相关文档（当前无真实文档源时整段不渲染） -->
        <section v-if="relatedLinks.length" class="shell-context__section">
          <h3 class="shell-context__heading">
            {{ t("shell.app.context.related") }}
          </h3>
          <a
            v-for="link in relatedLinks"
            :key="link.id"
            class="shell-context__link"
            href="#"
            @click.prevent
          >
            <FileText :size="14" aria-hidden="true" />
            <span class="shell-context__link-text">{{ link.label }}</span>
            <ArrowUpRight :size="12" aria-hidden="true" />
          </a>
        </section>

        <hr class="shell-context__divider" />

        <!-- 已附加对象（Attached） -->
        <section class="shell-context__section">
          <h3 class="shell-context__heading">
            {{ t("shell.app.context.attached") }}
          </h3>
          <p v-if="attachedObjects.length === 0" class="shell-context__empty">
            {{ t("shell.app.context.notAnchored") }}
          </p>
          <ul v-else class="shell-context__objects">
            <li
              v-for="item in attachedObjects"
              :key="item.key"
              class="shell-context__object"
            >
              <span class="shell-context__object-name">{{ item.label }}</span>
              <span class="shell-context__object-version">{{
                item.version ?? t("shell.app.context.objectTypes.project")
              }}</span>
              <Badge variant="info">{{
                t("shell.app.context.state.attached")
              }}</Badge>
            </li>
          </ul>
        </section>

        <hr class="shell-context__divider" />

        <!-- 可用对象（Available）：当前页面对象，默认不附加 -->
        <section class="shell-context__section">
          <h3 class="shell-context__heading">
            {{ t("shell.app.context.available") }}
          </h3>
          <p v-if="!availableObject" class="shell-context__empty">
            {{ t("shell.app.context.noAvailableObject") }}
          </p>
          <ul v-else class="shell-context__objects">
            <li class="shell-context__object">
              <span class="shell-context__object-name">
                {{ availableObject.typeLabel }}
              </span>
              <span class="shell-context__object-version">
                {{ availableObject.id }}
              </span>
              <Badge>{{ t("shell.app.context.state.available") }}</Badge>
            </li>
          </ul>
        </section>
      </template>

      <!-- ── Sources：真实注入记账（不冒充注入面） ── -->
      <template v-else-if="activeTab === 'sources'">
        <section class="shell-context__ai">
          <div class="shell-context__ai-head">
            <Sparkles :size="14" aria-hidden="true" />
            <span>{{ t("shell.app.context.aiNotice") }}</span>
          </div>
          <p class="shell-context__ai-text">
            {{ commandScope }} · {{ t("shell.app.context.noAttachment") }}
          </p>
          <div class="shell-context__ai-sources">
            <span class="shell-context__ai-sources-label">{{
              t("shell.app.context.injectionSources")
            }}</span>
            <span class="shell-context__ai-source">
              {{ t("shell.app.context.sourceProject") }}
              <template v-if="assistantContext.projectName">
                · {{ assistantContext.projectName }}
              </template>
            </span>
            <span class="shell-context__ai-source">
              {{ t("shell.app.context.sourceRepository") }}
              <template v-if="assistantContext.repositoryName">
                · {{ assistantContext.repositoryName
                }}<template v-if="assistantContext.defaultBranch">
                  /{{ assistantContext.defaultBranch }}</template
                >
              </template>
              <template v-else>
                · {{ t("shell.app.context.notAnchored") }}
              </template>
            </span>
          </div>
        </section>

        <hr class="shell-context__divider" />

        <!-- 注入记账：只认后端落库的 metadata.injection -->
        <section class="shell-context__section">
          <h3 class="shell-context__heading">
            {{ t("shell.app.context.injectionLedger") }}
          </h3>
          <p v-if="!injection" class="shell-context__empty">
            {{ t("shell.app.context.noInjection") }}
          </p>
          <dl v-else class="shell-context__meta">
            <div class="shell-context__meta-row">
              <dt class="shell-context__meta-label">
                {{ t("shell.app.context.injectionIncluded") }}
              </dt>
              <dd class="shell-context__meta-value">
                {{
                  injection.sectionsIncluded.length
                    ? injection.sectionsIncluded.join(", ")
                    : t("shell.app.context.none")
                }}
              </dd>
            </div>
            <div class="shell-context__meta-row">
              <dt class="shell-context__meta-label">
                {{ t("shell.app.context.injectionDropped") }}
              </dt>
              <dd class="shell-context__meta-value">
                {{
                  injection.dropped.length
                    ? injection.dropped.join(", ")
                    : t("shell.app.context.none")
                }}
              </dd>
            </div>
            <div class="shell-context__meta-row">
              <dt class="shell-context__meta-label">
                {{ t("shell.app.context.injectionChars") }}
              </dt>
              <dd class="shell-context__meta-value">
                {{ injection.charsTotal }}
              </dd>
            </div>
          </dl>
        </section>
      </template>

      <!-- ── Results：当前会话的 AI 产出 ── -->
      <template v-else-if="activeTab === 'results'">
        <p v-if="results.length === 0" class="shell-context__empty">
          {{ t("shell.app.context.noResults") }}
        </p>
        <ul v-else class="shell-context__objects">
          <li
            v-for="item in results"
            :key="item.id"
            class="shell-context__object"
          >
            <span class="shell-context__object-name">{{ item.title }}</span>
            <span class="shell-context__object-version">
              {{ t(`shell.app.context.responseTypes.${item.responseType}`) }}
            </span>
            <Badge v-if="item.status" variant="info">
              {{ item.status }}
            </Badge>
          </li>
        </ul>
      </template>

      <!-- ── Actions：交互记录（**权威读面** GET interactions，B5）── -->
      <template v-else>
        <p v-if="actions.length === 0" class="shell-context__empty">
          {{ t("shell.app.context.noActions") }}
        </p>
        <ul v-else class="shell-context__objects">
          <li
            v-for="entry in actions"
            :key="entry.id"
            class="shell-context__object"
          >
            <span class="shell-context__object-name">
              {{ entry.action }} · {{ entry.reference }}
            </span>
            <span class="shell-context__object-version">
              {{ entry.decided_by_name ?? t("shell.app.context.initiator.ai") }}
            </span>
            <Badge v-if="entry.status" variant="info">
              {{ entry.status }}
            </Badge>
          </li>
        </ul>
      </template>
    </div>

    <!-- History Drawer：覆盖本栏视觉区，不改四栏宽度 -->
    <div
      v-if="assistant.historyOpen"
      class="shell-context__drawer"
      role="dialog"
      :aria-label="t('shell.app.context.history')"
    >
      <div class="shell-context__drawer-head">
        <span class="shell-context__drawer-title">{{
          t("shell.app.context.history")
        }}</span>
        <button
          type="button"
          class="shell-context__icon-button"
          :aria-label="t('shell.app.context.historyClose')"
          @click="assistant.closeHistory()"
        >
          <PanelRightClose :size="16" aria-hidden="true" />
        </button>
      </div>
      <div class="shell-context__drawer-filters">
        <button
          v-for="category in historyCategories"
          :key="category.id"
          type="button"
          class="shell-context__chip"
          :class="{
            'is-active': assistant.historyCategory === category.id
          }"
          @click="assistant.openHistory(category.id)"
        >
          {{ t(category.labelKey) }}
        </button>
      </div>
      <ul class="shell-context__drawer-list">
        <li
          v-for="event in historyEvents"
          :key="event.id"
          class="shell-context__drawer-item"
        >
          <span class="shell-context__object-name">{{ event.name }}</span>
          <span class="shell-context__object-version">
            {{
              t(
                `shell.app.context.historyCategories.category.${event.category}`
              )
            }}
            <template v-if="event.actor"> · {{ event.actor }} </template>
          </span>
          <span v-if="event.status" class="shell-context__object-version">
            {{ event.status }}
          </span>
        </li>
      </ul>
    </div>
  </aside>
</template>

<style scoped>
.shell-context {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.shell-context__header {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--ws-border);
}

.shell-context__title {
  flex: 1;
  margin: 0;
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.shell-context__icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--size-control-sm);
  height: var(--size-control-sm);
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--ws-text-secondary);
  cursor: pointer;
}

.shell-context__icon-button:hover {
  background: var(--ws-hover-veil);
  color: var(--ws-text-primary);
}

.shell-context__icon-button.is-pinned {
  color: var(--ws-accent);
}

/* 四个一级 Tab */
.shell-context__tabs {
  flex: none;
  display: flex;
  gap: var(--space-1);
  padding: var(--space-2) var(--space-3) 0;
  border-bottom: 1px solid var(--ws-border);
}

.shell-context__tab {
  flex: 1;
  padding: var(--space-1) 0 var(--space-2);
  border: none;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
  cursor: pointer;
}

.shell-context__tab.is-active {
  color: var(--ws-accent);
  border-bottom-color: var(--ws-accent);
}

.shell-context__tab:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
}

.shell-context__body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--space-3);
}

.shell-context__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.shell-context__divider {
  margin: var(--space-3) 0;
  border: none;
  border-top: 1px solid var(--ws-border);
}

.shell-context__eyebrow {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-micro);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.shell-context__heading {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-body);
  font-weight: var(--weight-medium);
}

.shell-context__identity {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.shell-context__project-icon {
  display: inline-flex;
  color: var(--ws-text-secondary);
}

.shell-context__identity-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.shell-context__project-name {
  color: var(--ws-text-primary);
  font-weight: var(--weight-medium);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shell-context__project-org {
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-micro);
}

.shell-context__description {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
  line-height: 1.5;
}

.shell-context__meta {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
}

.shell-context__meta-row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-2);
}

.shell-context__meta-label {
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
}

.shell-context__meta-value {
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-caption);
  text-align: right;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shell-context__link {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
  text-decoration: none;
}

.shell-context__link:hover {
  color: var(--ws-accent);
}

.shell-context__link-text {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shell-context__ai {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  border: 1px solid var(--ws-border);
  border-radius: var(--ws-radius-card);
  background: var(--ws-elevated);
}

.shell-context__ai-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--ws-accent);
  font-size: var(--ws-font-size-caption);
}

.shell-context__ai-text {
  margin: 0;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-caption);
}

.shell-context__ai-sources {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.shell-context__ai-sources-label {
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-micro);
}

.shell-context__ai-source {
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-micro);
}

.shell-context__objects {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.shell-context__object {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-sm);
  background: var(--ws-surface);
}

.shell-context__object-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-caption);
}

.shell-context__object-version {
  flex: none;
  max-width: 45%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-micro);
}

.shell-context__empty {
  margin: 0;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-caption);
}

/* History Drawer：覆盖本栏视觉区，不改四栏宽度 */
.shell-context__drawer {
  position: absolute;
  inset: 0;
  z-index: 2;
  display: flex;
  flex-direction: column;
  background: var(--ws-sidebar);
}

.shell-context__drawer-head {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--ws-border);
}

.shell-context__drawer-title {
  flex: 1;
  color: var(--ws-text-primary);
  font-size: var(--ws-font-size-section);
  font-weight: var(--weight-semibold);
}

.shell-context__drawer-filters {
  flex: none;
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1);
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--ws-border);
}

.shell-context__chip {
  padding: 0 var(--space-2);
  height: var(--size-control-sm);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-full);
  background: transparent;
  color: var(--ws-text-secondary);
  font-size: var(--ws-font-size-micro);
  cursor: pointer;
}

.shell-context__chip.is-active {
  border-color: var(--ws-accent);
  color: var(--ws-accent);
}

.shell-context__drawer-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  margin: 0;
  padding: var(--space-2) var(--space-3);
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.shell-context__drawer-item {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-2);
  border: 1px solid var(--ws-border);
  border-radius: var(--radius-sm);
  background: var(--ws-surface);
}
</style>
