"use client";

import { useCallback, useEffect, useState } from "react";
import { X, Copy, Check } from "lucide-react";
import type { DiscoveredContact, ContactRelationship } from "@/types/api";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function relativeTime(iso: string): string {
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diff = now - then;
  if (diff < 0) return "just now";
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  const weeks = Math.floor(days / 7);
  if (weeks < 5) return `${weeks}w ago`;
  const months = Math.floor(days / 30);
  if (months < 12) return `${months}mo ago`;
  return ">1y ago";
}

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleDateString("en-US", {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  } catch {
    return iso;
  }
}

const CHANNEL_COLORS: Record<string, { bg: string; text: string }> = {
  telegram: { bg: "bg-sky-500/20", text: "text-sky-400" },
  discord: { bg: "bg-indigo-500/20", text: "text-indigo-400" },
  teams: { bg: "bg-violet-500/20", text: "text-violet-400" },
};

const RELATIONSHIP_BADGE: Record<
  string,
  { bg: string; text: string; label: string }
> = {
  work: { bg: "bg-ds-gray-alpha-200", text: "text-ds-gray-1000", label: "Work" },
  "personal-client": { bg: "bg-red-700/20", text: "text-red-700", label: "Personal" },
  contributor: { bg: "bg-amber-500/20", text: "text-amber-400", label: "Contributor" },
  social: { bg: "bg-emerald-500/20", text: "text-emerald-400", label: "Social" },
};

// ---------------------------------------------------------------------------
// Copy Button
// ---------------------------------------------------------------------------

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard API not available
    }
  };

  return (
    <button
      type="button"
      onClick={(e) => void handleCopy(e)}
      className="flex items-center justify-center w-6 h-6 rounded text-ds-gray-900 hover:text-ds-gray-1000 hover:bg-ds-gray-100/40 transition-colors"
      aria-label={`Copy ${text}`}
    >
      {copied ? <Check size={12} /> : <Copy size={12} />}
    </button>
  );
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface ContactDetailPanelProps {
  contact: DiscoveredContact;
  relationships: ContactRelationship[];
  onClose: () => void;
}

