"use client";

import type { DiscoveredContact } from "@/types/api";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface ContactCardProps {
  contact: DiscoveredContact;
  relatedPeople: string[];
  onClick: () => void;
}

export default function ContactCard({
  contact,
  relatedPeople,
  onClick,
}: ContactCardProps) {
  const notesPreview =
    contact.notes
      ? contact.notes.length > 80
        ? `${contact.notes.slice(0, 80)}...`
        : contact.notes
      : null;

  const badge = contact.relationship_type
    ? RELATIONSHIP_BADGE[contact.relationship_type]
    : null;

  return (
    <button
      type="button"
      onClick={onClick}
      className="w-full text-left bg-ds-bg-100 border border-ds-gray-400 rounded-xl p-4 hover:border-ds-gray-1000/40 transition-colors space-y-2"
    >
      {/* Name + relationship badge */}
      <div className="flex items-center gap-2 flex-wrap">
        <span className="text-sm font-medium text-ds-gray-1000">
          {contact.name}
        </span>
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

      {/* Channel badges */}
      <div className="flex items-center gap-1.5 flex-wrap">
        {contact.channels.map((ch) => {
          const colors = CHANNEL_COLORS[ch] ?? {
            bg: "bg-ds-gray-100",
            text: "text-ds-gray-900",
          };
          return (
            <span
              key={ch}
              className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium capitalize ${colors.bg} ${colors.text}`}
            >
              {ch}
            </span>
          );
        })}
      </div>

      {/* Message count + last seen */}
      <p className="text-xs text-ds-gray-900">
        {contact.message_count.toLocaleString()} messages
        {" · "}
        last seen {relativeTime(contact.last_seen)}
      </p>

      {/* Notes preview */}
      {notesPreview && (
        <p className="text-xs text-ds-gray-900 italic">{notesPreview}</p>
      )}

      {/* Related people */}
      {relatedPeople.length > 0 && (
        <p className="text-xs text-ds-gray-900">
          Also talks with:{" "}
          {relatedPeople.slice(0, 3).join(", ")}
          {relatedPeople.length > 3 && ` +${relatedPeople.length - 3} more`}
        </p>
      )}
    </button>
  );
}
