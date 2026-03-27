"use client";

import { useEffect, useRef, useState } from "react";
import { Users, Search, AlertCircle, RefreshCw } from "lucide-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type {
  DiscoveredContact,
  ContactRelationship,
} from "@/types/api";
import { trpc } from "@/lib/trpc/react";
import ContactCard from "@/components/ContactCard";
import ContactDetailPanel from "@/components/ContactDetailPanel";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type FilterTab =
  | "all"
  | "work"
  | "personal-client"
  | "contributor"
  | "social"
  | "untagged";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const FILTER_TABS: { key: FilterTab; label: string }[] = [
  { key: "all", label: "All" },
  { key: "work", label: "Work" },
  { key: "personal-client", label: "Personal" },
  { key: "contributor", label: "Contributor" },
  { key: "social", label: "Social" },
  { key: "untagged", label: "Untagged" },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Compute related people names for a given contact from relationships data. */
function getRelatedPeople(
  contactName: string,
  relationships: ContactRelationship[],
): string[] {
  const names = new Set<string>();
  for (const r of relationships) {
    if (r.person_a === contactName) names.add(r.person_b);
    else if (r.person_b === contactName) names.add(r.person_a);
  }
  return Array.from(names);
}

// ---------------------------------------------------------------------------
// Contacts Page
// ---------------------------------------------------------------------------

export default function ContactsPage() {
  const queryClient = useQueryClient();

  // State
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [filterTab, setFilterTab] = useState<FilterTab>("all");
  const [selectedContact, setSelectedContact] =
    useState<DiscoveredContact | null>(null);

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Queries
  const discoveredQuery = useQuery(trpc.contact.discovered.queryOptions());
  const relQuery = useQuery(trpc.contact.relationships.queryOptions({}));

  const contacts = discoveredQuery.data?.contacts ?? [];
  const totalSenders = discoveredQuery.data?.total_senders ?? 0;
  const totalMessages = discoveredQuery.data?.total_messages_scanned ?? 0;
  const relationships = relQuery.data?.relationships ?? [];
  const loading = discoveredQuery.isLoading;
  const error = discoveredQuery.error;

  const fetchData = () => {
    void queryClient.invalidateQueries({ queryKey: trpc.contact.discovered.queryKey() });
    void queryClient.invalidateQueries({ queryKey: trpc.contact.relationships.queryKey() });
  };

  // Debounced search
  const handleSearchChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setSearch(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      setDebouncedSearch(value);
    }, 300);
  };

  // Cleanup debounce
  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, []);

  // Filter contacts
  const filteredContacts = contacts.filter((c) => {
    // Search filter
    if (
      debouncedSearch &&
      !c.name.toLowerCase().includes(debouncedSearch.toLowerCase())
    ) {
      return false;
    }
    // Tab filter
    if (filterTab === "all") return true;
    if (filterTab === "untagged") return c.relationship_type === null;
    return c.relationship_type === filterTab;
  });

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    <>
      <div className="p-4 space-y-3 w-full">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-2">
              <Users size={20} className="text-ds-gray-900" />
              <h1 className="text-heading-24 text-ds-gray-1000">
                Contacts
              </h1>
            </div>
            <p className="mt-1 text-copy-13 text-ds-gray-900">
              {loading
                ? "Loading..."
                : `Discovered ${totalSenders} contacts from ${totalMessages.toLocaleString()} conversations`}
            </p>
          </div>
          <button
            type="button"
            onClick={fetchData}
            disabled={loading}
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
          >
            <RefreshCw
              size={14}
              className={discoveredQuery.isFetching ? "animate-spin" : ""}
            />
            Refresh
          </button>
        </div>

        {/* Search bar */}
        <div className="relative">
          <Search
            size={14}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-ds-gray-900 pointer-events-none"
          />
          <input
            type="text"
            value={search}
            onChange={handleSearchChange}
            placeholder="Search contacts..."
            className="w-full bg-ds-gray-100 border border-ds-gray-400 rounded-lg pl-9 pr-4 py-2 text-copy-13 text-ds-gray-1000 placeholder-ds-gray-700 focus:outline-hidden focus:border-ds-gray-1000/60 transition-colors"
          />
        </div>

        {/* Filter tabs */}
        <div className="flex items-center gap-2 flex-wrap">
          {FILTER_TABS.map(({ key, label }) => (
            <button
              key={key}
              type="button"
              onClick={() => setFilterTab(key)}
              className={[
                "px-3 py-1 rounded-full text-label-13 transition-colors border",
                filterTab === key
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
            <span className="text-copy-13">{error.message}</span>
            <button
              type="button"
              onClick={fetchData}
              className="ml-auto text-copy-13 underline hover:no-underline"
            >
              Retry
            </button>
          </div>
        )}

        {/* Loading skeleton */}
        {loading ? (
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
            {Array.from({ length: 3 }).map((_, i) => (
              <div
                key={i}
                className="h-36 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
              />
            ))}
          </div>
        ) : filteredContacts.length === 0 ? (
          /* Empty state */
          <div className="flex flex-col items-center gap-4 py-12">
            <Users size={48} className="text-ds-gray-900" />
            <div className="text-center">
              <h3 className="font-semibold text-ds-gray-1000">
                {search || filterTab !== "all"
                  ? "No contacts found"
                  : "No conversations yet"}
              </h3>
              <p className="text-copy-13 text-ds-gray-900 max-w-sm mt-1">
                {search || filterTab !== "all"
                  ? "Try a different search or filter."
                  : "Nova hasn't received any messages yet. Contacts will appear automatically as people message Nova across channels."}
              </p>
            </div>
          </div>
        ) : (
          /* Contact grid */
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4 animate-crossfade-in">
            {filteredContacts.map((contact) => (
              <ContactCard
                key={contact.name}
                contact={contact}
                relatedPeople={getRelatedPeople(
                  contact.name,
                  relationships,
                )}
                onClick={() => setSelectedContact(contact)}
              />
            ))}
          </div>
        )}
      </div>

      {/* Detail panel */}
      {selectedContact && (
        <ContactDetailPanel
          contact={selectedContact}
          relationships={relationships}
          onClose={() => setSelectedContact(null)}
        />
      )}
    </>
  );
}
