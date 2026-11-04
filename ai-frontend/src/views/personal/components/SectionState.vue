<script setup lang="ts">
import { computed, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { PackageOpen, Shield, TriangleAlert } from "lucide-vue-next";
import Alert from "@/components/ui/alert/Alert.vue";
import Button from "@/components/ui/button/Button.vue";
import EmptyState from "@/components/ui/empty-state/EmptyState.vue";

/**
 * 区块状态容器：空态 / 错误态 / 无权限态。
 *
 * 设计约束（高保真稿 §8）：
 *   - 空态与错误态必须可区分 —— 失败绝不渲染成"没有数据"；
 *   - 错误态只塌陷本区块，其余区块保持可用；
 *   - 错误态提供 Retry（只重试本区块）；
 *   - 无权限态说明原因并给出真实可执行的下一步。
 */
type Variant = "empty" | "error" | "forbidden";

interface Props {
  variant: Variant;
  title: string;
  description?: string;
  /** 错误码 / 请求 id 等技术信息 */
  note?: string;
  actionLabel?: string;
  secondaryActionLabel?: string;
}

const props = withDefaults(defineProps<Props>(), {
  description: "",
  note: "",
  actionLabel: "",
  secondaryActionLabel: ""
});

const emit = defineEmits<{
  (e: "action"): void;
  (e: "secondaryAction"): void;
}>();

const { t } = useI18n();

const icon = computed<Component | undefined>(() => {
  if (props.variant === "error") return TriangleAlert;
  if (props.variant === "forbidden") return Shield;
  return PackageOpen;
});
</script>

<template>
  <div class="pm-state" :data-variant="variant">
    <Alert v-if="variant === 'error'" variant="danger" size="md" :title="title">
      <p>{{ description }}</p>
      <p v-if="note" class="pm-state__note">{{ note }}</p>
      <div class="pm-state__actions">
        <Button variant="secondary" size="sm" @click="emit('action')">
          {{ actionLabel || t("pm.personalHome.state.retry") }}
        </Button>
      </div>
    </Alert>

    <EmptyState
      v-else
      size="md"
      :icon="icon"
      :title="title"
      :description="description"
      :action-label="actionLabel"
      :secondary-action-label="secondaryActionLabel"
      @action="emit('action')"
      @secondary-action="emit('secondaryAction')"
    />
  </div>
</template>

<style scoped>
.pm-state {
  padding: var(--space-6);
}

.pm-state__note {
  margin: var(--space-3) 0 0;
  font-size: var(--font-size-micro);
  color: var(--ws-text-secondary);
}

.pm-state__actions {
  margin-top: var(--space-4);
}
</style>
