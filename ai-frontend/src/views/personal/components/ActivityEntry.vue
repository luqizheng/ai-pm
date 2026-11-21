<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Bot, CircleCheck, Cog, Send, Sparkles, User } from "lucide-vue-next";
import Avatar from "@/components/ui/avatar/Avatar.vue";
import Badge from "@/components/ui/badge/Badge.vue";

import type { ActivityActorKind, ActivityEventKind } from "@/api/personal-home";
import { useRelativeTime } from "../useRelativeTime";

/**
 * 近期活动条目。
 *
 * 设计约束（高保真稿 §5.2）：四类事件必须可辨，且**AI 的产出不得表现为已获人类确认的正式事实**：
 *   - 事实类（人类确认 / 系统正式提交）= 实心语义 chip + 对勾类图标；
 *   - AI 产出类（提案 / 执行完成）= accent 描边 chip + sparkles，且文案自带 AI 前缀；
 *   - 无法判定的事件一律降级为中性 chip，不凭颜色伪造事实。
 */
interface Props {
  actorKind: ActivityActorKind;
  actorName: string;
  eventKind: ActivityEventKind;
  /** 原始 action 标识 */
  action: string;
  targetType: string;
  projectName: string;
  objectRef: string | null;
  occurredAt: string;
  /** PA-05-09 未读（调用者 read_at 为空）：标题左侧 accent 圆点 */
  unread?: boolean;
}

const props = defineProps<Props>();

const emit = defineEmits<{ (e: "open"): void }>();

const { t, te } = useI18n();
const { relative } = useRelativeTime();

/** 活动摘要：已知动作走 i18n，未知动作原样展示（不猜测语义） */
const summary = computed(() => {
  const key = `pm.personalHome.activity.actions.${props.action.replace(/\./g, "_")}`;
  return te(key) ? t(key) : props.action;
});

const eventLabel = computed(() => {
  if (props.eventKind === "generic") return props.targetType;
  return t(`pm.personalHome.activity.events.${props.eventKind}`);
});

const eventVariant = computed(() => {
  if (props.eventKind === "confirmed") return "success";
  if (props.eventKind === "submitted") return "info";
  if (props.eventKind === "proposal" || props.eventKind === "aiCompleted") {
    return "accent";
  }
  return "neutral";
});

/** AI 产出用描边 chip：与"已成立的正式事实"在形状上就不同 */
const eventOutline = computed(
  () => props.eventKind === "proposal" || props.eventKind === "aiCompleted"
);

const eventIcon = computed(() => {
  if (props.eventKind === "confirmed") return CircleCheck;
  if (props.eventKind === "submitted") return Send;
  if (props.eventKind === "proposal" || props.eventKind === "aiCompleted") {
    return Sparkles;
  }
  return null;
});

const actorIcon = computed(() => {
  if (props.actorKind === "ai") return Bot;
  if (props.actorKind === "system") return Cog;
  return User;
});
</script>

<template>
  <article class="pm-activity">
    <div class="pm-activity__top">
      <span class="pm-activity__actor">
        <Avatar size="sm" :name="actorKind === 'human' ? actorName : ''">
          <template #fallback>
            <component
              :is="actorIcon"
              class="pm-activity__actor-icon"
              :data-kind="actorKind"
              :size="12"
              aria-hidden="true"
            />
          </template>
        </Avatar>
        <span class="pm-activity__name">
          {{ actorName || t(`pm.personalHome.activity.actors.${actorKind}`) }}
        </span>
        <Badge size="sm" :variant="eventVariant" :outline="eventOutline">
          <component
            :is="eventIcon"
            v-if="eventIcon"
            :size="12"
            aria-hidden="true"
          />
          {{ eventLabel }}
        </Badge>
      </span>
      <span class="pm-activity__time">{{ relative(occurredAt) }}</span>
    </div>
    <p class="pm-activity__summary">
      <span
        v-if="unread"
        class="pm-activity__unread-dot"
        :title="t('pm.personalHome.activity.unread')"
        aria-hidden="true"
      />
      {{ summary }}
    </p>
    <button type="button" class="pm-activity__object" @click="emit('open')">
      {{ projectName }}<template v-if="objectRef"> · {{ objectRef }}</template>
    </button>
  </article>
</template>

<style scoped>
.pm-activity {
  padding: var(--space-3) var(--space-4);
  border-left: var(--border-1) solid var(--ws-border);
  min-width: 0;
}

.pm-activity__top {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.pm-activity__actor {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
  flex: 1;
}

.pm-activity__actor-icon[data-kind="ai"] {
  color: var(--ws-accent);
}

.pm-activity__actor-icon[data-kind="system"] {
  color: var(--ws-text-secondary);
}

.pm-activity__name {
  font-size: var(--font-size-caption);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pm-activity__time {
  font-family: var(--font-mono);
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
  flex: none;
}

.pm-activity__summary {
  margin: var(--space-2) 0 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--ws-text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

/* PA-05-09 未读点：Read 是个人状态，未读仅提示，不强制确认 */
.pm-activity__unread-dot {
  display: inline-block;
  width: 0.5rem;
  height: 0.5rem;
  margin-right: var(--space-1);
  border-radius: var(--radius-full);
  background: var(--ws-accent);
  vertical-align: middle;
}

/* 文本级链接钮：显式清掉浏览器默认 buttonface 底色（违和根因） */
.pm-activity__object {
  display: inline-flex;
  align-items: center;
  margin-top: var(--space-2);
  padding: 0;
  background: transparent;
  border: 0;
  font: inherit;
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
  text-align: left;
  cursor: pointer;
}

.pm-activity__object:hover {
  color: var(--ws-text-primary);
  text-decoration: underline;
  text-underline-offset: 2px;
}
</style>
