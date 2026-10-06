// Assembles the website into _site/: the pages in site/, plus the icon and
// screenshots, which live with the app so there is only ever one copy.
//
//   npm run site               build _site/
//   npx vite preview --outDir _site    look at it locally

import { cpSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "_site");

rmSync(out, { recursive: true, force: true });
cpSync(join(root, "site"), out, { recursive: true });
cpSync(join(root, "brand", "icon.svg"), join(out, "icon.svg"));

mkdirSync(join(out, "screenshots"));
for (const name of readdirSync(join(root, "docs", "screenshots"))) {
  if (name.endsWith(".png")) {
    cpSync(join(root, "docs", "screenshots", name), join(out, "screenshots", name));
  }
}
console.log("Built _site/");
