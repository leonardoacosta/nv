"use client";

import { useEffect, useState } from "react";
import {
  Save,
  RefreshCw,
  CheckCircle,
  AlertTriangle,
  Cpu,
  Radio,
  Plug,
  Brain,
} from "lucide-react";
import { useQuery, useMutation } from "@tanstack/react-query";
import PageShell from "@/components/layout/PageShell";
import ErrorBanner from "@/components/layout/ErrorBanner";
import SettingsSection from "@/app/settings/components/SettingsSection";
import SaveRestartBar from "@/app/settings/components/SaveRestartBar";
import FieldRow from "@/app/settings/components/FieldRow";
import ChannelStatusCard from "@/app/settings/components/ChannelStatusCard";
import IntegrationStatusCard from "@/app/settings/components/IntegrationStatusCard";
import MemorySummaryCard from "@/app/settings/components/MemorySummaryCard";
import type { IntegrationService } from "@/app/settings/components/IntegrationStatusCard";
import type { ConfigSourceEntry } from "@/app/settings/components/ConfigSourceBadge";
import { getFieldMeta } from "@/app/settings/lib/field-registry";
import type { PutConfigRequest } from "@/types/api";
import { useTRPC } from "@/lib/trpc/react";
import { trpcClient } from "@/lib/trpc/client";
// apiFetch retained for config PUT (no tRPC updateConfig procedure exists yet)
import { apiFetch } from "@/lib/api-client";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type ConfigValue = string | number | boolean | null | ConfigObject;
type ConfigObject = { [key: string]: ConfigValue };

type FieldType = "text" | "number" | "boolean" | "secret";

interface FieldDef {
  key: string;
  label: string;
  type: FieldType;
  requires_restart?: boolean;
}

interface SectionDef {
  id: "daemon" | "channels" | "integrations" | "memory";
  label: string;
  icon: React.ElementType;
  description: string;
  keys: string[];
}

// ---------------------------------------------------------------------------
// Section schema
// ---------------------------------------------------------------------------

const SECTIONS: SectionDef[] = [
  {
    id: "daemon",
    label: "Daemon",
    icon: Cpu,
    description: "Core daemon process settings",
    keys: ["daemon", "agent", "proactive_watcher", "server", "log_level", "debug", "port", "host", "interval_ms"],
  },
  {
    id: "channels",
    label: "Channels",
    icon: Radio,
    description: "Messaging channel and notification configuration",
    keys: ["telegram", "teams", "discord", "slack", "websocket", "ws", "channels", "notifications", "pubsub"],
  },
  {
    id: "integrations",
    label: "Integrations",
    icon: Plug,
    description: "External service integrations and API keys",
    keys: [
      "openai",
      "anthropic",
      "elevenlabs",
      "github",
      "stripe",
      "resend",
      "sentry",
      "posthog",
      "integrations",
      "api_key",
      "token",
      "secret",
      "webhook",
    ],
  },
  {
    id: "memory",
    label: "Memory",
    icon: Brain,
    description: "Memory, personas and context storage settings",
    keys: ["memory", "personas", "context", "storage", "db", "database", "cache"],
  },
];

const SECRET_PATTERNS = ["token", "secret", "password", "key", "api_key", "auth"];

function isSecret(key: string): boolean {
  const lower = key.toLowerCase();
  return SECRET_PATTERNS.some((p) => lower.includes(p));
}

// ---------------------------------------------------------------------------
// Config traversal
// ---------------------------------------------------------------------------

function flattenConfig(
  obj: ConfigObject,
  prefix = "",
): Array<{ key: string; value: ConfigValue; topKey: string }> {
  const result: Array<{ key: string; value: ConfigValue; topKey: string }> = [];
  for (const [k, v] of Object.entries(obj)) {
    const fullKey = prefix ? `${prefix}.${k}` : k;
    const topKey = prefix ? prefix.split(".")[0]! : k;
    if (v !== null && typeof v === "object" && !Array.isArray(v)) {
      result.push(...flattenConfig(v as ConfigObject, fullKey));
    } else {
      result.push({ key: fullKey, value: v, topKey });
    }
  }
  return result;
}

