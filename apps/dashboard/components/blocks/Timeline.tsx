"use client";

interface TimelineEvent {
  time: string;
  label: string;
  detail?: string;
  severity?: "info" | "warning" | "error";
}

interface TimelineData {
  events: TimelineEvent[];
}

interface TimelineProps {
  title?: string;
  data: TimelineData;
  className?: string;
}

function severityDotClass(severity?: "info" | "warning" | "error"): string {
  if (severity === "warning") return "bg-ds-amber-700";
  if (severity === "error") return "bg-ds-red-700";
  return "bg-ds-blue-700";
}

export default function Timeline({ title, data, className }: TimelineProps) {
  return (
    <div className={`surface-card p-5 space-y-3 ${className ?? ""}`}>
      {title && (
        <h3 className="text-label-12 text-ds-gray-700">{title}</h3>
      )}
      <div className="relative space-y-0">
        {data.events.map((event, i) => (
          <div key={i} className="flex gap-3">
            {/* Timeline spine */}
            <div className="flex flex-col items-center">
              <div
                className={`w-2 h-2 rounded-full mt-1.5 shrink-0 ${severityDotClass(event.severity)}`}
              />
              {i < data.events.length - 1 && (
                <div className="w-px flex-1 bg-ds-gray-400 my-1" />
              )}
            </div>
            {/* Content */}
            <div className="pb-3 min-w-0">
              <div className="flex items-center gap-2">
                <span className="text-label-12 text-ds-gray-700 shrink-0">
                  {event.time}
                </span>
                <span className="text-copy-13 text-ds-gray-1000 truncate">
                  {event.label}
                </span>
              </div>
              {event.detail && (
                <p className="mt-0.5 text-copy-13 text-ds-gray-700 leading-relaxed">
                  {event.detail}
                </p>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
