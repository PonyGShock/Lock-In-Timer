// Rebuilds every platform icon from the two SVGs in brand/.
//
// brand/icon.svg is a full-bleed tile, which is what Windows, Linux and the
// phone platforms want. macOS icons sit on a smaller tile with room for a
// shadow, so the .icns comes from brand/icon-macos.svg instead.
//
// Node rather than a shell script, so it runs the same on Windows.

import { execSync } from "node:child_process";
import { copyFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const run = (command) => execSync(command, { cwd: root, stdio: "inherit" });

run("npx tauri icon brand/icon.svg");

const scratch = mkdtempSync(join(tmpdir(), "lockin-icons-"));
try {
  run(`npx tauri icon brand/icon-macos.svg -o "${scratch}"`);
  copyFileSync(join(scratch, "icon.icns"), join(root, "src-tauri", "icons", "icon.icns"));
} finally {
  rmSync(scratch, { recursive: true, force: true });
}
console.log("Wrote src-tauri/icons, with the macOS icon from brand/icon-macos.svg");
