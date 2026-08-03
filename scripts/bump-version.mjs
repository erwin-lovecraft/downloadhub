#!/usr/bin/env node
// The root VERSION file (a single "x.y.z" line) is the single source of
// truth for the app version — package.json, Cargo.toml, and tauri.conf.json
// all get overwritten to match it here rather than any of them being
// trusted as authoritative, since letting them drift independently is
// exactly how they went out of sync before this script existed.
//
// Usage: node scripts/bump-version.mjs [x.y.z|major|minor|patch]
// No argument defaults to a patch bump of whatever VERSION currently says.
import { readFileSync, writeFileSync } from "node:fs";

const VERSION_FILE = "VERSION";
const SEMVER = /^\d+\.\d+\.\d+$/;

function readCurrentVersion() {
  const text = readFileSync(VERSION_FILE, "utf8").trim();
  if (!SEMVER.test(text)) {
    throw new Error(
      `${VERSION_FILE}: expected a single "x.y.z" line, got ${JSON.stringify(text)}`
    );
  }
  return text;
}

function bump(current, keyword) {
  const [major, minor, patch] = current.split(".").map(Number);
  switch (keyword) {
    case "major":
      return `${major + 1}.0.0`;
    case "minor":
      return `${major}.${minor + 1}.0`;
    case "patch":
      return `${major}.${minor}.${patch + 1}`;
    default:
      throw new Error(`unknown bump keyword: ${keyword}`);
  }
}

function resolveTarget(arg, current) {
  if (!arg || arg === "major" || arg === "minor" || arg === "patch") {
    return bump(current, arg || "patch");
  }
  if (SEMVER.test(arg)) {
    return arg;
  }
  console.error("Usage: node scripts/bump-version.mjs [x.y.z|major|minor|patch]");
  process.exit(1);
}

function replaceOnce(path, pattern, replacement) {
  const text = readFileSync(path, "utf8");
  if (!pattern.test(text)) {
    throw new Error(`${path}: version pattern not found, refusing to write`);
  }
  writeFileSync(path, text.replace(pattern, replacement));
}

const current = readCurrentVersion();
const target = resolveTarget(process.argv[2], current);

writeFileSync(VERSION_FILE, `${target}\n`);
replaceOnce("package.json", /"version":\s*"[^"]*"/, `"version": "${target}"`);
replaceOnce("Cargo.toml", /^version = "[^"]*"/m, `version = "${target}"`);
replaceOnce(
  "src-tauri/tauri.conf.json",
  /"version":\s*"[^"]*"/,
  `"version": "${target}"`
);

console.log(`Bumped ${current} -> ${target}`);
