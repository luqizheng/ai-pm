export type SidebarItemSize = "sm" | "md" | "lg";
export type SidebarItemState =
  | "default"
  | "hover"
  | "active"
  | "focus"
  | "restricted"
  | "loading";

export const SIDEBAR_ITEM_SIZES: SidebarItemSize[] = ["sm", "md", "lg"];
export const SIDEBAR_ITEM_STATES: SidebarItemState[] = [
  "default",
  "hover",
  "active",
  "focus",
  "restricted",
  "loading"
];
