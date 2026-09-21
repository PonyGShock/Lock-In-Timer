import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import App from "./App";
import { inTauri } from "./bridge";
import "./styles.css";

// Outside the menu bar the window is a normal page, so give the card a
// backdrop to sit on instead of leaving it floating on white.
if (!inTauri) document.body.classList.add("web");

// A menu bar popover has no browser chrome to fall back on, so the usual
// right-click menu is just a way to get stuck.
window.addEventListener("contextmenu", (event) => event.preventDefault());

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
