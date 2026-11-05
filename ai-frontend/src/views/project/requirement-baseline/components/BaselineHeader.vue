<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { FolderPlus, ShieldCheck } from "lucide-vue-next";
import Button from "@/components/ui/button/Button.vue";

/**
 * 基线管理页头（S02-UI-06 §2）。
 *
 * 标题 Requirement Baselines + 副标题 + 两动作
 * （查看当前基线 / 准备新基线）。不带 title 走 PageFrame，由本组件承担。
 */
interface Props {
  compareMode: boolean;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "view-current"): void;
  (e: "prepare"): void;
  (e: "compare"): void;
}>();

const { t } = useI18n();
</script>

<template>
  <header class="rb-header">
    <nav
      class="rb-header__breadcrumb"
      :aria-label="t('pm.requirementBaseline.breadcrumbAria')"
    >
      <RouterLink class="rb-header__crumb" to="/projects">
        {{ t("pm.projects.pageTitle") }}
      </RouterLink>
      <span class="rb-header__crumb-sep" aria-hidden="true">/</span>
      <RouterLink class="rb-header__crumb" to="/project/requirements">
        {{ t("pm.requirements.pageTitle") }}
      </RouterLink>
      <span class="rb-header__crumb-sep" aria-hidden="true">/</span>
      <span
        class="rb-header__crumb rb-header__crumb--current"
        aria-current="page"
      >
        {{ t("pm.requirementBaseline.pageTitle") }}
      </span>
    </nav>

    <div class="rb-header__row">
      <div class="rb-header__identity">
        <h1 class="rb-header__title">
          {{ t("pm.requirementBaseline.pageTitle") }}
        </h1>
        <p class="rb-header__subtitle">
          {{ t("pm.requirementBaseline.pageSubtitle") }}
        </p>
      </div>
      <div class="rb-header__actions">
        <Button
          variant="secondary"
          size="sm"
          :aria-pressed="props.compareMode"
          @click="emit('compare')"
        >
          {{ t("pm.requirementBaseline.actionCompare") }}
        </Button>
        <Button variant="secondary" size="sm" @click="emit('view-current')">
          <ShieldCheck aria-hidden="true" />
          {{ t("pm.requirementBaseline.actionViewCurrent") }}
        </Button>
        <Button variant="primary" size="sm" @click="emit('prepare')">
          <FolderPlus aria-hidden="true" />
          {{ t("pm.requirementBaseline.actionPrepare") }}
        </Button>
      </div>
    </div>
  </header>
</template>

<style scoped>
.rb-header {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-width: 0;
}

.rb-header__breadcrumb {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
}

.rb-header__crumb {
  color: var(--text-secondary);
  text-decoration: none;
}

a.rb-header__crumb:hover {
  color: var(--accent);
}

a.rb-header__crumb:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.rb-header__crumb--current {
  color: var(--text-primary);
  font-weight: var(--weight-medium);
}

.rb-header__crumb-sep {
  color: var(--text-disabled);
}

.rb-header__row {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.rb-header__identity {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.rb-header__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  line-height: var(--line-height-h4);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h3);
  color: var(--text-primary);
}

.rb-header__subtitle {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.rb-header__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
  flex-wrap: wrap;
}
</style>
