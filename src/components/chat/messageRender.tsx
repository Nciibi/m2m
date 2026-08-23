import type { ReactNode } from "react";
import type { ChatMessage } from "../../types";

/** Simple markdown renderer: bold, italic, inline code, links */
export function renderMarkdown(content: string): ReactNode {
  // Inline code first (so markdown inside backticks isn't parsed)
  const parts = content.split(/(`[^`]+`)/g);
  return parts.map((p, i) => {
    if (p.startsWith("`") && p.endsWith("`")) {
      return <code key={i} className="msg-code-inline">{p.slice(1, -1)}</code>;
    }
    // Bold **text** or __text__
    let rendered: ReactNode = p;
    const boldParts = p.split(/(\*\*[^*]+\*\*|__[^_]+__)/g);
    if (boldParts.length > 1) {
      rendered = boldParts.map((bp, j) => {
        if ((bp.startsWith("**") && bp.endsWith("**")) || (bp.startsWith("__") && bp.endsWith("__"))) {
          return <strong key={j}>{bp.slice(2, -2)}</strong>;
        }
        // Italic *text* or _text_
        const italicParts = bp.split(/(\*[^*]+\*|_[^_]+_)/g);
        if (italicParts.length > 1) {
          return italicParts.map((ip, k) => {
            if ((ip.startsWith("*") && ip.endsWith("*")) || (ip.startsWith("_") && ip.endsWith("_"))) {
              return <em key={k}>{ip.slice(1, -1)}</em>;
            }
            // Link detection (simple URL pattern)
            return renderLinks(ip, `${j}-${k}`);
          });
        }
        return renderLinks(bp, `${j}`);
      });
    } else {
      rendered = renderLinks(p, `${i}`);
    }
    return <span key={i}>{rendered}</span>;
  });
}

/** Detect URLs and render as clickable links */
export function renderLinks(text: string, key: string): ReactNode {
  const urlRegex = /(https?:\/\/[^\s<]+)/g;
  const parts = text.split(urlRegex);
  if (parts.length === 1) return text;
  return parts.map((part, i) => {
    if (urlRegex.test(part)) {
      return <a key={`${key}-${i}`} href={part} target="_blank" rel="noopener noreferrer" className="msg-link">{part}</a>;
    }
    return part;
  });
}

export function groupByDate(msgs: ChatMessage[]): Record<string, ChatMessage[]> {
  const g: Record<string, ChatMessage[]> = {};
  for (const m of msgs) {
    const d = new Date(m.timestamp * 1000), t = new Date(), y = new Date(t); y.setDate(y.getDate() - 1);
    const l = d.toDateString() === t.toDateString() ? "Today" : d.toDateString() === y.toDateString() ? "Yesterday" : d.toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" });
    if (!g[l]) g[l] = []; g[l].push(m);
  }
  return g;
}
