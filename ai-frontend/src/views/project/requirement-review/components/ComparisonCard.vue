<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { ArrowLeftRight } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import type { ComparisonBlock, ComparisonBlockKind } from "../review-contract";

/**
 * 与既有需求的并排比较卡（S02-UI-03 §4D + Deliverable 3）。
 *
 * 每个差异块按类型高亮：新增（success）/ 修改（warning）/ 删除（danger 删除线）/
 * 潜在重复（accent 描边），左右两列分别为提案内容与既有需求内容；
 * 无相似需求时显示「按新建需求处理」提示。
 */
interface Props {
  existingCode: string | null;
  blocks: ComparisonBlock[];
}

const props = defineProps<Props>();
const { t } = useI18n();

const blockMeta: Record<
  ComparisonBlockKind,
  { key: string; variant: "success" | "warning" | "danger" | "accent" }
> = {
  added: { key: "blockAdded", variant: "success" },
  modified: { key: "blockModified", variant: "warning" },
  removed: { key: "blockRemoved", variant: "danger" },
  duplication: { key: "blockDuplication", variant: "accent" }
};

const hasBlocks = () => props.blocks.length > 0;
</script>

<template>
  <section class="rrc" :aria-label="t('pm.requirementReview.comparisonTitle')">
    <div class="rrc__head">
      <h4 class="rrc__title">
        {{ t("pm.requirementReview.comparisonTitle") }}
      </h4>
      <ArrowLeftRight class="rrc__icon" aria-hidden="true" />
    </div>

    <p v-if="!hasBlocks()" class="rrc__none">
      {{ t("pm.requirementReview.comparisonNone") }}
    </p>

    <template v-else>
      <div class="rrc__columns" role="presentation">
        <span class="rrc__col-label">{{
          t("pm.requirementReview.comparisonProposalSide")
        }}</span>
        <span class="rrc__col-label">
          {{
            t("pm.requirementReview.comparisonExistingSide", {
              code: existingCode ?? ""
            })
          }}
        </span>
      </div>
      <ul class="rrc__blocks">
        <li
          v-for="block in props.blocks"
          :key="block.id"
          class="rrc__block"
          :class="`rrc__block--${block.kind}`"
        >
          <div class="rrc__block-head">
            <Badge
              :variant="blockMeta[block.kind].variant"
              size="sm"
              :outline="block.kind === 'duplication'"
            >
              {{ t(`pm.requirementReview.${blockMeta[block.kind].key}`) }}
            </Badge>
          </div>
          <div class="rrc__block-body">
            <p
              class="rrc__cell"
              :class="{ 'rrc__cell--removed': block.kind === 'removed' }"
            >
              <span v-if="block.proposalText">{{ block.proposalText }}</span>
              <span v-else class="rrc__cell-empty">—</span>
            </p>
            <p
              class="rrc__cell"
              :class="{ 'rrc__cell--removed': block.kind === 'removed' }"
            >
              <span v-if="block.existingText">{{ block.existingText }}</span>
              <span v-else class="rrc__cell-empty">—</span>
            </p>
          </div>
          <p v-if="block.note" class="rrc__note">{{ block.note }}</p>
        </li>
      </ul>
    </template>
  </section>
</template>

<style scoped>
.rrc {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  padding: var(--space-4);
  min-width: 0;
}

.rrc__head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.rrc__title {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
  color: var(--text-secondary);
  text-transform: uppercase;
}

.rrc__icon {
  width: var(--size-icon-xs);
  height: var(--size-icon-xs);
  color: var(--text-disabled);
}

.rrc__none {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-disabled);
}

.rrc__columns {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-2);
}

.rrc__col-label {
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.rrc__blocks {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.rrc__block {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  padding: var(--space-3);
}

.rrc__block--added {
  border-color: var(--ws-color-success);
}

.rrc__block--modified {
  border-color: var(--ws-color-warning);
}

.rrc__block--removed {
  border-color: var(--ws-color-danger);
}

.rrc__block--duplication {
  border-color: var(--accent);
}

.rrc__block-head {
  display: flex;
  align-items: center;
}

.rrc__block-body {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-2);
}

.rrc__cell {
  margin: 0;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-xs);
  background: var(--surface-raised);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
  min-width: 0;
}

.rrc__cell--removed {
  text-decoration: line-through;
  color: var(--text-secondary);
}

.rrc__cell-empty {
  color: var(--text-disabled);
}

.rrc__note {
  margin: 0;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

@media (max-width: 1100px) {
  .rrc__columns,
  .rrc__block-body {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
