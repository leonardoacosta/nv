"use client";

import { useState } from "react";
import { CheckCircle, XCircle, RefreshCw, AlertCircle } from "lucide-react";

export type IntegrationService =
  | "anthropic"
  | "openai"
  | "elevenlabs"
  | "github"
  | "sentry"
  | "posthog";

export interface IntegrationEntry {
  service: IntegrationService;
  displayName: string;
  /** Whether the API key env var appears to be set (non-empty). */
  hasKey: boolean;
}

interface IntegrationStatusCardProps {
  integration: IntegrationEntry;
  onTest: (service: IntegrationService) => Promise<{
    valid: boolean;
    error: string | null;
    latencyMs: number;
  }>;
}

type TestState = "idle" | "pending" | "success" | "error";

const SERVICE_COLORS: Record<IntegrationService, string> = {
  anthropic: "#D97706",
  openai: "#10A37F",
  elevenlabs: "#8B5CF6",
  github: "#6e7681",
  sentry: "#FB4226",
  posthog: "#F54E00",
};

export default function IntegrationStatusCard({
  integration,
  onTest,
}: IntegrationStatusCardProps) {
  const [testState, setTestState] = useState<TestState>("idle");
  const [testResult, setTestResult] = useState<{
    valid: boolean;
    error: string | null;
    latencyMs: number;
  } | null>(null);

  const accentColor = SERVICE_COLORS[integration.service] ?? "#888";

  const handleTest = async () => {
    setTestState("pending");
    setTestResult(null);
    try {
      const result = await onTest(integration.service);
      setTestResult(result);
      setTestState(result.valid ? "success" : "error");
    } catch {
      setTestState("error");
      setTestResult({ valid: false, error: "Request failed", latencyMs: 0 });
    }
    setTimeout(() => setTestState("idle"), 5000);
  };

  const keyBadge = integration.hasKey ? (
    <span data-testid="integration-key-badge" className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-mono bg-green-700/10 text-green-700 border border-green-700/30">
      <CheckCircle size={9} />
      KEY SET
    </span>
  ) : (
    <span data-testid="integration-key-badge" className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-mono bg-ds-gray-alpha-200 text-ds-gray-700 border border-ds-gray-400">
      <AlertCircle size={9} />
      NO KEY
    </span>
  );

  return (
    <div
      data-testid={`integration-card-${integration.service}`}
      className="surface-card p-4 space-y-3"
      style={{ borderLeft: `3px solid ${accentColor}` }}
    >
      {/* Header */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2.5 flex-wrap">
          <span className="text-label-14 text-ds-gray-1000 font-semibold">
            {integration.displayName}
          </span>
          {keyBadge}
        </div>

        {/* Test button */}
        <button
          type="button"
          disabled={testState === "pending" || !integration.hasKey}
          onClick={() => void handleTest()}
          data-testid="integration-test-button"
          className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-label-13 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-50 shrink-0"
          title={!integration.hasKey ? "API key not configured" : undefined}
        >
          {testState === "pending" ? (
            <RefreshCw size={11} className="animate-spin" />
          ) : testState === "success" ? (
            <CheckCircle size={11} className="text-green-700" />
          ) : testState === "error" ? (
            <XCircle size={11} className="text-red-700" />
          ) : (
            <RefreshCw size={11} />
          )}
          Test
        </button>
      </div>

      {/* Test result */}
      {testResult && (
        <div
          data-testid="integration-test-result"
          className={`text-copy-13 ${testResult.valid ? "text-green-700" : "text-red-700"}`}
        >
          {testResult.valid
            ? `API key valid (${testResult.latencyMs}ms)`
            : testResult.error ?? "Validation failed"}
        </div>
      )}
    </div>
  );
}
