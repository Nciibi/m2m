import type { ReactNode } from "react";
import type { ChatMessage } from "../../types";
import { makeT, type Translator } from "../../i18n/catalog";

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

/**
 * Partition messages into display groups keyed by a human-readable date label.
 *
 * Note this does NOT sort — the backend already returns messages in timestamp
 * order, and re-sorting client-side would reorder optimistically-appended
 * local messages against the ones the server sent, making a just-sent message
 * appear to jump.
 *
 * `translate` is injected rather than imported from the i18n context so this
 * stays a pure function (and therefore testable without a provider).
 */
export function groupByDate(
  msgs: ChatMessage[],
  translate: Translator = makeT("en"),
): Record<string, ChatMessage[]> {
  const g: Record<string, ChatMessage[]> = {};
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);

  for (const m of msgs) {
    const d = new Date(m.timestamp * 1000);
    const label =
      d.toDateString() === today.toDateString()
        ? translate("time.today")
        : d.toDateString() === yesterday.toDateString()
          ? translate("time.yesterday")
          : d.toLocaleDateString(undefined, {
              weekday: "long",
              month: "long",
              day: "numeric",
            });
    if (!g[label]) g[label] = [];
    g[label].push(m);
  }
  return g;
}
