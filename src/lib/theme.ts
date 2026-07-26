/**
 * Theme state for the Design System v2 (presentation only).
 *
 * The theme is applied as `data-theme` on `<html>`, which swaps the token values
 * in `styles/tokens.css`. Nothing else in the app needs to know the theme —
 * components consume semantic classes, so no component reads this except the
 * toolbar toggle.
 */

import { useCallback, useEffect, useState } from 'react';

export type Theme = 'light' | 'dark';

const STORAGE_KEY = 'statelab.theme';

/** The theme to start in: the user's last choice, else the OS preference. */
export function resolveInitialTheme(): Theme {
  if (typeof window === 'undefined') {
    return 'dark';
  }
  const stored = window.localStorage?.getItem(STORAGE_KEY);
  if (stored === 'light' || stored === 'dark') {
    return stored;
  }
  return window.matchMedia?.('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

function apply(theme: Theme): void {
  if (typeof document === 'undefined') {
    return;
  }
  document.documentElement.setAttribute('data-theme', theme);
  // Keeps native form controls and scrollbars in step with the app chrome.
  document.documentElement.style.colorScheme = theme;
}

/** Current theme plus a toggle. Persists the choice across launches. */
export function useTheme(): { theme: Theme; toggleTheme: () => void } {
  const [theme, setTheme] = useState<Theme>(resolveInitialTheme);

  useEffect(() => {
    apply(theme);
    try {
      window.localStorage?.setItem(STORAGE_KEY, theme);
    } catch {
      // Private mode or a locked-down WebView — the theme still applies for
      // this session, it just will not be remembered.
    }
  }, [theme]);

  const toggleTheme = useCallback(() => {
    setTheme((current) => (current === 'dark' ? 'light' : 'dark'));
  }, []);

  return { theme, toggleTheme };
}
