#!/usr/bin/env node

import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  parseParityCompletionArgs,
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
assert.deepEqual(parseParityCompletionArgs([]), {
  target: "reports",
  expectedUrl: "",
  help: false,
});
assert.deepEqual(parseParityCompletionArgs(["reports/final.json", "--url", manualReport.targetUrl]), {
  target: "reports/final.json",
  expectedUrl: manualReport.targetUrl,
  help: false,
});
assert.deepEqual(parseParityCompletionArgs(["reports", `--url=${manualReport.targetUrl}`]), {
  target: "reports",
  expectedUrl: manualReport.targetUrl,
  help: false,
});
assert.throws(() => parseParityCompletionArgs(["reports", "--bogus"]), {
  message: /Unknown argument: --bogus/,
});
assert.throws(() => parseParityCompletionArgs(["reports", "--url"]), {
  message: /--url requires a value/,
});
assert.throws(() => parseParityCompletionArgs(["reports", "--url="]), {
  message: /--url requires a value/,
});

assert.throws(() => validateParityCompletionReport(gateReport, { manualReport }), {
  message: /visualManifest should be present/,
});

assertRejects(
  gateReport,
  'targetUrl expected "https://example.invalid/Other/"',
  { expectedUrl: "https://example.invalid/Other/" }
);

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
    draft.steps.push(step("diagnosticExtra", draft.generatedAt, "extra diagnostic failed"));
    draft.steps.at(-1).exitCode = 1;
  }),
  "steps.diagnosticExtra.exitCode expected 0"
);

assertRejects(
  withChange(gateReport, (draft) => {
    draft.steps.push(structuredClone(draft.steps[0]));
  }),
  `steps.${gateReport.steps[0].name} should appear exactly once`
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
  await validateParityCompletionReportFile(gatePath, { expectedUrl: manualReport.targetUrl });
  await assert.rejects(
    validateParityCompletionReportFile(gatePath, {
      expectedUrl: "https://example.invalid/Other/",
    }),
    /targetUrl expected/
  );

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
    visualOut: "reports/web-visuals",
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
  const initialVisual = visualState({ width: 1600, height: 1000, frameCount: 10 });
  const largeVisual = visualState({ width: 2400, height: 1400, frameCount: 11 });
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
  checks.find((check) => check.name === "manualVisualInspection").evidence = {
    initial: initialVisual,
    inspectedLarge: largeVisual,
  };
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
  checks.push({
    name: "manualBrowserFileFlows",
    pass: true,
    evidence: {
      downloadFileName: "project.orbifold",
      downloadSize: 128,
      project: "orbifold_project=1\n",
      assetCount: 1,
      scaleDescription: "Browser 5-EDO",
      scalaPath: "browser_5_edo.scl",
      lumatonePath: "classic.ltn",
      lumatoneLoaded: true,
    },
  });
  checks.push({
    name: "manualShortcutParity",
    pass: true,
    evidence: {
      requiredWorkflows: ["transport", "editing", "file", "help", "uiZoom"],
      before: {
        frameCount: 10,
        lastAction: "file.save",
        lastStatus: "Saved project",
        noteCount: 3,
        transportPlaying: false,
        uiScale: 1,
        downloadFileName: "project.orbifold",
        downloadSize: 128,
        project: "orbifold_project=1\nnote\t1\t0\t0.25\t0\t60\t0\t60\t100\n",
      },
      after: {
        frameCount: 11,
        lastAction: "ui.scale_up",
        lastStatus: "UI scale 110%",
        noteCount: 3,
        transportPlaying: false,
        uiScale: 1.1,
        downloadFileName: "project.orbifold",
        downloadSize: 128,
        project: "orbifold_project=1\nnote\t1\t0\t0.25\t0\t60\t0\t60\t100\n",
      },
    },
  });
  checks.push({
    name: "manualPianoRollParity",
    pass: true,
    evidence: {
      requiredWorkflows: ["noteEdit", "velocityEdit", "scrollOrZoom", "seekOrLoop", "panelResize"],
      before: {
        frameCount: 12,
        noteCount: 3,
        project: "orbifold_project=1\nnote\t1\t0\t0.25\t0\t60\t0\t60\t100\n",
        transportPositionBeats: 0,
        loopBeats: 16,
        pianoViewStart: 0,
        pianoViewBeats: 16,
        pianoGridWidth: 800,
        pianoGridHeight: 420,
        pianoRollHeight: 500,
        rightPanelWidth: 300,
      },
      after: {
        frameCount: 13,
        noteCount: 4,
        project: "orbifold_project=1\nnote\t1\t1\t0.5\t0\t60\t0\t60\t80\n",
        transportPositionBeats: 2,
        loopBeats: 12,
        pianoViewStart: 1,
        pianoViewBeats: 8,
        pianoGridWidth: 800,
        pianoGridHeight: 420,
        pianoRollHeight: 560,
        rightPanelWidth: 340,
      },
    },
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
      browserFileFlows: true,
      shortcutParity: true,
      pianoRollParity: true,
    },
    states: {
      runtime: { hasGpu: true, canvasWidth: 1600, canvasHeight: 1000 },
      beforeVisualInspection: initialVisual,
      afterLargeVisualInspection: largeVisual,
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
      afterBrowserFileFlows: { frameCount: 10 },
      beforeShortcutParity: { frameCount: 10 },
      afterShortcutParity: { frameCount: 11 },
      beforePianoRollParity: { frameCount: 12 },
      afterPianoRollParity: { frameCount: 13 },
    },
  };
}

function visualState({ width, height, frameCount }) {
  return {
    className: "runtime-ready",
    frameCount,
    viewportWidth: width,
    viewportHeight: height,
    uiScale: width >= 2400 ? 1.5 : 1,
    devicePixelRatio: 1,
    innerWidth: width,
    innerHeight: height,
    documentScrollWidth: width,
    documentScrollHeight: height,
    canvasClientWidth: width,
    canvasClientHeight: height,
    canvasWidth: width,
    canvasHeight: height,
    canvasLeft: 0,
    canvasTop: 0,
    canvasRectWidth: width,
    canvasRectHeight: height,
    textAuditReady: "1",
    textAuditCount: 64,
    textAuditIssueCount: 0,
    textAuditNonFiniteCount: 0,
    textAuditSampleIssue: "",
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
