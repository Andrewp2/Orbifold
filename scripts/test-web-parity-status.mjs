#!/usr/bin/env node

import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  inspectWebParityStatus,
  parseWebParityStatusArgs,
  printWebParityStatus,
} from "./check-web-parity-status.mjs";

assert.deepEqual(parseWebParityStatusArgs([]), {
  target: "reports",
  expectedUrl: "",
  help: false,
});
assert.deepEqual(parseWebParityStatusArgs(["evidence", "--url", "https://example.invalid/Orbifold/"]), {
  target: "evidence",
  expectedUrl: "https://example.invalid/Orbifold/",
  help: false,
});
assert.deepEqual(parseWebParityStatusArgs(["--url=https://example.invalid/Orbifold/"]), {
  target: "reports",
  expectedUrl: "https://example.invalid/Orbifold/",
  help: false,
});
assert.throws(() => parseWebParityStatusArgs(["reports", "--bogus"]), {
  message: /Unknown argument: --bogus/,
});
assert.throws(() => parseWebParityStatusArgs(["reports", "--url"]), {
  message: /--url requires a value/,
});
assert.throws(() => parseWebParityStatusArgs(["reports", "--url="]), {
  message: /--url requires a value/,
});

const tempDir = await mkdtemp(path.join(os.tmpdir(), "orbifold-web-parity-status-"));
try {
  const missingStatus = await inspectWebParityStatus(path.join(tempDir, "missing"));
  assert.equal(missingStatus.complete, false);
  assert.equal(missingStatus.manualReport.ok, false);
  assert.match(missingStatus.manualReport.error, /no such file or directory|ENOENT/);
  assert.equal(missingStatus.completionReport.ok, false);

  const reportsDir = path.join(tempDir, "reports");
  const visualRunDir = path.join(tempDir, "screenshots", "final", "2026-05-18T120500Z");
  await mkdir(reportsDir, { recursive: true });
  await mkdir(visualRunDir, { recursive: true });
  const manualReport = validManualReport();
  const manualPath = path.join(reportsDir, "web-manual-devices-2026-05-18T120000Z.json");
  const gatePath = path.join(reportsDir, "web-parity-gate-2026-05-18T120500Z.json");
  const manifestPath = path.join(visualRunDir, "manifest.json");
  const gateReport = validGateReport(manualReport, visualRunDir, manifestPath);
  const visualManifest = validVisualManifest(manualReport.targetUrl, (label) =>
    path.join(visualRunDir, `${label}.svg`)
  );
  for (const capture of visualManifest.captures) {
    await writeFile(capture.snapshot, "<svg><text>Orbifold visual evidence</text></svg>\n");
  }
  await writeFile(manualPath, `${JSON.stringify(manualReport, null, 2)}\n`);
  await writeFile(manifestPath, `${JSON.stringify(visualManifest, null, 2)}\n`);
  await writeFile(gatePath, `${JSON.stringify(gateReport, null, 2)}\n`);

  const completeStatus = await inspectWebParityStatus(reportsDir);
  assert.equal(completeStatus.complete, true);
  assert.equal(completeStatus.manualReport.ok, true);
  assert.equal(completeStatus.manualReport.path, manualPath);
  assert.equal(completeStatus.manualReport.checkCount, 13);
  assert.equal(completeStatus.completionReport.ok, true);
  assert.equal(completeStatus.completionReport.path, gatePath);

  const targetedCompleteStatus = await inspectWebParityStatus(reportsDir, {
    expectedUrl: manualReport.targetUrl,
  });
  assert.equal(targetedCompleteStatus.complete, true);
  assert.equal(targetedCompleteStatus.expectedUrl, manualReport.targetUrl);

  const wrongTargetStatus = await inspectWebParityStatus(reportsDir, {
    expectedUrl: "https://example.invalid/Other/",
  });
  assert.equal(wrongTargetStatus.complete, false);
  assert.equal(wrongTargetStatus.manualReport.ok, false);
  assert.match(wrongTargetStatus.manualReport.error, /does not match expected/);
  assert.equal(wrongTargetStatus.completionReport.ok, false);
  assert.match(wrongTargetStatus.completionReport.error, /targetUrl expected/);

  const statusOutput = captureConsole(() => printWebParityStatus(wrongTargetStatus));
  assert.match(
    statusOutput,
    /scripts\/check-web-manual-devices\.mjs https:\/\/example\.invalid\/Other\/ --finalize/
  );
  assert.match(statusOutput, /real Web Audio output, Web MIDI hardware/);
  assert.match(statusOutput, /file-flow, shortcut, and piano-roll checks/);
  assert.match(statusOutput, /validate the manual report and final parity gate separately/);
} finally {
  await rm(tempDir, { recursive: true, force: true });
}

