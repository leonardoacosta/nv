"use client";

import { useEffect, useState } from "react";
import { Plug, AlertCircle, RefreshCw } from "lucide-react";
import IntegrationCard, {
  type Integration,
} from "@/components/IntegrationCard";
import ConfigureModal from "@/components/ConfigureModal";
import type { PutConfigRequest } from "@/types/api";
import { apiFetch } from "@/lib/api-client";

const CATEGORY_LABELS: Record<Integration["category"], string> = {
  channels: "Channels",
  tools: "Tools",
  services: "Services",
};

export default function IntegrationsPage() {
  const [integrations, setIntegrations] = useState<Integration[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [configuring, setConfiguring] = useState<Integration | null>(null);

  const fetchIntegrations = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await apiFetch("/api/config");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const raw = (await res.json()) as Record<string, unknown>;

      // Transform config into integration list if no dedicated endpoint
      const items: Integration[] = raw.integrations
        ? (raw.integrations as Integration[])
        : buildFromConfig(raw);
      setIntegrations(items);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load integrations"
      );
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void fetchIntegrations();
  }, []);

  const handleSave = async (
    _id: string,
    config: Record<string, string>
  ): Promise<void> => {
    const res = await apiFetch(`/api/config`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ fields: config } satisfies PutConfigRequest),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    void fetchIntegrations();
  };

  const grouped = Object.entries(CATEGORY_LABELS).map(([cat, label]) => ({
    key: cat as Integration["category"],
    label,
    items: integrations.filter((i) => i.category === cat),
  }));

  return (
    <div className="p-4 space-y-3 w-full animate-fade-in-up">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-heading-20 text-ds-gray-1000">
            Integrations
          </h1>
          <p className="mt-0.5 text-copy-13 text-ds-gray-900">
            Connected channels, tools, and services
          </p>
        </div>
        <button
          type="button"
          onClick={() => void fetchIntegrations()}
          disabled={loading}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {error && (
        <div
          className="flex items-start gap-3 p-4 rounded-md"
          style={{
            background: "rgba(229, 72, 77, 0.08)",
            borderLeft: "3px solid var(--ds-red-700)",
          }}
        >
          <AlertCircle size={16} className="text-red-700 shrink-0 mt-0.5" />
          <span className="text-copy-14 text-red-700">{error}</span>
        </div>
      )}

      {loading ? (
        <div className="space-y-6">
          {Array.from({ length: 3 }).map((_, g) => (
            <div key={g} className="space-y-2">
              <div className="h-3 w-20 animate-pulse rounded bg-ds-gray-300" />
              {Array.from({ length: 3 }).map((_, i) => (
                <div
                  key={i}
                  className="h-16 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-alpha-400"
                />
              ))}
            </div>
          ))}
        </div>
      ) : integrations.length === 0 ? (
        <p className="text-copy-13 text-ds-gray-900 py-3">No integrations configured</p>
      ) : (
        <div className="space-y-4">
          {grouped.map(({ key, label, items }) => (
            <section key={key}>
              <div className="flex items-center gap-2 mb-3">
                <h2 className="text-label-12 text-ds-gray-700">{label}</h2>
                <span className="px-1.5 py-0.5 rounded-full bg-ds-gray-alpha-200 text-label-12 text-ds-gray-900 font-mono normal-case tracking-normal">
                  {items.length}
                </span>
              </div>
              {items.length === 0 ? (
                <p className="text-copy-13 text-ds-gray-900 py-2 pl-1 italic">
                  No integrations configured.
                </p>
              ) : (
                <div className="space-y-2">
                  {items.map((integration) => (
                    <IntegrationCard
                      key={integration.id}
                      integration={integration}
                      onConfigure={setConfiguring}
                    />
                  ))}
                </div>
              )}
            </section>
          ))}
        </div>
      )}

      <ConfigureModal
        integration={configuring}
        onClose={() => setConfiguring(null)}
        onSave={handleSave}
      />
    </div>
  );
}

// Known integration key → category mapping.
const KNOWN_INTEGRATIONS: Record<
  string,
  { category: Integration["category"]; displayName: string }
> = {
  telegram: { category: "channels", displayName: "Telegram" },
  discord: { category: "channels", displayName: "Discord" },
  slack: { category: "channels", displayName: "Slack" },
  teams: { category: "channels", displayName: "Microsoft Teams" },
  github: { category: "tools", displayName: "GitHub" },
  linear: { category: "tools", displayName: "Linear" },
  notion: { category: "tools", displayName: "Notion" },
  openai: { category: "services", displayName: "OpenAI" },
  anthropic: { category: "services", displayName: "Anthropic" },
  stripe: { category: "services", displayName: "Stripe" },
  resend: { category: "services", displayName: "Resend" },
  sentry: { category: "services", displayName: "Sentry" },
  posthog: { category: "services", displayName: "PostHog" },
};

/** Determine integration status from a config value. */
function inferStatus(value: unknown): Integration["status"] {
  if (!value) return "disconnected";
  if (typeof value === "object" && value !== null) {
    if ("enabled" in value) {
      return (value as { enabled: boolean }).enabled ? "connected" : "disconnected";
    }
    // Has nested values — check if any key looks like a credential
    const obj = value as Record<string, unknown>;
    const hasCredential = Object.entries(obj).some(
      ([k, v]) =>
        (k.includes("token") || k.includes("key") || k.includes("secret")) &&
        Boolean(v),
    );
    return hasCredential ? "connected" : "disconnected";
  }
  return "connected";
}

/** Fallback: build integration list from raw config object. */
function buildFromConfig(raw: Record<string, unknown>): Integration[] {
  const items: Integration[] = [];

  for (const [key, value] of Object.entries(raw)) {
    const lower = key.toLowerCase();
    const known = KNOWN_INTEGRATIONS[lower];

    if (known) {
      items.push({
        id: key,
        name: known.displayName,
        status: inferStatus(value),
        category: known.category,
        config:
          typeof value === "object" && value !== null
            ? (value as Record<string, string | number | boolean>)
            : undefined,
      });
    } else {
      // Unknown key — derive category heuristically
      const channelKeys = ["channel", "chat", "message"];
      const toolKeys = ["tool", "git", "issue", "tracker"];
      const category: Integration["category"] = channelKeys.some((c) =>
        lower.includes(c),
      )
        ? "channels"
        : toolKeys.some((t) => lower.includes(t))
          ? "tools"
          : "services";

      items.push({
        id: key,
        name: key.charAt(0).toUpperCase() + key.slice(1),
        status: inferStatus(value),
        category,
        config:
          typeof value === "object" && value !== null
            ? (value as Record<string, string | number | boolean>)
            : undefined,
      });
    }
  }

  return items;
}
