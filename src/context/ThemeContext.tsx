import { createContext, useContext, useState, useEffect, ReactNode, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useApp } from "./AppContext";

export type ThemeMode = "light" | "dark" | "system";

interface ThemeContextValue {
  theme: ThemeMode;
  setTheme: (theme: ThemeMode) => void;
  resolvedTheme: "light" | "dark";
  accentColor: string;
  setAccentColor: (color: string) => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme() must be used within <ThemeProvider>");
  return ctx;
}

const DEFAULT_ACCENT = "#6366f1";

export function ThemeProvider({ children }: { children: ReactNode }) {
  const { addToast } = useApp();
  const [theme, setThemeState] = useState<ThemeMode>("system");
  const [resolvedTheme, setResolvedTheme] = useState<"light" | "dark">("dark");
  const [accentColor, setAccentColorState] = useState<string>(DEFAULT_ACCENT);
  const [initialized, setInitialized] = useState(false);

  const applyAccent = useCallback((color: string) => {
    document.documentElement.style.setProperty("--color-accent", color);
  }, []);

  const applyTheme = useCallback((mode: ThemeMode) => {
    let resolved: "light" | "dark";
    if (mode === "system") {
      resolved = window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
    } else {
      resolved = mode;
    }
    document.documentElement.setAttribute("data-theme", resolved);
    setResolvedTheme(resolved);
  }, []);

  useEffect(() => {
    const loadTheme = async () => {
      try {
        const prefs = await invoke<any>("get_theme_preference");
        const themeMode = typeof prefs === "string" ? prefs : prefs?.theme;
        const accent = typeof prefs === "object" && prefs?.accent_color ? prefs.accent_color : null;
        const validThemes: ThemeMode[] = ["light", "dark", "system"];
        if (validThemes.includes(themeMode as ThemeMode)) {
          setThemeState(themeMode as ThemeMode);
          applyTheme(themeMode as ThemeMode);
        }
        if (accent) {
          setAccentColorState(accent);
          applyAccent(accent);
        }
      } catch {
        applyTheme("system");
      } finally {
        setInitialized(true);
      }
    };
    loadTheme();
  }, [applyTheme, applyAccent]);

  useEffect(() => {
    if (!initialized) return;
    applyTheme(theme);
    invoke("set_theme_preference", { theme }).catch((e) => {
      addToast("Failed to save theme: " + e, "error");
    });
  }, [theme, initialized, applyTheme, addToast]);

  useEffect(() => {
    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    const handler = (_e: MediaQueryListEvent) => applyTheme("system");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme, applyTheme]);

  const setTheme = useCallback((newTheme: ThemeMode) => {
    setThemeState(newTheme);
  }, []);

  const setAccentColor = useCallback((color: string) => {
    setAccentColorState(color);
    applyAccent(color);
    invoke("set_theme_preference", { theme: theme, accentColor: color }).catch(() => {});
  }, [theme, applyAccent]);

  if (!initialized) {
    return <>{children}</>;
  }

  return (
    <ThemeContext.Provider value={{ theme, setTheme, resolvedTheme, accentColor, setAccentColor }}>
      {children}
    </ThemeContext.Provider>
  );
}