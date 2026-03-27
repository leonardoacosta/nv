import type { ReactNode } from "react";

// ---------------------------------------------------------------------------
// Inline parsing — bold, italic, inline code
// ---------------------------------------------------------------------------

function parseInline(text: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  // Match: **bold**, *italic*, `code`
  const pattern = /(\*\*(.+?)\*\*)|(\*(.+?)\*)|(`(.+?)`)/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(text)) !== null) {
    // Push text before this match
    if (match.index > lastIndex) {
      nodes.push(text.slice(lastIndex, match.index));
    }

    if (match[2] !== undefined) {
      // **bold**
      nodes.push(
        <strong key={match.index} className="font-semibold text-ds-gray-1000">
          {match[2]}
        </strong>,
      );
    } else if (match[4] !== undefined) {
      // *italic*
      nodes.push(
        <em key={match.index} className="italic">
          {match[4]}
        </em>,
      );
    } else if (match[6] !== undefined) {
      // `inline code`
      nodes.push(
        <code
          key={match.index}
          className="px-1.5 py-0.5 rounded bg-ds-gray-100 text-ds-gray-1000 font-mono text-[0.85em]"
        >
          {match[6]}
        </code>,
      );
    }

    lastIndex = match.index + match[0].length;
  }

  // Push remaining text
  if (lastIndex < text.length) {
    nodes.push(text.slice(lastIndex));
  }

  return nodes;
}

// ---------------------------------------------------------------------------
// Block parsing — code blocks, list items, paragraphs
// ---------------------------------------------------------------------------

interface Block {
  type: "code" | "list" | "paragraph";
  content: string;
  lang?: string;
  items?: string[];
}

function parseBlocks(markdown: string): Block[] {
  const lines = markdown.split("\n");
  const blocks: Block[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i]!;

    // Fenced code block
    if (line.startsWith("```")) {
      const lang = line.slice(3).trim();
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i]!.startsWith("```")) {
        codeLines.push(lines[i]!);
        i++;
      }
      blocks.push({
        type: "code",
        content: codeLines.join("\n"),
        lang: lang || undefined,
      });
      i++; // skip closing ```
      continue;
    }

    // List items (- or * prefix)
    if (/^[-*]\s/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^[-*]\s/.test(lines[i]!)) {
        items.push(lines[i]!.replace(/^[-*]\s+/, ""));
        i++;
      }
      blocks.push({ type: "list", content: "", items });
      continue;
    }

    // Empty line — skip
    if (line.trim() === "") {
      i++;
      continue;
    }

    // Paragraph — collect consecutive non-empty, non-special lines
    const paraLines: string[] = [];
    while (
      i < lines.length &&
      lines[i]!.trim() !== "" &&
      !lines[i]!.startsWith("```") &&
      !/^[-*]\s/.test(lines[i]!)
    ) {
      paraLines.push(lines[i]!);
      i++;
    }
    blocks.push({ type: "paragraph", content: paraLines.join("\n") });
  }

  return blocks;
}

// ---------------------------------------------------------------------------
// MarkdownContent — renders markdown string as styled React elements
// ---------------------------------------------------------------------------

export function MarkdownContent({ content }: { content: string }) {
  const blocks = parseBlocks(content);

  return (
    <div className="space-y-2 text-copy-14 leading-relaxed">
      {blocks.map((block, idx) => {
        if (block.type === "code") {
          return (
            <div key={idx} className="rounded-md overflow-hidden">
              {block.lang && (
                <div className="px-3 py-1.5 bg-ds-gray-100 border-b border-ds-gray-400 text-[10px] font-mono text-ds-gray-900 uppercase tracking-wide">
                  {block.lang}
                </div>
              )}
              <pre className="px-3 py-2.5 bg-ds-gray-100 overflow-x-auto">
                <code className="text-xs font-mono text-ds-gray-1000 leading-relaxed">
                  {block.content}
                </code>
              </pre>
            </div>
          );
        }

        if (block.type === "list") {
          return (
            <ul key={idx} className="space-y-1 pl-4">
              {block.items?.map((item, j) => (
                <li key={j} className="flex items-start gap-2">
                  <span className="mt-2 w-1 h-1 rounded-full bg-ds-gray-700 shrink-0" />
                  <span>{parseInline(item)}</span>
                </li>
              ))}
            </ul>
          );
        }

        // paragraph
        return (
          <p key={idx}>{parseInline(block.content)}</p>
        );
      })}
    </div>
  );
}
