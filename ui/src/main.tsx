import { createRoot } from "react-dom/client";

import { App } from "./App";
import { applyTheme, browserStorage, loadPref } from "./theme";

// The remembered scheme goes on before the first paint, so a page left
// light does not open dark and flash over.
applyTheme(loadPref(browserStorage()), document.documentElement);

const root = document.getElementById("root");
if (!root) throw new Error("missing #root element");
createRoot(root).render(<App />);
