import { ref, type Ref } from "vue";

export type ResourceState = "loading" | "ready" | "stale" | "error";

/**
 * 数据资源生命周期（workspace design §5.5）：loading → ready / error；
 * 外部事件可 markStale，随后 refresh 重新拉取。
 */
export function useResource<T>(fetcher: () => Promise<T>) {
  const state = ref<ResourceState>("loading");
  const data = ref<T | undefined>(undefined) as Ref<T | undefined>;

  async function refresh(): Promise<void> {
    state.value = data.value === undefined ? "loading" : "stale";
    try {
      data.value = await fetcher();
      state.value = "ready";
    } catch {
      state.value = "error";
    }
  }

  function markStale(): void {
    if (state.value === "ready") state.value = "stale";
  }

  return { state, data, refresh, markStale };
}