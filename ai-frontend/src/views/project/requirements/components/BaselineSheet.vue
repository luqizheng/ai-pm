<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ShieldCheck } from "lucide-vue-next";
import Badge from "@/components/ui/badge/Badge.vue";
import Button from "@/components/ui/button/Button.vue";
import Sheet from "@/components/ui/sheet/Sheet.vue";
import StatusIndicator from "@/components/ui/status-indicator/StatusIndicator.vue";
import type { StatusIndicatorTone } from "@/components/ui/status-indicator/status-indicator";
import type { BaselineSnapshot, RequirementEntry } from "../requirement-entry";

/**
 * 需求基线抽屉（S02-UI-01 §9 + §11 状态 8/9）。
 *
 * 查看当前基线 / 审查含入需求 / 识别未确认与冲突需求 / 比较修订（链接页根）/
 * 准备新基线（仅 confirmed 需求可纳入）。建立基线是显式人类确认操作：
 * 本组件只收集含入清单并发出请求，最终确认弹窗由页根编排。
 * 红线：draft / pending_confirmation / conflict 需求一律不得纳入基线。
 */
interface Props {
  open: boolean;
  requirements: RequirementEntry[];
  baseline: BaselineSnapshot;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
  (e: "request-establish", codes: string[]): void;
  (e: "open-history"): void;
}>();

const { t } = useI18n();

const preparing = ref(false);
const pickedCodes = ref<string[]>([]);

watch(
  () => props.open,
  open => {
    if (!open) return;
    preparing.value = false;
    pickedCodes.value = props.requirements
      .filter(entry => entry.status === "confirmed")
      .map(entry => entry.code);
  },
  { immediate: true }
);

const baselineEntries = computed(() =>
  props.requirements.filter(entry =>
    props.baseline.requirement_codes.includes(entry.code)
  )
);

const excludedEntries = computed(() =>
  props.requirements.filter(
    entry => !props.baseline.requirement_codes.includes(entry.code)
  )
);

const candidateEntries = computed(() =>
  props.requirements.filter(entry => entry.status === "confirmed")
);

const statusLabel = (status: string) =>
  ({
    draft: t("pm.requirements.statusDraft"),
    pending_confirmation: t("pm.requirements.statusPending"),
    confirmed: t("pm.requirements.statusConfirmed"),
    conflict: t("pm.requirements.statusConflict")
  })[status] ?? status;

const statusTone = (status: string): StatusIndicatorTone =>
  (
    ({
      draft: "neutral",
      pending_confirmation: "warning",
      confirmed: "success",
      conflict: "danger"
    }) as const
  )[status] ?? "neutral";

function togglePick(code: string) {
  if (pickedCodes.value.includes(code)) {
    pickedCodes.value = pickedCodes.value.filter(item => item !== code);
  } else {
    pickedCodes.value = [...pickedCodes.value, code];
  }
}

function onPrepare() {
  preparing.value = true;
}

function onRequestEstablish() {
  if (pickedCodes.value.length === 0) return;
  emit("request-establish", [...pickedCodes.value]);
}
</script>

