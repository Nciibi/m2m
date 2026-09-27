import { createContext, useCallback, useContext, useMemo, useState, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LOCALES, LocaleCode, makeT, Translation } from "./catalog";

type Translator = (path: string, values?: Record<string, string | number>) => string;

interface I18nContextValue {
  locale: LocaleCode;
  setLocale: (l: LocaleCode) => void;
  t: Translator;
  /** The full resolved bundle — use for `aria-label` maps and tests. */
  strings: Translation;
}

const I18nContext = createContext<I18nContextValue | null>(null);

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n() must be used within <I18nProvider>");
  return ctx;
}

/** Shorthand for the common case. */
export function useT(): Translator {
  return useI18n().t;
}

const STORAGE_KEY = "m2m.locale";

/** Resolve a stored locale, falling back to `en` for anything unrecognised. */
function resolveLocale(stored: string | null): LocaleCode {
  if (stored && stored in LOCALES) return stored as LocaleCode;
  return "en";
}

export function I18nProvider({ children }: { children: ReactNode }) {
  // Read the stored preference synchronously so the first paint is already in
  // the right language — a flash of English before switching is worse than no
  // localisation for a user who needs it urgently.
  const [locale, setLocaleState] = useState<LocaleCode>(() => {
    try {
      return resolveLocale(window.localStorage.getItem(STORAGE_KEY));
    } catch {
      // Private-mode / storage-disabled browsers throw on access.
      return "en";
    }
  });

  const setLocale = useCallback((l: LocaleCode) => {
    setLocaleState(l);
    try {
      window.localStorage.setItem(STORAGE_KEY, l);
    } catch {
      // Non-fatal: the choice simply will not persist.
    }
  }, []);

  const value = useMemo<I18nContextValue>(() => {
    const t = makeT(locale);
    // Resolve the bundle with English fallbacks so `strings.x.y` is always a
    // string, never undefined.
    const strings = LOCALES[locale] as Translation;
    return { locale, setLocale, t, strings };
  }, [locale, setLocale]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}
