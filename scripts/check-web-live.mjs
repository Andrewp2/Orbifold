#!/usr/bin/env node

const target = process.argv[2];

if (!target) {
  console.error("usage: scripts/check-web-live.mjs <https://pages-url/>");
  process.exit(2);
}

const normalizedTarget = target.endsWith("/") ? target : `${target}/`;
const rootUrl = new URL(normalizedTarget);

async function fetchRequired(relativePath, options = {}) {
  const url = new URL(relativePath, rootUrl);
  const response = await fetch(url, {
    cache: "no-store",
    redirect: "follow",
  });
  if (!response.ok) {
    throw new Error(`${url.href} returned HTTP ${response.status}`);
  }

  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length === 0) {
    throw new Error(`${url.href} returned an empty response`);
  }

  const contentType = response.headers.get("content-type") ?? "";
  if (options.contentType && !contentType.includes(options.contentType)) {
    throw new Error(
      `${url.href} returned content-type ${contentType}, expected ${options.contentType}`
    );
  }

  return {
    bytes,
    contentType,
    text: options.text ? new TextDecoder().decode(bytes) : "",
    url,
  };
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

const index = await fetchRequired("./", { text: true, contentType: "text/html" });
requireContains(index.text, '<title>Orbifold</title>', "index.html");
requireContains(index.text, 'import init, { start_orbifold } from "./pkg/orbifold_web.js"', "index.html");
requireContains(index.text, '<link rel="icon" href="./favicon.ico" sizes="any" />', "index.html");
requireContains(
  index.text,
  '<link rel="icon" type="image/png" sizes="64x64" href="./orbifold_icon.png" />',
  "index.html"
);
requireContains(index.text, "window.orbifoldRuntimeReady", "index.html");
requireContains(index.text, "runtime-ready", "index.html");
requireContains(index.text, "runtime-failed", "index.html");
requireContains(index.text, "static fallback", "index.html");
requireNotContains(index.text, 'href="/', "index.html");
requireNotContains(index.text, 'src="/', "index.html");
requireNotContains(index.text, 'from "/', "index.html");

const js = await fetchRequired("./pkg/orbifold_web.js", { text: true });
requireContains(js.text, "orbifold_web_bg.wasm", "orbifold_web.js");
requireContains(js.text, "start_orbifold", "orbifold_web.js");

const wasm = await fetchRequired("./pkg/orbifold_web_bg.wasm");
if (wasm.bytes[0] !== 0x00 || wasm.bytes[1] !== 0x61 || wasm.bytes[2] !== 0x73 || wasm.bytes[3] !== 0x6d) {
  throw new Error(`${wasm.url.href} is not a wasm binary`);
}

await fetchRequired("./favicon.ico");
await fetchRequired("./orbifold_icon.png");

console.log(`live web artifact ok: ${rootUrl.href}`);
