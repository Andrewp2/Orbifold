#!/usr/bin/env node

import { pathToFileURL } from "node:url";
import { requireContains, requireWebIndexHtml } from "./check-web-dist.mjs";

if (isCliEntrypoint()) {
  try {
    const options = parseLiveArgs(process.argv.slice(2));
    if (options.help || !options.target) {
      console.error("usage: scripts/check-web-live.mjs <https://pages-url/>");
      process.exit(2);
    }
    const rootHref = await checkWebLive(options.target);
    console.log(`live web artifact ok: ${rootHref}`);
  } catch (error) {
    console.error(`live web artifact failed: ${error.message ?? error}`);
    process.exit(1);
  }
}

export function parseLiveArgs(args) {
  const parsed = { target: "", help: false };
  for (const arg of args) {
    if (arg === "--help" || arg === "-h") {
      parsed.help = true;
      return parsed;
    }
    if (!arg.startsWith("--") && !parsed.target) {
      parsed.target = arg;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }
  return parsed;
}

export async function checkWebLive(target, fetchImpl = globalThis.fetch) {
  if (!target) {
    throw new Error("target URL is required");
  }
  if (typeof fetchImpl !== "function") {
    throw new Error("fetch implementation is required");
  }
  const rootUrl = normalizeWebLiveUrl(target);

  const index = await fetchRequired(
    rootUrl,
    "./",
    { text: true, contentType: "text/html" },
    fetchImpl
  );
  requireContains(index.text, '<title>Orbifold</title>', "index.html");
  requireWebIndexHtml(index.text);
  requireContains(index.text, "static fallback", "index.html");

  const js = await fetchRequired(rootUrl, "./pkg/orbifold_web.js", { text: true }, fetchImpl);
  requireContains(js.text, "orbifold_web_bg.wasm", "orbifold_web.js");
  requireContains(js.text, "start_orbifold", "orbifold_web.js");

  const wasm = await fetchRequired(rootUrl, "./pkg/orbifold_web_bg.wasm", {}, fetchImpl);
  if (
    wasm.bytes[0] !== 0x00 ||
    wasm.bytes[1] !== 0x61 ||
    wasm.bytes[2] !== 0x73 ||
    wasm.bytes[3] !== 0x6d
  ) {
    throw new Error(`${wasm.url.href} is not a wasm binary`);
  }

  await fetchRequired(rootUrl, "./favicon.ico", {}, fetchImpl);
  await fetchRequired(rootUrl, "./orbifold_icon.png", {}, fetchImpl);

  return rootUrl.href;
}

export function normalizeWebLiveUrl(target) {
  const url = new URL(target);
  url.hash = "";
  url.search = "";
  if (!url.pathname.endsWith("/")) {
    url.pathname = `${url.pathname}/`;
  }
  return url;
}

export async function fetchRequired(rootUrl, relativePath, options = {}, fetchImpl = globalThis.fetch) {
  const url = new URL(relativePath, rootUrl);
  const response = await fetchImpl(url, {
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

function isCliEntrypoint() {
  return process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
}
