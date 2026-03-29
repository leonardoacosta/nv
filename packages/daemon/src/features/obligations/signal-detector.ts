/**
 * Lightweight keyword-based obligation signal detector.
 * No LLM calls — pure regex/pattern matching. Used as a fast gate before
 * escalating to Haiku-model obligation detection.
 */

// ─── Pattern definitions ──────────────────────────────────────────────────────

interface PatternEntry {
  pattern: RegExp;
  /** High-confidence patterns count as 2 signals on their own. */
  highConfidence: boolean;
}

const OBLIGATION_PATTERNS: PatternEntry[] = [
  // High-confidence: deadline/date-bound language
  { pattern: /\bdeadline\b/i, highConfidence: true },
  { pattern: /\bdue\s+by\b/i, highConfidence: true },
  { pattern: /\bbefore\s+(monday|tuesday|wednesday|thursday|friday|saturday|sunday|today|tomorrow|end\s+of\s+(the\s+)?(day|week|month))\b/i, highConfidence: true },
  { pattern: /\bby\s+(end\s+of\s+(the\s+)?(day|week|month)|eod|eow)\b/i, highConfidence: true },

  // Normal confidence: commitment/obligation language
  { pattern: /\bneed\s+to\b/i, highConfidence: false },
  { pattern: /\bshould\b/i, highConfidence: false },
  { pattern: /\bmust\b/i, highConfidence: false },
  { pattern: /\bhave\s+to\b/i, highConfidence: false },
  { pattern: /\bdon'?t\s+forget\b/i, highConfidence: false },
  { pattern: /\bfollow[\s-]up\b/i, highConfidence: false },
  { pattern: /\bget\s+back\s+to\b/i, highConfidence: false },
  { pattern: /\bcheck\s+on\b/i, highConfidence: false },
  { pattern: /\bmake\s+sure\b/i, highConfidence: false },
  { pattern: /\bpromise\b/i, highConfidence: false },
  { pattern: /\bcommitt?ed\s+to\b/i, highConfidence: false },
  { pattern: /\bagree[ds]?\s+to\b/i, highConfidence: false },
  { pattern: /\bremind\s+me\b/i, highConfidence: false },
];

// ─── Types ────────────────────────────────────────────────────────────────────

export interface SignalResult {
  /** Whether the message meets the obligation detection threshold. */
  detected: boolean;
  /**
   * Confidence score 0.0–1.0. Derived from signal count and pattern weight.
   * Callers can use this for logging/analytics.
   */
  confidence: number;
  /** The matched signal strings (pattern source strings). */
  signals: string[];
}

// ─── detectSignals ────────────────────────────────────────────────────────────

/**
 * Run keyword-based signal detection on a text message.
 *
 * Threshold rules:
 * - 1 high-confidence signal → detected
 * - 2+ low-confidence signals → detected
 * - Fewer than 2 low-confidence signals and no high-confidence signals → not detected
 */
export function detectSignals(text: string): SignalResult {
  const signals: string[] = [];
  let highConfidenceCount = 0;
  let lowConfidenceCount = 0;

  for (const entry of OBLIGATION_PATTERNS) {
    if (entry.pattern.test(text)) {
      signals.push(entry.pattern.source);
      if (entry.highConfidence) {
        highConfidenceCount++;
      } else {
        lowConfidenceCount++;
      }
    }
  }

  const totalSignals = signals.length;
  const detected = highConfidenceCount >= 1 || lowConfidenceCount >= 2;

  // Confidence: normalize against a "strong match" of 3 signals
  const rawScore = highConfidenceCount * 2 + lowConfidenceCount;
  const confidence = Math.min(1.0, rawScore / 4);

  return {
    detected,
    confidence,
    signals,
  };
}
