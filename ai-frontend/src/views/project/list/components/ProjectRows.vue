<script setup lang="ts">
import { useI18n } from "vue-i18n";
import Avatar from "@/components/ui/avatar/Avatar.vue";
import type { ProjectActionKind, ProjectEntry } from "../project-entry";
import ProjectActionsMenu from "./ProjectActionsMenu.vue";
import ProjectMeta from "./ProjectMeta.vue";

/**
 * 项目列表视图（Projects List §6）：信息密集单列行。
 *
 * 行结构 = 头像 + 名称/短描述（可点击区）+ 客户 + 元信息 + 操作菜单。
 * 行整体可点击打开项目总控台；键盘可达性由名称按钮承载；
 * 操作菜单点击不冒泡到行。
 */
interface Props {
  entries: ProjectEntry[];
}

defineProps<Props>();
const emit = defineEmits<{
  (e: "open", id: string): void;
  (e: "action", kind: ProjectActionKind, id: string): void;
}>();

const { t } = useI18n();
</script>

<template>
  <section :aria-label="t('pm.projects.listAria')">
    <ul class="proj-rows">
      <li v-for="entry in entries" :key="entry.id">
        <div class="proj-row" @click="emit('open', entry.id)">
          <Avatar :name="entry.name" :label="entry.name" size="md" />
          <span class="proj-row__main">
            <button
              type="button"
              class="proj-row__name"
              @click.stop="emit('open', entry.id)"
            >
              {{ entry.name }}
            </button>
            <span class="proj-row__desc">{{ entry.description }}</span>
          </span>
          <span v-if="entry.customer_name" class="proj-row__customer">{{
            entry.customer_name
          }}</span>
          <ProjectMeta :entry="entry" layout="row" class="proj-row__meta" />
          <div class="proj-row__actions" @click.stop>
            <ProjectActionsMenu
              :entry="entry"
              @action="(kind, id) => emit('action', kind, id)"
            />
          </div>
        </div>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.proj-rows {
  margin: 0;
  padding: 0;
  list-style: none;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  overflow: hidden;
}

.proj-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  min-height: var(--size-row);
  padding: var(--space-2) var(--space-4);
  border-bottom: var(--hairline) solid var(--border-default);
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease-standard);
}

.proj-rows li:last-child .proj-row {
  border-bottom: 0;
}

.proj-row:hover {
  background: var(--ws-hover-veil);
}

.proj-row:focus-within {
  box-shadow: inset var(--ws-focus-ring);
}

.proj-row__main {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  flex: 1 1 0;
  min-width: 0;
}

.proj-row__name {
  padding: 0;
  border: 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
  text-align: left;
  cursor: pointer;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.proj-row__name:hover {
  color: var(--accent);
  text-decoration: underline;
}

.proj-row__name:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.proj-row__desc {
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.proj-row__customer {
  flex: none;
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.proj-row__meta {
  flex: none;
  max-width: 40%;
}

.proj-row__actions {
  flex: none;
  /* 操作菜单 hover/聚焦时显现（Projects List §6） */
  opacity: 0;
  transition: opacity var(--duration-fast) var(--ease-standard);
}

.proj-row:hover .proj-row__actions,
.proj-row:focus-within .proj-row__actions {
  opacity: 1;
}

@media (max-width: 1100px) {
  .proj-row {
    flex-wrap: wrap;
  }

  .proj-row__customer {
    display: none;
  }

  .proj-row__meta {
    max-width: none;
  }

  .proj-row__actions {
    opacity: 1;
  }
}
</style>
