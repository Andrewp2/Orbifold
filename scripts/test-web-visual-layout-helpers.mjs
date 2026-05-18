#!/usr/bin/env node

import assert from "node:assert/strict";
import zlib from "node:zlib";
import {
  browserFailures,
  isReadyForLayoutCheck,
  layoutFailures,
  urlForViewport as layoutUrlForViewport,
} from "./check-web-layout.mjs";
import {
  isReadyForCapture,
  parseArgs as parseVisualArgs,
  pngStats,
  screenshotFallbackReason,
  timestampForPath,
  urlForViewport as visualUrlForViewport,
  visualCaptureFailures,
} from "./capture-web-visuals.mjs";

const viewport = { label: "desktop-1600x1000", width: 1600, height: 1000, deviceScaleFactor: 1 };
const goodLayout = layoutState();

assert.equal(
  layoutUrlForViewport("https://example.invalid/Orbifold/?old=1", "wide-3840x2160"),
  "https://example.invalid/Orbifold/?old=1&orbifold_layout=wide-3840x2160"
);
assert.deepEqual(layoutFailures(goodLayout, viewport), []);
assert.equal(isReadyForLayoutCheck(goodLayout, viewport), true);
assert.equal(isReadyForLayoutCheck({ ...goodLayout, textAuditReady: "" }, viewport), false);
assert.deepEqual(
  layoutFailures(
    {
      ...goodLayout,
      canvasClientWidth: 400,
      textAuditIssueCount: 1,
      textAuditSampleIssue: "top bar overlap",
      documentScrollWidth: 1700,
    },
    viewport
  ),
  [
    "canvasClientWidth 400 < 1598",
    "textAuditIssueCount 1 !== 0",
    "textAuditSampleIssue top bar overlap",
    "documentScrollWidth 1700 > 1601",
  ]
);

assert.deepEqual(
  browserFailures([
    { method: "Runtime.consoleAPICalled", type: "error", text: "renderer failed" },
    { method: "Runtime.consoleAPICalled", type: "log", text: "ignored" },
    { method: "Runtime.exceptionThrown", text: "panic" },
    { method: "Network.loadingFailed", url: "https://example.invalid/pkg/app.wasm" },
    { method: "Log.entryAdded", level: "error", text: "gpu failed" },
  ]),
  [
    "console error: renderer failed",
    "exception: panic",
    "network load failed: https://example.invalid/pkg/app.wasm",
    "browser log error: gpu failed",
  ]
);

assert.deepEqual(parseVisualArgs(["https://example.invalid/Orbifold"]), {
  target: "https://example.invalid/Orbifold",
  outDir: "screenshots/web",
});
assert.deepEqual(
  parseVisualArgs([
    "https://example.invalid/Orbifold",
    "--out",
    "screenshots/final",
  ]),
  {
    target: "https://example.invalid/Orbifold",
    outDir: "screenshots/final",
  }
);
assert.deepEqual(parseVisualArgs(["--out=screenshots/final"]), {
  target: "",
  outDir: "screenshots/final",
});
assert.match(timestampForPath(), /^\d{4}-\d{2}-\d{2}T\d{6}Z$/);
assert.equal(
  visualUrlForViewport("https://example.invalid/Orbifold/?old=1", "compact-1200x760"),
  "https://example.invalid/Orbifold/?old=1&orbifold_visual=compact-1200x760"
);

const captureState = {
  ...goodLayout,
  visualSnapshotReady: "1",
  visualSnapshotBytes: 1200,
};
assert.equal(isReadyForCapture(captureState, viewport), true);
assert.equal(isReadyForCapture({ ...captureState, visualSnapshotBytes: 999 }, viewport), false);

assert.equal(screenshotFallbackReason(null), "screenshot statistics unavailable");
assert.equal(
  screenshotFallbackReason({ nonTransparentPixels: 0, rgbRange: 10 }),
  "screenshot is blank/transparent"
);
assert.equal(
  screenshotFallbackReason({ nonTransparentPixels: 1, rgbRange: 0 }),
  "screenshot has no color variation"
);
assert.equal(
  screenshotFallbackReason({ nonTransparentPixels: 1, rgbRange: 10 }),
  "screenshot could not be used as visual evidence"
);

const stats = pngStats(
  makeRgbaPng(2, 1, [
    [0, 0, 0, 0],
    [255, 128, 64, 255],
  ])
);
assert.deepEqual(stats, {
  width: 2,
  height: 1,
  colorType: 6,
  bitDepth: 8,
  nonTransparentPixels: 1,
  alphaRange: 255,
  rgbRange: 255,
});
assert.equal(pngStats(Buffer.from("not a png")), null);

assert.deepEqual(
  visualCaptureFailures([
    { method: "Runtime.consoleAPICalled", type: "assert", text: "bad invariant" },
    { method: "Runtime.consoleAPICalled", type: "warning", text: "ignored" },
    { method: "Runtime.exceptionThrown", text: "", description: "TypeError" },
    { method: "Network.loadingFailed", errorText: "blocked" },
    { method: "Log.entryAdded", level: "error", text: "webgpu failed" },
  ]),
  [
    "console assert: bad invariant",
    "exception: TypeError",
    "network load failed: blocked",
    "browser log error: webgpu failed",
  ]
);

console.log("web visual layout helper behavior ok");

function layoutState() {
  return {
    className: "runtime-ready",
    frameCount: 2,
    viewportWidth: 1600,
    viewportHeight: 1000,
    canvasClientWidth: 1600,
    canvasClientHeight: 1000,
    canvasWidth: 1600,
    canvasHeight: 1000,
    pianoGridWidth: 900,
    pianoGridHeight: 320,
    pianoRollHeight: 300,
    rightPanelWidth: 260,
    textAuditReady: "1",
    textAuditCount: 42,
    textAuditIssueCount: 0,
    textAuditNonFiniteCount: 0,
    textAuditSampleIssue: "",
    canvasLeft: 0,
    canvasTop: 0,
    canvasRectWidth: 1600,
    canvasRectHeight: 1000,
    documentScrollWidth: 1600,
    documentScrollHeight: 1000,
    bodyScrollWidth: 1600,
    bodyScrollHeight: 1000,
  };
}

function makeRgbaPng(width, height, pixels) {
  const bytesPerPixel = 4;
  const rowBytes = width * bytesPerPixel;
  const rows = [];
  for (let y = 0; y < height; y += 1) {
    const row = Buffer.alloc(1 + rowBytes);
    row[0] = 0;
    for (let x = 0; x < width; x += 1) {
      const pixel = pixels[y * width + x];
      const offset = 1 + x * bytesPerPixel;
      row[offset] = pixel[0];
      row[offset + 1] = pixel[1];
      row[offset + 2] = pixel[2];
      row[offset + 3] = pixel[3];
    }
    rows.push(row);
  }

  return Buffer.concat([
    Buffer.from("89504e470d0a1a0a", "hex"),
    pngChunk("IHDR", ihdr(width, height)),
    pngChunk("IDAT", zlib.deflateSync(Buffer.concat(rows))),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);
}

function ihdr(width, height) {
  const data = Buffer.alloc(13);
  data.writeUInt32BE(width, 0);
  data.writeUInt32BE(height, 4);
  data[8] = 8;
  data[9] = 6;
  return data;
}

function pngChunk(type, data) {
  const chunk = Buffer.alloc(12 + data.length);
  chunk.writeUInt32BE(data.length, 0);
  chunk.write(type, 4, 4, "ascii");
  data.copy(chunk, 8);
  return chunk;
}