export default function ContactDetailPanel({
  contact,
  relationships,
  onClose,
}: ContactDetailPanelProps) {
  // Close on Escape key
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    },
    [onClose],
  );

  useEffect(() => {
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  // Compute related people for this contact
  const relatedPeople = relationships
    .filter(
      (r) => r.person_a === contact.name || r.person_b === contact.name,
    )
    .map((r) => ({
      name: r.person_a === contact.name ? r.person_b : r.person_a,
      channel: r.shared_channel,
      count: r.co_occurrence_count,
    }))
    // Deduplicate by name, keeping highest count
    .reduce<{ name: string; channel: string; count: number }[]>((acc, cur) => {
      const existing = acc.find((a) => a.name === cur.name);
      if (existing) {
        if (cur.count > existing.count) {
          existing.count = cur.count;
          existing.channel = cur.channel;
        }
      } else {
        acc.push(cur);
      }
      return acc;
    }, [])
    .sort((a, b) => b.count - a.count);

  const badge = contact.relationship_type
    ? RELATIONSHIP_BADGE[contact.relationship_type]
    : null;

  // Channel identifiers from channel_ids map
  const channelEntries = contact.channel_ids
    ? Object.entries(contact.channel_ids).filter(
        ([, v]) => v !== undefined && v !== "",
      )
    : [];

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/40 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Panel */}
      <div className="relative z-10 w-full md:w-[400px] h-full bg-ds-bg-100 border-l border-ds-gray-400 overflow-y-auto">
        {/* Close button */}
        <button
          type="button"
          onClick={onClose}
          className="absolute top-4 right-4 flex items-center justify-center w-8 h-8 rounded-lg text-ds-gray-900 hover:text-ds-gray-1000 hover:bg-ds-gray-100/40 transition-colors"
          aria-label="Close panel"
        >
          <X size={16} />
        </button>

        <div className="p-6 space-y-6">
          {/* Header */}
          <div className="space-y-2 pr-8">
            <h2 className="text-lg font-semibold text-ds-gray-1000">
              {contact.name}
            </h2>
            {badge ? (
              <span
                className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${badge.bg} ${badge.text}`}
              >
                {badge.label}
              </span>
            ) : (
              <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-ds-gray-100 text-ds-gray-900">
                Untagged
              </span>
            )}
          </div>

          {/* Channels */}
          <div className="space-y-2">
            <h3 className="text-xs font-medium text-ds-gray-900 uppercase tracking-wider">
              Channels
            </h3>
            <div className="space-y-1.5">
              {contact.channels.map((ch) => {
                const colors = CHANNEL_COLORS[ch] ?? {
                  bg: "bg-ds-gray-100",
                  text: "text-ds-gray-900",
                };
                const identifier = channelEntries.find(
                  ([k]) => k === ch,
                )?.[1];
                return (
                  <div
                    key={ch}
                    className="flex items-center justify-between gap-2"
                  >
                    <div className="flex items-center gap-2 min-w-0">
                      <span
                        className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium capitalize shrink-0 ${colors.bg} ${colors.text}`}
                      >
                        {ch}
                      </span>
                      {identifier && (
                        <span className="text-xs text-ds-gray-1000 truncate">
                          {identifier}
                        </span>
                      )}
                    </div>
                    {identifier && <CopyButton text={identifier} />}
                  </div>
                );
              })}
            </div>
          </div>

          {/* Activity */}
          <div className="space-y-2">
            <h3 className="text-xs font-medium text-ds-gray-900 uppercase tracking-wider">
              Activity
            </h3>
            <div className="grid grid-cols-2 gap-3">
              <div className="bg-ds-gray-100 rounded-lg p-3">
                <p className="text-xs text-ds-gray-900">Messages</p>
                <p className="text-sm font-medium text-ds-gray-1000">
                  {contact.message_count.toLocaleString()}
                </p>
              </div>
              <div className="bg-ds-gray-100 rounded-lg p-3">
                <p className="text-xs text-ds-gray-900">Last Seen</p>
                <p className="text-sm font-medium text-ds-gray-1000">
                  {relativeTime(contact.last_seen)}
                </p>
              </div>
              <div className="bg-ds-gray-100 rounded-lg p-3">
                <p className="text-xs text-ds-gray-900">First Seen</p>
                <p className="text-sm font-medium text-ds-gray-1000">
                  {formatDate(contact.first_seen)}
                </p>
              </div>
              <div className="bg-ds-gray-100 rounded-lg p-3">
                <p className="text-xs text-ds-gray-900">Last Seen</p>
                <p className="text-sm font-medium text-ds-gray-1000">
                  {formatDate(contact.last_seen)}
                </p>
              </div>
            </div>
          </div>

          {/* Notes */}
          <div className="space-y-2">
            <h3 className="text-xs font-medium text-ds-gray-900 uppercase tracking-wider">
              Notes
            </h3>
            <p className="text-sm text-ds-gray-1000">
              {contact.notes ?? "No notes"}
            </p>
          </div>

          {/* Related People */}
          {relatedPeople.length > 0 && (
            <div className="space-y-2">
              <h3 className="text-xs font-medium text-ds-gray-900 uppercase tracking-wider">
                Related People
              </h3>
              <div className="space-y-2">
                {relatedPeople.map((person) => {
                  const colors = CHANNEL_COLORS[person.channel] ?? {
                    bg: "bg-ds-gray-100",
                    text: "text-ds-gray-900",
                  };
                  return (
                    <div
                      key={`${person.name}-${person.channel}`}
                      className="flex items-center justify-between gap-2"
                    >
                      <div className="flex items-center gap-2 min-w-0">
                        <span className="text-sm text-ds-gray-1000">
                          {person.name}
                        </span>
                        <span
                          className={`inline-flex items-center px-1.5 py-0.5 rounded-full text-xs capitalize ${colors.bg} ${colors.text}`}
                        >
                          {person.channel}
                        </span>
                      </div>
                      <span className="text-xs text-ds-gray-900 shrink-0">
                        {person.count} co-occurrences
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
