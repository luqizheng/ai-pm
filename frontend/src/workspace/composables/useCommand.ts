import { ref } from "vue";

export type CommandPhase = "idle" | "submitting" | "succeeded" | "failed";

/**
 * 命令生命周期（workspace design §5.5）：idle → submitting → succeeded/failed；
 * submitting 期间重复 submit 被忽略（防连点）。
 */
export function useCommand<TArgs>(executor: (args: TArgs) => Promise<unknown>) {
  const phase = ref<CommandPhase>("idle");
  const error = ref<string | undefined>(undefined);

  async function submit(args: TArgs): Promise<void> {
    if (phase.value === "submitting") return;
    phase.value = "submitting";
    error.value = undefined;
    try {
      await executor(args);
      phase.value = "succeeded";
    } catch (err) {
      phase.value = "failed";
      error.value = err instanceof Error ? err.message : String(err);
    }
  }

  return { phase, error, submit };
}