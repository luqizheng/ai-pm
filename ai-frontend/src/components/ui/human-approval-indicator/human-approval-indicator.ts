export type ApprovalSize = "sm" | "md" | "lg";
export type ApprovalState =
  | "pending"
  | "approved"
  | "rejected"
  | "not-required";

export const APPROVAL_SIZES: ApprovalSize[] = ["sm", "md", "lg"];
export const APPROVAL_STATES: ApprovalState[] = [
  "pending",
  "approved",
  "rejected",
  "not-required"
];
