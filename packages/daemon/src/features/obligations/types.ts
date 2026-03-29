export enum ObligationStatus {
  Open = "open",
  InProgress = "in_progress",
  ProposedDone = "proposed_done",
  Done = "done",
  Dismissed = "dismissed",
  Escalated = "escalated",
}

/** Which routing tier or mechanism detected/created this obligation. */
export type DetectionSource = "tier1" | "tier2" | "tier3" | "manual";

export interface ObligationRecord {
  id: string;
  detectedAction: string;
  owner: string;
  status: ObligationStatus;
  priority: number;
  projectCode: string | null;
  sourceChannel: string;
  sourceMessage: string | null;
  deadline: Date | null;
  attemptCount: number;
  lastAttemptAt: Date | null;
  createdAt: Date;
  updatedAt: Date;
  /** Which routing tier detected this obligation. Null for pre-migration rows. */
  detectionSource: DetectionSource | null;
  /** Which fleet tool handled the original message (e.g. "set_reminder"). Null if not applicable. */
  routedTool: string | null;
}

/**
 * Input for creating a new obligation.
 * Omits server-generated fields: id, createdAt, updatedAt, lastAttemptAt.
 */
export interface CreateObligationInput {
  detectedAction: string;
  owner: string;
  status: ObligationStatus;
  priority: number;
  projectCode: string | null;
  sourceChannel: string;
  sourceMessage: string | null;
  deadline: Date | null;
  detectionSource?: DetectionSource | null;
  routedTool?: string | null;
}
