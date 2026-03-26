import { useEffect, useRef, useState } from "react";
import {
  Users,
  Search,
  Plus,
  Edit2,
  Trash2,
  RefreshCw,
  AlertCircle,
  X,
  Check,
} from "lucide-react";

// ── Types ───────────────────────────────────────────────────────────

export interface Contact {
  id: string;
  name: string;
  channel_ids: Record<string, string>;
  relationship_type: RelationshipType;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

type RelationshipType = "work" | "personal-client" | "contributor" | "social";

const RELATIONSHIP_LABELS: Record<RelationshipType, string> = {
  work: "Work",
  "personal-client": "Personal Client",
  contributor: "Contributor",
  social: "Social",
};

const RELATIONSHIP_COLORS: Record<RelationshipType, string> = {
  work: "bg-blue-500/20 text-blue-300 border-blue-500/30",
  "personal-client": "bg-purple-500/20 text-purple-300 border-purple-500/30",
  contributor: "bg-green-500/20 text-green-300 border-green-500/30",
  social: "bg-amber-500/20 text-amber-300 border-amber-500/30",
};

type FilterTab = "all" | RelationshipType;

const FILTER_TABS: { key: FilterTab; label: string }[] = [
  { key: "all", label: "All" },
  { key: "work", label: "Work" },
  { key: "personal-client", label: "Personal Client" },
  { key: "contributor", label: "Contributor" },
  { key: "social", label: "Social" },
];

// ── Empty Form State ─────────────────────────────────────────────────

interface ContactForm {
  name: string;
  relationship_type: RelationshipType;
  telegram: string;
  discord: string;
  teams: string;
  notes: string;
}

const emptyForm = (): ContactForm => ({
  name: "",
  relationship_type: "social",
  telegram: "",
  discord: "",
  teams: "",
  notes: "",
});

function formFromContact(c: Contact): ContactForm {
  return {
    name: c.name,
    relationship_type: c.relationship_type,
    telegram: c.channel_ids.telegram ?? "",
    discord: c.channel_ids.discord ?? "",
    teams: c.channel_ids.teams ?? "",
    notes: c.notes ?? "",
  };
}

// ── API Helpers ──────────────────────────────────────────────────────

const API = "/api/contacts";

async function fetchContacts(
  q?: string,
  relationship?: string
): Promise<Contact[]> {
  const params = new URLSearchParams();
  if (q) params.set("q", q);
  if (relationship && relationship !== "all")
    params.set("relationship", relationship);
  const url = params.toString() ? `${API}?${params.toString()}` : API;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json() as Promise<Contact[]>;
}

async function createContact(form: ContactForm): Promise<Contact> {
  const res = await fetch(API, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      name: form.name,
      relationship_type: form.relationship_type,
      channel_ids: buildChannelIds(form),
      notes: form.notes || null,
    }),
  });
  if (!res.ok) {
    const err = (await res.json()) as { error?: string };
    throw new Error(err.error ?? `HTTP ${res.status}`);
  }
  return res.json() as Promise<Contact>;
}

async function updateContact(id: string, form: ContactForm): Promise<Contact> {
  const res = await fetch(`${API}/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      name: form.name,
      relationship_type: form.relationship_type,
      channel_ids: buildChannelIds(form),
      notes: form.notes || null,
    }),
  });
  if (!res.ok) {
    const err = (await res.json()) as { error?: string };
    throw new Error(err.error ?? `HTTP ${res.status}`);
  }
  return res.json() as Promise<Contact>;
}

async function deleteContact(id: string): Promise<void> {
  const res = await fetch(`${API}/${id}`, { method: "DELETE" });
  if (!res.ok && res.status !== 204)
    throw new Error(`HTTP ${res.status}`);
}

function buildChannelIds(form: ContactForm): Record<string, string> {
  const ids: Record<string, string> = {};
  if (form.telegram.trim()) ids.telegram = form.telegram.trim();
  if (form.discord.trim()) ids.discord = form.discord.trim();
  if (form.teams.trim()) ids.teams = form.teams.trim();
  return ids;
}

// ── Channel Badge ────────────────────────────────────────────────────

function ChannelBadges({ channels }: { channels: Record<string, string> }) {
  const entries = Object.entries(channels);
  if (entries.length === 0)
    return <span className="text-cosmic-muted text-xs">None</span>;
  return (
    <div className="flex flex-wrap gap-1">
      {entries.map(([ch, id]) => (
        <span
          key={ch}
          className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs bg-cosmic-surface border border-cosmic-border text-cosmic-muted"
          title={id}
        >
          <span className="capitalize">{ch}</span>
          <span className="text-cosmic-text truncate max-w-[80px]">{id}</span>
        </span>
      ))}
    </div>
  );
}

// ── Relationship Pill ─────────────────────────────────────────────────

function RelationshipPill({ type }: { type: RelationshipType }) {
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium border ${RELATIONSHIP_COLORS[type]}`}
    >
      {RELATIONSHIP_LABELS[type]}
    </span>
  );
}

