"use client";

import { useState } from "react";
import { CheckCircle, XCircle, RefreshCw } from "lucide-react";

interface ChannelEntry {
  name: string;
  connected: boolean;
  error: string | null;
  identity: { username?: string; displayName?: string } | null;
  lastMessageAt: string | null;
}

interface ChannelStatusCardProps {
  channel: ChannelEntry;
  onTest: (channelName: string) => Promise<{ valid: boolean; error: string | null; latencyMs: number }>;
}

type TestState = "idle" | "pending" | "success" | "error";

function formatRelative(iso: string | null): string {
  if (!iso) return "never";
  const d = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  return d.toLocaleDateString([], { month: "short", day: "numeric" });
}

const CHANNEL_COLORS: Record<string, string> = {
  telegram: "#229ED9",
  discord: "#5865F2",
  "microsoft teams": "#6264A7",
  teams: "#6264A7",
  slack: "#E01E5A",
};

export default function ChannelStatusCard({
  channel,
  onTest,
}: ChannelStatusCardProps) {
  const [testState, setTestState] = useState<TestState>("idle");
  const [testResult, setTestResult] = useState<{
    valid: boolean;
    error: string | null;
    latencyMs: number;
  } | null>(null);

  const accentColor =
    CHANNEL_COLORS[channel.name.toLowerCase()] ?? "#888";

  const statusDot = channel.connected
    ? "bg-green-700"
    : channel.error
      ? "bg-red-700"
      : "bg-amber-500";

  const statusLabel = channel.connected
    ? "Connected"
    : channel.error
      ? "Error"
      : "Disconnected";

  const identityLine =
    channel.identity?.displayName ??
    channel.identity?.username
      ? `@${channel.identity.username}`
      : null;

  const handleTest = async () => {
    setTestState("pending");
    setTestResult(null);
    try {
      // Use the channel name as both channel and a default target
      const result = await onTest(channel.name);
      setTestResult(result);
      setTestState(result.valid ? "success" : "error");
    } catch {
      setTestState("error");
      setTestResult({ valid: false, error: "Request failed", latencyMs: 0 });
    }
    setTimeout(() => setTestState("idle"), 4000);
  };

  return (
    <div
      className="surface-card p-4 space-y-3"
      style={{ borderLeft: `3px solid ${accentColor}` }}
    >
      {/* Header row */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2.5">
          {/* Status dot */}
          <div
            className={`w-2 h-2 rounded-full shrink-0 ${statusDot}`}
            title={statusLabel}
          />
          <span className="text-label-14 text-ds-gray-1000 font-semibold">
            {channel.name}
          </span>
          <span className="text-copy-13 text-ds-gray-700">{statusLabel}</span>
        </div>

        {/* Test button */}
        <button
          type="button"
          disabled={testState === "pending"}
          onClick={() => void handleTest()}
          className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-label-13 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-50 shrink-0"
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

      {/* Identity + last message */}
      <div className="flex items-center gap-4 text-copy-13 text-ds-gray-700">
        {identityLine && (
          <span className="font-mono">{identityLine}</span>
        )}
        <span>
          Last message:{" "}
          <span suppressHydrationWarning>
            {formatRelative(channel.lastMessageAt)}
          </span>
        </span>
      </div>

      {/* Test result */}
      {testResult && (
        <div
          className={`text-copy-13 ${testResult.valid ? "text-green-700" : "text-red-700"}`}
        >
          {testResult.valid
            ? `Connection OK (${testResult.latencyMs}ms)`
            : testResult.error ?? "Connection failed"}
        </div>
      )}

      {/* Error detail */}
      {channel.error && !testResult && (
        <p className="text-copy-13 text-red-700/70">{channel.error}</p>
      )}
    </div>
  );
}
