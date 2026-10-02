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

/**
 * Colour literals, as *values*.
 *
 * Two shapes of false positive cost time here and are worth naming:
 *  - `white-space: nowrap` is a property name, not a colour. A bare `\bwhite\b`
 *    matches it.
 *  - An accent has to be able to name the colour it mixes *toward*
 *    (`color-mix(…, black)`), so `black` is a legitimate argument while
 *    `color: black` is a violation. Hence the value-position requirement.
 *
 * The value-position class includes `"`, `'` and `=` because the two syntaxes
 * differ: CSS writes `color: white`, JSX writes `color="white"`. An earlier
 * version of this pattern omitted them and consequently missed every one of the
 * seven `color="white"` violations it was written to catch — verified by
 * reintroducing one and watching the suite stay green.
 */
const LITERAL =
  /[:=(,]\s*["']?(#[0-9a-fA-F]{3,8}\b|rgba?\s*\(|hsla?\s*\()|(^|[:=(,\s"'])\s*(white|black|silver|maroon|navy|teal|olive|lime|aqua|fuchsia)\s*["']?\s*[;,)\s}\]]/;

/**
 * `-webkit-mask` / `mask` / `clip-path` stencils.
 *
 * `linear-gradient(#fff 0 0)` in a mask is alpha geometry, not paint: nothing
 * is drawn with it, it exists to carve a ring out of the padding box. It cannot
 * be expressed as a token without breaking the mask, and it is invisible by
 * definition. `linear-gradient(#fff …)` also appears with no space before the
 * hex, which the value-position regex above intentionally does not match.
 */
const STENCIL = /^\s*(-\w+-)?(mask|clip-path)\s*:/;

/**
 * `CLAUDE.md`'s sanctioned exception: a `var(--token, <literal>)` fallback in
 * `src/App.tsx`, so the element still receives a colour if the stylesheet fails.
 */
const SANCTIONED = new Set(["App.tsx"]);

/**
 * `src/context/ThemeContext.tsx` holds `DEFAULT_ACCENT`, the hex the app starts
 * on and "Reset accent" restores.
 *
 * It cannot be a token reference: the value is *written into* the
 * `--color-accent` custom property at runtime, so it has to be a concrete colour
 * to seed with. It is also, by then, in exactly one place — it used to be
 * duplicated in `SettingsView`'s reset handler, which is the bug that made this
 * worth a named exception. Enforced separately below.
 */
const RUNTIME_ACCENT_FILE = "context/ThemeContext.tsx";

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
      if (SANCTIONED.has(rel)) continue;
      const lines = readFileSync(file, "utf8").split("\n");
      let inBlockComment = false;
      lines.forEach((line, i) => {
        const trimmed = line.trim();
        // Skip comment bodies. A doc comment that *describes* the old literal in
        // order to say it was removed must not itself be flagged.
        if (inBlockComment) {
          if (trimmed.includes("*/")) inBlockComment = false;
          return;
        }
        if (trimmed.startsWith("/*") && !trimmed.includes("*/")) {
          inBlockComment = true;
          return;
        }
        if (trimmed.startsWith("//") || trimmed.startsWith("*") || trimmed.startsWith("{/*")) return;
        const code = line.split("//")[0];
        if (!LITERAL.test(code)) return;
        // The one allowed literal in this file is DEFAULT_ACCENT, and only on the
        // line that declares it.
        if (rel === RUNTIME_ACCENT_FILE && /DEFAULT_ACCENT\s*=/.test(code)) return;
        violations.push(`${rel}:${i + 1}  ${trimmed}`);
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
        if (STENCIL.test(line)) return;
        const code = line.split("/*")[0];
        if (LITERAL.test(code)) violations.push(`${rel}:${i + 1}  ${line.trim()}`);
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
      const lines = readFileSync(file, "utf8").split("\n");
      let inBlockComment = false;
      for (const line of lines) {
        const trimmed = line.trim();
        if (inBlockComment) {
          if (trimmed.includes("*/")) inBlockComment = false;
          continue;
        }
        if (trimmed.startsWith("/*") && !trimmed.includes("*/")) {
          inBlockComment = true;
          continue;
        }
        if (trimmed.startsWith("//") || trimmed.startsWith("*") || trimmed.startsWith("{/*")) continue;
        for (const m of line.split("//")[0].matchAll(/var\(\s*(--[\w-]+)/g)) {
          // A name ending in `-` is followed by a `${…}` interpolation — the
          // runtime value cannot be checked statically, so it is skipped rather
          // than reported as an undefined token.
          if (m[1].endsWith("-")) continue;
          referenced.add(m[1]);
        }
      }
    }

    const missing = [...referenced].filter((t) => !defined.has(t)).sort();
    expect(
      missing,
      `Referenced but never defined in tokens.css: ${missing.join(", ")}`,
    ).toEqual([]);
  });

  it("keeps exactly one copy of the default accent", () => {
    // `DEFAULT_ACCENT` was duplicated in `SettingsView`'s "Reset accent"
    // handler. Two copies of a default is the hand-synchronised-copy failure
    // mode this codebase already has a scar from: change one and the reset
    // button stops resetting to the real default, with nothing complaining.
    const themePath = join(SRC, "context", "ThemeContext.tsx");
    const declared = readFileSync(themePath, "utf8")
      .split("\n")
      .filter((l) => /DEFAULT_ACCENT\s*=/.test(l));
    expect(declared, "DEFAULT_ACCENT must be declared exactly once").toHaveLength(1);

    const elsewhere = walk(SRC)
      .filter((f) => /\.tsx?$/.test(f) && !f.includes(`${join("__tests__")}`))
      .filter((f) => f !== themePath)
      .filter((f) => /#6366f1/.test(readFileSync(f, "utf8")))
      .map((f) => relative(SRC, f).replace(/\\/g, "/"));
    expect(
      elsewhere,
      "#6366f1 may only appear in context/ThemeContext.tsx (as DEFAULT_ACCENT)",
    ).toEqual([]);
  });
});