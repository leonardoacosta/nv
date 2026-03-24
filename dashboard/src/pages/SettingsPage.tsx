import { useEffect, useState } from "react";
import {
  Settings,
  Save,
  RefreshCw,
  AlertCircle,
  CheckCircle,
  ChevronDown,
  ChevronRight,
} from "lucide-react";
import type { PutConfigRequest } from "@/types/api";

type ConfigValue = string | number | boolean | null | ConfigObject;
type ConfigObject = { [key: string]: ConfigValue };

interface FieldDef {
  key: string;
  label: string;
  type: "text" | "number" | "boolean" | "password";
  description?: string;
  section: string;
}

function inferFields(obj: ConfigObject, prefix = "", section = "General"): FieldDef[] {
  const fields: FieldDef[] = [];
  for (const [key, value] of Object.entries(obj)) {
    const fullKey = prefix ? `${prefix}.${key}` : key;
    if (value !== null && typeof value === "object" && !Array.isArray(value)) {
      fields.push(...inferFields(value as ConfigObject, fullKey, key));
    } else {
      const isSecret =
        key.toLowerCase().includes("token") ||
        key.toLowerCase().includes("secret") ||
        key.toLowerCase().includes("password") ||
        key.toLowerCase().includes("key");
      fields.push({
        key: fullKey,
        label: key.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()),
        type:
          typeof value === "boolean"
            ? "boolean"
            : typeof value === "number"
              ? "number"
              : isSecret
                ? "password"
                : "text",
        section,
      });
    }
  }
  return fields;
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

function setNestedValue(
  obj: ConfigObject,
  path: string,
  value: ConfigValue
): ConfigObject {
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
    value
  );
  return result;
}

interface SectionProps {
  title: string;
  fields: FieldDef[];
  config: ConfigObject;
  onChange: (key: string, value: ConfigValue) => void;
}

function ConfigSection({ title, fields, config, onChange }: SectionProps) {
  const [open, setOpen] = useState(true);

  return (
    <div className="rounded-cosmic border border-cosmic-border bg-cosmic-surface overflow-hidden">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="w-full flex items-center gap-3 px-4 py-3 hover:bg-cosmic-border/20 transition-colors text-left"
      >
        <div className="shrink-0 text-cosmic-muted">
          {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </div>
        <h2 className="text-sm font-semibold text-cosmic-text capitalize">
          {title}
        </h2>
        <span className="text-xs font-mono text-cosmic-muted">{fields.length}</span>
      </button>

      {open && (
        <div className="border-t border-cosmic-border divide-y divide-cosmic-border">
          {fields.map((field) => {
            const raw = getNestedValue(config, field.key);
            const value = raw !== null ? raw : "";

            return (
              <div
                key={field.key}
                className="flex items-center gap-4 px-4 py-3"
              >
                <div className="flex-1 min-w-0">
                  <label className="text-xs font-medium text-cosmic-text block mb-0.5">
                    {field.label}
                  </label>
                  {field.description && (
                    <p className="text-xs text-cosmic-muted">
                      {field.description}
                    </p>
                  )}
                </div>

                <div className="shrink-0 w-64">
                  {field.type === "boolean" ? (
                    <button
                      type="button"
                      role="switch"
                      aria-checked={Boolean(value)}
                      onClick={() => onChange(field.key, !value)}
                      className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
                        value ? "bg-cosmic-purple" : "bg-cosmic-border"
                      }`}
                    >
                      <span
                        className={`inline-block h-3.5 w-3.5 transform rounded-full bg-white transition-transform ${
                          value ? "translate-x-4.5" : "translate-x-0.5"
                        }`}
                      />
                    </button>
                  ) : (
                    <input
                      type={field.type}
                      value={String(value)}
                      onChange={(e) =>
                        onChange(
                          field.key,
                          field.type === "number"
                            ? Number(e.target.value)
                            : e.target.value
                        )
                      }
                      className="w-full px-3 py-1.5 rounded-lg bg-cosmic-dark border border-cosmic-border text-sm text-cosmic-text font-mono placeholder:text-cosmic-muted focus:outline-none focus:border-cosmic-purple/60 transition-colors"
                    />
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

export default function SettingsPage() {
  const [config, setConfig] = useState<ConfigObject>({});
  const [original, setOriginal] = useState<ConfigObject>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

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

  useEffect(() => {
    void fetchConfig();
  }, []);

  const handleChange = (key: string, value: ConfigValue) => {
    setConfig((prev) => setNestedValue(prev, key, value));
    setSaved(false);
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
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save config");
    } finally {
      setSaving(false);
    }
  };

  const hasChanges =
    JSON.stringify(config) !== JSON.stringify(original);

  const fields = inferFields(config);
  const sections = [...new Set(fields.map((f) => f.section))];

  return (
    <div className="p-8 space-y-6 max-w-4xl">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-cosmic-bright">
            Settings
          </h1>
          <p className="mt-1 text-sm text-cosmic-muted">
            Configure Nova daemon preferences
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => void fetchConfig()}
            disabled={loading}
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors disabled:opacity-50"
          >
            <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
            Reset
          </button>
          <button
            type="button"
            onClick={() => void handleSave()}
            disabled={saving || !hasChanges}
            className="flex items-center gap-2 px-4 py-1.5 rounded-lg text-sm font-medium bg-cosmic-purple text-white hover:bg-cosmic-purple/80 transition-colors disabled:opacity-50"
          >
            {saved ? (
              <CheckCircle size={14} />
            ) : (
              <Save size={14} />
            )}
            {saving ? "Saving..." : saved ? "Saved!" : "Save Changes"}
          </button>
        </div>
      </div>

      {error && (
        <div className="flex items-center gap-3 p-4 rounded-cosmic bg-cosmic-rose/10 border border-cosmic-rose/30 text-cosmic-rose">
          <AlertCircle size={16} />
          <span className="text-sm">{error}</span>
        </div>
      )}

      {saved && !hasChanges && (
        <div className="flex items-center gap-3 p-4 rounded-cosmic bg-emerald-500/10 border border-emerald-500/30 text-emerald-400">
          <CheckCircle size={16} />
          <span className="text-sm">Settings saved successfully</span>
        </div>
      )}

      {loading ? (
        <div className="space-y-4">
          {Array.from({ length: 3 }).map((_, i) => (
            <div
              key={i}
              className="h-40 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
            />
          ))}
        </div>
      ) : fields.length === 0 ? (
        <div className="flex flex-col items-center gap-3 py-16 text-cosmic-muted">
          <Settings size={36} />
          <p className="text-sm">No configuration available</p>
        </div>
      ) : (
        <div className="space-y-4">
          {sections.map((section) => (
            <ConfigSection
              key={section}
              title={section}
              fields={fields.filter((f) => f.section === section)}
              config={config}
              onChange={handleChange}
            />
          ))}
        </div>
      )}

      {hasChanges && (
        <div className="sticky bottom-4 flex items-center justify-between p-4 rounded-cosmic bg-cosmic-surface border border-cosmic-purple/40 shadow-cosmic">
          <p className="text-sm text-cosmic-muted">You have unsaved changes</p>
          <button
            type="button"
            onClick={() => void handleSave()}
            disabled={saving}
            className="flex items-center gap-2 px-4 py-1.5 rounded-lg text-sm font-medium bg-cosmic-purple text-white hover:bg-cosmic-purple/80 transition-colors disabled:opacity-50"
          >
            <Save size={14} />
            {saving ? "Saving..." : "Save Changes"}
          </button>
        </div>
      )}
    </div>
  );
}
