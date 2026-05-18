#!/usr/bin/env node

import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { checkWebDist, requireWebIndexHtml } from "./check-web-dist.mjs";
import { checkWebLive, normalizeWebLiveUrl } from "./check-web-live.mjs";

const tempDir = await mkdtemp(path.join(os.tmpdir(), "orbifold-web-artifact-checks-"));

try {
  const distDir = path.join(tempDir, "dist");
  await writeDistFixture(distDir, webIndexHtml());

  await checkWebDist(distDir);
  assert.doesNotThrow(() => requireWebIndexHtml(webIndexHtml()));

  await assert.rejects(
    checkWebDist(path.join(tempDir, "missing")),
    /ENOENT|no such file or directory/
  );

  await rm(path.join(distDir, "pkg", "orbifold_web_bg.wasm"));
  await assert.rejects(checkWebDist(distDir), /missing pkg\/orbifold_web_bg\.wasm/);

  await writeDistFixture(distDir, webIndexHtml({ absolutePath: true }));
  await assert.rejects(checkWebDist(distDir), /index\.html should not contain href="\//);

  assert.equal(
    normalizeWebLiveUrl("https://example.invalid/Orbifold?old=1#section").href,
    "https://example.invalid/Orbifold/"
  );

  assert.equal(
    await checkWebLive("https://example.invalid/Orbifold", mockFetch()),
    "https://example.invalid/Orbifold/"
  );

  await assert.rejects(
    checkWebLive("https://example.invalid/Orbifold", mockFetch({ wasmBytes: [1, 2, 3, 4] })),
    /is not a wasm binary/
  );

  await assert.rejects(
    checkWebLive(
      "https://example.invalid/Orbifold",
      mockFetch({ indexContentType: "text/plain" })
    ),
    /returned content-type text\/plain, expected text\/html/
  );

  await assert.rejects(
    checkWebLive(
      "https://example.invalid/Orbifold",
      mockFetch({ missingPath: "/Orbifold/pkg/orbifold_web.js" })
    ),
    /returned HTTP 404/
  );

  console.log("web artifact checks behavior ok");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}

async function writeDistFixture(distDir, html) {
  await rm(distDir, { recursive: true, force: true });
  await mkdir(path.join(distDir, "pkg"), { recursive: true });
  await writeFile(path.join(distDir, "index.html"), html);
  await writeFile(
    path.join(distDir, "pkg", "orbifold_web.js"),
    "start_orbifold orbifold_web_bg.wasm"
  );
  await writeFile(
    path.join(distDir, "pkg", "orbifold_web_bg.wasm"),
    new Uint8Array([0, 0x61, 0x73, 0x6d])
  );
  await writeFile(path.join(distDir, "favicon.ico"), new Uint8Array([1]));
  await writeFile(path.join(distDir, "orbifold_icon.png"), new Uint8Array([2]));
  await writeFile(path.join(distDir, ".nojekyll"), "");
}

function mockFetch(options = {}) {
  const files = new Map([
    [
      "/Orbifold/",
      {
        body: webIndexHtml(),
        contentType: options.indexContentType ?? "text/html; charset=utf-8",
      },
    ],
    [
      "/Orbifold/pkg/orbifold_web.js",
      { body: "start_orbifold orbifold_web_bg.wasm", contentType: "text/javascript" },
    ],
    [
      "/Orbifold/pkg/orbifold_web_bg.wasm",
      {
        body: new Uint8Array(options.wasmBytes ?? [0, 0x61, 0x73, 0x6d, 1]),
        contentType: "application/wasm",
      },
    ],
    ["/Orbifold/favicon.ico", { body: new Uint8Array([1]), contentType: "image/x-icon" }],
    ["/Orbifold/orbifold_icon.png", { body: new Uint8Array([2]), contentType: "image/png" }],
  ]);

  return async (url) => {
    const pathname = new URL(url).pathname;
    if (pathname === options.missingPath || !files.has(pathname)) {
      return new Response("missing", { status: 404 });
    }
    const file = files.get(pathname);
    return new Response(file.body, {
      status: 200,
      headers: { "content-type": file.contentType },
    });
  };
}

function webIndexHtml(options = {}) {
  const extraAbsoluteLink = options.absolutePath
    ? '<link rel="stylesheet" href="/absolute.css" />'
    : "";
  return `<!doctype html>
<html>
  <head>
    <title>Orbifold</title>
    <link rel="icon" href="./favicon.ico" sizes="any" />
    <link rel="icon" type="image/png" sizes="64x64" href="./orbifold_icon.png" />
    ${extraAbsoluteLink}
  </head>
  <body>
    <div>static fallback</div>
    <script>
      window.orbifoldRuntimeReady = () => document.body.classList.add("runtime-ready");
      document.body.classList.add("runtime-failed");
    </script>
    <script type="module">
      import init, { start_orbifold } from "./pkg/orbifold_web.js";
      await init();
      start_orbifold();
    </script>
  </body>
</html>`;
}
