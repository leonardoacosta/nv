export type ApprovalActionType =
  | "file_write"
  | "file_delete"
  | "shell_exec"
  | "git_push"
  | "api_call"
  | "other";

export type ApprovalStatus = "pending" | "approved" | "dismissed";

export interface Approval {
  id: string;
  title: string;
  description?: string;
  action_type: ApprovalActionType;
  project?: string;
  proposed_changes?: string;
  context?: string;
  urgency: "low" | "medium" | "high" | "critical";
  status: ApprovalStatus;
  created_at: string;
}
