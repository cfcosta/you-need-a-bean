import { describe, expect, it } from "bun:test";

import { applyTheme, loadPref, readPref, rootTheme, savePref } from "./theme";

describe("readPref", () => {
  it("takes a saved preference at its word", () => {
    expect(readPref("light")).toBe("light");
    expect(readPref("dark")).toBe("dark");
    expect(readPref("auto")).toBe("auto");
  });

  it("follows the system when nothing, or nonsense, was saved", () => {
    expect(readPref(null)).toBe("auto");
    expect(readPref(undefined)).toBe("auto");
    expect(readPref("sepia")).toBe("auto");
    expect(readPref("")).toBe("auto");
  });
});

describe("rootTheme", () => {
  it("stamps the root only when the reader overrode the system", () => {
    expect(rootTheme("auto")).toBeNull();
    expect(rootTheme("light")).toBe("light");
    expect(rootTheme("dark")).toBe("dark");
  });
});

describe("applyTheme", () => {
  const root = () => {
    const attrs = new Map<string, string>();
    return {
      attrs,
      setAttribute: (k: string, v: string) => {
        attrs.set(k, v);
      },
      removeAttribute: (k: string) => {
        attrs.delete(k);
      },
    };
  };

  it("writes the override onto the root", () => {
    const r = root();
    applyTheme("dark", r);
    expect(r.attrs.get("data-theme")).toBe("dark");
  });

  it("clears the override on the way back to auto", () => {
    const r = root();
    applyTheme("light", r);
    applyTheme("auto", r);
    expect(r.attrs.has("data-theme")).toBe(false);
  });
});

describe("loadPref / savePref", () => {
  it("round-trips through storage", () => {
    const store = new Map<string, string>();
    const storage = {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => {
        store.set(k, v);
      },
    };
    savePref("dark", storage);
    expect(loadPref(storage)).toBe("dark");
    savePref("auto", storage);
    expect(loadPref(storage)).toBe("auto");
  });

  it("shrugs when storage is off limits", () => {
    const broken = {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    };
    expect(loadPref(broken)).toBe("auto");
    expect(() => savePref("dark", broken)).not.toThrow();
  });
});