function buildField(key: string, value: ConfigValue): FieldDef {
  const parts = key.split(".");
  const leafKey = parts[parts.length - 1] ?? key;
  const label = leafKey.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());

  const secret = isSecret(leafKey);
  const type: FieldType =
    typeof value === "boolean"
      ? "boolean"
      : typeof value === "number"
        ? "number"
        : secret
          ? "secret"
          : "text";

  return {
    key,
    label,
    type,
    requires_restart: key.includes("daemon") || key.includes("port") || key.includes("host"),
  };
}

function getNestedValue(obj: ConfigObject, path: string): ConfigValue {
  const parts = path.split(".");
  let cur: ConfigValue = obj;
  for (const part of parts) {
    if (cur === null || typeof cur !== "object") return null;
    cur = (cur as ConfigObject)[part] ?? null;
  }
  return cur;
}

function setNestedValue(obj: ConfigObject, path: string, value: ConfigValue): ConfigObject {
  const result = { ...obj };
  const parts = path.split(".");
  if (parts.length === 1) {
    result[path] = value;
    return result;
  }
  const [head, ...rest] = parts;
  result[head!] = setNestedValue(
    (result[head!] as ConfigObject) ?? {},
    rest.join("."),
    value,
  );
  return result;
}

function assignFieldsToSections(
  flat: Array<{ key: string; value: ConfigValue; topKey: string }>,
): Map<SectionDef["id"], FieldDef[]> {
  const map = new Map<SectionDef["id"], FieldDef[]>(
    SECTIONS.map((s) => [s.id, []]),
  );
  const assigned = new Set<string>();

  for (const section of SECTIONS) {
    const fields = map.get(section.id)!;
    for (const entry of flat) {
      const matchesSection = section.keys.some(
        (k) =>
          entry.topKey.toLowerCase() === k.toLowerCase() ||
          entry.key.toLowerCase().startsWith(k.toLowerCase()),
      );
      if (matchesSection && !assigned.has(entry.key)) {
        fields.push(buildField(entry.key, entry.value));
        assigned.add(entry.key);
      }
    }
  }

  for (const entry of flat) {
    if (!assigned.has(entry.key)) {
      map.get("daemon")!.push(buildField(entry.key, entry.value));
    }
  }

  return map;
}

// ---------------------------------------------------------------------------
// Integration definitions
// ---------------------------------------------------------------------------

const INTEGRATIONS: Array<{
  service: IntegrationService;
  displayName: string;
  envVar: string;
}> = [
  { service: "anthropic", displayName: "Anthropic", envVar: "ANTHROPIC_API_KEY" },
  { service: "openai", displayName: "OpenAI", envVar: "OPENAI_API_KEY" },
  { service: "elevenlabs", displayName: "ElevenLabs", envVar: "ELEVENLABS_API_KEY" },
  { service: "github", displayName: "GitHub", envVar: "GITHUB_TOKEN" },
  { service: "sentry", displayName: "Sentry", envVar: "SENTRY_AUTH_TOKEN" },
  { service: "posthog", displayName: "PostHog", envVar: "POSTHOG_API_KEY" },
];

// ---------------------------------------------------------------------------
// SettingsPage
// ---------------------------------------------------------------------------

