export enum ObligationStatus {
  Open = "open",
  InProgress = "in_progress",
  ProposedDone = "proposed_done",
  Done = "done",
  Dismissed = "dismissed",
  Escalated = "escalated",
}

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
}
