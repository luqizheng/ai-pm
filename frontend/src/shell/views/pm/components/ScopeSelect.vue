<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";

import { Select } from "@/components/ui/select";
import { useScopeStore, SCOPE_ALL } from "@/api/scope";

const { t } = useI18n();
const scope = useScopeStore();

onMounted(() => {
  void scope.ensureLoaded().catch(() => {
    // 项目列表失败不阻塞页面：下拉为空，各列表仍可全量查询
  });
});

const options = computed(() => [
  { value: SCOPE_ALL, label: t("pm.scope.all") },
  ...scope.projects.map(project => ({
    value: project.id,
    label: `${project.code} · ${project.name}`
  }))
]);
</script>

<template>
  <Select
    v-model="scope.currentProjectId"
    :options="options"
    :aria-label="t('pm.scope.label')"
    size="sm"
  />
</template>
