#!/usr/bin/env node
import { execSync } from "node:child_process";

// "build 2026-09-27 16:42 · 1c2190a" (commit id, "+dirty" when there are uncommitted changes).
function buildStamp() {
  const now = new Date();
  const pad = (n) => String(n).padStart(2, "0");
  const when = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}:${pad(now.getMinutes())}`;
  let rev = "";
  try {
    rev = execSync("git rev-parse --short HEAD", { encoding: "utf8" }).trim();
    if (execSync("git status --porcelain", { encoding: "utf8" }).trim()) rev += "+dirty";
  } catch {
    rev = "no-git";
  }
  return `build ${when} · ${rev}`;
}

// Builds web/dist/index.html: a single static file with the compiled wasm module inlined as
// base64, so it opens directly via file:// with no server and no bundler. Also copies the raw
// .wasm into web/dist/ for anyone who wants it standalone (e.g. web/test.mjs).
//
// Usage: node web/build.mjs
// Prerequisite: cargo build -p dominion-wasm --release --target wasm32-unknown-unknown

import { readFileSync, writeFileSync, mkdirSync, copyFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const webDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(webDir, "..");
const wasmPath = join(repoRoot, "target", "wasm32-unknown-unknown", "release", "dominion_wasm.wasm");
const distDir = join(webDir, "dist");

function main() {
  if (!existsSync(wasmPath)) {
    console.error(`Could not find built wasm module at:\n  ${wasmPath}`);
    console.error("Build it first with:");
    console.error("  ~/.cargo/bin/cargo build -p dominion-wasm --release --target wasm32-unknown-unknown");
    process.exit(1);
  }

  mkdirSync(distDir, { recursive: true });

  const wasmBytes = readFileSync(wasmPath);
  copyFileSync(wasmPath, join(distDir, "dominion_wasm.wasm"));

  const template = readFileSync(join(webDir, "template.html"), "utf8");
  const style = readFileSync(join(webDir, "style.css"), "utf8");
  const appJs = readFileSync(join(webDir, "app.js"), "utf8");
  const wasmBase64 = wasmBytes.toString("base64");

  let html = template;
  html = html.replace("/*__STYLE__*/", () => style);
  html = html.replace("__WASM_BASE64__", () => wasmBase64);
  html = html.replace("/*__APP_JS__*/", () => appJs);
  html = html.replace("__BUILD_STAMP__", () => buildStamp());

  const outPath = join(distDir, "index.html");
  writeFileSync(outPath, html, "utf8");

  const kb = (n) => (n / 1024).toFixed(1) + " KiB";
  console.log(`Built ${outPath}`);
  console.log(`  wasm module: ${kb(wasmBytes.length)} (${kb(wasmBase64.length)} as base64)`);
  console.log(`  index.html:  ${kb(Buffer.byteLength(html))}`);
  console.log(`Open it directly, e.g.:  file://${outPath.replace(/\\/g, "/")}`);
}

main();