// ── Contact Modal ─────────────────────────────────────────────────────

interface ModalProps {
  contact: Contact | null; // null = create mode
  onClose: () => void;
  onSave: () => void;
}

function ContactModal({ contact, onClose, onSave }: ModalProps) {
  const [form, setForm] = useState<ContactForm>(
    contact ? formFromContact(contact) : emptyForm()
  );
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    nameRef.current?.focus();
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.name.trim()) {
      setError("Name is required.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      if (contact) {
        await updateContact(contact.id, form);
      } else {
        await createContact(form);
      }
      onSave();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to save contact."
      );
    } finally {
      setSaving(false);
    }
  };

  const field = (
    key: keyof ContactForm,
    label: string,
    placeholder?: string,
    required?: boolean,
    type?: string
  ) => (
    <div>
      <label className="block text-xs font-medium text-cosmic-muted mb-1">
        {label}
        {required && <span className="text-red-400 ml-0.5">*</span>}
      </label>
      {type === "textarea" ? (
        <textarea
          className="w-full bg-cosmic-surface border border-cosmic-border rounded-lg px-3 py-2 text-sm text-cosmic-text placeholder-cosmic-muted resize-none focus:outline-none focus:border-cosmic-purple/60"
          rows={3}
          value={form[key] as string}
          onChange={(e) =>
            setForm((f) => ({ ...f, [key]: e.target.value }))
          }
          placeholder={placeholder}
        />
      ) : (
        <input
          ref={key === "name" ? nameRef : undefined}
          type="text"
          required={required}
          className="w-full bg-cosmic-surface border border-cosmic-border rounded-lg px-3 py-2 text-sm text-cosmic-text placeholder-cosmic-muted focus:outline-none focus:border-cosmic-purple/60"
          value={form[key] as string}
          onChange={(e) =>
            setForm((f) => ({ ...f, [key]: e.target.value }))
          }
          placeholder={placeholder}
        />
      )}
    </div>
  );

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="relative bg-cosmic-dark border border-cosmic-border rounded-xl shadow-xl w-full max-w-lg mx-4 max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-cosmic-border">
          <h2 className="text-base font-semibold text-cosmic-bright">
            {contact ? "Edit Contact" : "New Contact"}
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="text-cosmic-muted hover:text-cosmic-text transition-colors"
          >
            <X size={18} />
          </button>
        </div>

        {/* Form */}
        <form onSubmit={(e) => void handleSubmit(e)} className="p-5 space-y-4">
          {field("name", "Name", "Leo Acosta", true)}

          {/* Relationship dropdown */}
          <div>
            <label className="block text-xs font-medium text-cosmic-muted mb-1">
              Relationship <span className="text-red-400">*</span>
            </label>
            <select
              className="w-full bg-cosmic-surface border border-cosmic-border rounded-lg px-3 py-2 text-sm text-cosmic-text focus:outline-none focus:border-cosmic-purple/60"
              value={form.relationship_type}
              onChange={(e) =>
                setForm((f) => ({
                  ...f,
                  relationship_type: e.target.value as RelationshipType,
                }))
              }
            >
              {Object.entries(RELATIONSHIP_LABELS).map(([v, l]) => (
                <option key={v} value={v}>
                  {l}
                </option>
              ))}
            </select>
          </div>

          <p className="text-xs text-cosmic-muted font-medium uppercase tracking-wider pt-1">
            Channel Identifiers
          </p>
          {field("telegram", "Telegram", "@handle or first name")}
          {field("discord", "Discord", "User ID (numeric)")}
          {field("teams", "Microsoft Teams", "UPN (user@domain.com)")}

          {field("notes", "Notes", "Timezone, preferred style, ongoing projects…", false, "textarea")}

          {error && (
            <p className="text-sm text-red-400 flex items-center gap-1.5">
              <AlertCircle size={14} />
              {error}
            </p>
          )}

          <div className="flex gap-3 pt-1">
            <button
              type="button"
              onClick={onClose}
              className="flex-1 px-4 py-2 rounded-lg text-sm font-medium border border-cosmic-border text-cosmic-muted hover:text-cosmic-text hover:border-cosmic-purple/40 transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={saving}
              className="flex-1 px-4 py-2 rounded-lg text-sm font-medium bg-cosmic-purple/80 hover:bg-cosmic-purple text-white transition-colors disabled:opacity-50"
            >
              {saving ? "Saving…" : contact ? "Save Changes" : "Create Contact"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ── Delete Confirmation ───────────────────────────────────────────────

interface DeleteConfirmProps {
  contact: Contact;
  onCancel: () => void;
  onConfirm: () => void;
  deleting: boolean;
}

function DeleteConfirm({ contact, onCancel, onConfirm, deleting }: DeleteConfirmProps) {
  return (
    <span className="flex items-center gap-1.5">
      <span className="text-xs text-cosmic-muted">Delete {contact.name}?</span>
      <button
        type="button"
        onClick={onConfirm}
        disabled={deleting}
        className="p-0.5 text-red-400 hover:text-red-300 disabled:opacity-50"
        title="Confirm delete"
      >
        <Check size={14} />
      </button>
      <button
        type="button"
        onClick={onCancel}
        className="p-0.5 text-cosmic-muted hover:text-cosmic-text"
        title="Cancel"
      >
        <X size={14} />
      </button>
    </span>
  );
}

// ── Main Page ─────────────────────────────────────────────────────────

export default function ContactsPage() {
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [filterTab, setFilterTab] = useState<FilterTab>("all");
  const [modal, setModal] = useState<
    { mode: "create" } | { mode: "edit"; contact: Contact } | null
  >(null);
  const [deleteTarget, setDeleteTarget] = useState<Contact | null>(null);
  const [deleting, setDeleting] = useState(false);

  // Debounce search input (300ms)
  useEffect(() => {
    const t = setTimeout(() => setDebouncedSearch(search), 300);
    return () => clearTimeout(t);
  }, [search]);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await fetchContacts(
        debouncedSearch || undefined,
        filterTab !== "all" ? filterTab : undefined
      );
      setContacts(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load contacts");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [debouncedSearch, filterTab]);

  const handleSaved = () => {
    setModal(null);
    void load();
  };

  const handleDelete = async (contact: Contact) => {
    setDeleting(true);
    try {
      await deleteContact(contact.id);
      setDeleteTarget(null);
      void load();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to delete contact"
      );
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="p-8 space-y-6 max-w-5xl">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-cosmic-bright flex items-center gap-2">
            <Users size={22} className="text-cosmic-purple" />
            Contacts
          </h1>
          <p className="mt-1 text-sm text-cosmic-muted">
            Identity consolidation across Telegram, Discord, and Teams
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => void load()}
            disabled={loading}
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors disabled:opacity-50"
          >
            <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
            Refresh
          </button>
          <button
            type="button"
            onClick={() => setModal({ mode: "create" })}
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm font-medium bg-cosmic-purple/80 hover:bg-cosmic-purple text-white transition-colors"
          >
            <Plus size={14} />
            New Contact
          </button>
        </div>
      </div>

      {/* Search */}
      <div className="relative">
        <Search
          size={15}
          className="absolute left-3 top-1/2 -translate-y-1/2 text-cosmic-muted pointer-events-none"
        />
        <input
          type="text"
          placeholder="Search by name or notes…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="w-full pl-9 pr-4 py-2 bg-cosmic-surface border border-cosmic-border rounded-lg text-sm text-cosmic-text placeholder-cosmic-muted focus:outline-none focus:border-cosmic-purple/60"
        />
        {search && (
          <button
            type="button"
            onClick={() => setSearch("")}
            className="absolute right-3 top-1/2 -translate-y-1/2 text-cosmic-muted hover:text-cosmic-text"
          >
            <X size={14} />
          </button>
        )}
      </div>

      {/* Relationship Filter Tabs */}
      <div className="flex gap-1 p-1 rounded-lg bg-cosmic-surface border border-cosmic-border w-fit flex-wrap">
        {FILTER_TABS.map(({ key, label }) => (
          <button
            key={key}
            type="button"
            onClick={() => setFilterTab(key)}
            className={`px-3 py-1.5 rounded text-xs font-medium transition-colors ${
              filterTab === key
                ? "bg-cosmic-purple/20 text-cosmic-bright"
                : "text-cosmic-muted hover:text-cosmic-text"
            }`}
          >
            {label}
          </button>
        ))}
      </div>

      {/* Error */}
      {error && (
        <div className="flex items-center gap-2 p-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-400 text-sm">
          <AlertCircle size={16} />
          {error}
        </div>
      )}

      {/* Loading */}
      {loading && (
        <div className="space-y-3">
          {[1, 2, 3].map((i) => (
            <div
              key={i}
              className="h-16 animate-pulse rounded-lg bg-cosmic-surface"
            />
          ))}
        </div>
      )}

      {/* Empty */}
      {!loading && !error && contacts.length === 0 && (
        <div className="flex flex-col items-center gap-4 py-16">
          <Users size={40} className="text-cosmic-muted" />
          <div className="text-center">
            <p className="text-cosmic-text font-medium">No contacts yet</p>
            <p className="text-sm text-cosmic-muted mt-1">
              Create a contact to start linking channel identities.
            </p>
          </div>
          <button
            type="button"
            onClick={() => setModal({ mode: "create" })}
            className="flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium bg-cosmic-purple/80 hover:bg-cosmic-purple text-white transition-colors"
          >
            <Plus size={14} />
            New Contact
          </button>
        </div>
      )}

      {/* Contact List */}
      {!loading && contacts.length > 0 && (
        <div className="space-y-2">
          {contacts.map((contact) => (
            <div
              key={contact.id}
              className="flex items-start justify-between gap-4 p-4 rounded-lg bg-cosmic-surface border border-cosmic-border hover:border-cosmic-border/80 transition-colors"
            >
              <div className="flex-1 min-w-0 space-y-1.5">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="font-medium text-cosmic-bright">
                    {contact.name}
                  </span>
                  <RelationshipPill type={contact.relationship_type} />
                </div>
                <ChannelBadges channels={contact.channel_ids} />
                {contact.notes && (
                  <p className="text-xs text-cosmic-muted truncate max-w-xl">
                    {contact.notes.slice(0, 120)}
                    {contact.notes.length > 120 && "…"}
                  </p>
                )}
              </div>

              {/* Actions */}
              <div className="flex items-center gap-2 shrink-0">
                {deleteTarget?.id === contact.id ? (
                  <DeleteConfirm
                    contact={contact}
                    onCancel={() => setDeleteTarget(null)}
                    onConfirm={() => void handleDelete(contact)}
                    deleting={deleting}
                  />
                ) : (
                  <>
                    <button
                      type="button"
                      onClick={() =>
                        setModal({ mode: "edit", contact })
                      }
                      className="p-1.5 rounded text-cosmic-muted hover:text-cosmic-text hover:bg-cosmic-dark transition-colors"
                      title="Edit"
                    >
                      <Edit2 size={14} />
                    </button>
                    <button
                      type="button"
                      onClick={() => setDeleteTarget(contact)}
                      className="p-1.5 rounded text-cosmic-muted hover:text-red-400 hover:bg-cosmic-dark transition-colors"
                      title="Delete"
                    >
                      <Trash2 size={14} />
                    </button>
                  </>
                )}
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Create/Edit Modal */}
      {modal && (
        <ContactModal
          contact={modal.mode === "edit" ? modal.contact : null}
          onClose={() => setModal(null)}
          onSave={handleSaved}
        />
      )}
    </div>
  );
}
