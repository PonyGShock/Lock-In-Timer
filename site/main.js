// Points the download buttons at the files in the latest release, and puts
// the visitor's own platform first. Without JavaScript, or if GitHub cannot
// be reached, every button still leads to the releases page.

const REPO = "PonyGShock/Lock-In-Timer";
const RELEASES = `https://github.com/${REPO}/releases`;

function platform() {
  const hint = (navigator.userAgentData?.platform || navigator.platform || navigator.userAgent).toLowerCase();
  if (hint.includes("mac")) return "mac";
  if (hint.includes("win")) return "windows";
  return null;
}

async function latestRelease() {
  const response = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!response.ok) throw new Error(`GitHub answered ${response.status}`);
  return response.json();
}

function assetUrl(release, suffix) {
  return release?.assets.find((asset) => asset.name.endsWith(suffix))?.browser_download_url;
}

async function main() {
  const os = platform();
  const primary = document.querySelector("[data-primary]");
  const label = document.querySelector("[data-primary-label]");

  if (os === "mac") label.textContent = "Download for Mac";
  if (os === "windows") label.textContent = "Download for Windows";
  if (os) primary.href = "#download";

  let release = null;
  try {
    release = await latestRelease();
  } catch {
    // No published release yet, or GitHub is unreachable: the links already
    // point at the releases page, which is the right fallback for both.
    return;
  }

  for (const link of document.querySelectorAll("[data-asset]")) {
    const url = assetUrl(release, link.dataset.asset);
    if (url) link.href = url;
  }

  const direct = os === "mac" ? assetUrl(release, "aarch64.dmg") : os === "windows" ? assetUrl(release, "x64-setup.exe") : null;
  if (direct) primary.href = direct;

  const version = release.tag_name;
  const fineprint = document.querySelector("[data-fineprint]");
  fineprint.textContent = `Version ${version.replace(/^v/, "")} · ${
    os === "mac" ? "Apple silicon · Intel below" : "macOS and Windows"
  } · Free forever`;

  const line = document.querySelector("[data-release-line]");
  line.innerHTML = "";
  line.append(
    `Version ${version.replace(/^v/, "")}, built straight from the source you can read. `,
    Object.assign(document.createElement("a"), { href: release.html_url || RELEASES, textContent: "What’s new" }),
  );
}

main();
