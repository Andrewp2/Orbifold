#!/usr/bin/env node

import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  resolveParityCompletionReportPath,
  validateParityCompletionReport,
  validateParityCompletionReportFile,
} from "./check-web-parity-complete.mjs";

const manualReport = validManualReport();
const gateReport = validGateReport(manualReport);
const visualManifest = validVisualManifest(manualReport.targetUrl);

assert.doesNotThrow(() =>
  validateParityCompletionReport(gateReport, { manualReport, visualManifest })
);

assert.throws(() => validateParityCompletionReport(gateReport, { manualReport }), {
  message: /visualManifest should be present/,
});

assertRejects(
  withChange(gateReport, (draft) => {
    draft.passed = false;
  }),
  "passed expected true, got false"
);

assertRejects(
  withChange(gateReport, (draft) => {
    draft.skippedVisualCapture = true;
  }),
  "skippedVisualCapture expected false, got true"
);

assertRejects(
  withChange(gateReport, (draft) => {
    draft.steps = draft.steps.filter((step) => step.name !== "deployedSmoke");
  }),
  "steps.deployedSmoke should be an object"
);

assertRejects(
  withChange(gateReport, (draft) => {
    draft.steps.find((step) => step.name === "deployedVisualCapture").command = ["skipped"];
  }),
  "steps.deployedVisualCapture.command should not be skipped"
);

assertRejects(
  withChange(gateReport, (draft) => {
    draft.steps.find((step) => step.name === "manualReportArtifact").stdoutTail = "";
  }),
  "manualReportArtifact should confirm the manual artifact matches live"
);

assertRejects(
  withChange(gateReport, (draft) => {
    draft.liveArtifact.files.wasm.sha256 = "f".repeat(64);
  }),
  "manual report artifact should match live artifact"
);

assertRejects(gateReport, "visualManifest.failures should be empty", {
  visualManifest: withChange(visualManifest, (draft) => {
    draft.failures = ["console error: boom"];
  }),
});

assertRejects(gateReport, "visualManifest.captures.wide-3840x2160 should be an object", {
  visualManifest: withChange(visualManifest, (draft) => {
    draft.captures = draft.captures.filter((capture) => capture.label !== "wide-3840x2160");
  }),
});

assertRejects(gateReport, "compact-1200x760.state.canvasWidth expected at least", {
  visualManifest: withChange(visualManifest, (draft) => {
    draft.captures[0].state.canvasWidth = 400;
  }),
});

assertRejects(gateReport, "compact-1200x760.snapshotStats.itemCount expected at least 10", {
  visualManifest: withChange(visualManifest, (draft) => {
    draft.captures[0].snapshotStats.itemCount = 1;
  }),
});

const tempDir = await mkdtemp(path.join(os.tmpdir(), "orbifold-web-parity-complete-"));
try {
  const reportsDir = path.join(tempDir, "reports");
  const visualRunDir = path.join(tempDir, "screenshots", "final", "2026-05-18T120500Z");
  await mkdir(reportsDir, { recursive: true });
  await mkdir(visualRunDir, { recursive: true });
  const manualPath = path.join(reportsDir, "web-manual-devices-2026-05-18T120000Z.json");
  const gatePath = path.join(reportsDir, "web-parity-gate-2026-05-18T120500Z.json");
  const visualManifestPath = path.join(visualRunDir, "manifest.json");
  const visualManifestWithFiles = validVisualManifest(manualReport.targetUrl, (label) =>
    path.join(visualRunDir, `${label}.svg`)
  );
  for (const capture of visualManifestWithFiles.captures) {
    await writeFile(capture.snapshot, "<svg><text>Orbifold visual evidence</text></svg>\n");
  }
  await writeFile(visualManifestPath, `${JSON.stringify(visualManifestWithFiles, null, 2)}\n`);

  const gateReportWithFiles = structuredClone(gateReport);
  gateReportWithFiles.manualReportPath = manualPath;
  gateReportWithFiles.visualOut = path.dirname(visualRunDir);
  gateReportWithFiles.steps.find((step) => step.name === "deployedVisualCapture").stdoutTail =
    `Orbifold web visual captures wrote ${visualRunDir}\n- manifest: ${visualManifestPath}\n`;

  await writeFile(manualPath, `${JSON.stringify(manualReport, null, 2)}\n`);
  await writeFile(gatePath, `${JSON.stringify(gateReportWithFiles, null, 2)}\n`);

  assert.equal(await resolveParityCompletionReportPath(reportsDir), gatePath);
  await validateParityCompletionReportFile(gatePath);

  await assert.rejects(
    resolveParityCompletionReportPath(path.join(tempDir, "missing")),
    /no such file or directory|ENOENT/
  );
} finally {
  await rm(tempDir, { recursive: true, force: true });
}

