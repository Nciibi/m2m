import { describe, it, expect } from "vitest";
import { render } from "./setup";
import { renderMarkdown, renderLinks, groupByDate } from "../components/chat/messageRender";

/**
 * The markdown / link renderer.
 *
 * This is the ONLY place peer-controlled text becomes DOM in the entire
 * frontend, and it had **zero tests** — the single largest untested attack
 * surface in the app. Everything here is driven by message content that
 * arrived over the network from another person.
 *
 * The good news the tests lock in: there is no `dangerouslySetInnerHTML`
 * anywhere, the renderer builds React elements (so content is always escaped),
 * linkification is `https?`-only (so `javascript:` and `data:` can never
 * become an `href`), and every link gets `rel="noopener noreferrer"`.
 */

describe("renderMarkdown", () => {
  it("escapes HTML rather than interpreting it", () => {
    const { container } = render(<div>{renderMarkdown("<script>alert(1)</script>")}</div>);
    // The tag must survive as text, not become an element.
    expect(container.querySelector("script")).toBeNull();
    expect(container.textContent).toContain("<script>");
  });

  it("does not let img onerror execute", () => {
    const { container } = render(
      <div>{renderMarkdown('<img src=x onerror="alert(1)">')}</div>,
    );
    expect(container.querySelector("img")).toBeNull();
  });

  it("renders bold and italic as elements", () => {
    const { container } = render(<div>{renderMarkdown("**bold** and *italic*")}</div>);
    expect(container.querySelector("strong")?.textContent).toBe("bold");
    expect(container.querySelector("em")?.textContent).toBe("italic");
  });

  it("renders inline code without evaluating it", () => {
    const { container } = render(<div>{renderMarkdown("`a < b`")}</div>);
    const code = container.querySelector("code");
    expect(code?.textContent).toBe("a < b");
    expect(container.querySelector("b")).toBeNull();
  });

  it("handles empty and whitespace input without throwing", () => {
    for (const input of ["", "   ", "\n\n"]) {
      expect(() => render(<div>{renderMarkdown(input)}</div>)).not.toThrow();
    }
  });
});

describe("renderLinks", () => {
  it("linkifies an https URL and marks it safe", () => {
    const { container } = render(<div>{renderLinks("see https://example.com now", "k")}</div>);
    const a = container.querySelector("a");
    expect(a?.getAttribute("href")).toBe("https://example.com");
    expect(a?.getAttribute("rel")).toBe("noopener noreferrer");
    expect(a?.getAttribute("target")).toBe("_blank");
  });

  it("refuses javascript: URLs", () => {
    // Linkification is https-only, so this must stay plain text. If the regex
    // is ever widened carelessly, this is the test that catches it.
    const { container } = render(<div>{renderLinks("javascript:alert(1)", "k")}</div>);
    expect(container.querySelector("a")).toBeNull();
    expect(container.textContent).toContain("javascript:alert(1)");
  });

  it("refuses data: URLs", () => {
    const { container } = render(
      <div>{renderLinks("data:text/html,<script>alert(1)</script>", "k")}</div>,
    );
    expect(container.querySelector("a")).toBeNull();
  });

  it("refuses vbscript: and file: URLs", () => {
    for (const url of ["vbscript:msgbox(1)", "file:///etc/passwd"]) {
      const { container } = render(<div>{renderLinks(url, "k")}</div>);
      expect(container.querySelector("a")).toBeNull();
    }
  });

  it("renders multiple links in one message", () => {
    const { container } = render(
      <div>{renderLinks("https://a.example and https://b.example", "k")}</div>,
    );
    // Regression guard for the `/g`-flag-in-a-loop hazard: `RegExp.test` on a
    // global regex mutates `lastIndex`, which can make every second link
    // vanish.
    expect(container.querySelectorAll("a")).toHaveLength(2);
  });

  it("renders repeated identical links", () => {
    const { container } = render(
      <div>{renderLinks("https://a.example https://a.example", "k")}</div>,
    );
    expect(container.querySelectorAll("a")).toHaveLength(2);
  });

  it("returns plain text when there is no URL", () => {
    const { container } = render(<div>{renderLinks("no links here", "k")}</div>);
    expect(container.querySelector("a")).toBeNull();
    expect(container.textContent).toBe("no links here");
  });
});

describe("groupByDate", () => {
  const base = 1_700_000_000_000;
  const msg = (id: string, ts: number) => ({
    id,
    conversation_id: "c",
    peer_key_hex: "p",
    direction: "received" as const,
    content: id,
    timestamp: Math.floor(ts / 1000),
    is_read: false,
  });

  it("groups messages by calendar day", () => {
    const day1 = base;
    const day2 = base + 24 * 60 * 60 * 1000;
    const grouped = groupByDate([
      msg("a", day1),
      msg("b", day1 + 1000),
      msg("c", day2),
    ]);
    expect(Object.keys(grouped)).toHaveLength(2);
    const total = Object.values(grouped).reduce((n, g) => n + g.length, 0);
    expect(total).toBe(3);
  });

  it("returns nothing for an empty message list", () => {
    expect(Object.keys(groupByDate([]))).toHaveLength(0);
  });

  it("preserves the input order within a day", () => {
    // Deliberate: `groupByDate` does NOT sort. The backend already returns
    // messages in timestamp order, and re-sorting client-side would reorder
    // optimistically-appended local messages relative to the ones the backend
    // sent — making a just-sent message jump. It only partitions.
    const grouped = groupByDate([msg("a", base + 3000), msg("b", base + 1000)]);
    const only = Object.values(grouped)[0];
    expect(only.map((m) => m.id)).toEqual(["a", "b"]);
  });

  it("labels the current and previous day", () => {
    // The labels come from the catalog, so this also pins the grouping key.
    const now = Date.now();
    const day = 24 * 60 * 60 * 1000;
    const grouped = groupByDate([
      { ...msg("now", now), timestamp: Math.floor(now / 1000) },
      { ...msg("yest", now - day), timestamp: Math.floor((now - day) / 1000) },
    ]);
    const keys = Object.keys(grouped);
    expect(keys).toContain("Today");
    expect(keys).toContain("Yesterday");
  });
});
