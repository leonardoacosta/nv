"use client";

interface AlertData {
  severity: "info" | "warning" | "error";
  message: string;
}

interface AlertBlockProps {
  title?: string;
  data: AlertData;
  className?: string;
}

function severityBorder(severity: "info" | "warning" | "error"): string {
  if (severity === "warning") return "border-l-ds-amber-700";
  if (severity === "error") return "border-l-ds-red-700";
  return "border-l-ds-blue-700";
}

function severityIconColor(severity: "info" | "warning" | "error"): string {
  if (severity === "warning") return "text-ds-amber-700";
  if (severity === "error") return "text-ds-red-700";
  return "text-ds-blue-700";
}

function AlertIcon({ severity }: { severity: "info" | "warning" | "error" }) {
  if (severity === "info") {
    return (
      <svg width="14" height="14" viewBox="0 0 14 14" fill="none" className="shrink-0 mt-0.5">
        <circle cx="7" cy="7" r="6" stroke="currentColor" strokeWidth="1.25" />
        <path d="M7 6v4M7 4.5v.5" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" />
      </svg>
    );
  }
  if (severity === "warning") {
    return (
      <svg width="14" height="14" viewBox="0 0 14 14" fill="none" className="shrink-0 mt-0.5">
        <path d="M7 1.5L12.5 11H1.5L7 1.5Z" stroke="currentColor" strokeWidth="1.25" strokeLinejoin="round" />
        <path d="M7 5.5v3M7 9.5v.5" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" />
      </svg>
    );
  }
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" className="shrink-0 mt-0.5">
      <circle cx="7" cy="7" r="6" stroke="currentColor" strokeWidth="1.25" />
      <path d="M5 5l4 4M9 5l-4 4" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" />
    </svg>
  );
}

export default function AlertBlock({ title, data, className }: AlertBlockProps) {
  return (
    <div
      className={`surface-card pl-4 pr-5 py-4 border-l-2 ${severityBorder(data.severity)} ${className ?? ""}`}
    >
      <div className={`flex items-start gap-2.5 ${severityIconColor(data.severity)}`}>
        <AlertIcon severity={data.severity} />
        <div className="space-y-0.5 min-w-0">
          {title && (
            <p className="text-label-13 text-ds-gray-1000 font-medium">{title}</p>
          )}
          <p className="text-copy-13 text-ds-gray-900 leading-relaxed">{data.message}</p>
        </div>
      </div>
    </div>
  );
}
