"use client";

import { useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { trpc } from "@/lib/trpc/react";

export interface InlineCreateProps {
  owner: "nova" | "leo";
  onCreated: () => void;
  onCancel: () => void;
}

export default function InlineCreate({ owner, onCreated, onCancel }: InlineCreateProps) {
  const queryClient = useQueryClient();
  const [value, setValue] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const createMutation = useMutation(
    trpc.obligation.create.mutationOptions({
      onSuccess: () => {
        void queryClient.invalidateQueries({ queryKey: trpc.obligation.list.queryKey() });
      },
    }),
  );

  const handleSubmit = async () => {
    const trimmed = value.trim();
    if (!trimmed) {
      onCancel();
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await createMutation.mutateAsync({
        detected_action: trimmed,
        owner,
        status: "open",
        priority: 2,
        source_channel: "dashboard",
      });
      setValue("");
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create obligation");
    } finally {
      setSubmitting(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      void handleSubmit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      onCancel();
    }
  };

  const handleBlur = () => {
    if (!value.trim()) {
      onCancel();
    }
  };

  return (
    <div className="px-1.5 pb-1.5 animate-fade-in-up">
      <input
        ref={inputRef}
        autoFocus
        type="text"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        onBlur={handleBlur}
        disabled={submitting}
        placeholder="What needs to be done?"
        className="w-full px-3 py-1.5 text-copy-13 text-ds-gray-1000 bg-ds-gray-100 border border-ds-gray-400 rounded-lg focus:outline-hidden focus:border-ds-gray-500 focus:ring-1 focus:ring-ds-gray-500 placeholder:text-ds-gray-700 disabled:opacity-50 transition-colors"
      />
      {error && (
        <p className="mt-1 text-[11px] text-red-500 px-1">{error}</p>
      )}
      <p className="mt-1 text-[11px] text-ds-gray-700 px-1">
        Enter to save, Esc to cancel
      </p>
    </div>
  );
}