console.log("web parity status behavior ok");

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
      before: fileFlowState({
        navigationType: "navigate",
        timeOrigin: 1000,
        frameCount: 10,
      }),
      after: fileFlowState({
        navigationType: "reload",
        timeOrigin: 2000,
        frameCount: 12,
      }),
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
      afterRealMidiNote: {
        lastMidiStatus: 144,
        lastMidiNote: 60,
      },
      afterMidiRecording: {
        noteCount: 3,
      },
      beforeBrowserFileFlows: fileFlowState({
        navigationType: "navigate",
        timeOrigin: 1000,
        frameCount: 10,
      }),
      afterBrowserFileFlows: {
        frameCount: 10,
      },
      beforeShortcutParity: {
        frameCount: 10,
      },
      afterShortcutParity: {
        frameCount: 11,
      },
      beforePianoRollParity: {
        frameCount: 12,
      },
      afterPianoRollParity: {
        frameCount: 13,
      },
    },
  };
}

function validGateReport(sourceManualReport, visualRunDir, manifestPath) {
  const generatedAt = "2026-05-18T12:05:00.000Z";
  return {
    schema: "orbifold.web_parity_gate.v1",
    generatedAt,
    targetUrl: sourceManualReport.targetUrl,
    manualReport: "reports",
    manualReportPath: "web-manual-devices-2026-05-18T120000Z.json",
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
        `Orbifold web visual captures wrote ${visualRunDir}\n- manifest: ${manifestPath}`
      ),
    ],
  };
}

function validVisualManifest(targetUrl, evidencePath) {
  return {
    target: targetUrl,
    capturedAt: "2026-05-18T12:05:00.000Z",
    failures: [],
    captures: [
      capture("compact-1200x760", 1200, 760, 1, evidencePath),
      capture("desktop-1600x1000", 1600, 1000, 1, evidencePath),
      capture("hidpi-1920x1080-dpr2", 1920, 1080, 2, evidencePath),
      capture("wide-3840x2160", 3840, 2160, 1, evidencePath),
    ],
  };
}

function capture(label, width, height, deviceScaleFactor, evidencePath) {
  return {
    label,
    width,
    height,
    deviceScaleFactor,
    mode: "paint-snapshot-svg",
    snapshot: evidencePath(label),
    state: {
      className: "runtime-ready",
      frameCount: 2,
      canvasClientWidth: width,
      canvasClientHeight: height,
      canvasWidth: width * deviceScaleFactor,
      canvasHeight: height * deviceScaleFactor,
      visualSnapshotReady: "1",
      visualSnapshotBytes: 2000,
    },
    snapshotStats: {
      bytes: 2000,
      itemCount: 20,
      unsupportedCount: 0,
    },
  };
}

function fileFlowState({ navigationType, timeOrigin, frameCount }) {
  return {
    frameCount,
    locationHref: "https://example.invalid/Orbifold/",
    navigationType,
    timeOrigin,
    project: "orbifold_project=1\n",
    settings: "ui_scale=1\n",
    assetCount: 1,
    scaleDescription: "Browser 5-EDO",
    scalaPath: "browser_5_edo.scl",
    lumatonePath: "classic.ltn",
    lumatoneLoaded: true,
    downloadFileName: "project.orbifold",
    downloadSize: 128,
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

function captureConsole(callback) {
  const lines = [];
  const originalLog = console.log;
  console.log = (...args) => lines.push(args.join(" "));
  try {
    callback();
  } finally {
    console.log = originalLog;
  }
  return lines.join("\n");
}
