import { AlertCircle, RefreshCw } from "lucide-react";

export interface ErrorBannerProps {
  message: string;
  detail?: string;
  onRetry?: () => void;
}

/**
 * ErrorBanner — inline error display with Geist red styling.
 * Background: red-700 at 8%. Left border: 3px solid ds-red-700.
 * Optionally shows a ghost retry button when `onRetry` is provided.
 */
export default function ErrorBanner({
  message,
  detail,
  onRetry,
}: ErrorBannerProps) {
  return (
    <div
      className="flex items-start gap-3 p-4 rounded-md"
      style={{
        background: "rgba(229, 72, 77, 0.08)",
        borderLeft: "3px solid var(--ds-red-700)",
      }}
    >
      <AlertCircle
        size={16}
        className="text-red-700 shrink-0 mt-0.5"
        aria-hidden="true"
      />

      <div className="flex-1 min-w-0">
        <p className="text-copy-14 font-medium text-red-700">{message}</p>
        {detail && (
          <p className="mt-0.5 text-label-13-mono text-red-900 break-words">
            {detail}
          </p>
        )}
      </div>

      {onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="flex items-center gap-1.5 px-3 py-1.5 min-h-11 min-w-11 rounded-md text-label-13 font-medium text-red-700 hover:bg-red-700/10 transition-colors shrink-0"
          aria-label="Retry"
        >
          <RefreshCw size={12} />
          <span>Retry</span>
        </button>
      )}
    </div>
  );
}
