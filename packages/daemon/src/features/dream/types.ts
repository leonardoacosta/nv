/** Dream consolidation types used by the daemon orchestrator + scheduler. */

export interface TopicStats {
  topic: string;
  sizeBytes: number;
  lineCount: number;
  updatedAt: Date;
}

export interface RuleStats {
  dedupedLines: number;
  datesNormalized: number;
  stalePathsRemoved: number;
  whitespaceFixed: number;
}

export interface RuleResult {
  content: string;
  needsLlm: boolean;
  stats: RuleStats;
}

export interface DreamResult {
  topicsProcessed: number;
  bytesBefore: number;
  bytesAfter: number;
  llmTopics: string[];
  durationMs: number;
}

export interface DreamSchedulerConfig {
  enabled: boolean;
  cronHour: number;
  interactionThreshold: number;
  sizeThresholdKb: number;
  debounceHours: number;
  topicMaxKb: number;
}
