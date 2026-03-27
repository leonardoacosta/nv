"use client";

import { AlertCircle, RefreshCw } from "lucide-react";

/**
 * QueryErrorState — error display for failed queries.
 * Shows error icon, message text, and an optional retry button.
 * Uses ds-token classes for consistency with the design system.
 */

interface QueryErrorStateProps {
  /** Error message to display */
  message: string;
  /** Callback to retry the failed query */
  onRetry?: () => void;
}

export default function QueryErrorState({
  message,
  onRetry,
}: QueryErrorStateProps) {
  return (
    <div className="flex flex-col items-center gap-4 py-12">
      <AlertCircle size={32} className="text-red-700" aria-hidden="true" />
      <p className="text-copy-14 text-ds-gray-900 text-center max-w-md">
        {message}
      </p>
      {onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors"
        >
          <RefreshCw size={12} />
          Try Again
        </button>
      )}
    </div>
  );
}
