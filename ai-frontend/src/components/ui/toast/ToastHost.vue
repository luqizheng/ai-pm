<script setup lang="ts">
import Toast from "./Toast.vue";
import { toastApi, useToastState } from "./useToast";

/**
 * Toast 宿主：应用根挂载一个 <ToastHost />（App.vue，覆盖 bare 页与壳页）。
 * 状态为应用级单例（见 useToast.ts），页面统一走 useToast() 触发。
 */
const { current, seq } = useToastState();
const { dismiss } = toastApi;
</script>

<template>
  <div class="ui-toast-host">
    <Toast v-if="current" :key="seq" v-bind="current" @update:open="dismiss" />
  </div>
</template>

<style scoped>
.ui-toast-host {
  position: relative;
  z-index: 60;
}
</style>
