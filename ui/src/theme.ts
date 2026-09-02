/**
 * Which look the page wears.
 *
 * The stylesheet follows the system by default and can be pinned either
 * way with `data-theme` on the root element. This module owns that
 * attribute and the preference behind it; nothing here touches the
 * document or the browser's storage directly, so it can be read in a
 * test the same way the page reads it.
 */

export type ThemePref = "auto" | "light" | "dark";

/** The order the switch offers them in. */
export const THEMES: readonly ThemePref[] = ["auto", "light", "dark"];

const KEY = "bean:theme";

/** What a saved value means; anything unexpected is "follow the system". */
export function readPref(raw: string | null | undefined): ThemePref {
  return raw === "light" || raw === "dark" ? raw : "auto";
}

/** The attribute the root should carry: none while following the system. */
export function rootTheme(pref: ThemePref): "light" | "dark" | null {
  return pref === "auto" ? null : pref;
}

interface Root {
  setAttribute(name: string, value: string): void;
  removeAttribute(name: string): void;
}

export function applyTheme(pref: ThemePref, root: Root): void {
  const t = rootTheme(pref);
  if (t == null) root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", t);
}

interface Storage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

/** The saved preference, or "auto" when storage is empty or off limits. */
export function loadPref(storage: Storage | null): ThemePref {
  try {
    return readPref(storage?.getItem(KEY));
  } catch {
    return "auto";
  }
}

/** Remember the choice. A browser that refuses (private windows, a full
 * quota) just forgets it at the end of the session. */
export function savePref(pref: ThemePref, storage: Storage | null): void {
  try {
    storage?.setItem(KEY, pref);
  } catch {
    // Nothing to do: the page already wears the chosen theme.
  }
}

/** The browser's storage, or null where even asking for it throws. */
export function browserStorage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}
