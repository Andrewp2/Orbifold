#!/usr/bin/env node

import { access, readFile, stat } from "node:fs/promises";
import path from "node:path";

const distDir = process.argv[2] ?? "dist";

async function requireFile(relativePath) {
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

async function requireText(relativePath) {
  return await readFile(await requireFile(relativePath), "utf8");
}

function requireContains(text, needle, label) {
  if (!text.includes(needle)) {
    throw new Error(`${label} should contain ${needle}`);
  }
}

function requireNotContains(text, needle, label) {
  if (text.includes(needle)) {
    throw new Error(`${label} should not contain ${needle}`);
  }
}

await access(distDir);

const html = await requireText("index.html");
await requireFile("pkg/orbifold_web.js");
await requireFile("pkg/orbifold_web_bg.wasm");
await requireFile("favicon.ico");
await requireFile("orbifold_icon.png");
await requireFile(".nojekyll");

requireContains(html, 'import init, { start_orbifold } from "./pkg/orbifold_web.js"', "index.html");
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

console.log(`web dist artifact ok: ${distDir}`);
