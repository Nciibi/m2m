import { describe, it, expect } from "vitest";
import {
  estimateEntropy,
  errorMessage,
  evictionNoticeText,
  hashToColor,
  formatTime,
  DEFAULT_STUN_SERVERS,
} from "../utils";

describe("estimateEntropy", () => {
  it("returns 0 for empty input", () => {
    expect(estimateEntropy("")).toBe(0);
  });

  it("scales with length for a uniform lowercase passphrase", () => {
    const short = estimateEntropy("kqhzlxmp");
    const long = estimateEntropy("kqhzlxmpbwcvmfje");
    expect(long).toBeGreaterThan(short);
    // 16 chars * log2(26) ≈ 75 bits, no penalties, capped at 128
    expect(estimateEntropy("kqhzlxmpbwcvmfje")).toBeCloseTo(75.2, 0);
  });

  it("rewards larger character pools", () => {
    const lower = estimateEntropy("abcdefghijkl");           // 26 pool
    const mixed = estimateEntropy("abcdefghJK12!");       // 26+26+10+32 pool
    expect(mixed).toBeGreaterThan(lower);
  });

  it("penalizes sequential characters", () => {
    const random = estimateEntropy("kqhzlp12xma!");
    const sequential = estimateEntropy("abcdefghijk");
    expect(sequential).toBeLessThan(random);
  });

  it("penalizes repeated characters", () => {
    const repeated = estimateEntropy("aaaaaaaaaaaa");
    const varied = estimateEntropy("axbzcmkq12rt");
    expect(repeated).toBeLessThan(varied);
  });

  it("penalizes keyboard patterns like qwerty", () => {
    const keyboard = estimateEntropy("qwertyqwerty");
    const neutral = estimateEntropy("qwertzuiopas");
    expect(keyboard).toBeLessThanOrEqual(neutral);
  });

  it("applies the NIST floor for short passphrases (>=8 chars)", () => {
    // "aaaaaaaaaa" would score near zero without the floor
    expect(estimateEntropy("aaaaaaaaaa")).toBeGreaterThanOrEqual(14);
  });

  it("applies the stronger NIST floor below 8 chars", () => {
    expect(estimateEntropy("aaaaa")).toBeGreaterThanOrEqual(8);
  });

  it("caps entropy at 128 bits", () => {
    expect(estimateEntropy("Correct Horse Battery Staple XYZ!")).toBeLessThanOrEqual(128);
  });

  it("never returns negative values", () => {
    expect(estimateEntropy("1234567890abcdef")).toBeGreaterThanOrEqual(0);
  });
});

