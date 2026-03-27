/** Dream consolidation engine types. */

export interface DreamConfig {
  /** Per-topic size ceiling in KB; topics exceeding this after rules go to LLM. */
  topic_max_kb: number;
  /** Minimum hours between dream runs. */
  debounce_hours: number;
}

export interface TopicStats {
  topic: string;
  sizeBytes: number;
  lineCount: number;
  updatedAt: Date;
}

export interface DreamOrientation {
  topics: TopicStats[];
  totalSizeBytes: number;
  timestamp: Date;
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
