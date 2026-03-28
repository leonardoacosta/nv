"use client";

import ReactMarkdown from "react-markdown";

interface SectionBlockData {
  body: string;
}

interface SectionBlockProps {
  title?: string;
  data: SectionBlockData;
  className?: string;
}

export default function SectionBlock({ title, data, className }: SectionBlockProps) {
  return (
    <div className={`surface-card p-5 space-y-2 ${className ?? ""}`}>
      {title && (
        <h3 className="text-label-12 text-ds-gray-700">{title}</h3>
      )}
      <div className="prose prose-sm prose-invert max-w-none text-copy-14 text-ds-gray-1000 leading-relaxed">
        <ReactMarkdown>{data.body}</ReactMarkdown>
      </div>
    </div>
  );
}
