"use client";

import { useEffect, useState } from "react";
import {
  Save,
  RefreshCw,
  CheckCircle,
  ChevronDown,
  ChevronRight,
  AlertTriangle,
  Cpu,
  Radio,
  Plug,
  Brain,
} from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import ErrorBanner from "@/components/layout/ErrorBanner";
import type { PutConfigRequest } from "@/types/api";

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
  description?: string;
  /** If changed, the daemon needs a restart */
  requires_restart?: boolean;
}

interface SectionDef {
  id: "daemon" | "channels" | "integrations" | "memory";
  label: string;
  icon: React.ElementType;
  description: string;
  /** Keys from config object to include in this section */
  keys: string[];
}

// ---------------------------------------------------------------------------
// Explicit section schema
// ---------------------------------------------------------------------------

const SECTIONS: SectionDef[] = [
  {
    id: "daemon",
    label: "Daemon",
    icon: Cpu,
    description: "Core daemon process settings",
    keys: ["daemon", "server", "log_level", "debug", "port", "host", "interval_ms"],
  },
  {
    id: "channels",
    label: "Channels",
    icon: Radio,
    description: "WebSocket and notification channel configuration",
    keys: ["websocket", "ws", "channels", "notifications", "pubsub"],
  },
  {
    id: "integrations",
    label: "Integrations",
    icon: Plug,
    description: "External service integrations and API keys",
    keys: [
      "openai",
      "anthropic",
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
    description: "Memory and context storage settings",
    keys: ["memory", "context", "storage", "db", "database", "cache"],
  },
];

const SECRET_PATTERNS = [
  "token",
  "secret",
  "password",
  "key",
  "api_key",
  "auth",
];

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
  // Fallback bucket: daemon catches anything unmatched
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

  // Unmatched fields go into daemon section
  for (const entry of flat) {
    if (!assigned.has(entry.key)) {
      map.get("daemon")!.push(buildField(entry.key, entry.value));
    }
  }

  return map;
}

// ---------------------------------------------------------------------------
// FieldRow — single config field
// ---------------------------------------------------------------------------

interface FieldRowProps {
  field: FieldDef;
  config: ConfigObject;
  onChange: (key: string, value: ConfigValue) => void;
}

