"use client";

interface StatusTableData {
  columns: string[];
  rows: Record<string, string>[];
}

interface StatusTableProps {
  title?: string;
  data: StatusTableData;
  className?: string;
}

export default function StatusTable({ title, data, className }: StatusTableProps) {
  return (
    <div className={`surface-card overflow-hidden ${className ?? ""}`}>
      {title && (
        <div className="px-5 pt-4 pb-2">
          <h3 className="text-label-12 text-ds-gray-700">{title}</h3>
        </div>
      )}
      <div className="overflow-x-auto">
        <table className="w-full">
          <thead>
            <tr className="border-b border-ds-gray-400">
              {data.columns.map((col) => (
                <th
                  key={col}
                  className="px-5 py-2.5 text-left text-label-12 text-ds-gray-700 font-medium"
                >
                  {col}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.rows.map((row, i) => (
              <tr
                key={i}
                className="border-b border-ds-gray-400 last:border-0 hover:bg-ds-gray-100 transition-colors"
              >
                {data.columns.map((col) => (
                  <td
                    key={col}
                    className="px-5 py-2.5 text-copy-13 text-ds-gray-1000"
                  >
                    {row[col] ?? "—"}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
