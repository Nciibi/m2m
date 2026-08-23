import { describe, it, expect } from "vitest";
import { estimateEntropy, hashToColor, formatTime, DEFAULT_STUN_SERVERS } from "../utils";

describe("estimateEntropy", () => {
  it("returns 0 for empty input", () => {
    expect(estimateEntropy("")).toBe(0);
  });

  it("scales with length for a uniform lowercase passphrase", () => {
    const short = estimateEntropy("abcdefgh");
    const long = estimateEntropy("abcdefghijklmnop");
    expect(long).toBeGreaterThan(short);
    // 16 chars * log2(26) ≈ 75 bits, no penalties, capped at 128
    expect(estimateEntropy("abcdefghijklmnop")).toBeCloseTo(75.2, 0);
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

  it("produces valid HSL strings in-range", () => {
    const c = hashToColor("alice");
    expect(c).toMatch(/^hsl\(\d+, 55%, 48%\)$/);
    expect(parseInt(c.slice(4), 10)).toBeLessThan(360);
  });

  it("differs across most inputs", () => {
    const a = hashToColor("alice");
    const b = hashToColor("bob");
    expect(a).not.toBe(b);
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