console.log("web parity completion behavior ok");

function assertRejects(candidate, message, optionOverrides = {}) {
  const options = { manualReport, visualManifest, ...optionOverrides };
  assert.throws(() => validateParityCompletionReport(candidate, options), {
    message: new RegExp(escapeRegExp(message)),
  });
}

function withChange(source, change) {
  const draft = structuredClone(source);
  change(draft);
  return draft;
}

function validGateReport(sourceManualReport) {
  const generatedAt = "2026-05-18T12:05:00.000Z";
  return {
    schema: "orbifold.web_parity_gate.v1",
    generatedAt,
    targetUrl: sourceManualReport.targetUrl,
    manualReport: "reports",
    manualReportPath: "reports/web-manual-devices-2026-05-18T120000Z.json",
    visualOut: "screenshots/web-parity",
    skippedVisualCapture: false,
    passed: true,
    liveArtifact: structuredClone(sourceManualReport.artifact),
    steps: [
      step("manualDeviceReport", generatedAt, "manual web device report ok"),
      step("manualReportTarget", generatedAt, "manual report target https://example.invalid/Orbifold/"),
      step(
        "manualReportArtifact",
        generatedAt,
        "manual report artifact matches live https://example.invalid/Orbifold/"
      ),
      step("deployedArtifact", generatedAt, "live web artifact ok"),
      step("deployedLayout", generatedAt, "Orbifold web layout checks passed"),
      step("deployedSmoke", generatedAt, "Orbifold web smoke passed"),
      step(
        "deployedVisualCapture",
        generatedAt,
        "Orbifold web visual captures wrote screenshots/final/2026-05-18T120500Z\n- manifest: screenshots/final/2026-05-18T120500Z/manifest.json"
      ),
    ],
  };
}

function step(name, at, stdoutTail) {
  return {
    name,
    command: ["node", `${name}.mjs`],
    startedAt: at,
    endedAt: at,
    exitCode: 0,
    signal: null,
    stdoutTail,
    stderrTail: "",
  };
}