function FieldRow({ field, config, onChange }: FieldRowProps) {
  const raw = getNestedValue(config, field.key);
  const value = raw !== null ? raw : "";

  if (field.type === "boolean") {
    return (
      <div className="flex items-center gap-4 px-4 py-3.5 min-h-11">
        <div className="flex-1 min-w-0">
          <span className="text-label-13 text-ds-gray-1000">{field.label}</span>
          {field.requires_restart && (
            <span className="ml-2 text-label-13-mono text-amber-700 opacity-70">
              restart required
            </span>
          )}
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={Boolean(value)}
          onClick={() => onChange(field.key, !value)}
          className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors shrink-0 ${
            value ? "bg-ds-gray-700" : "bg-ds-gray-400"
          }`}
        >
          <span
            className={`inline-block h-3.5 w-3.5 transform rounded-full bg-white transition-transform ${
              value ? "translate-x-[18px]" : "translate-x-0.5"
            }`}
          />
        </button>
      </div>
    );
  }

  if (field.type === "secret") {
    return (
      <div className="flex items-center gap-4 px-4 py-3.5 min-h-11">
        <div className="flex-1 min-w-0">
          <span className="text-label-13 text-ds-gray-1000">{field.label}</span>
        </div>
        <div className="shrink-0 w-64">
          <div className="surface-inset flex items-center gap-2 px-3 py-1.5">
            <span className="text-label-13-mono text-ds-gray-900 tracking-widest select-none">
              {value ? "••••••••••••" : "(not set)"}
            </span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-4 px-4 py-3.5 min-h-11">
      <div className="flex-1 min-w-0">
        <span className="text-label-13 text-ds-gray-1000">{field.label}</span>
        {field.requires_restart && (
          <span className="ml-2 text-label-13-mono text-amber-700 opacity-70">
            restart required
          </span>
        )}
      </div>
      <div className="shrink-0 w-64">
        <input
          type={field.type === "number" ? "number" : "text"}
          value={String(value)}
          onChange={(e) =>
            onChange(
              field.key,
              field.type === "number" ? Number(e.target.value) : e.target.value,
            )
          }
          className="w-full px-3 py-1.5 surface-inset text-label-13-mono text-ds-gray-1000 placeholder:text-ds-gray-700 focus:outline-none focus:border-ds-gray-500 transition-colors"
        />
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// ConfigSection — collapsible section
// ---------------------------------------------------------------------------

interface ConfigSectionProps {
  section: SectionDef;
  fields: FieldDef[];
  config: ConfigObject;
  onChange: (key: string, value: ConfigValue) => void;
}

function ConfigSection({ section, fields, config, onChange }: ConfigSectionProps) {
  const [open, setOpen] = useState(true);
  const SectionIcon = section.icon;

  if (fields.length === 0) {
    return (
      <div className="surface-card overflow-hidden opacity-60">
        <div className="w-full flex items-center gap-3 px-4 py-3.5 min-h-11 text-left">
          <SectionIcon size={15} className="text-ds-gray-700 shrink-0" />
          <div className="flex-1 min-w-0">
            <h2 className="text-label-16 text-ds-gray-1000">{section.label}</h2>
          </div>
          <span className="text-label-13-mono text-ds-gray-900">0</span>
        </div>
        <div
          className="px-4 py-3"
          style={{ borderTop: "1px solid var(--ds-gray-alpha-200)" }}
        >
          <p className="text-copy-13 text-ds-gray-900 italic">No fields configured.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="surface-card overflow-hidden">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="w-full flex items-center gap-3 px-4 py-3.5 min-h-11 hover:bg-ds-gray-alpha-100 transition-colors text-left"
      >
        <SectionIcon size={15} className="text-ds-gray-700 shrink-0" />
        <div className="flex-1 min-w-0">
          <h2 className="text-label-16 text-ds-gray-1000">{section.label}</h2>
        </div>
        <span className="text-label-13-mono text-ds-gray-900">{fields.length}</span>
        <div className="text-ds-gray-700 shrink-0">
          {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </div>
      </button>

      {open && (
        <div
          style={{ borderTop: "1px solid var(--ds-gray-alpha-200)" }}
          className="divide-y divide-ds-gray-alpha-200"
        >
          {fields.map((field) => (
            <FieldRow
              key={field.key}
              field={field}
              config={config}
              onChange={onChange}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// SettingsPage
// ---------------------------------------------------------------------------

export default function SettingsPage() {
  // 1. State
  const [config, setConfig] = useState<ConfigObject>({});
  const [original, setOriginal] = useState<ConfigObject>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [restartFields, setRestartFields] = useState<string[]>([]);

  // 2. Derived
  const hasChanges = JSON.stringify(config) !== JSON.stringify(original);
  const flat = flattenConfig(config);
  const sectionFields = assignFieldsToSections(flat);

  // 3. Fetch
  const fetchConfig = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/config");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as ConfigObject;
      setConfig(data);
      setOriginal(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load config");
    } finally {
      setLoading(false);
    }
  };

  // 4. Initial load
  useEffect(() => {
    void fetchConfig();
  }, []);

  // 5. Handlers
  const handleChange = (key: string, value: ConfigValue) => {
    setConfig((prev) => setNestedValue(prev, key, value));
    setSaved(false);
    // Track restart-required fields
    const field = flat.find((f) => f.key === key);
    if (field) {
      const fieldDef = buildField(field.key, field.value);
      if (fieldDef.requires_restart) {
        setRestartFields((prev) =>
          prev.includes(key) ? prev : [...prev, key],
        );
      }
    }
  };

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      const res = await fetch("/api/config", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ fields: config } satisfies PutConfigRequest),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      setOriginal(config);
      setSaved(true);
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
        <div className="space-y-4">
          {SECTIONS.map((section) => (
            <ConfigSection
              key={section.id}
              section={section}
              fields={sectionFields.get(section.id) ?? []}
              config={config}
              onChange={handleChange}
            />
          ))}
        </div>
      )}

      {/* Unsaved-changes sticky footer */}
      {hasChanges && (
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
              disabled={saving}
              className="flex items-center gap-2 px-4 py-2 min-h-11 rounded-lg text-button-14 font-medium bg-ds-gray-700 text-white hover:bg-ds-gray-600 transition-colors disabled:opacity-50"
            >
              <Save size={14} />
              {saving ? "Saving…" : "Save Changes"}
            </button>
          </div>
        </div>
      )}
    </PageShell>
  );
}
