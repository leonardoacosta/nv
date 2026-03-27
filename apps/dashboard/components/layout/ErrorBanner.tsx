import { AlertCircle, RefreshCw } from "lucide-react";
import { Alert, AlertTitle, AlertDescription } from "@nova/ui";
import { Button } from "@nova/ui";

export interface ErrorBannerProps {
  message: string;
  detail?: string;
  onRetry?: () => void;
}

/**
 * ErrorBanner — inline error display composed with shadcn Alert.
 * Uses destructive variant for red styling. Optionally shows a ghost
 * retry button when `onRetry` is provided.
 */
export default function ErrorBanner({
  message,
  detail,
  onRetry,
}: ErrorBannerProps) {
  return (
    <Alert variant="destructive" className="flex items-start gap-3">
      <AlertCircle
        size={16}
        className="shrink-0 mt-0.5"
        aria-hidden="true"
      />

      <div className="flex-1 min-w-0">
        <AlertTitle>{message}</AlertTitle>
        {detail && (
          <AlertDescription className="mt-0.5 text-label-13-mono break-words">
            {detail}
          </AlertDescription>
        )}
      </div>

      {onRetry && (
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={onRetry}
          className="shrink-0 text-red-700 hover:bg-red-700/10"
        >
          <RefreshCw size={12} />
          Retry
        </Button>
      )}
    </Alert>
  );
}
