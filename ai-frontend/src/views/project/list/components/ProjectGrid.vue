<script setup lang="ts">
import { useI18n } from "vue-i18n";
import Avatar from "@/components/ui/avatar/Avatar.vue";
import type { ProjectActionKind, ProjectEntry } from "../project-entry";
import ProjectActionsMenu from "./ProjectActionsMenu.vue";
import ProjectMeta from "./ProjectMeta.vue";

/**
 * 项目网格视图（Projects List §7）：等宽卡片。
 *
 * 卡片 = 头像 + 名称（可点击）+ 操作菜单 + 短描述 + 元信息（纵排）。
 * 卡片整体可点击打开项目总控台；hover 抬升为轻量反馈（阴影 + 边框增强）。
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
  <section :aria-label="t('pm.projects.gridAria')">
    <div class="proj-grid">
      <article
        v-for="entry in entries"
        :key="entry.id"
        class="proj-card"
        @click="emit('open', entry.id)"
      >
        <header class="proj-card__head">
          <Avatar :name="entry.name" :label="entry.name" size="sm" />
          <button
            type="button"
            class="proj-card__name"
            :title="entry.name"
            @click.stop="emit('open', entry.id)"
          >
            {{ entry.name }}
          </button>
          <div class="proj-card__actions" @click.stop>
            <ProjectActionsMenu
              :entry="entry"
              @action="(kind, id) => emit('action', kind, id)"
            />
          </div>
        </header>
        <p class="proj-card__desc">{{ entry.description }}</p>
        <ProjectMeta :entry="entry" layout="card" class="proj-card__meta" />
      </article>
    </div>
  </section>
</template>

<style scoped>
.proj-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: var(--space-4);
}

.proj-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  cursor: pointer;
  transition:
    background var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard),
    box-shadow var(--duration-fast) var(--ease-standard);
}

.proj-card:hover {
  background: var(--ws-hover-veil);
  border-color: var(--border-strong);
  box-shadow: var(--shadow-md);
}

.proj-card:focus-within {
  box-shadow: var(--ws-focus-ring);
}

.proj-card__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.proj-card__name {
  flex: 1 1 0;
  min-width: 0;
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

.proj-card__name:hover {
  color: var(--accent);
  text-decoration: underline;
}

.proj-card__name:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.proj-card__actions {
  flex: none;
  opacity: 0;
  transition: opacity var(--duration-fast) var(--ease-standard);
}

.proj-card:hover .proj-card__actions,
.proj-card:focus-within .proj-card__actions {
  opacity: 1;
}

.proj-card__desc {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  min-height: calc(var(--line-height-caption) * 2);
}

.proj-card__meta {
  margin-top: auto;
  padding-top: var(--space-2);
  border-top: var(--hairline) solid var(--border-default);
}
</style>
