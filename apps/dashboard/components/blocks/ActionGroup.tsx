"use client";

interface ActionItem {
  label: string;
  url?: string;
  status?: "pending" | "completed" | "dismissed";
}

interface ActionGroupData {
  actions: ActionItem[];
}

interface ActionGroupProps {
  title?: string;
  data: ActionGroupData;
  className?: string;
}

function chipClass(status?: "pending" | "completed" | "dismissed"): string {
  if (status === "completed") {
    return "bg-green-700/10 border-green-700/30 text-green-700";
  }
  if (status === "dismissed") {
    return "bg-ds-gray-100 border-ds-gray-400 text-ds-gray-900 line-through";
  }
  // pending (default)
  return "bg-ds-gray-alpha-100 border-ds-gray-1000/30 text-ds-gray-1000";
}

export default function ActionGroup({ title, data, className }: ActionGroupProps) {
  return (
    <div className={`surface-card p-5 space-y-3 ${className ?? ""}`}>
      {title && (
        <h3 className="text-label-12 text-ds-gray-700">{title}</h3>
      )}
      <div className="flex flex-wrap gap-2">
        {data.actions.map((action, i) => {
          const cls = `px-3 py-1.5 rounded-full border text-label-13 transition-colors ${chipClass(action.status)}`;
          if (action.url) {
            return (
              <a
                key={i}
                href={action.url}
                target="_blank"
                rel="noopener noreferrer"
                className={cls}
              >
                {action.label}
              </a>
            );
          }
          return (
            <span key={i} className={cls}>
              {action.label}
            </span>
          );
        })}
      </div>
    </div>
  );
}
