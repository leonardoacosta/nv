"use client";

import { Badge } from "@nova/ui";
import type { DiscoveredContact } from "@/types/api";
import { getPlatformColor } from "@/lib/brand-colors";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const RELATIONSHIP_BADGE: Record<
  string,
  { variant: "default" | "destructive" | "warning" | "success" | "outline"; label: string }
> = {
  work: { variant: "default", label: "Work" },
  "personal-client": { variant: "destructive", label: "Personal" },
  contributor: { variant: "warning", label: "Contributor" },
  social: { variant: "success", label: "Social" },
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
      className="w-full text-left bg-ds-bg-100 border border-ds-gray-400 rounded-xl p-4 hover:border-ds-gray-1000/40 transition-colors flex flex-col gap-2"
    >
      {/* Name + relationship badge */}
      <div className="flex items-center gap-2 flex-wrap">
        <span className="text-copy-14 font-medium text-ds-gray-1000">
          {contact.name}
        </span>
        {badge ? (
          <Badge variant={badge.variant}>{badge.label}</Badge>
        ) : (
          <Badge variant="outline">Untagged</Badge>
        )}
      </div>

      {/* Channel badges */}
      <div className="flex items-center gap-1.5 flex-wrap">
        {contact.channels.map((ch) => {
          const brand = getPlatformColor(ch);
          return (
            <span
              key={ch}
              className={`inline-flex items-center px-2 py-0.5 rounded-full text-label-12 font-medium capitalize ${brand.bg} ${brand.text}`}
            >
              {ch}
            </span>
          );
        })}
      </div>

      {/* Message count + last seen */}
      <p className="text-copy-13 text-ds-gray-900">
        {contact.message_count.toLocaleString()} messages
        {" \u00B7 "}
        last seen {relativeTime(contact.last_seen)}
      </p>

      {/* Notes preview */}
      {notesPreview && (
        <p className="text-copy-13 text-ds-gray-900 italic">{notesPreview}</p>
      )}

      {/* Related people */}
      {relatedPeople.length > 0 && (
        <p className="text-copy-13 text-ds-gray-900">
          Also talks with:{" "}
          {relatedPeople.slice(0, 3).join(", ")}
          {relatedPeople.length > 3 && ` +${relatedPeople.length - 3} more`}
        </p>
      )}
    </button>
  );
}
