export {
  ObligationStatus,
  type ObligationRecord,
  type CreateObligationInput,
} from "./types.js";

export { ObligationStore } from "./store.js";

export { detectObligations, type DetectedObligation } from "./detector.js";

export {
  ObligationExecutor,
  buildExecutionPrompt,
  selectModel,
  estimateCost,
  type ExecutorConfig,
  type TelegramNotifier,
} from "./executor.js";

export {
  handleObligationConfirm,
  handleObligationReopen,
  handleEscalationRetry,
  handleEscalationDismiss,
  handleEscalationTakeover,
  OBLIGATION_CONFIRM_PREFIX,
  OBLIGATION_REOPEN_PREFIX,
  OBLIGATION_ESCALATION_RETRY_PREFIX,
  OBLIGATION_ESCALATION_DISMISS_PREFIX,
  OBLIGATION_ESCALATION_TAKEOVER_PREFIX,
  type TelegramSender,
} from "./callbacks.js";
