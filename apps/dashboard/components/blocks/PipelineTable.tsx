"use client";

interface PipelineItem {
  name: string;
  status: "success" | "failed" | "running" | "pending";
  duration?: string;
}

interface PipelineTableData {
  pipelines: PipelineItem[];
}

interface PipelineTableProps {
  title?: string;
  data: PipelineTableData;
  className?: string;
}

function statusBadgeClass(status: "success" | "failed" | "running" | "pending"): string {
  if (status === "success") return "bg-green-700/10 text-green-700 border-green-700/30";
  if (status === "failed") return "bg-ds-red-700/10 text-ds-red-700 border-ds-red-700/30";
  if (status === "running") return "bg-ds-amber-700/10 text-ds-amber-700 border-ds-amber-700/30";
  return "bg-ds-gray-100 text-ds-gray-700 border-ds-gray-400";
}

function statusLabel(status: "success" | "failed" | "running" | "pending"): string {
  if (status === "success") return "Success";
  if (status === "failed") return "Failed";
  if (status === "running") return "Running";
  return "Pending";
}

export default function PipelineTable({ title, data, className }: PipelineTableProps) {
  return (
    <div className={`surface-card overflow-hidden ${className ?? ""}`}>
      {title && (
        <div className="px-5 pt-4 pb-2">
          <h3 className="text-label-12 text-ds-gray-700">{title}</h3>
        </div>
      )}
      <table className="w-full">
        <thead>
          <tr className="border-b border-ds-gray-400">
            <th className="px-5 py-2.5 text-left text-label-12 text-ds-gray-700 font-medium">Pipeline</th>
            <th className="px-5 py-2.5 text-left text-label-12 text-ds-gray-700 font-medium">Status</th>
            <th className="px-5 py-2.5 text-left text-label-12 text-ds-gray-700 font-medium">Duration</th>
          </tr>
        </thead>
        <tbody>
          {data.pipelines.map((pipeline, i) => (
            <tr
              key={i}
              className="border-b border-ds-gray-400 last:border-0 hover:bg-ds-gray-100 transition-colors"
            >
              <td className="px-5 py-2.5 text-copy-13 text-ds-gray-1000">{pipeline.name}</td>
              <td className="px-5 py-2.5">
                <span
                  className={`inline-flex items-center px-2 py-0.5 rounded-full border text-label-12 ${statusBadgeClass(pipeline.status)}`}
                >
                  {statusLabel(pipeline.status)}
                </span>
              </td>
              <td className="px-5 py-2.5 text-copy-13 text-ds-gray-700">
                {pipeline.duration ?? "—"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
