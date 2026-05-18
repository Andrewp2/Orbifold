#!/usr/bin/env node

import { access, readFile, stat } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

if (isCliEntrypoint()) {
  const distDir = process.argv[2] ?? "dist";
  await checkWebDist(distDir);
  console.log(`web dist artifact ok: ${distDir}`);
}

export async function checkWebDist(distDir = "dist") {
  await access(distDir);

  const html = await requireText(distDir, "index.html");
  await requireFile(distDir, "pkg/orbifold_web.js");
  await requireFile(distDir, "pkg/orbifold_web_bg.wasm");
  await requireFile(distDir, "favicon.ico");
  await requireFile(distDir, "orbifold_icon.png");
  await requireFile(distDir, ".nojekyll");

  requireWebIndexHtml(html);
}

async function requireFile(distDir, relativePath) {
  const fullPath = path.join(distDir, relativePath);
  let fileStat;
  try {
    fileStat = await stat(fullPath);
  } catch (error) {
    throw new Error(`missing ${relativePath}: ${error.message}`);
  }
  if (!fileStat.isFile()) {
    throw new Error(`${relativePath} is not a file`);
  }
  if (relativePath !== ".nojekyll" && fileStat.size <= 0) {
    throw new Error(`${relativePath} is empty`);
  }
  return fullPath;
}

async function requireText(distDir, relativePath) {
  return await readFile(await requireFile(distDir, relativePath), "utf8");
}

export function requireWebIndexHtml(html) {
  requireContains(
    html,
    'import init, { start_orbifold } from "./pkg/orbifold_web.js"',
    "index.html"
  );
  requireContains(html, '<link rel="icon" href="./favicon.ico" sizes="any" />', "index.html");
  requireContains(
    html,
    '<link rel="icon" type="image/png" sizes="64x64" href="./orbifold_icon.png" />',
    "index.html"
  );
  requireContains(html, "window.orbifoldRuntimeReady", "index.html");
  requireContains(html, "runtime-ready", "index.html");
  requireContains(html, "runtime-failed", "index.html");
  requireNotContains(html, 'href="/', "index.html");
  requireNotContains(html, 'src="/', "index.html");
  requireNotContains(html, 'from "/', "index.html");
}

export function requireContains(text, needle, label) {
  if (!text.includes(needle)) {
    throw new Error(`${label} should contain ${needle}`);
  }
}

export function requireNotContains(text, needle, label) {
  if (text.includes(needle)) {
    throw new Error(`${label} should not contain ${needle}`);
  }
}

function isCliEntrypoint() {
  return process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
}
