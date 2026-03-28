"use client";

import { useState } from "react";
import type { FieldMeta } from "../lib/field-registry";
import ConfigSourceBadge from "./ConfigSourceBadge";
import SecretField from "./SecretField";
import type { ConfigSourceEntry } from "./ConfigSourceBadge";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type ConfigValue = string | number | boolean | null;
type FieldType = "text" | "number" | "boolean" | "secret";

export interface FieldDef {
  key: string;
  label: string;
  type: FieldType;
  requires_restart?: boolean;
}

interface FieldRowProps {
  field: FieldDef;
  meta: FieldMeta;
  value: ConfigValue;
  source?: ConfigSourceEntry;
  onChange: (key: string, value: ConfigValue) => void;
  onValidationChange?: (key: string, hasError: boolean) => void;
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

function validate(value: ConfigValue, meta: FieldMeta, type: FieldType): string | null {
  if (value === null || value === "") return null;

  if (type === "number") {
    const num = Number(value);
    if (isNaN(num)) return "Must be a number";
    if (meta.min !== undefined && num < meta.min) return `Minimum is ${meta.min}`;
    if (meta.max !== undefined && num > meta.max) return `Maximum is ${meta.max}`;
  }

  if (type === "text" && meta.pattern) {
    try {
      const re = new RegExp(meta.pattern);
      if (!re.test(String(value))) {
        return meta.patternHint ?? "Invalid format";
      }
    } catch {
      // Invalid regex in registry — ignore
    }
  }

  return null;
}

// ---------------------------------------------------------------------------
// FieldRow
// ---------------------------------------------------------------------------

export default function FieldRow({
  field,
  meta,
  value,
  source,
  onChange,
  onValidationChange,
}: FieldRowProps) {
  const [error, setError] = useState<string | null>(null);

  const isEnvLocked = source?.source === "env";
  const displayValue = value !== null ? value : "";
  const placeholder = meta.placeholder ?? (meta.default !== undefined ? String(meta.default) : undefined);

  const handleBlur = () => {
    const err = validate(displayValue, meta, field.type);
    setError(err);
    onValidationChange?.(field.key, err !== null);
  };

  if (field.type === "boolean") {
    return (
      <div className="flex items-start gap-4 px-4 py-3.5 min-h-11">
        <div className="flex-1 min-w-0 pt-0.5">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-label-13 text-ds-gray-1000">{field.label}</span>
            {source && <ConfigSourceBadge source={source} />}
            {field.requires_restart && (
              <span className="text-label-13-mono text-amber-700 opacity-70 text-[10px]">
                restart required
              </span>
            )}
          </div>
          {meta.description && (
            <p className="text-copy-13 text-ds-gray-700 mt-0.5">{meta.description}</p>
          )}
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={Boolean(displayValue)}
          disabled={isEnvLocked}
          onClick={() => onChange(field.key, !displayValue)}
          className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors shrink-0 mt-0.5 ${
            displayValue ? "bg-ds-gray-700" : "bg-ds-gray-400"
          } ${isEnvLocked ? "opacity-50 cursor-not-allowed" : ""}`}
        >
          <span
            className={`inline-block h-3.5 w-3.5 transform rounded-full bg-white transition-transform ${
              displayValue ? "translate-x-[18px]" : "translate-x-0.5"
            }`}
          />
        </button>
      </div>
    );
  }

  if (field.type === "secret") {
    return (
      <div className="flex items-start gap-4 px-4 py-3.5 min-h-11">
        <div className="flex-1 min-w-0 pt-0.5">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-label-13 text-ds-gray-1000">{field.label}</span>
            {source && <ConfigSourceBadge source={source} />}
          </div>
          {meta.description && (
            <p className="text-copy-13 text-ds-gray-700 mt-0.5">{meta.description}</p>
          )}
        </div>
        <div className="shrink-0 w-64">
          <SecretField
            value={String(displayValue)}
            disabled={isEnvLocked}
            onChange={(v) => onChange(field.key, v)}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="flex items-start gap-4 px-4 py-3.5 min-h-11">
      <div className="flex-1 min-w-0 pt-0.5">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-label-13 text-ds-gray-1000">{field.label}</span>
          {source && <ConfigSourceBadge source={source} />}
          {field.requires_restart && (
            <span className="text-label-13-mono text-amber-700 opacity-70 text-[10px]">
              restart required
            </span>
          )}
        </div>
        {meta.description && (
          <p className="text-copy-13 text-ds-gray-700 mt-0.5">{meta.description}</p>
        )}
        {error && (
          <p className="text-copy-13 text-red-700 mt-0.5">{error}</p>
        )}
        {!error && meta.patternHint && (
          <p className="text-copy-13 text-ds-gray-700 mt-0.5 font-mono text-[11px]">{meta.patternHint}</p>
        )}
      </div>
      <div className="shrink-0 w-64">
        <div className="flex items-center gap-1.5">
          <input
            type={field.type === "number" ? "number" : "text"}
            value={String(displayValue)}
            placeholder={placeholder}
            disabled={isEnvLocked}
            min={meta.min}
            max={meta.max}
            onChange={(e) =>
              onChange(
                field.key,
                field.type === "number" ? Number(e.target.value) : e.target.value,
              )
            }
            onBlur={handleBlur}
            className={`flex-1 min-w-0 px-3 py-1.5 surface-inset text-label-13-mono text-ds-gray-1000 placeholder:text-ds-gray-700 focus:outline-hidden focus:border-ds-gray-500 transition-colors ${
              isEnvLocked ? "opacity-50 cursor-not-allowed" : ""
            } ${error ? "border-red-700/50" : ""}`}
          />
          {meta.unit && (
            <span className="shrink-0 text-[11px] font-mono text-ds-gray-700 px-1.5 py-1 rounded bg-ds-gray-100 border border-ds-gray-400">
              {meta.unit}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
