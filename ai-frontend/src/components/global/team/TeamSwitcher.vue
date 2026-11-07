<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Building2, Check, ChevronsUpDown, Users } from "lucide-vue-next";
import {
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger
} from "reka-ui";
import {
  TooltipContent,
  TooltipPortal,
  TooltipProvider,
  TooltipRoot,
  TooltipTrigger
} from "reka-ui";

import { useTeamStore } from "@/stores/team";

/**
 * Team Switcher（Team Onboarding P0；演进自只读 TenantIndicator）。
 *
 * UI 细则 #2：切换的是**同一账号已加入的团队**（memberships），不是账号
 * 切换器——账号操作只存在于 UserMenu。V1 现实（2026-11-06 裁决）：上游 lmms
 * 一个用户只属一个租户， memberships 实际为 0/1 行；多团队切换项置灰并说明
 * 「多团队待上游支持」。数据 = team store（GET /v1/pm/me 团队扩展面），
 * 契约已按多团队 shape 设计，上游放开后本组件无需改结构。
 */
const { t } = useI18n();
const teamStore = useTeamStore();

const open = ref(false);

const teamName = computed(
  () => teamStore.currentTeam?.name ?? t("team.switcher.noTeam")
);

const tooltipLabel = computed(() =>
  t("team.switcher.tooltip", {
    team: teamName.value,
    role: teamStore.teamRole.toUpperCase()
  })
);

const canSwitch = computed(() => teamStore.memberships.length > 1);

interface SwitcherRow {
  id: string;
  name: string;
  role: string;
  current: boolean;
  disabled: boolean;
}

const rows = computed<SwitcherRow[]>(() => {
  const joined = teamStore.memberships.map(membership => ({
    id: membership.team_id,
    name: membership.team_name,
    role: membership.role.toUpperCase(),
    current: membership.team_id === teamStore.context?.tenant_id,
    disabled: false
  }));
  // V1 单团队如实呈现：占位行说明多团队待上游（置灰，不可点）。
  if (joined.length <= 1) {
    joined.push({
      id: "__pending__",
      name: t("team.switcher.pendingRow"),
      role: "",
      current: false,
      disabled: true
    });
  }
  return joined;
});
</script>

<template>
  <TooltipProvider :delay-duration="200">
    <TooltipRoot>
      <PopoverRoot v-model:open="open">
        <TooltipTrigger as-child>
          <PopoverTrigger as-child>
            <button
              type="button"
              class="team-switcher"
              :aria-label="tooltipLabel"
              data-testid="team-switcher"
            >
              <Building2 :size="16" aria-hidden="true" />
              <span class="team-switcher__name">{{ teamName }}</span>
              <ChevronsUpDown
                :size="13"
                class="team-switcher__caret"
                aria-hidden="true"
              />
            </button>
          </PopoverTrigger>
        </TooltipTrigger>

        <TooltipPortal>
          <TooltipContent
            class="team-switcher__tooltip"
            side="bottom"
            align="start"
            :side-offset="4"
          >
            {{ tooltipLabel }}
          </TooltipContent>
        </TooltipPortal>

        <PopoverPortal>
          <PopoverContent
            class="team-switcher__card"
            side="bottom"
            align="start"
            :side-offset="4"
          >
            <p class="team-switcher__head">{{ t("team.switcher.myTeams") }}</p>
            <ul class="team-switcher__list">
              <li v-for="row in rows" :key="row.id">
                <button
                  type="button"
                  class="team-switcher__row"
                  :class="{
                    'is-current': row.current,
                    'is-disabled': row.disabled
                  }"
                  :disabled="row.disabled"
                  :data-testid="`team-switcher-row-${row.id}`"
                  :title="
                    row.disabled ? t('team.switcher.pendingHint') : undefined
                  "
                  @click="row.disabled ? undefined : (open = false)"
                >
                  <Users :size="14" aria-hidden="true" />
                  <span class="team-switcher__row-name">{{ row.name }}</span>
                  <span v-if="row.role" class="team-switcher__row-role">{{
                    row.role
                  }}</span>
                  <Check
                    v-if="row.current"
                    :size="14"
                    class="team-switcher__check"
                    aria-hidden="true"
                  />
                </button>
              </li>
            </ul>
            <p class="team-switcher__hint">{{ t("team.switcher.hint") }}</p>
          </PopoverContent>
        </PopoverPortal>
      </PopoverRoot>
    </TooltipRoot>
  </TooltipProvider>
</template>

<style scoped>
.team-switcher {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--size-control-compact);
  padding: 0 var(--space-3);
  color: var(--ws-text-primary);
  font-size: var(--font-size-body);
  white-space: nowrap;
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: var(--radius-sm);
}

.team-switcher:hover {
  background: var(--ws-elevated);
}

.team-switcher:focus-visible {
  outline: none;
  border: var(--border-1) solid var(--ws-accent);
  box-shadow: inset 0 0 0 1px var(--ws-accent);
}

.team-switcher svg {
  flex: none;
  color: var(--ws-text-secondary);
}

.team-switcher__name {
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 160px;
}

.team-switcher__caret {
  color: var(--ws-text-tertiary);
}

/* reka-ui 浮层（Tooltip/Popover 内容）被 Portal 传送到 body，class 落在不带
   本组件 scope 属性的 popper 内层元素上——须用 :deep 命中，否则整条规则失效
   （背景透明透字，同 ui/popover 与 UserMenu 定式）。 */
:deep(.team-switcher__tooltip),
.team-switcher__tooltip {
  z-index: 60;
  padding: var(--space-1) var(--space-2);
  border-radius: var(--radius-sm);
  background: var(--ws-elevated);
  border: var(--border-1) solid var(--ws-border);
  font-size: var(--font-size-caption);
  color: var(--ws-text-primary);
}

:deep(.team-switcher__card),
.team-switcher__card {
  z-index: 60;
  min-width: 264px;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  border-radius: var(--radius-md);
  background: var(--surface-raised);
  border: var(--border-1) solid var(--border-default);
  box-shadow: var(--shadow-lg);
}

.team-switcher__head {
  margin: 0;
  font-size: var(--font-size-caption);
  font-weight: var(--weight-medium);
  color: var(--text-tertiary);
}

.team-switcher__list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.team-switcher__row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  cursor: pointer;
  color: var(--text-primary);
  font-size: var(--font-size-caption);
  text-align: left;
}

.team-switcher__row.is-current {
  background: var(--accent-surface);
}

.team-switcher__row.is-disabled {
  cursor: not-allowed;
  color: var(--text-disabled);
  opacity: var(--opacity-disabled-option, 0.5);
}

.team-switcher__row-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.team-switcher__row-role {
  font-size: var(--font-size-nano, 10px);
  color: var(--text-tertiary);
}

.team-switcher__check {
  color: var(--accent);
}

.team-switcher__hint {
  margin: 0;
  font-size: var(--font-size-nano, 10px);
  line-height: 1.4;
  color: var(--text-tertiary);
}
</style>
