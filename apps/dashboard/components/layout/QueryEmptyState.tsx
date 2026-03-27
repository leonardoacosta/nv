"use client";

import { Inbox } from "lucide-react";

/**
 * QueryEmptyState — placeholder for queries that returned no data.
 * Shows an icon, title, description, and an optional CTA button.
 * Uses ds-token classes for consistency with the design system.
 */

interface QueryEmptyStateProps {
  /** Heading text. Default: "No items yet" */
  title?: string;
  /** Supporting description. Default: "Data will appear here when available." */
  description?: string;
  /** Optional callback to create a new item */
  onCreate?: () => void;
  /** Label for the create button. Default: "Create" */
  createLabel?: string;
}

export default function QueryEmptyState({
  title = "No items yet",
  description = "Data will appear here when available.",
  onCreate,
  createLabel = "Create",
}: QueryEmptyStateProps) {
  return (
    <div className="flex flex-col items-center gap-4 py-12">
      <Inbox size={32} className="text-ds-gray-600" aria-hidden="true" />
      <div className="text-center">
        <h3 className="text-heading-16 text-ds-gray-1000">{title}</h3>
        <p className="text-copy-13 text-ds-gray-900 mt-1">{description}</p>
      </div>
      {onCreate && (
        <button
          type="button"
          onClick={onCreate}
          className="px-4 py-2 rounded-lg text-button-14 font-medium bg-ds-gray-1000 text-ds-bg-100 hover:bg-ds-gray-900 transition-colors"
        >
          {createLabel}
        </button>
      )}
    </div>
  );
}
