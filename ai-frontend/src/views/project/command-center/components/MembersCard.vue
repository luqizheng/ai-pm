<script setup lang="ts">
import Avatar from "@/components/ui/avatar/Avatar.vue";
import Card from "@/components/ui/card/Card.vue";
import SectionHead from "./SectionHead.vue";
import type { MemberChip } from "../members";

/**
 * 项目成员卡（Pixso「Project Command Center」3:7862）。
 *
 * 只读名单：Avatar（首字母回退）+ 显示名的 pill 列表；人数构成走标题旁 meta chip。
 * 非管理入口 —— 成员管理在设置域。文案由调用方翻译后传入。
 */
interface Props {
  title: string;
  meta?: string;
  members: MemberChip[];
}

withDefaults(defineProps<Props>(), { meta: "" });
</script>

<template>
  <Card class="pm-members">
    <template #header>
      <SectionHead :title="title" :meta="meta" />
    </template>

    <ul class="pm-members__list">
      <li v-for="member in members" :key="member.id" class="pm-members__pill">
        <Avatar
          class="pm-members__avatar"
          size="sm"
          :name="member.name"
          :label="member.name"
        />
        <span class="pm-members__name">{{ member.name }}</span>
      </li>
    </ul>
  </Card>
</template>

<style scoped>
.pm-members :deep(.ui-card__header) {
  padding: var(--space-3) var(--space-3) 0;
  border-bottom: 0;
}

.pm-members :deep(.ui-card__body) {
  padding: 0 var(--space-3) var(--space-3);
}

.pm-members__list {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.pm-members__pill {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: 2px var(--space-2) 2px 4px;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-full);
}

.pm-members__avatar {
  flex: none;
}

.pm-members__name {
  font-size: var(--font-size-nano);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  white-space: nowrap;
}
</style>
