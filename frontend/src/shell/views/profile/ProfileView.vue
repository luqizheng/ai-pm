<script setup lang="ts">
import { computed, ref } from "vue";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { FormField } from "@/components/ui/form-field";
import { FormDialog } from "@/shell/components";
import { useToast } from "@/components/ui/toast";
import { iam } from "@/shell/api/iam";
import { useAuthStore } from "../../store/auth";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

const auth = useAuthStore();
const { toast } = useToast();

const name = computed(() => auth.user?.nickname || auth.user?.username || "");
const roles = computed(() => auth.user?.roles ?? []);

const pwOpen = ref(false);
const pwSubmitting = ref(false);
const pw = ref({ old_password: "", new_password: "", confirm: "" });

function openPw() {
  pw.value = { old_password: "", new_password: "", confirm: "" };
  pwOpen.value = true;
}

async function onPwSubmit() {
  if (pw.value.new_password !== pw.value.confirm) {
    toast({ variant: "warning", title: t("system.profile.passwordMismatch") });
    return;
  }
  pwSubmitting.value = true;
  try {
    await iam.auth.changePassword({
      old_password: pw.value.old_password,
      new_password: pw.value.new_password
    });
    pwOpen.value = false;
    toast({ variant: "success", title: t("system.profile.passwordSuccess") });
  } catch (error) {
    toast({
      variant: "danger",
      title: t("system.profile.passwordFailed"),
      description: error instanceof Error ? error.message : String(error)
    });
  } finally {
    pwSubmitting.value = false;
  }
}
</script>

<template>
  <section class="ui-profile">
    <h1 class="ui-profile__title">{{ t("system.profile.title") }}</h1>
    <Card>
      <dl class="ui-profile__list">
        <div class="ui-profile__row">
          <dt>{{ t("system.common.nickname") }}</dt>
          <dd>{{ name }}</dd>
        </div>
        <div class="ui-profile__row">
          <dt>{{ t("system.common.email") }}</dt>
          <dd>{{ auth.user?.email || "—" }}</dd>
        </div>
        <div class="ui-profile__row">
          <dt>{{ t("system.profile.roles") }}</dt>
          <dd class="ui-profile__roles">
            <Badge v-for="role in roles" :key="role" variant="accent">{{
              role
            }}</Badge>
          </dd>
        </div>
      </dl>
    </Card>

    <div class="ui-profile__security">
      <Button variant="secondary" @click="openPw">
        {{ t("system.profile.changePassword") }}
      </Button>
    </div>

    <FormDialog
      v-model:open="pwOpen"
      :title="t('system.profile.changePassword')"
      :submitting="pwSubmitting"
      @submit="onPwSubmit"
      @cancel="pwOpen = false"
    >
      <FormField :label="t('system.profile.oldPassword')" required>
        <Input v-model="pw.old_password" type="password" />
      </FormField>
      <FormField :label="t('system.profile.newPassword')" required>
        <Input v-model="pw.new_password" type="password" />
      </FormField>
      <FormField :label="t('system.profile.confirmPassword')" required>
        <Input v-model="pw.confirm" type="password" />
      </FormField>
    </FormDialog>
  </section>
</template>

<style scoped>
.ui-profile {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.ui-profile__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h2);
  line-height: var(--line-height-h2);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h2);
  color: var(--text-primary);
}

.ui-profile__list {
  margin: 0;
  display: flex;
  flex-direction: column;
}

.ui-profile__row {
  display: flex;
  align-items: center;
  gap: var(--space-6);
  padding: var(--space-3) 0;
  border-bottom: 1px solid var(--border-subtle);
}

.ui-profile__row dt {
  width: 96px;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  color: var(--text-tertiary);
}

.ui-profile__row dd {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--text-primary);
}

.ui-profile__roles {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.ui-profile__security {
  display: flex;
  justify-content: flex-end;
}
</style>
