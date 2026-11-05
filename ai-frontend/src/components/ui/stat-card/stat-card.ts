export type StatCardTone =
  | "neutral"
  | "info"
  | "success"
  | "warning"
  | "danger"
  | "accent";
export type StatCardLayout = "stacked" | "compact";
export type StatTrend = { direction: "up" | "down"; value: string };

export const STAT_CARD_TONES: StatCardTone[] = [
  "neutral",
  "info",
  "success",
  "warning",
  "danger",
  "accent"
];

export const STAT_CARD_LAYOUTS: StatCardLayout[] = ["stacked", "compact"];
