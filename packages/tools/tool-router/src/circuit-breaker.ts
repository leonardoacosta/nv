export type CircuitState = "CLOSED" | "OPEN" | "HALF_OPEN";

export interface CircuitBreakerConfig {
  /** Consecutive failure count that trips the breaker (default: 3). */
  failureThreshold: number;
  /** Error rate 0–1 within the sliding window that trips the breaker (default: 0.5). */
  errorRateThreshold: number;
  /** Sliding window duration in ms (default: 60_000). */
  errorRateWindowMs: number;
  /** Time in OPEN state before transitioning to HALF_OPEN, in ms (default: 30_000). */
  cooldownMs: number;
  /** Maximum entries in the ring buffer (default: 100). */
  ringBufferSize: number;
}

export interface CircuitBreakerSnapshot {
  state: CircuitState;
  failures: number;
  successes: number;
  lastFailureAt: string | null; // ISO 8601
  lastStateChange: string;      // ISO 8601
}

const DEFAULTS: CircuitBreakerConfig = {
  failureThreshold: 3,
  errorRateThreshold: 0.5,
  errorRateWindowMs: 60_000,
  cooldownMs: 30_000,
  ringBufferSize: 100,
};

/** Entry stored in the ring buffer sliding window. */
interface WindowEntry {
  success: boolean;
  timestampMs: number;
}

/**
 * Per-service circuit breaker with a sliding window ring buffer.
 *
 * State machine:
 *   CLOSED   → OPEN       when consecutive failures >= failureThreshold
 *                         OR error rate > errorRateThreshold within window
 *   OPEN     → HALF_OPEN  when cooldownMs has elapsed since last trip
 *   HALF_OPEN → CLOSED    when probe request succeeds
 *   HALF_OPEN → OPEN      when probe request fails
 */
export class CircuitBreaker {
  private readonly config: CircuitBreakerConfig;
  private _state: CircuitState = "CLOSED";
  private _consecutiveFailures = 0;
  private _lastFailureAt: number | null = null;
  private _lastStateChange: number = Date.now();
  private _openedAt: number | null = null;
  /** Whether a probe is currently in-flight in HALF_OPEN state. */
  private _probeInFlight = false;

  /** Ring buffer for sliding window error-rate calculation. */
  private readonly _window: WindowEntry[];
  private _windowHead = 0;
  private _windowSize = 0;

  /** Callback for external observers (used for WARN logging). */
  onStateChange?: (from: CircuitState, to: CircuitState, reason: string) => void;

  constructor(
    public readonly serviceName: string,
    config?: Partial<CircuitBreakerConfig>,
  ) {
    this.config = { ...DEFAULTS, ...config };
    this._window = new Array<WindowEntry>(this.config.ringBufferSize);
  }

  get state(): CircuitState {
    return this._state;
  }

  /**
   * Check if a request should be allowed through.
   *
   * - CLOSED: always true
   * - OPEN: false unless cooldown elapsed → transitions to HALF_OPEN
   * - HALF_OPEN: true for exactly one probe; false while probe is in-flight
   */
  allowRequest(): boolean {
    if (this._state === "CLOSED") {
      return true;
    }

    if (this._state === "OPEN") {
      const now = Date.now();
      const elapsed = this._openedAt !== null ? now - this._openedAt : Infinity;
      if (elapsed >= this.config.cooldownMs) {
        this._transition("HALF_OPEN", "cooldown elapsed");
        this._probeInFlight = true;
        return true;
      }
      return false;
    }

    // HALF_OPEN: allow one probe at a time
    if (!this._probeInFlight) {
      this._probeInFlight = true;
      return true;
    }
    return false;
  }

  /** Record a successful request outcome. */
  onSuccess(): void {
    this._consecutiveFailures = 0;
    this._addToWindow(true);

    if (this._state === "HALF_OPEN") {
      this._probeInFlight = false;
      this._transition("CLOSED", "probe succeeded");
    }
  }

  /** Record a failed request outcome. */
  onFailure(): void {
    this._consecutiveFailures++;
    this._lastFailureAt = Date.now();
    this._addToWindow(false);

    if (this._state === "HALF_OPEN") {
      this._probeInFlight = false;
      this._transition("OPEN", "probe failed");
      return;
    }

    if (this._state === "CLOSED") {
      if (this._consecutiveFailures >= this.config.failureThreshold) {
        this._transition("OPEN", `${this._consecutiveFailures} consecutive failures`);
        return;
      }
      if (this._errorRate() > this.config.errorRateThreshold) {
        this._transition("OPEN", `error rate ${(this._errorRate() * 100).toFixed(0)}% exceeded threshold`);
      }
    }
  }

  /**
   * Force the circuit to a specific state from external health data.
   * Used by the health route to synchronize circuit state with periodic health checks.
   */
  forceState(state: CircuitState): void {
    if (this._state !== state) {
      this._transition(state, "forced by health check");
    }
  }

  /** Return a snapshot of current state for observability. */
  snapshot(): CircuitBreakerSnapshot {
    const windowEntries = this._getWindowEntries();
    const failures = windowEntries.filter((e) => !e.success).length;
    const successes = windowEntries.filter((e) => e.success).length;

    return {
      state: this._state,
      failures,
      successes,
      lastFailureAt: this._lastFailureAt !== null
        ? new Date(this._lastFailureAt).toISOString()
        : null,
      lastStateChange: new Date(this._lastStateChange).toISOString(),
    };
  }

  /** Seconds until the cooldown expires (0 if not OPEN or already elapsed). */
  retryAfterSeconds(): number {
    if (this._state !== "OPEN" || this._openedAt === null) return 0;
    const remaining = this.config.cooldownMs - (Date.now() - this._openedAt);
    return remaining > 0 ? Math.ceil(remaining / 1000) : 0;
  }

  // ── Private helpers ────────────────────────────────────────────────

  private _transition(to: CircuitState, reason: string): void {
    const from = this._state;
    this._state = to;
    this._lastStateChange = Date.now();

    if (to === "OPEN") {
      this._openedAt = Date.now();
    } else if (to === "CLOSED") {
      this._openedAt = null;
      this._consecutiveFailures = 0;
    }

    this.onStateChange?.(from, to, reason);
  }

  private _addToWindow(success: boolean): void {
    const entry: WindowEntry = { success, timestampMs: Date.now() };
    this._window[this._windowHead] = entry;
    this._windowHead = (this._windowHead + 1) % this.config.ringBufferSize;
    if (this._windowSize < this.config.ringBufferSize) {
      this._windowSize++;
    }
  }

  /** Get window entries that fall within the configured time window. */
  private _getWindowEntries(): WindowEntry[] {
    const cutoff = Date.now() - this.config.errorRateWindowMs;
    const entries: WindowEntry[] = [];
    for (let i = 0; i < this._windowSize; i++) {
      const idx = (this._windowHead - 1 - i + this.config.ringBufferSize) % this.config.ringBufferSize;
      const entry = this._window[idx];
      if (entry && entry.timestampMs >= cutoff) {
        entries.push(entry);
      }
    }
    return entries;
  }

  /** Current error rate within the sliding window (0–1). */
  private _errorRate(): number {
    const entries = this._getWindowEntries();
    if (entries.length === 0) return 0;
    const failures = entries.filter((e) => !e.success).length;
    return failures / entries.length;
  }
}
