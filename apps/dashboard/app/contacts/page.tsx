"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import {
  Users,
  Search,
  AlertCircle,
  RefreshCw,
  Edit2,
  Trash2,
} from "lucide-react";
import type { Contact } from "@/types/api";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type RelationshipFilter = "all" | Contact["relationship_type"];

type ModalState =
  | { mode: "closed" }
  | { mode: "create" }
  | { mode: "edit"; contact: Contact };

interface FormFields {
  name: string;
  relationship_type: Contact["relationship_type"];
  telegram: string;
  discord: string;
  teams: string;
  notes: string;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const RELATIONSHIP_FILTERS: { key: RelationshipFilter; label: string }[] = [
  { key: "all", label: "All" },
  { key: "work", label: "Work" },
  { key: "personal-client", label: "Personal" },
  { key: "contributor", label: "Contributor" },
  { key: "social", label: "Social" },
];

const RELATIONSHIP_BADGE: Record<
  Contact["relationship_type"],
  { bg: string; text: string; label: string }
> = {
  work: { bg: "bg-ds-gray-alpha-200", text: "text-ds-gray-1000", label: "Work" },
  "personal-client": { bg: "bg-red-700/20", text: "text-red-700", label: "Personal" },
  contributor: { bg: "bg-amber-500/20", text: "text-amber-400", label: "Contributor" },
  social: { bg: "bg-emerald-500/20", text: "text-emerald-400", label: "Social" },
};

const EMPTY_FORM: FormFields = {
  name: "",
  relationship_type: "social",
  telegram: "",
  discord: "",
  teams: "",
  notes: "",
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formToPayload(fields: FormFields) {
  const channel_ids: Record<string, string> = {};
  if (fields.telegram.trim()) channel_ids.telegram = fields.telegram.trim();
  if (fields.discord.trim()) channel_ids.discord = fields.discord.trim();
  if (fields.teams.trim()) channel_ids.teams = fields.teams.trim();
  return {
    name: fields.name.trim(),
    relationship_type: fields.relationship_type,
    channel_ids,
    notes: fields.notes.trim() || null,
  };
}

function contactToForm(c: Contact): FormFields {
  return {
    name: c.name,
    relationship_type: c.relationship_type,
    telegram: c.channel_ids.telegram ?? "",
    discord: c.channel_ids.discord ?? "",
    teams: c.channel_ids.teams ?? "",
    notes: c.notes ?? "",
  };
}

// ---------------------------------------------------------------------------
// Relationship badge
// ---------------------------------------------------------------------------

function RelationshipBadge({ type }: { type: Contact["relationship_type"] }) {
  const cfg = RELATIONSHIP_BADGE[type];
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${cfg.bg} ${cfg.text}`}
    >
      {cfg.label}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Channel list
// ---------------------------------------------------------------------------

function ChannelList({ channelIds }: { channelIds: Contact["channel_ids"] }) {
  const entries = Object.entries(channelIds).filter(
    ([, v]) => v !== undefined && v !== "",
  ) as [string, string][];

  if (entries.length === 0) return null;

  const labels: Record<string, string> = {
    telegram: "Telegram",
    discord: "Discord",
    teams: "Teams",
  };

  return (
    <p className="text-xs text-ds-gray-900 mt-1">
      {entries
        .map(([k, v]) => `${labels[k] ?? k}: ${v}`)
        .join(" · ")}
    </p>
  );
}

// ---------------------------------------------------------------------------
// Create / Edit Modal
// ---------------------------------------------------------------------------

interface ContactModalProps {
  state: ModalState;
  onClose: () => void;
  onSaved: () => void;
}

function ContactModal({ state, onClose, onSaved }: ContactModalProps) {
  const [fields, setFields] = useState<FormFields>(EMPTY_FORM);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  // Sync form fields when modal opens or contact changes
  useEffect(() => {
    if (state.mode === "edit") {
      setFields(contactToForm(state.contact));
    } else if (state.mode === "create") {
      setFields(EMPTY_FORM);
    }
    setFormError(null);
  }, [state]);

  // Close on Escape key
  useEffect(() => {
    if (state.mode === "closed") return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [state.mode, onClose]);

  if (state.mode === "closed") return null;

  const isEdit = state.mode === "edit";
  const title = isEdit ? "Edit Contact" : "Create Contact";

  const set = (key: keyof FormFields, value: string) =>
    setFields((prev) => ({ ...prev, [key]: value }));

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!fields.name.trim()) {
      setFormError("Name is required.");
      return;
    }
    setSaving(true);
    setFormError(null);
    try {
      const payload = formToPayload(fields);
      const url =
        isEdit ? `/api/contacts/${state.contact.id}` : "/api/contacts";
      const method = isEdit ? "PATCH" : "POST";
      const res = await fetch(url, {
        method,
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) {
        const body = (await res.json().catch(() => ({}))) as { error?: string };
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      onSaved();
      onClose();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : "Failed to save contact");
    } finally {
      setSaving(false);
    }
  };

  const inputClass =
    "w-full bg-ds-gray-100 border border-ds-gray-400 rounded-lg px-3 py-2 text-sm text-ds-gray-1000 placeholder-ds-gray-700 focus:outline-none focus:border-ds-gray-1000/60 transition-colors";
  const labelClass = "block text-xs font-medium text-ds-gray-900 mb-1";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
      aria-label={title}
    >
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Panel */}
      <div className="relative z-10 max-w-md w-full bg-ds-bg-100 border border-ds-gray-400 rounded-xl shadow-lg">
        <div className="px-6 py-5 border-b border-ds-gray-400">
          <h2 className="text-base font-semibold text-ds-gray-1000">{title}</h2>
        </div>

        <form onSubmit={(e) => void handleSubmit(e)} className="px-6 py-5 space-y-4">
          {/* Name */}
          <div>
            <label className={labelClass} htmlFor="contact-name">Name *</label>
            <input
              id="contact-name"
              type="text"
              value={fields.name}
              onChange={(e) => set("name", e.target.value)}
              className={inputClass}
              placeholder="Full name"
              required
            />
          </div>

          {/* Relationship */}
          <div>
            <label className={labelClass} htmlFor="contact-relationship">Relationship *</label>
            <select
              id="contact-relationship"
              value={fields.relationship_type}
              onChange={(e) => set("relationship_type", e.target.value as Contact["relationship_type"])}
              className={inputClass}
            >
              <option value="work">Work</option>
              <option value="personal-client">Personal</option>
              <option value="contributor">Contributor</option>
              <option value="social">Social</option>
            </select>
          </div>

          {/* Channel identifiers */}
          <div>
            <label className={labelClass} htmlFor="contact-telegram">Telegram</label>
            <input
              id="contact-telegram"
              type="text"
              value={fields.telegram}
              onChange={(e) => set("telegram", e.target.value)}
              className={inputClass}
              placeholder="@handle"
            />
          </div>

          <div>
            <label className={labelClass} htmlFor="contact-discord">Discord</label>
            <input
              id="contact-discord"
              type="text"
              value={fields.discord}
              onChange={(e) => set("discord", e.target.value)}
              className={inputClass}
              placeholder="user ID"
            />
          </div>

          <div>
            <label className={labelClass} htmlFor="contact-teams">Teams UPN</label>
            <input
              id="contact-teams"
              type="text"
              value={fields.teams}
              onChange={(e) => set("teams", e.target.value)}
              className={inputClass}
              placeholder="user@company.com"
            />
          </div>

          {/* Notes */}
          <div>
            <label className={labelClass} htmlFor="contact-notes">Notes</label>
            <textarea
              id="contact-notes"
              rows={3}
              value={fields.notes}
              onChange={(e) => set("notes", e.target.value)}
              className={inputClass}
              placeholder="Optional notes…"
            />
          </div>

          {/* Form error */}
          {formError && (
            <p className="text-xs text-red-700">{formError}</p>
          )}

          {/* Actions */}
          <div className="flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 rounded-lg text-sm text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={saving}
              className="px-4 py-2 rounded-lg text-sm font-medium bg-ds-gray-alpha-200 text-ds-gray-1000 border border-ds-gray-1000/40 hover:bg-ds-gray-700/30 transition-colors disabled:opacity-50"
            >
              {saving ? "Saving…" : isEdit ? "Save" : "Create"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Contact Card
// ---------------------------------------------------------------------------

interface ContactCardProps {
  contact: Contact;
  onEdit: (c: Contact) => void;
  onDeleted: () => void;
}

function ContactCard({ contact, onEdit, onDeleted }: ContactCardProps) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const autoResetRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleDeleteClick = () => {
    setConfirmDelete(true);
    // Auto-reset confirm state after 2 seconds
    autoResetRef.current = setTimeout(() => {
      setConfirmDelete(false);
    }, 2000);
  };

  const handleCancelDelete = () => {
    if (autoResetRef.current) clearTimeout(autoResetRef.current);
    setConfirmDelete(false);
  };

  const handleConfirmDelete = async () => {
    if (autoResetRef.current) clearTimeout(autoResetRef.current);
    setDeleting(true);
    try {
      const res = await fetch(`/api/contacts/${contact.id}`, { method: "DELETE" });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      onDeleted();
    } catch {
      setDeleting(false);
      setConfirmDelete(false);
    }
  };

  // Cleanup timer on unmount
  useEffect(() => {
    return () => {
      if (autoResetRef.current) clearTimeout(autoResetRef.current);
    };
  }, []);

  const notesPreview =
    contact.notes
      ? contact.notes.length > 80
        ? `${contact.notes.slice(0, 80)}…`
        : contact.notes
      : null;

  return (
    <div className="flex items-start justify-between gap-4 p-4 rounded-xl bg-ds-gray-100 border border-ds-gray-400 hover:border-ds-gray-1000/30 transition-colors">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-sm font-medium text-ds-gray-1000">{contact.name}</span>
          <RelationshipBadge type={contact.relationship_type} />
        </div>
        <ChannelList channelIds={contact.channel_ids} />
        {notesPreview && (
          <p className="text-xs text-ds-gray-900 mt-1 italic">{notesPreview}</p>
        )}
      </div>

      {/* Actions */}
      <div className="flex items-center gap-1 shrink-0">
        {confirmDelete ? (
          <>
            <button
              type="button"
              onClick={() => void handleConfirmDelete()}
              disabled={deleting}
              className="px-2 py-1 rounded text-xs font-medium text-red-700 border border-red-700/40 hover:bg-red-700/10 transition-colors disabled:opacity-50"
            >
              {deleting ? "…" : "Confirm?"}
            </button>
            <button
              type="button"
              onClick={handleCancelDelete}
              className="px-2 py-1 rounded text-xs text-ds-gray-900 hover:text-ds-gray-1000 transition-colors"
            >
              Cancel
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              onClick={() => onEdit(contact)}
              aria-label={`Edit ${contact.name}`}
              className="flex items-center justify-center w-8 h-8 rounded-lg text-ds-gray-900 hover:text-ds-gray-1000 hover:bg-ds-bg-100 transition-colors"
            >
              <Edit2 size={14} />
            </button>
            <button
              type="button"
              onClick={handleDeleteClick}
              aria-label={`Delete ${contact.name}`}
              className="flex items-center justify-center w-8 h-8 rounded-lg text-ds-gray-900 hover:text-red-700 hover:bg-red-700/10 transition-colors"
            >
              <Trash2 size={14} />
            </button>
          </>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Contacts Page
// ---------------------------------------------------------------------------

export default function ContactsPage() {
  // 1. Local state
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [relationshipFilter, setRelationshipFilter] = useState<RelationshipFilter>("all");
  const [modalState, setModalState] = useState<ModalState>({ mode: "closed" });

  // Debounce ref
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 2. Fetch function
  const fetchContacts = useCallback(
    async (q?: string, relationship?: RelationshipFilter) => {
      setLoading(true);
      setError(null);
      try {
        const params = new URLSearchParams();
        if (q?.trim()) params.set("q", q.trim());
        if (relationship && relationship !== "all")
          params.set("relationship", relationship);
        const query = params.toString();
        const url = query ? `/api/contacts?${query}` : "/api/contacts";
        const res = await fetch(url);
        if (!res.ok) {
          if (res.status === 503) {
            // Contact store not configured — treat as empty list, not an error
            setContacts([]);
            setError(null);
            return;
          }
          throw new Error(`HTTP ${res.status}`);
        }
        const data = (await res.json()) as unknown;
        setContacts(Array.isArray(data) ? (data as Contact[]) : []);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load contacts");
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  // 3. Initial fetch
  useEffect(() => {
    void fetchContacts();
  }, [fetchContacts]);

  // 4. Handlers
  const handleSearchChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setSearchQuery(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      void fetchContacts(value, relationshipFilter);
    }, 300);
  };

  const handleFilterChange = (filter: RelationshipFilter) => {
    setRelationshipFilter(filter);
    void fetchContacts(searchQuery, filter);
  };

  const handleRefresh = () => {
    void fetchContacts(searchQuery, relationshipFilter);
  };

  const handleEdit = (contact: Contact) => {
    setModalState({ mode: "edit", contact });
  };

  const handleDeleted = () => {
    void fetchContacts(searchQuery, relationshipFilter);
  };

  const handleModalSaved = () => {
    void fetchContacts(searchQuery, relationshipFilter);
  };

  // Cleanup debounce on unmount
  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, []);

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    <>
      <div className="p-8 space-y-6 max-w-3xl">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-semibold text-ds-gray-1000">Contacts</h1>
            <p className="mt-1 text-sm text-ds-gray-900">
              {loading
                ? "Loading…"
                : `${contacts.length} contact${contacts.length === 1 ? "" : "s"}`}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setModalState({ mode: "create" })}
              className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm font-medium bg-ds-gray-alpha-200 text-ds-gray-1000 border border-ds-gray-1000/40 hover:bg-ds-gray-700/30 transition-colors"
            >
              <Users size={14} />
              New contact
            </button>
            <button
              type="button"
              onClick={handleRefresh}
              disabled={loading}
              className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
            >
              <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
              Refresh
            </button>
          </div>
        </div>

        {/* Search bar */}
        <div className="relative">
          <Search
            size={14}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-ds-gray-900 pointer-events-none"
          />
          <input
            type="text"
            value={searchQuery}
            onChange={handleSearchChange}
            placeholder="Search contacts…"
            className="w-full bg-ds-gray-100 border border-ds-gray-400 rounded-lg pl-9 pr-4 py-2 text-sm text-ds-gray-1000 placeholder-ds-gray-700 focus:outline-none focus:border-ds-gray-1000/60 transition-colors"
          />
        </div>

        {/* Relationship filter chips */}
        <div className="flex items-center gap-2 flex-wrap">
          {RELATIONSHIP_FILTERS.map(({ key, label }) => (
            <button
              key={key}
              type="button"
              onClick={() => handleFilterChange(key)}
              className={[
                "px-3 py-1 rounded-full text-xs font-medium transition-colors border",
                relationshipFilter === key
                  ? "bg-ds-gray-alpha-200 text-ds-gray-1000 border-ds-gray-1000/40"
                  : "text-ds-gray-900 border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-1000/30",
              ].join(" ")}
            >
              {label}
            </button>
          ))}
        </div>

        {/* Error state */}
        {error && (
          <div className="flex items-center gap-3 p-4 rounded-xl bg-red-700/10 border border-red-700/30 text-red-700">
            <AlertCircle size={16} />
            <span className="text-sm">{error}</span>
          </div>
        )}

        {/* Loading state */}
        {loading ? (
          <div className="space-y-2">
            {Array.from({ length: 5 }).map((_, i) => (
              <div
                key={i}
                className="h-16 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
              />
            ))}
          </div>
        ) : contacts.length === 0 ? (
          /* Empty state */
          <div className="flex flex-col items-center gap-4 py-16 text-ds-gray-900">
            <Users size={40} />
            <div className="text-center">
              <p className="text-sm font-medium text-ds-gray-1000">No contacts yet</p>
              <p className="text-xs mt-1">
                {searchQuery || relationshipFilter !== "all"
                  ? "Try a different search or filter."
                  : "Get started by creating your first contact."}
              </p>
            </div>
            {!searchQuery && relationshipFilter === "all" && (
              <button
                type="button"
                onClick={() => setModalState({ mode: "create" })}
                className="px-4 py-2 rounded-lg text-sm font-medium bg-ds-gray-alpha-200 text-ds-gray-1000 border border-ds-gray-1000/40 hover:bg-ds-gray-700/30 transition-colors"
              >
                Create contact
              </button>
            )}
          </div>
        ) : (
          /* Contact list */
          <div key={relationshipFilter} className="animate-crossfade-in space-y-2">
            {contacts.map((contact) => (
              <ContactCard
                key={contact.id}
                contact={contact}
                onEdit={handleEdit}
                onDeleted={handleDeleted}
              />
            ))}
          </div>
        )}
      </div>

      {/* Create / Edit Modal */}
      <ContactModal
        state={modalState}
        onClose={() => setModalState({ mode: "closed" })}
        onSaved={handleModalSaved}
      />
    </>
  );
}
