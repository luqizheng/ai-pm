<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { GitBranch } from "lucide-vue-next";

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
</style>
