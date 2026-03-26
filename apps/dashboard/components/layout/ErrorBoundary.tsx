import { Component, type ErrorInfo, type ReactNode } from "react";
import { AlertTriangle, ChevronDown, ChevronUp, RefreshCw } from "lucide-react";

export interface ErrorBoundaryProps {
  children: ReactNode;
  /** Optional custom fallback UI. When provided, replaces the default error card entirely. */
  fallback?: ReactNode;
  /** Called after the boundary resets its error state. */
  onReset?: () => void;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  showDetails: boolean;
}

/**
 * ErrorBoundary -- catches render errors in children and displays a
 * recoverable fallback card. Uses Geist ds-red / ds-gray tokens to match
 * the dashboard design language (see ErrorBanner, EmptyState).
 *
 * Must be a class component because React requires getDerivedStateFromError.
 */
export default class ErrorBoundary extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null, showDetails: false };
  }

  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    // Log for observability; replace with Sentry/reporting when available.
    console.error("[ErrorBoundary]", error, info.componentStack);
  }

  private handleReset = (): void => {
    this.setState({ hasError: false, error: null, showDetails: false });
    this.props.onReset?.();
  };

  private toggleDetails = (): void => {
    this.setState((prev) => ({ showDetails: !prev.showDetails }));
  };

  render(): ReactNode {
    if (!this.state.hasError) {
      return this.props.children;
    }

    // Custom fallback takes priority.
    if (this.props.fallback) {
      return this.props.fallback;
    }

    const { error, showDetails } = this.state;

    return (
      <div className="flex flex-col items-center justify-center gap-4 py-16 px-6 text-center animate-fade-in-up">
        {/* Icon */}
        <div
          className="flex items-center justify-center w-10 h-10 rounded-full"
          style={{ background: "rgba(229, 72, 77, 0.08)" }}
          aria-hidden="true"
        >
          <AlertTriangle size={20} className="text-red-700" />
        </div>

        {/* Message */}
        <div className="space-y-1">
          <h3 className="text-heading-16 text-ds-gray-1000">
            Something went wrong
          </h3>
          <p className="text-copy-14 text-ds-gray-900 max-w-xs">
            An unexpected error prevented this section from rendering.
          </p>
        </div>

        {/* Actions */}
        <div className="flex items-center gap-3 mt-2">
          <button
            type="button"
            onClick={this.handleReset}
            className="flex items-center gap-1.5 px-3 py-1.5 min-h-11 min-w-11 rounded-md text-label-13 font-medium text-red-700 hover:bg-red-700/10 transition-colors"
          >
            <RefreshCw size={12} />
            <span>Try again</span>
          </button>

          <button
            type="button"
            onClick={this.toggleDetails}
            className="flex items-center gap-1.5 px-3 py-1.5 min-h-11 min-w-11 rounded-md text-label-13 font-medium text-ds-gray-900 hover:bg-ds-gray-alpha-200 transition-colors"
          >
            <span>Show details</span>
            {showDetails ? (
              <ChevronUp size={12} />
            ) : (
              <ChevronDown size={12} />
            )}
          </button>
        </div>

        {/* Collapsible error details */}
        {showDetails && error && (
          <div
            className="mt-2 w-full max-w-lg rounded-md p-4 text-left overflow-auto"
            style={{
              background: "rgba(229, 72, 77, 0.08)",
              borderLeft: "3px solid var(--ds-red-700)",
            }}
          >
            <p className="text-copy-14 font-medium text-red-700">
              {error.message}
            </p>
            {error.stack && (
              <pre className="mt-2 text-label-13-mono text-ds-gray-900 whitespace-pre-wrap break-words">
                {error.stack}
              </pre>
            )}
          </div>
        )}
      </div>
    );
  }
}