<template>
  <Sheet
    :open="props.open"
    :title="t('pm.requirements.baselineTitle')"
    :description="t('pm.requirements.baselineDesc')"
    side="right"
    @update:open="emit('update:open', $event)"
  >
    <div class="req-baseline">
      <section
        v-if="!preparing"
        class="req-baseline__current"
        :aria-label="t('pm.requirements.baselineCurrent')"
      >
        <div class="req-baseline__current-head">
          <ShieldCheck class="req-baseline__shield" aria-hidden="true" />
          <span class="req-baseline__version">
            {{
              t("pm.requirements.baselineVersion", {
                version: props.baseline.version
              })
            }}
          </span>
          <span class="req-baseline__meta">
            {{
              t("pm.requirements.baselineEstablished", {
                by: props.baseline.established_by,
                time: props.baseline.established_at
              })
            }}
          </span>
        </div>
        <p class="req-baseline__hint">
          {{ t("pm.requirements.baselineCurrentHint") }}
        </p>

        <ul class="req-baseline__rows">
          <li
            v-for="entry in baselineEntries"
            :key="`in-${entry.id}`"
            class="req-baseline__row"
          >
            <StatusIndicator status="success" size="sm" />
            <span class="req-baseline__code">{{ entry.code }}</span>
            <span class="req-baseline__row-title">{{ entry.title }}</span>
            <Badge variant="success" size="sm">
              {{ t("pm.requirements.baselineIncluded") }}
            </Badge>
          </li>
          <li
            v-for="entry in excludedEntries"
            :key="`out-${entry.id}`"
            class="req-baseline__row"
          >
            <StatusIndicator :status="statusTone(entry.status)" size="sm" />
            <span class="req-baseline__code">{{ entry.code }}</span>
            <span class="req-baseline__row-title">{{ entry.title }}</span>
            <span class="req-baseline__excluded">
              {{
                t("pm.requirements.baselineExcludedReason", {
                  status: statusLabel(entry.status)
                })
              }}
            </span>
          </li>
        </ul>
      </section>

      <section
        v-else
        class="req-baseline__prepare"
        :aria-label="t('pm.requirements.baselinePrepare')"
      >
        <p class="req-baseline__hint">
          {{ t("pm.requirements.baselinePrepareHint") }}
        </p>
        <ul class="req-baseline__rows">
          <li
            v-for="entry in candidateEntries"
            :key="`cand-${entry.id}`"
            class="req-baseline__row"
          >
            <label class="req-baseline__pick">
              <input
                type="checkbox"
                :checked="pickedCodes.includes(entry.code)"
                class="req-baseline__checkbox"
                @change="togglePick(entry.code)"
              />
              <span class="req-baseline__code">{{ entry.code }}</span>
              <span class="req-baseline__row-title">{{ entry.title }}</span>
            </label>
          </li>
          <li
            v-for="entry in props.requirements.filter(
              item => item.status !== 'confirmed'
            )"
            :key="`locked-${entry.id}`"
            class="req-baseline__row req-baseline__row--locked"
          >
            <StatusIndicator :status="statusTone(entry.status)" size="sm" />
            <span class="req-baseline__code">{{ entry.code }}</span>
            <span class="req-baseline__row-title">{{ entry.title }}</span>
            <span class="req-baseline__excluded">
              {{
                t("pm.requirements.baselineLocked", {
                  status: statusLabel(entry.status)
                })
              }}
            </span>
          </li>
        </ul>
        <p v-if="pickedCodes.length === 0" class="req-baseline__warn">
          {{ t("pm.requirements.baselineEmptyPick") }}
        </p>
      </section>

      <div class="req-baseline__footer">
        <template v-if="!preparing">
          <Button variant="secondary" size="sm" @click="emit('open-history')">
            {{ t("pm.requirements.actionHistory") }}
          </Button>
          <Button variant="primary" size="sm" @click="onPrepare">
            {{ t("pm.requirements.baselinePrepare") }}
          </Button>
        </template>
        <template v-else>
          <Button
            variant="primary"
            size="sm"
            :disabled="pickedCodes.length === 0"
            @click="onRequestEstablish"
          >
            {{ t("pm.requirements.baselineEstablishAction") }}
          </Button>
          <Button variant="ghost" size="sm" @click="preparing = false">
            {{ t("pm.requirements.historyClose") }}
          </Button>
        </template>
      </div>
    </div>
  </Sheet>
</template>

<style scoped>
.req-baseline {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.req-baseline__current,
.req-baseline__prepare {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.req-baseline__current-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.req-baseline__shield {
  width: var(--size-icon-sm);
  height: var(--size-icon-sm);
  color: var(--success-strong);
}

.req-baseline__version {
  font-size: var(--font-size-body-strong);
  line-height: var(--line-height-body-strong);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.req-baseline__meta {
  margin-left: auto;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.req-baseline__hint {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.req-baseline__rows {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
  border: var(--border-1) solid var(--border-default);
  border-radius: var(--radius-sm);
  overflow: hidden;
}

.req-baseline__row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: var(--size-nav-item);
  padding: 0 var(--space-3);
  border-bottom: var(--hairline) solid var(--border-default);
}

.req-baseline__rows li:last-child {
  border-bottom: 0;
}

.req-baseline__row--locked {
  background: var(--ws-hover-veil);
}

.req-baseline__pick {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  cursor: pointer;
  min-width: 0;
}

.req-baseline__checkbox {
  accent-color: var(--accent);
}

.req-baseline__checkbox:focus-visible {
  outline: none;
  box-shadow: var(--ws-focus-ring);
  border-radius: var(--radius-xs);
}

.req-baseline__code {
  flex: none;
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-secondary);
}

.req-baseline__row-title {
  flex: 1 1 0;
  min-width: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.req-baseline__excluded {
  flex: none;
  font-size: var(--font-size-micro);
  line-height: var(--line-height-micro);
  color: var(--text-secondary);
}

.req-baseline__warn {
  margin: 0;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--warning-strong);
}

.req-baseline__footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  padding-top: var(--space-3);
  border-top: var(--hairline) solid var(--border-default);
}
</style>