export default function SettingsPage() {
  const trpc = useTRPC();

  // 1. Local config state (still managed imperatively for PUT flow)
  const [config, setConfig] = useState<ConfigObject>({});
  const [original, setOriginal] = useState<ConfigObject>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [saveFlash, setSaveFlash] = useState(false);
  const [restartFields, setRestartFields] = useState<string[]>([]);
  const [fieldErrors, setFieldErrors] = useState<Set<string>>(new Set());

  // 2. Supplementary tRPC queries
  const configSourcesQuery = useQuery(trpc.system.configSources.queryOptions());
  const channelStatusQuery = useQuery(trpc.system.channelStatus.queryOptions());
  const memorySummaryQuery = useQuery(trpc.system.memorySummary.queryOptions());

  // Mutation: test channel
  const testChannelMutation = useMutation(trpc.system.testChannel.mutationOptions());

  // Mutation: test integration
  const testIntegrationMutation = useMutation(trpc.system.testIntegration.mutationOptions());

  // 3. Derived
  const hasChanges = JSON.stringify(config) !== JSON.stringify(original);
  const flat = flattenConfig(config);
  const sectionFields = assignFieldsToSections(flat);
  const hasFieldErrors = fieldErrors.size > 0;

  // Build source map from configSources query
  const sourceMap = new Map<string, ConfigSourceEntry>();
  for (const entry of configSourcesQuery.data ?? []) {
    sourceMap.set(entry.key, entry);
  }

  // 4. Fetch config on mount
  const fetchConfig = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = (await trpcClient.system.config.query()) as ConfigObject;
      setConfig(result);
      setOriginal(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load config");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void fetchConfig();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // 5. Handlers
  const handleChange = (key: string, value: ConfigValue) => {
    setConfig((prev) => setNestedValue(prev, key, value));
    setSaved(false);
    const fieldDef = buildField(key, value);
    if (fieldDef.requires_restart) {
      setRestartFields((prev) => prev.includes(key) ? prev : [...prev, key]);
    }
  };

  const handleValidationChange = (key: string, hasError: boolean) => {
    setFieldErrors((prev) => {
      const next = new Set(prev);
      if (hasError) next.add(key);
      else next.delete(key);
      return next;
    });
  };

  const handleSave = async () => {
    if (hasFieldErrors) return;
    setSaving(true);
    setError(null);
    try {
      const res = await apiFetch("/api/config", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ fields: config } satisfies PutConfigRequest),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      setOriginal(config);
      setSaved(true);
      setSaveFlash(true);
      setTimeout(() => setSaveFlash(false), 300);
      setTimeout(() => setSaved(false), 3000);
      setRestartFields([]);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save config");
    } finally {
      setSaving(false);
    }
  };

  const handleReset = () => {
    setConfig(original);
    setSaved(false);
    setRestartFields([]);
    setFieldErrors(new Set());
  };

  const handleTestChannel = async (channelName: string) => {
    const result = await testChannelMutation.mutateAsync({
      channel: channelName.toLowerCase(),
      target: "self",
    });
    return result;
  };

  const handleTestIntegration = async (service: IntegrationService) => {
    const result = await testIntegrationMutation.mutateAsync({ service });
    return result;
  };

  // 6. Action slot
  const action = (
    <div className="flex items-center gap-2">
      <button
        type="button"
        onClick={() => void fetchConfig()}
        disabled={loading}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
      >
        <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
        <span className="hidden sm:inline">Reload</span>
      </button>
      {saved && !hasChanges && (
        <span className="flex items-center gap-1.5 text-label-13 text-green-700">
          <CheckCircle size={13} />
          Saved
        </span>
      )}
    </div>
  );

  return (
    <PageShell
      title="Settings"
      subtitle="Configure Nova daemon preferences"
      action={action}
    >
      {error && (
        <div className="mb-4">
          <ErrorBanner message={error} onRetry={() => void fetchConfig()} />
        </div>
      )}

      {/* Field validation error banner */}
      {hasFieldErrors && (
        <div className="mb-4 flex items-start gap-3 p-4 rounded-md bg-red-700/08 border border-red-700/30">
          <AlertTriangle size={16} className="text-red-700 shrink-0 mt-0.5" />
          <p className="text-copy-13 text-red-700">
            {fieldErrors.size} field{fieldErrors.size !== 1 ? "s have" : " has"} validation errors. Fix before saving.
          </p>
        </div>
      )}

      {/* Restart notice banner */}
      {restartFields.length > 0 && (
        <div
          className="mb-4 flex items-start gap-3 p-4 rounded-md"
          style={{
            background: "rgba(245, 166, 35, 0.08)",
            borderLeft: "3px solid var(--ds-amber-700)",
          }}
        >
          <AlertTriangle size={16} className="text-amber-700 shrink-0 mt-0.5" />
          <div className="flex-1 min-w-0">
            <p className="text-label-14 font-medium text-amber-700">
              Daemon restart required
            </p>
            <p className="text-copy-13 text-amber-700/70 mt-0.5">
              Changes to daemon settings take effect after restarting the Nova daemon.
            </p>
          </div>
        </div>
      )}

      {loading ? (
        <div className="space-y-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <div
              key={i}
              className="h-40 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
            />
          ))}
        </div>
      ) : (
        <div
          className="space-y-3 transition-colors duration-300"
          style={{
            backgroundColor: saveFlash ? "rgba(12, 206, 107, 0.08)" : "transparent",
            borderRadius: "12px",
          }}
        >
          {SECTIONS.map((section) => {
            const fields = sectionFields.get(section.id) ?? [];

            return (
              <SettingsSection
                key={section.id}
                id={section.id}
                title={section.label}
                icon={section.icon}
                description={section.description}
                itemCount={fields.length}
              >
                {/* Channel status cards above channel config fields */}
                {section.id === "channels" && (
                  <>
                    {channelStatusQuery.isLoading && (
                      <div className="p-4 space-y-2">
                        {Array.from({ length: 2 }).map((_, i) => (
                          <div key={i} className="h-20 animate-pulse rounded-lg bg-ds-gray-100 border border-ds-gray-400" />
                        ))}
                      </div>
                    )}
                    {channelStatusQuery.data && channelStatusQuery.data.length > 0 && (
                      <div className="p-4 space-y-3">
                        <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest font-semibold">
                          Live Status
                        </p>
                        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
                          {channelStatusQuery.data.map((ch) => (
                            <ChannelStatusCard
                              key={ch.name}
                              channel={ch}
                              onTest={handleTestChannel}
                            />
                          ))}
                        </div>
                      </div>
                    )}
                  </>
                )}

                {/* Integration status cards above integration config fields */}
                {section.id === "integrations" && (
                  <div className="p-4 space-y-3">
                    <p className="text-[10px] text-ds-gray-900 uppercase tracking-widest font-semibold">
                      API Key Status
                    </p>
                    <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
                      {INTEGRATIONS.map((intg) => (
                        <IntegrationStatusCard
                          key={intg.service}
                          integration={{
                            service: intg.service,
                            displayName: intg.displayName,
                            // Presence inferred from config sources — key is set if source is env or file
                            hasKey: sourceMap.get(intg.envVar)?.source === "env" ||
                              sourceMap.get(intg.envVar.toLowerCase())?.source === "env" ||
                              // Fallback: check if field exists in config with a non-empty value
                              Boolean(
                                getNestedValue(
                                  config,
                                  intg.service + ".api_key",
                                ) ?? getNestedValue(config, intg.service + ".token"),
                              ),
                          }}
                          onTest={handleTestIntegration}
                        />
                      ))}
                    </div>
                  </div>
                )}

                {/* Memory summary card above memory config fields */}
                {section.id === "memory" && (
                  <>
                    {memorySummaryQuery.isLoading && (
                      <div className="p-4">
                        <div className="h-20 animate-pulse rounded-lg bg-ds-gray-100 border border-ds-gray-400" />
                      </div>
                    )}
                    {memorySummaryQuery.data && (
                      <MemorySummaryCard data={memorySummaryQuery.data} />
                    )}
                  </>
                )}

                {/* Config fields */}
                {fields.map((field) => (
                  <FieldRow
                    key={field.key}
                    field={field}
                    meta={getFieldMeta(field.key)}
                    value={getNestedValue(config, field.key) as string | number | boolean | null}
                    source={sourceMap.get(field.key)}
                    onChange={handleChange}
                    onValidationChange={handleValidationChange}
                  />
                ))}
              </SettingsSection>
            );
          })}
        </div>
      )}

      {/* Restart-required floating bar */}
      <SaveRestartBar
        dirtyCount={restartFields.length}
        saving={saving}
        onSaveRestart={() => void handleSave()}
        onDiscard={handleReset}
      />

      {/* Unsaved-changes sticky footer (non-restart changes) */}
      {hasChanges && restartFields.length === 0 && (
        <div className="sticky bottom-4 mt-6 surface-raised flex items-center justify-between gap-4 p-4 shadow-md">
          <p className="text-copy-14 text-ds-gray-900">You have unsaved changes</p>
          <div className="flex items-center gap-2 shrink-0">
            <button
              type="button"
              onClick={handleReset}
              disabled={saving}
              className="flex items-center gap-2 px-3 py-2 min-h-11 surface-base text-label-13 text-ds-gray-900 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
            >
              Reset
            </button>
            <button
              type="button"
              onClick={() => void handleSave()}
              disabled={saving || hasFieldErrors}
              className="flex items-center gap-2 px-4 py-2 min-h-11 rounded-lg text-button-14 font-medium bg-ds-gray-700 text-white hover:bg-ds-gray-600 transition-colors disabled:opacity-50"
            >
              <Save size={14} />
              {saving ? "Saving..." : hasFieldErrors ? `${fieldErrors.size} errors` : "Save Changes"}
            </button>
          </div>
        </div>
      )}
    </PageShell>
  );
}
