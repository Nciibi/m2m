import { describe, it, expect } from "vitest";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";

/**
 * Design-token rule enforcement.
 *
 * `CLAUDE.md`: *"No colour literal may appear outside `src/styles/tokens.css`
 * and `src/styles/theme.css`, with the single exception of a
 * `var(--token, <literal>)` fallback used inline in `src/App.tsx`."*
 *
 * This is the frontend counterpart of the Rust architecture test at
 * `src-tauri/src/dial.rs` that greps for a raw `TcpStream::connect`. Both exist
 * because the rule is not visible at the call site: a hardcoded `color="white"`
 * on an indigo chip is correct-looking in dark mode, so nothing about the code
 * says "this is now unthemed". The rule was violated seven times in `.tsx` before
 * this file existed, and thirty times in `.css` — and on the light theme those
 * render white-on-white.
 *
 * `var(--token, #fallback)` is allowed anywhere: the fallback only applies if the
 * stylesheet fails to load, which is the single sanctioned exception in
 * CLAUDE.md.
 */

const SRC = resolve(__dirname, "..");

function walk(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      walk(full, out);
    } else {
      out.push(full);
    }
  }
  return out;
}

/** Colour literals that are not inside a `var(--x, …)` fallback. */
const LITERAL =
  /#[0-9a-fA-F]{3,8}\b|\brgba?\s*\(|\bhsla?\s*\(|\b(?:white|black|silver|maroon|navy|teal|olive|lime|aqua|fuchsia)\b/;

/**
 * Strip `var(--token, <fallback>)` down to just the token name, so a sanctioned
 * fallback does not read as a violation. Handles one level of nesting, which is
 * all the codebase uses.
 */
function stripVarFallbacks(css: string): string {
  let out = css;
  let previous: string;
  do {
    previous = out;
    out = out.replace(/var\(\s*--[\w-]+\s*,[\s\S]*?\)/g, "var(--token)");
  } while (out !== previous);
  return out;
}

describe("design tokens: no colour literals outside tokens.css / theme.css", () => {
  const TOKEN_FILES = new Set([
    join(SRC, "styles", "tokens.css"),
    join(SRC, "styles", "theme.css"),
  ]);

  it("tokens.css and theme.css are the only permitted colour sources", () => {
    for (const f of TOKEN_FILES) {
      expect(statSync(f).isFile(), `${relative(SRC, f)} missing`).toBe(true);
    }
  });

  it("finds no colour literal in .ts / .tsx source", () => {
    const violations: string[] = [];
    for (const file of walk(SRC)) {
      if (!/\.tsx?$/.test(file)) continue;
      if (file.includes(`${join("__tests__")}`)) continue;
      const rel = relative(SRC, file).replace(/\\/g, "/");
      readFileSync(file, "utf8")
        .split("\n")
        .forEach((line, i) => {
          const code = line.split("//")[0];
          if (LITERAL.test(code)) violations.push(`${rel}:${i + 1}  ${line.trim()}`);
        });
    }
    expect(
      violations,
      `Colour literals in TS/TSX. Use var(--token), or add the value to\n` +
        `tokens.css (dark) + theme.css (light):\n${violations.join("\n")}`,
    ).toEqual([]);
  });

  it("finds no colour literal in .css outside the two token files", () => {
    const violations: string[] = [];
    for (const file of walk(join(SRC, "styles"))) {
      if (!file.endsWith(".css")) continue;
      if (TOKEN_FILES.has(file)) continue;
      const rel = relative(SRC, file).replace(/\\/g, "/");
      const stripped = stripVarFallbacks(readFileSync(file, "utf8"));
      stripped.split("\n").forEach((line, i) => {
        if (LITERAL.test(line)) violations.push(`${rel}:${i + 1}  ${line.trim()}`);
      });
    }
    expect(
      violations,
      `Colour literals in CSS. Move them into tokens.css and override in\n` +
        `theme.css:\n${violations.join("\n")}`,
    ).toEqual([]);
  });

  it("every token referenced from source is defined in tokens.css", () => {
    const defined = new Set(
      Array.from(readFileSync(join(SRC, "styles", "tokens.css"), "utf8").matchAll(
        /^\s*(--[\w-]+)\s*:/gm,
      )).map((m) => m[1]),
    );

    const referenced = new Set<string>();
    for (const file of walk(SRC)) {
      if (!/\.tsx?$/.test(file) || file.includes(`${join("__tests__")}`)) continue;
      for (const m of readFileSync(file, "utf8").matchAll(/var\(\s*(--[\w-]+)/g)) {
        referenced.add(m[1]);
      }
    }

    const missing = [...referenced].filter((t) => !defined.has(t)).sort();
    expect(
      missing,
      `Referenced but never defined in tokens.css: ${missing.join(", ")}`,
    ).toEqual([]);
  });
});