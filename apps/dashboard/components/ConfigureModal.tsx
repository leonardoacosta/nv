"use client";

import { useState, useEffect } from "react";
import { X, Save, AlertCircle } from "lucide-react";
import type { Integration } from "@/components/IntegrationCard";

interface ConfigureModalProps {
  integration: Integration | null;
  onClose: () => void;
  onSave: (id: string, config: Record<string, string>) => Promise<void>;
}

export default function ConfigureModal({
  integration,
  onClose,
  onSave,
}: ConfigureModalProps) {
  const [fields, setFields] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (integration?.config) {
      const stringified: Record<string, string> = {};
      for (const [k, v] of Object.entries(integration.config)) {
        stringified[k] = String(v);
      }
      setFields(stringified);
    } else {
      setFields({});
    }
    setError(null);
  }, [integration]);

  if (!integration) return null;

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      await onSave(integration.id, fields);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save");
    } finally {
      setSaving(false);
    }
  };

  const configKeys = integration.config ? Object.keys(integration.config) : [];

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="modal-title"
    >
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Panel */}
      <div className="relative w-full max-w-md bg-cosmic-surface border border-cosmic-border rounded-cosmic shadow-cosmic-lg">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-cosmic-border">
          <div>
            <h2
              id="modal-title"
              className="text-sm font-semibold text-cosmic-bright"
            >
              Configure {integration.name}
            </h2>
            <p className="text-xs text-cosmic-muted mt-0.5">
              {integration.description}
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="flex items-center justify-center w-7 h-7 rounded-lg text-cosmic-muted hover:text-cosmic-text hover:bg-cosmic-border transition-colors"
          >
            <X size={14} />
          </button>
        </div>

        {/* Body */}
        <div className="px-5 py-4 space-y-4">
          {error && (
            <div className="flex items-center gap-2 p-3 rounded-lg bg-cosmic-rose/10 border border-cosmic-rose/30 text-cosmic-rose text-xs">
              <AlertCircle size={14} />
              {error}
            </div>
          )}

          {configKeys.length === 0 ? (
            <p className="text-sm text-cosmic-muted text-center py-4">
              No configurable settings for {integration.name}
            </p>
          ) : (
            configKeys.map((key) => (
              <div key={key}>
                <label className="block text-xs font-medium text-cosmic-muted uppercase tracking-wide mb-1.5">
                  {key.replace(/_/g, " ")}
                </label>
                <input
                  type={key.toLowerCase().includes("token") || key.toLowerCase().includes("secret") || key.toLowerCase().includes("key") ? "password" : "text"}
                  value={fields[key] ?? ""}
                  onChange={(e) =>
                    setFields((prev) => ({ ...prev, [key]: e.target.value }))
                  }
                  className="w-full px-3 py-2 rounded-lg bg-cosmic-dark border border-cosmic-border text-sm text-cosmic-text font-mono placeholder:text-cosmic-muted focus:outline-none focus:border-cosmic-purple/60 transition-colors"
                  placeholder={`Enter ${key.replace(/_/g, " ").toLowerCase()}`}
                />
              </div>
            ))
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-5 py-4 border-t border-cosmic-border">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-1.5 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors"
          >
            Cancel
          </button>
          {configKeys.length > 0 && (
            <button
              type="button"
              onClick={() => void handleSave()}
              disabled={saving}
              className="flex items-center gap-2 px-4 py-1.5 rounded-lg text-sm font-medium bg-cosmic-purple text-white hover:bg-cosmic-purple/80 transition-colors disabled:opacity-50"
            >
              <Save size={13} />
              {saving ? "Saving..." : "Save"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
