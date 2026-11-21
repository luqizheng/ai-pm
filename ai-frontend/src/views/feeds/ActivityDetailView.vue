<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { FileText } from "lucide-vue-next";

import PageFrame from "@/components/shell/PageFrame.vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { fetchActivityDetail, type FeedItem } from "@/api/feeds";
import { translateError } from "@/api/base";
import { useRelativeTime } from "@/views/personal/useRelativeTime";

/**
 * 活动详情（PA-05 UI 屏 05「Activity detail」）。
 *
 * 历史记录语义（画布口径）：活动描述过去，不改变流程状态——打开即标已读
 * （仅调用者本人 read 状态，后端完成），「已读 ≠ 已处理/已批准」提示常驻。
 */
const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const { relative } = useRelativeTime();

const loading = ref(true);
const error = ref<string | null>(null);
const activity = ref<FeedItem | null>(null);

const activityId = computed(() => String(route.params.activityId));

const detailJson = ref<string | null>(null);

async function load(): Promise<void> {
  if (!activityId.value) return;
  loading.value = true;
  error.value = null;
  try {
    activity.value = await fetchActivityDetail(activityId.value);
    detailJson.value = activity.value.detail
      ? JSON.stringify(activity.value.detail, null, 2)
      : null;
  } catch (err) {
    error.value = translateError(err);
  } finally {
    loading.value = false;
  }
}

function openItem(): void {
  if (!activity.value) return;
  if (activity.value.target_type === "requirement") {
    void router.push(`/project/requirements/${activity.value.target_id}`);
  } else if (activity.value.target_type === "adr") {
    void router.push("/project/design");
  } else if (activity.value.project_id) {
    void router.push(
      `/project/command-center?project_id=${activity.value.project_id}`
    );
  }
}

function back(): void {
  void router.push("/feeds");
}

onMounted(load);
watch(activityId, () => void load());
</script>

<template>
  <PageFrame
    :title="activity?.summary ?? $t('pm.feeds.detailActivityTitle')"
    :breadcrumbs="[
      { label: $t('shell.app.side.item.feeds'), to: '/feeds' },
      { label: $t('pm.feeds.detailActivityTitle') }
    ]"
  >
    <p v-if="error" class="ad-error">{{ error }}</p>

    <EmptyState
      v-if="!loading && !activity"
      :title="$t('pm.feeds.detailEmpty')"
    />

    <template v-else-if="activity">
      <!-- 眉行（屏 05）：HISTORICAL ACTIVITY / 类型 -->
      <p class="ad-eyebrow">
        {{ $t("pm.feeds.detailEyebrow", { type: activity.activity_type }) }}
      </p>

      <!-- 主体：actor + 时间 + 摘要 -->
      <div class="ad-meta">
        <Badge variant="neutral">
          {{
            activity.actor_type === "agent"
              ? $t("pm.feeds.actorAgent")
              : $t("pm.feeds.actorHuman")
          }}
        </Badge>
        <span class="ad-meta__time">{{
          $t("pm.feeds.detailMeta", {
            time: relative(activity.created_at),
            full: new Date(activity.created_at).toLocaleString()
          })
        }}</span>
      </div>
      <p class="ad-body">{{ activity.summary }}</p>

      <!-- 变更明细（detail JSON；结构化证据引用） -->
      <section v-if="detailJson" class="ad-card">
        <h2 class="ad-card__title">{{ $t("pm.feeds.detailChangeTitle") }}</h2>
        <pre class="ad-card__json">{{ detailJson }}</pre>
      </section>

      <!-- 受影响事项（Affected Work Item） -->
      <section class="ad-card">
        <h2 class="ad-card__title">{{ $t("pm.feeds.detailAffectedTitle") }}</h2>
        <button type="button" class="ad-affected" @click="openItem">
          <FileText :size="14" aria-hidden="true" />
          {{ activity.target_type }} · {{ activity.item_title }}
        </button>
        <p class="ad-note">
          {{ $t("pm.feeds.detailAffectedNote") }}
        </p>
      </section>

      <!-- 个人状态说明（PA-05-09：Read 仅本人，不等于处理/批准） -->
      <p class="ad-personal">
        {{ $t("pm.feeds.detailPersonalNote") }}
      </p>

      <div class="ad-actions">
        <Button variant="secondary" size="sm" @click="back">
          {{ $t("pm.feeds.closeActivity") }}
        </Button>
      </div>
    </template>
  </PageFrame>
</template>

<style scoped>
.ad-error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--ws-color-danger-text);
}

.ad-eyebrow {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  font-weight: var(--weight-medium);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-tertiary);
}

.ad-meta {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.ad-meta__time {
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.ad-body {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: 1.45;
  color: var(--text-primary);
}

.ad-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-sunken);
  padding: var(--space-4);
}

.ad-card__title {
  margin: 0 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ad-card__json {
  margin: 0;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-base);
  font-family: var(--font-mono, monospace);
  font-size: var(--font-size-micro);
  color: var(--text-secondary);
  white-space: pre-wrap;
}

.ad-affected {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  border: none;
  background: transparent;
  padding: 0 0 var(--space-2);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  font-weight: var(--weight-medium);
  color: var(--ws-accent);
  cursor: pointer;
}

.ad-note {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-micro);
  color: var(--text-tertiary);
}

.ad-personal {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  color: var(--text-tertiary);
}

.ad-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
</style>
