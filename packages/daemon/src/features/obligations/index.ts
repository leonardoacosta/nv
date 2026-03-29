export {
  ObligationStatus,
  type DetectionSource,
  type ObligationRecord,
  type CreateObligationInput,
} from "./types.js";

export { ObligationStore } from "./store.js";

export { detectObligations, detectObligationLightweight, type DetectedObligation, type LightweightDetectionInput, type LightweightDetectionResult } from "./detector.js";

export { detectSignals, type SignalResult } from "./signal-detector.js";

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
