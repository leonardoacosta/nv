"use client";

interface KVItem {
  key: string;
  value: string;
}

interface KVListData {
  items: KVItem[];
}

interface KVListProps {
  title?: string;
  data: KVListData;
  className?: string;
}

export default function KVList({ title, data, className }: KVListProps) {
  return (
    <div className={`surface-card p-5 space-y-2 ${className ?? ""}`}>
      {title && (
        <h3 className="text-label-12 text-ds-gray-700">{title}</h3>
      )}
      <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5">
        {data.items.map((item, i) => (
          <>
            <dt key={`k-${i}`} className="text-label-13 text-ds-gray-700">
              {item.key}
            </dt>
            <dd key={`v-${i}`} className="text-copy-13 text-ds-gray-1000">
              {item.value}
            </dd>
          </>
        ))}
      </dl>
    </div>
  );
}
