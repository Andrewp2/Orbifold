#!/usr/bin/env node

import assert from "node:assert/strict";
import {
  createManualDeviceReport,
  manualUrlSecurityEvidence,
  manualDeviceFinalizerCommands,
  manualDeviceNextStepLines,
  parseManualDeviceArgs,
  persistedNoteCount,
  printManualDevicePreflight,
} from "./check-web-manual-devices.mjs";

assert.deepEqual(parseManualDeviceArgs(["https://example.invalid/Orbifold/"]), {
  url: "https://example.invalid/Orbifold/",
  outDir: "reports",
  keepOpen: false,
  preflight: false,
  finalize: false,
});

assert.deepEqual(
  parseManualDeviceArgs([
    "https://example.invalid/Orbifold/",
    "--out",
    "manual-reports",
    "--keep-open",
    "--preflight",
    "--finalize",
  ]),
  {
    url: "https://example.invalid/Orbifold/",
    outDir: "manual-reports",
    keepOpen: true,
    preflight: true,
    finalize: true,
  }
);

assert.deepEqual(parseManualDeviceArgs(["https://example.invalid/Orbifold/", "--out=manual-reports"]), {
  url: "https://example.invalid/Orbifold/",
  outDir: "manual-reports",
  keepOpen: false,
  preflight: false,
  finalize: false,
});

assert.deepEqual(parseManualDeviceArgs(["--help"]), {
  url: null,
  outDir: "reports",
  keepOpen: false,
  preflight: false,
  finalize: false,
});

assert.throws(() => parseManualDeviceArgs(["https://example.invalid/Orbifold/", "--bogus"]), {
  message: /Unknown argument: --bogus/,
});
assert.throws(() => parseManualDeviceArgs(["https://example.invalid/Orbifold/", "--out"]), {
  message: /--out requires a value/,
});
assert.throws(() => parseManualDeviceArgs(["https://example.invalid/Orbifold/", "--out="]), {
  message: /--out requires a value/,
});

assert.equal(manualUrlSecurityEvidence("https://example.invalid/Orbifold/").passed, true);
assert.equal(manualUrlSecurityEvidence("http://localhost:4173/").passed, true);
assert.equal(manualUrlSecurityEvidence("http://127.0.0.1:4173/").passed, true);
assert.equal(manualUrlSecurityEvidence("http://example.invalid/Orbifold/").passed, false);
assert.match(
  manualUrlSecurityEvidence("http://example.invalid/Orbifold/").detail,
  /real Web MIDI requires a browser secure context/
);

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

const finalizerCommands = manualDeviceFinalizerCommands(
  "https://example.invalid/Orbifold/",
  "reports/web-manual-devices-test.json"
);
assert.deepEqual(
  finalizerCommands.map((step) => step.name),
  ["manualReport", "parityGate", "parityComplete"]
);
assert.match(finalizerCommands[0].command.join(" "), /check-web-manual-report\.mjs/);
assert.match(finalizerCommands[1].command.join(" "), /check-web-parity-gate\.mjs/);
assert.match(finalizerCommands[2].command.join(" "), /check-web-parity-complete\.mjs/);
assert(
  finalizerCommands[2].command.includes("https://example.invalid/Orbifold/"),
  "completion command should pin the deployed target URL"
);
assert.equal(
  manualDeviceNextStepLines("https://example.invalid/Orbifold/", "reports/report with space.json")
    .length,
  3
);
assert.match(
  manualDeviceNextStepLines("https://example.invalid/Orbifold/", "reports/report with space.json")[0],
  /'.*report with space\.json'/
);

const preflightOutput = captureConsole(() =>
  printManualDevicePreflight({
    passed: true,
    url: "https://example.invalid/Orbifold/",
    checks: [
      {
        name: "secure-context",
        passed: true,
        detail: "HTTPS target can use browser secure-context APIs",
      },
      {
        name: "chrome",
        passed: true,
        detail: "/usr/bin/google-chrome",
      },
    ],
  })
);
assert.match(preflightOutput, /real Web Audio output, Web MIDI hardware/);
assert.match(preflightOutput, /file-flow, shortcut, and piano-roll checks/);

console.log("manual web device runner behavior ok");

function captureConsole(callback) {
  const originalLog = console.log;
  let output = "";
  console.log = (...args) => {
    output += `${args.join(" ")}\n`;
  };
  try {
    callback();
  } finally {
    console.log = originalLog;
  }
  return output;
}
