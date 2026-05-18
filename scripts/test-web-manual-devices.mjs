#!/usr/bin/env node

import assert from "node:assert/strict";
import {
  createManualDeviceReport,
  parseManualDeviceArgs,
  persistedNoteCount,
} from "./check-web-manual-devices.mjs";

assert.deepEqual(parseManualDeviceArgs(["https://example.invalid/Orbifold/"]), {
  url: "https://example.invalid/Orbifold/",
  outDir: "reports",
  keepOpen: false,
  preflight: false,
});

assert.deepEqual(
  parseManualDeviceArgs([
    "https://example.invalid/Orbifold/",
    "--out",
    "manual-reports",
    "--keep-open",
    "--preflight",
  ]),
  {
    url: "https://example.invalid/Orbifold/",
    outDir: "manual-reports",
    keepOpen: true,
    preflight: true,
  }
);

assert.deepEqual(parseManualDeviceArgs(["--help"]), {
  url: null,
  outDir: "reports",
  keepOpen: false,
  preflight: false,
});

assert.throws(() => parseManualDeviceArgs(["https://example.invalid/Orbifold/", "--bogus"]), {
  message: /Unknown argument: --bogus/,
});

assert.equal(persistedNoteCount("orbifold_project=1\nnote\t1\nnote\t2\n"), 2);
assert.equal(persistedNoteCount("orbifold_project=1\n"), 0);
assert.equal(persistedNoteCount(null), 0);

const report = createManualDeviceReport(
  "https://example.invalid/Orbifold/",
  "/usr/bin/google-chrome",
  "2026-05-18T12:00:00.000Z"
);
assert.equal(report.schema, "orbifold.web_manual_device_parity.v1");
assert.equal(report.generatedAt, "2026-05-18T12:00:00.000Z");
assert.equal(report.targetUrl, "https://example.invalid/Orbifold/");
assert.equal(report.host.platform, process.platform);
assert.equal(report.host.arch, process.arch);
assert.equal(typeof report.host.release, "string");
assert.equal(report.chrome.path, "/usr/bin/google-chrome");
assert.deepEqual(report.checks, []);
assert.deepEqual(report.clicks, []);
assert.deepEqual(report.states, {});
assert.deepEqual(report.userConfirmations, {});
assert.deepEqual(report.browserEvents, []);

console.log("manual web device runner behavior ok");
