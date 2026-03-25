import { AlertCircle, RefreshCw } from "lucide-react";

export interface ErrorBannerProps {
  message: string;
  detail?: string;
  onRetry?: () => void;
}

/**
 * ErrorBanner — inline error display with cosmic-rose styling.
 * Optionally shows a retry button when `onRetry` is provided.
 */
export default function ErrorBanner({
  message,
  detail,
  onRetry,
}: ErrorBannerProps) {
  return (
    <div className="flex items-start gap-3 p-4 rounded-cosmic border border-cosmic-rose/30 bg-cosmic-rose/10">
      <AlertCircle
        size={16}
        className="text-cosmic-rose shrink-0 mt-0.5"
        aria-hidden="true"
      />

      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-cosmic-rose">{message}</p>
        {detail && (
          <p className="mt-0.5 text-xs text-cosmic-rose/70 font-mono break-words">
            {detail}
          </p>
        )}
      </div>

      {onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="flex items-center gap-1.5 px-3 py-1.5 min-h-11 min-w-11 rounded-lg text-xs font-medium text-cosmic-rose border border-cosmic-rose/40 hover:bg-cosmic-rose/20 transition-colors shrink-0"
          aria-label="Retry"
        >
          <RefreshCw size={12} />
          <span>Retry</span>
        </button>
      )}
    </div>
  );
}
