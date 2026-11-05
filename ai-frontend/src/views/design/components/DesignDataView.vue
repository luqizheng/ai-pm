<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  AlertTriangle,
  CheckCircle2,
  GitBranch,
  ShieldAlert,
  Timer
} from "lucide-vue-next";

import { StatCard } from "@/components/ui/stat-card";
import { AvatarGroup } from "@/components/ui/avatar-group";
import { ListItem } from "@/components/ui/list-item";
import { NotificationItem } from "@/components/ui/notification-item";
import { MenuItem } from "@/components/ui/menu-item";
import { Avatar } from "@/components/ui/avatar";
import DesignLayout from "../DesignLayout.vue";
import Specimen from "../design-kit/Specimen.vue";

/** /design/components/data —— DS/Data 族样板 */
const { t } = useI18n();
</script>

<template>
  <DesignLayout>
    <template #title>
      <h1 class="design-page__title">
        {{ t("design.components.data.title") }}
      </h1>
      <p class="design-page__subtitle">
        {{ t("design.components.data.subtitle") }}
      </p>
    </template>

    <section class="design-page__group">
      <h2>AvatarGroup (13:34379)</h2>
      <div class="design-page__row">
        <Specimen
          v-for="s in ['sm', 'md', 'lg'] as const"
          :key="s"
          :label="`Size=${s}`"
        >
          <AvatarGroup
            :avatars="[
              'Alex Morgan',
              'Rin Sato',
              'Kim Doe',
              'Ana Silva',
              'Li Wei'
            ]"
            :size="s"
          />
        </Specimen>
        <Specimen label="Max=3">
          <AvatarGroup :avatars="['A B', 'C D', 'E F', 'G H']" :max="3" />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>ListItem (13:34428)</h2>
      <div class="design-page__row">
        <Specimen label="State=Default" width="300px">
          <ListItem
            title="Atlas"
            description="Main development repository"
            :icon="GitBranch"
          />
        </Specimen>
        <Specimen label="State=Selected" width="300px">
          <ListItem
            title="Atlas"
            description="Main development repository"
            :icon="GitBranch"
            selected
          />
        </Specimen>
        <Specimen label="Trailing badge" width="300px">
          <ListItem title="Pull request 128" description="2 minutes ago">
            <template #trailing>
              <Badge variant="accent">new</Badge>
            </template>
          </ListItem>
        </Specimen>
        <Specimen label="State=Disabled" width="300px">
          <ListItem title="Archived item" disabled />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>NotificationItem (13:34472)</h2>
      <div class="design-page__row">
        <Specimen label="State=Unread" width="320px">
          <NotificationItem
            title="Review requested"
            body="Alex requested your review on PR 128."
            unread
            timestamp="2m"
          />
        </Specimen>
        <Specimen label="State=Read" width="320px">
          <NotificationItem
            title="Baseline frozen"
            body="S01 baseline was frozen by Rin."
            timestamp="1h"
          />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>MenuItem (13:34522)</h2>
      <div class="design-page__row">
        <Specimen label="State=Default" width="220px">
          <MenuItem label="Duplicate" shortcut="⌘ D" />
        </Specimen>
        <Specimen label="State=Selected" width="220px">
          <MenuItem label="Rename" shortcut="F2" selected />
        </Specimen>
        <Specimen label="State=Disabled" width="220px">
          <MenuItem label="Move to…" disabled />
        </Specimen>
        <Specimen label="Danger" width="220px">
          <MenuItem label="Delete" shortcut="⌫" danger />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>Avatar (already synced)</h2>
      <div class="design-page__row">
        <Specimen
          v-for="s in ['sm', 'md', 'lg'] as const"
          :key="s"
          :label="`Size=${s}`"
        >
          <Avatar name="Alex Morgan" :size="s" />
        </Specimen>
      </div>
    </section>

    <section class="design-page__group">
      <h2>
        StatCard — local extension (no Figma counterpart; DS visual language)
      </h2>
      <div class="design-page__row">
        <Specimen label="Stacked / Warning" width="280px">
          <StatCard
            label="Needs Attention"
            value="12"
            description="Across 3 projects"
            :icon="AlertTriangle"
            tone="warning"
          />
        </Specimen>
        <Specimen label="Stacked / Info" width="280px">
          <StatCard
            label="Active projects"
            value="8"
            description="2 in progress"
            :icon="GitBranch"
            tone="info"
          />
        </Specimen>
        <Specimen label="Stacked / Danger + trend" width="280px">
          <StatCard
            label="Blocked items"
            value="3"
            description="Needs your review"
            :icon="ShieldAlert"
            tone="danger"
            :trend="{ direction: 'up', value: '2' }"
          />
        </Specimen>
        <Specimen label="Stacked / Success + trend" width="280px">
          <StatCard
            label="Tasks completed"
            value="95%"
            description="Last 7 days"
            :icon="CheckCircle2"
            tone="success"
            :trend="{ direction: 'up', value: '12%' }"
          />
        </Specimen>
      </div>

      <div class="design-page__row" style="margin-top: var(--space-4)">
        <Specimen label="Trend down / Accent" width="280px">
          <StatCard
            label="Cycle time"
            value="4.2d"
            description="Rolling 30 days"
            :icon="Timer"
            tone="accent"
            :trend="{ direction: 'down', value: '0.8d' }"
          />
        </Specimen>
        <Specimen label="With action" width="280px">
          <StatCard
            label="Open reviews"
            value="5"
            description="Assigned to you"
            tone="accent"
            action-label="View details"
            @action="() => {}"
          />
        </Specimen>
        <Specimen label="Clickable + selected" width="280px">
          <StatCard
            label="My items"
            value="14"
            description="Click to filter"
            clickable
            selected
            @select="() => {}"
          />
        </Specimen>
        <Specimen label="Loading" width="280px">
          <StatCard
            label="Syncing stats"
            value="0"
            description="Fetching…"
            loading
          />
        </Specimen>
      </div>

      <div class="design-page__row" style="margin-top: var(--space-4)">
        <Specimen label="Compact row" width="520px">
          <div class="statdemo__grid">
            <StatCard label="Open" value="7" layout="compact" tone="info" />
            <StatCard
              label="Merged"
              value="23"
              layout="compact"
              tone="success"
            />
            <StatCard
              label="Blocked"
              value="2"
              layout="compact"
              tone="danger"
            />
          </div>
        </Specimen>
        <Specimen label="Dashboard strip" width="720px">
          <div class="statdemo__grid statdemo__grid--4">
            <StatCard
              label="Needs Attention"
              value="12"
              description="Across 3 projects"
              :icon="AlertTriangle"
              tone="warning"
            />
            <StatCard
              label="Active projects"
              value="8"
              description="2 in progress"
              :icon="GitBranch"
              tone="info"
            />
            <StatCard
              label="Tasks completed"
              value="95%"
              description="Last 7 days"
              :icon="CheckCircle2"
              tone="success"
              :trend="{ direction: 'up', value: '12%' }"
            />
            <StatCard
              label="Blocked items"
              value="3"
              description="Needs your review"
              :icon="ShieldAlert"
              tone="danger"
            />
          </div>
        </Specimen>
      </div>
    </section>
  </DesignLayout>
</template>

<style scoped>
.design-page__title {
  margin: 0;
  font-size: var(--ws-font-size-page);
  font-weight: var(--weight-semibold);
  color: var(--ws-text-primary);
}

.design-page__subtitle {
  margin: var(--space-1) 0 0;
  font-size: var(--ws-font-size-caption);
  color: var(--ws-text-secondary);
}

.design-page__group {
  margin: 0 0 var(--space-8);
}

.design-page__group h2 {
  margin: 0 0 var(--space-3);
  font-family: var(--font-mono);
  font-size: var(--font-size-nano);
  color: var(--ws-text-secondary);
}

.design-page__row {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: var(--space-4);
}

.statdemo__grid {
  display: grid;
  gap: var(--space-3);
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.statdemo__grid--4 {
  grid-template-columns: repeat(4, minmax(0, 1fr));
}
</style>