describe("hashToColor", () => {
  it("is deterministic", () => {
    expect(hashToColor("abc")).toBe(hashToColor("abc"));
  });

  it("returns a token reference, never a colour literal", () => {
    // Regression: this used to build `hsl(<hue>, 55%, 48%)` in JS, which put
    // every avatar outside tokens.css/theme.css and made it unable to respond to
    // the theme. The literal is what the old test asserted, so the old test
    // *pinned the violation in place*.
    const c = hashToColor("alice");
    expect(c).toMatch(/^var\(--color-avatar-[0-7]\)$/);
    expect(c).not.toMatch(/hsl|rgb|#/);
  });

  it("stays inside the eight-bucket ramp", () => {
    for (const input of ["alice", "bob", "", "a".repeat(64), "ffff", "0000"]) {
      const idx = Number(hashToColor(input).slice(-1));
      expect(idx).toBeGreaterThanOrEqual(0);
      expect(idx).toBeLessThanOrEqual(7);
    }
  });

  it("differs across most inputs", () => {
    const a = hashToColor("alice");
    const b = hashToColor("bob");
    expect(a).not.toBe(b);
  });

  it("uses a bucket that actually exists in both themes", () => {
    // The function must not be able to name a token that is missing, or the
    // avatar silently falls back to no background at all.
    const dark = readFileSync(resolve(__dirname, "../styles/tokens.css"), "utf8");
    const light = readFileSync(resolve(__dirname, "../styles/theme.css"), "utf8");
    for (let i = 0; i < 8; i++) {
      expect(dark).toContain(`--color-avatar-${i}:`);
      expect(light).toContain(`--color-avatar-${i}:`);
    }
  });
});

describe("formatTime", () => {
  it('shows "now" for anything under a minute', () => {
    const now = Math.floor(Date.now() / 1000);
    expect(formatTime(now)).toBe("now");
    expect(formatTime(now - 59)).toBe("now");
  });

  it("formats minutes and hours", () => {
    const now = Math.floor(Date.now() / 1000);
    expect(formatTime(now - 120)).toBe("2m ago");
    expect(formatTime(now - 7200)).toBe("2h ago");
  });

  it("formats days and weeks", () => {
    const now = Math.floor(Date.now() / 1000);
    expect(formatTime(now - 172800)).toBe("2d ago");
    expect(formatTime(now - 8 * 86400)).not.toContain("ago");
  });
});

describe("DEFAULT_STUN_SERVERS", () => {
  it("contains well-formed host:port entries", () => {
    expect(DEFAULT_STUN_SERVERS.length).toBeGreaterThan(0);
    for (const s of DEFAULT_STUN_SERVERS) {
      expect(s).toMatch(/^[\w.-]+:\d+$/);
    }
  });
});

/**
 * `errorMessage` is the display path for every command rejection.
 *
 * These cases are load-bearing rather than incidental: commands stopped
 * rejecting with a bare string when the `AppError` type landed, so
 * `"Failed to send: " + e` renders `"Failed to send: [object Object]"` — a
 * silent, universal regression across 45 call sites that `tsc` cannot see,
 * because `invoke<T>` does not type its rejection value.
 */
describe("errorMessage", () => {
  it("returns the message from an AppError-shaped rejection", () => {
    expect(errorMessage({ code: "network.io", message: "connection reset" })).toBe(
      "connection reset",
    );
  });

  it("never renders [object Object] for any object rejection", () => {
    // The specific failure the mechanical pass had to prevent.
    for (const e of [
      { code: "a", message: "boom" },
      { code: "a", message: "" },
      { message: "no code" },
      { code: "a" },
    ]) {
      expect(errorMessage(e)).not.toBe("[object Object]");
    }
  });

  it("falls back when an AppError carries no usable message", () => {
    // Display must degrade to the fallback, not to `""` or "undefined".
    expect(errorMessage({ code: "a" }, "Something went wrong")).toBe("Something went wrong");
    expect(errorMessage({ code: "a", message: "" })).toBe("Unknown error");
  });

  it("still handles the pre-taxonomy string rejection", () => {
    expect(errorMessage("plain failure")).toBe("plain failure");
    expect(errorMessage("", "fallback")).toBe("fallback");
  });

  it("handles Error instances and other thrown values", () => {
    expect(errorMessage(new Error("boom"))).toBe("boom");
    expect(errorMessage(new Error(""), "fallback")).toBe("fallback");
    expect(errorMessage(null)).toBe("Unknown error");
    expect(errorMessage(undefined, "fallback")).toBe("fallback");
    expect(errorMessage(42)).toBe("Unknown error");
  });
});

/**
 * The storage-cap eviction notice.
 *
 * This is the only thing the user is told when the cap permanently destroys
 * history, so its content is a security property, not copy. The three
 * assertions below are the three claims it must always make — the count, that
 * the loss is unrecoverable *including from a backup taken beforehand*, and
 * that raising the cap is the remedy. A rewrite that keeps the first and drops
 * either of the other two is the failure mode worth catching.
 */
describe("evictionNoticeText", () => {
  const base = {
    messages_evicted: 120,
    group_messages_evicted: 30,
    bytes_freed: 5 * 1024 ** 3,
    overrode_retention: [] as string[],
  };

  it("says how many messages were destroyed, across 1:1 and group", () => {
    const text = evictionNoticeText(base);
    expect(text).not.toBeNull();
    expect(text).toContain("150 old message(s)");
  });

  it("states that the loss is unrecoverable, including from a backup", () => {
    // The honest caveat. "Permanently deleted" alone invites the belief that a
    // copy exists somewhere — and the user is the one who knows whether they
    // took one before the eviction.
    expect(evictionNoticeText(base)).toContain(
      "cannot be recovered, including from backups taken beforehand",
    );
  });

  it("names the remedy — raising the cap", () => {
    expect(evictionNoticeText(base)).toContain("Raise the cap in Settings");
  });

  it("names the conversations whose retention policy was overridden", () => {
    // Silently discarding a preference the user set is the failure this clause
    // exists to prevent, so its absence has to be a test failure.
    const text = evictionNoticeText({
      ...base,
      overrode_retention: ["c1", "c2", "c3"],
    });
    expect(text).toContain("overrode the retention policy on 3 conversation(s)");
  });

  it("omits the override sentence when nothing was overridden", () => {
    expect(evictionNoticeText(base)).not.toContain("overrode the retention policy");
  });

  it("formats a sub-GB free in MB and a multi-GB free in GB", () => {
    expect(evictionNoticeText({ ...base, bytes_freed: 512 * 1024 ** 2 })).toContain("512 MB");
    expect(evictionNoticeText({ ...base, bytes_freed: 2 * 1024 ** 3 })).toContain("2.0 GB");
  });

  it("returns null when nothing was destroyed", () => {
    // A notice about a loss that did not happen is the mirror of silent loss:
    // it teaches the user to ignore the channel that reports real destruction.
    expect(
      evictionNoticeText({ ...base, messages_evicted: 0, group_messages_evicted: 0 }),
    ).toBeNull();
  });
});
