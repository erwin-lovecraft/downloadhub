// Builds the `mcp-server` binary and copies it to the Tauri-sidecar location
// (`src-tauri/binaries/mcp-server-<target-triple>[.exe]`) so `tauri build`
// bundles it inside the desktop app — one installer ships both. Run before
// `tauri build`/`tauri dev` (see the `sidecar`/`package` package.json
// scripts). Node-based so it works unchanged on Windows, macOS, and Linux.

import { execSync } from "node:child_process";
import { mkdirSync, copyFileSync } from "node:fs";
import { join } from "node:path";

function hostTargetTriple() {
  const out = execSync("rustc -vV", { encoding: "utf8" });
  const match = out.match(/host:\s*(\S+)/);
  if (!match) throw new Error("could not determine host target triple from `rustc -vV`");
  return match[1];
}

const triple = hostTargetTriple();
const exeExt = process.platform === "win32" ? ".exe" : "";

console.log(`Building mcp-server for ${triple}...`);
execSync("cargo build --release -p downloadhub-mcp-server", { stdio: "inherit" });

// The workspace builds to a single /target at the repo root (see
// docs/ARCHITECTURE.md), not src-tauri/target.
const builtBinary = join("target", "release", `mcp-server${exeExt}`);
const sidecarDir = join("src-tauri", "binaries");
const sidecarPath = join(sidecarDir, `mcp-server-${triple}${exeExt}`);

mkdirSync(sidecarDir, { recursive: true });
copyFileSync(builtBinary, sidecarPath);
console.log(`Sidecar ready: ${sidecarPath}`);
