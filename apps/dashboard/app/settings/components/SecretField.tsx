"use client";

import { useEffect, useRef, useState } from "react";

interface SecretFieldProps {
  value: string;
  disabled?: boolean;
  onChange: (value: string) => void;
}

export default function SecretField({
  value,
  disabled = false,
  onChange,
}: SecretFieldProps) {
  const [revealed, setRevealed] = useState(false);
  const [editing, setEditing] = useState(false);
  const revealTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const hasValue = Boolean(value && value.trim());
  const last4 = hasValue ? value.slice(-4) : null;
  const masked = hasValue ? `--------${last4}` : "(not set)";

  const handleReveal = () => {
    if (disabled) return;
    setRevealed(true);
    if (revealTimerRef.current) clearTimeout(revealTimerRef.current);
    revealTimerRef.current = setTimeout(() => {
      setRevealed(false);
    }, 5000);
  };

  useEffect(() => {
    return () => {
      if (revealTimerRef.current) clearTimeout(revealTimerRef.current);
    };
  }, []);

  if (editing) {
    return (
      <div className="flex items-center gap-1.5">
        <input
          autoFocus
          type="text"
          value={value}
          placeholder="Enter new value..."
          disabled={disabled}
          onChange={(e) => onChange(e.target.value)}
          onBlur={() => setEditing(false)}
          className="flex-1 min-w-0 px-3 py-1.5 surface-inset text-label-13-mono text-ds-gray-1000 placeholder:text-ds-gray-700 focus:outline-hidden focus:border-ds-gray-500 transition-colors"
        />
      </div>
    );
  }

  return (
    <div className="flex items-center gap-1.5" data-testid="secret-field">
      {/* Status dot */}
      <div
        data-testid="secret-field-dot"
        className={`shrink-0 w-2 h-2 rounded-full ${hasValue ? "bg-green-700" : "bg-red-700"}`}
        title={hasValue ? "Value is set" : "Value is not set"}
      />

      {/* Masked / revealed display */}
      <div
        data-testid="secret-field-display"
        className="flex-1 min-w-0 surface-inset flex items-center px-3 py-1.5 cursor-pointer"
        onClick={() => !disabled && setEditing(true)}
        role="button"
        tabIndex={disabled ? -1 : 0}
        onKeyDown={(e) => {
          if (!disabled && (e.key === "Enter" || e.key === " ")) {
            e.preventDefault();
            setEditing(true);
          }
        }}
        title="Click to edit"
      >
        <span data-testid="secret-field-value" className="text-label-13-mono text-ds-gray-900 tracking-widest select-none truncate">
          {revealed ? value : masked}
        </span>
      </div>

      {/* Reveal toggle — only shown when value is set */}
      {hasValue && (
        <button
          type="button"
          disabled={disabled}
          data-testid="secret-field-reveal-toggle"
          onClick={(e) => {
            e.stopPropagation();
            if (revealed) {
              setRevealed(false);
              if (revealTimerRef.current) clearTimeout(revealTimerRef.current);
            } else {
              handleReveal();
            }
          }}
          className="shrink-0 px-1.5 py-1 text-[10px] font-mono text-ds-gray-700 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 rounded transition-colors disabled:opacity-50"
          title={revealed ? "Hide value" : "Show value for 5 seconds"}
        >
          {revealed ? "hide" : "show"}
        </button>
      )}
    </div>
  );
}