function validManualReport() {
  const generatedAt = "2026-05-18T12:00:00.000Z";
  const targetUrl = "https://example.invalid/Orbifold/";
  const checks = [
    "browserRuntimeReady",
    "manualVisualInspection",
    "webAudioOutputsDiscovered",
    "webAudioConnectedState",
    "manualAudibleWebAudio",
    "webMidiInputsDiscovered",
    "webMidiConnectedState",
    "manualDeviceVerifierCompleted",
  ].map((name) => ({ name, pass: true, evidence: {} }));
  checks.push({
    name: "manualRealMidiInput",
    pass: true,
    evidence: {
      before: { lastMidiStatus: 0, lastMidiNote: -1 },
      after: { lastMidiStatus: 144, lastMidiNote: 60 },
    },
  });
  checks.push({
    name: "manualRealMidiRecording",
    pass: true,
    evidence: { beforeNoteCount: 2, afterNoteCount: 3 },
  });

  return {
    schema: "orbifold.web_manual_device_parity.v1",
    generatedAt,
    targetUrl,
    host: { platform: "linux", arch: "x64", release: "test" },
    chrome: {
      path: "/usr/bin/google-chrome",
      version: "Chrome/126.0.0.0",
      userAgent: "Mozilla/5.0 Chrome/126.0.0.0",
      protocolVersion: "1.3",
    },
    artifact: artifactFingerprint(targetUrl, generatedAt),
    checks,
    clicks: [
      click("viewDevices", 20, 20, generatedAt),
      click("audioRefresh", 30, 30, generatedAt),
      click("audioConnect", 40, 40, generatedAt),
      click("audioTestA4", 50, 50, generatedAt),
      click("midiRefresh", 60, 60, generatedAt),
      click("midiRefresh", 61, 60, generatedAt),
      click("midiConnect", 70, 70, generatedAt),
      click("record", 80, 80, generatedAt),
      click("record", 81, 80, generatedAt),
    ],
    browserEvents: [],
    userConfirmations: {
      visualInspection: true,
      audibleA4: true,
      realMidiNoteVisible: true,
      realMidiRecordingVisible: true,
    },
    states: {
      runtime: { hasGpu: true, canvasWidth: 1600, canvasHeight: 1000 },
      afterAudioRefresh: {
        audioOutputCount: 1,
        browserAudioOutputNames: "Default browser audio output",
      },
      afterAudioConnect: {
        audioStreamConnected: true,
        audioContextCreated: true,
        audioProcessorAttached: true,
        audioResumeRequested: true,
        audioResumeResolved: true,
      },
      afterAudioTest: {
        audioNonzero: true,
        audioCallbackCount: 1,
        audioFrameCount: 1024,
        audioPeak: 0.2,
      },
      afterMidiRefresh: {
        midiInputCount: 1,
        browserMidiInputNames: "Hardware MIDI",
      },
      afterMidiConnect: {
        connectedMidiInput: "Hardware MIDI",
        midiInputConnection: "open",
      },
      afterRealMidiNote: { lastMidiStatus: 144, lastMidiNote: 60 },
      afterMidiRecording: { noteCount: 3 },
    },
  };
}

function click(name, x, y, at) {
  return { name, point: { x, y }, at };
}

function artifactFingerprint(rootUrl, generatedAt) {
  return {
    schema: "orbifold.web_artifact_fingerprint.v1",
    rootUrl,
    generatedAt,
    files: Object.fromEntries(
      ["index", "js", "wasm", "favicon", "icon"].map((name, index) => [
        name,
        {
          url: `${rootUrl}${name}`,
          bytes: index + 1,
          sha256: `${index}`.repeat(64),
        },
      ])
    ),
  };
}

function validVisualManifest(targetUrl, snapshotPathForLabel = (label) => `${label}.svg`) {
  const capturedAt = "2026-05-18T12:05:00.000Z";
  return {
    target: targetUrl,
    capturedAt,
    chrome: "/usr/bin/google-chrome",
    captures: [
      visualCapture("compact-1200x760", 1200, 760, 1, snapshotPathForLabel),
      visualCapture("desktop-1600x1000", 1600, 1000, 1, snapshotPathForLabel),
      visualCapture("hidpi-1920x1080-dpr2", 1920, 1080, 2, snapshotPathForLabel),
      visualCapture("wide-3840x2160", 3840, 2160, 1, snapshotPathForLabel),
    ],
    events: [],
    failures: [],
  };
}

function visualCapture(label, width, height, deviceScaleFactor, snapshotPathForLabel) {
  return {
    label,
    width,
    height,
    deviceScaleFactor,
    mode: "paint-snapshot-svg",
    snapshot: snapshotPathForLabel(label),
    screenshotFallback: "screenshot is blank/transparent",
    imageStats: {
      width,
      height,
      colorType: 6,
      bitDepth: 8,
      nonTransparentPixels: 0,
      alphaRange: 0,
      rgbRange: 0,
    },
    screenshotAttempts: [],
    snapshotStats: { bytes: 4096, itemCount: 128, unsupportedCount: 2 },
    state: {
      title: "Orbifold",
      className: "runtime-ready",
      frameCount: 8,
      viewportWidth: width,
      viewportHeight: height,
      canvasClientWidth: width,
      canvasClientHeight: height,
      canvasWidth: width * deviceScaleFactor,
      canvasHeight: height * deviceScaleFactor,
      visualSnapshotReady: "1",
      visualSnapshotBytes: 4096,
    },
  };
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
