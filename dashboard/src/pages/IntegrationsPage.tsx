import { useEffect, useState } from "react";
import { Plug, AlertCircle, RefreshCw } from "lucide-react";
import IntegrationCard, {
  type Integration,
} from "@/components/IntegrationCard";
import ConfigureModal from "@/components/ConfigureModal";
import type { PutConfigRequest } from "@/types/api";

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
      const res = await fetch("/api/config");
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
    id: string,
    config: Record<string, string>
  ): Promise<void> => {
    const res = await fetch(`/api/config`, {
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
    <div className="p-8 space-y-6 max-w-4xl">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-cosmic-bright">
            Integrations
          </h1>
          <p className="mt-1 text-sm text-cosmic-muted">
            Connected channels, tools, and services
          </p>
        </div>
        <button
          type="button"
          onClick={() => void fetchIntegrations()}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {error && (
        <div className="flex items-center gap-3 p-4 rounded-cosmic bg-cosmic-rose/10 border border-cosmic-rose/30 text-cosmic-rose">
          <AlertCircle size={16} />
          <span className="text-sm">{error}</span>
        </div>
      )}

      {loading ? (
        <div className="space-y-6">
          {Array.from({ length: 3 }).map((_, g) => (
            <div key={g} className="space-y-2">
              <div className="h-3 w-20 animate-pulse rounded bg-cosmic-border" />
              {Array.from({ length: 3 }).map((_, i) => (
                <div
                  key={i}
                  className="h-16 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
                />
              ))}
            </div>
          ))}
        </div>
      ) : integrations.length === 0 ? (
        <div className="flex flex-col items-center gap-3 py-16 text-cosmic-muted">
          <Plug size={36} />
          <p className="text-sm">No integrations configured</p>
        </div>
      ) : (
        <div className="space-y-8">
          {grouped
            .filter((g) => g.items.length > 0)
            .map(({ key, label, items }) => (
              <section key={key}>
                <div className="flex items-center gap-2 mb-3">
                  <h2 className="text-sm font-semibold text-cosmic-text uppercase tracking-wide">
                    {label}
                  </h2>
                  <span className="text-xs font-mono text-cosmic-muted">
                    {items.length}
                  </span>
                </div>
                <div className="space-y-2">
                  {items.map((integration) => (
                    <IntegrationCard
                      key={integration.id}
                      integration={integration}
                      onConfigure={setConfiguring}
                    />
                  ))}
                </div>
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

/** Fallback: build integration list from raw config object */
function buildFromConfig(raw: Record<string, unknown>): Integration[] {
  const items: Integration[] = [];

  const channelKeys = ["telegram", "discord", "slack"];
  const toolKeys = ["github", "linear", "notion"];
  const serviceKeys = ["openai", "anthropic", "stripe", "resend"];

  for (const [key, value] of Object.entries(raw)) {
    const lower = key.toLowerCase();
    const category: Integration["category"] = channelKeys.some((c) =>
      lower.includes(c)
    )
      ? "channels"
      : toolKeys.some((t) => lower.includes(t))
        ? "tools"
        : serviceKeys.some((s) => lower.includes(s))
          ? "services"
          : "services";

    const status: Integration["status"] =
      value && typeof value === "object" && "enabled" in value
        ? (value as { enabled: boolean }).enabled
          ? "connected"
          : "disconnected"
        : value
          ? "connected"
          : "disconnected";

    items.push({
      id: key,
      name: key.charAt(0).toUpperCase() + key.slice(1),
      status,
      category,
      config:
        typeof value === "object" && value !== null
          ? (value as Record<string, string | number | boolean>)
          : undefined,
    });
  }

  return items;
}
