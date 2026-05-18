#!/usr/bin/env node

import assert from "node:assert/strict";
import { parseManualReportArgs, validateManualDeviceReport } from "./check-web-manual-report.mjs";

const report = validManualReport();

assert.deepEqual(parseManualReportArgs([]), {
  target: "reports",
  help: false,
});
assert.deepEqual(parseManualReportArgs(["reports/web-manual-devices-test.json"]), {
  target: "reports/web-manual-devices-test.json",
  help: false,
});
assert.deepEqual(parseManualReportArgs(["--help"]), {
  target: "reports",
  help: true,
});
assert.throws(() => parseManualReportArgs(["reports", "--bogus"]), {
  message: /Unknown argument: --bogus/,
});

assert.doesNotThrow(() => validateManualDeviceReport(report));

assertRejects(
  withChange(report, (draft) => {
    draft.checks.push({ name: "diagnosticExtra", pass: false, evidence: {} });
  }),
  "checks.diagnosticExtra.pass expected true"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.push(structuredClone(draft.checks[0]));
  }),
  `checks.${report.checks[0].name} should appear exactly once`
);

assertRejects(
  withChange(report, (draft) => {
    draft.browserEvents.push({
      method: "Runtime.consoleAPICalled",
      params: {
        type: "error",
        args: [{ value: "renderer failed" }],
      },
    });
  }),
  "browserEvents should not contain runtime errors"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find(
      (check) => check.name === "manualVisualInspection"
    ).evidence.inspectedLarge.textAuditIssueCount = 1;
  }),
  "manualVisualInspection.inspectedLarge.textAuditIssueCount expected 0"
);

assertRejects(
  withChange(report, (draft) => {
    const evidence = draft.checks.find((check) => check.name === "manualVisualInspection").evidence;
    evidence.inspectedLarge = structuredClone(evidence.initial);
  }),
  "manualVisualInspection evidence should show a resize or high-DPI checkpoint"
);

assertRejects(
  withChange(report, (draft) => {
    draft.clicks = draft.clicks.filter((click) => click.name !== "record");
  }),
  "clicks.record expected at least 2"
);

assertRejects(
  withChange(report, (draft) => {
    draft.artifact.rootUrl = "https://example.invalid/other/";
  }),
  "artifact.rootUrl expected"
);

assertRejects(
  withChange(report, (draft) => {
    draft.states.afterAudioTest.audioNonzero = false;
  }),
  "states.afterAudioTest.audioNonzero expected true"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find((check) => check.name === "manualRealMidiInput").evidence.after = {
      lastMidiStatus: 0,
      lastMidiNote: -1,
    };
  }),
  "manualRealMidiInput evidence should show a changed MIDI status or note"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find((check) => check.name === "manualRealMidiRecording").evidence.afterNoteCount =
      2;
  }),
  "manualRealMidiRecording evidence should show a new recorded note"
);

assertRejects(
  withChange(report, (draft) => {
    draft.userConfirmations.browserFileFlows = false;
  }),
  "browserFileFlows expected true"
);

assertRejects(
  withChange(report, (draft) => {
    const evidence = draft.checks.find((check) => check.name === "manualBrowserFileFlows").evidence;
    evidence.after.timeOrigin = evidence.before.timeOrigin;
  }),
  "manualBrowserFileFlows evidence should show a browser reload checkpoint"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find(
      (check) => check.name === "manualBrowserFileFlows"
    ).evidence.after.navigationType = "navigate";
  }),
  'manualBrowserFileFlows.after.navigationType expected "reload"'
);

assertRejects(
  withChange(report, (draft) => {
    draft.states.afterPianoRollParity.frameCount = 0;
  }),
  "states.afterPianoRollParity.frameCount should be a positive number"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find((check) => check.name === "manualBrowserFileFlows").evidence.assetCount = 0;
  }),
  "manualBrowserFileFlows.assetCount should be a positive number"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find((check) => check.name === "manualBrowserFileFlows").evidence.downloadFileName =
      "project.txt";
  }),
  "manualBrowserFileFlows.downloadFileName should end with .orbifold"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find((check) => check.name === "manualBrowserFileFlows").evidence.downloadFile.projectMarker =
      false;
  }),
  "manualBrowserFileFlows.downloadFile.projectMarker expected true"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find((check) => check.name === "manualBrowserFileFlows").evidence.downloadFile.sha256 =
      "not-a-sha";
  }),
  "manualBrowserFileFlows.downloadFile.sha256 should be a SHA-256 hex digest"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find((check) => check.name === "manualBrowserFileFlows").evidence.project =
      "not a project";
  }),
  "manualBrowserFileFlows.project should contain an orbifold project marker"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find((check) => check.name === "manualBrowserFileFlows").evidence.scalaPath = "";
  }),
  "manualBrowserFileFlows.scalaPath should be present"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find(
      (check) => check.name === "manualShortcutParity"
    ).evidence.requiredWorkflows = ["transport", "editing", "file", "help"];
  }),
  "manualShortcutParity.requiredWorkflows should include uiZoom"
);

assertRejects(
  withChange(report, (draft) => {
    const evidence = draft.checks.find((check) => check.name === "manualShortcutParity").evidence;
    evidence.after = structuredClone(evidence.before);
    evidence.after.lastAction = "ui.scale_up";
  }),
  "manualShortcutParity evidence should show a concrete shortcut state change"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find(
      (check) => check.name === "manualShortcutParity"
    ).evidence.after.lastAction = "";
  }),
  "manualShortcutParity.after.lastAction should be present"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find(
      (check) => check.name === "manualPianoRollParity"
    ).evidence.requiredWorkflows = ["noteEdit", "velocityEdit", "scrollOrZoom", "seekOrLoop"];
  }),
  "manualPianoRollParity.requiredWorkflows should include panelResize"
);

assertRejects(
  withChange(report, (draft) => {
    const evidence = draft.checks.find((check) => check.name === "manualPianoRollParity").evidence;
    evidence.after.project = evidence.before.project;
  }),
  "manualPianoRollParity evidence should show a note or velocity edit"
);

assertRejects(
  withChange(report, (draft) => {
    const evidence = draft.checks.find((check) => check.name === "manualPianoRollParity").evidence;
    evidence.after.pianoViewStart = evidence.before.pianoViewStart;
    evidence.after.pianoViewBeats = evidence.before.pianoViewBeats;
  }),
  "manualPianoRollParity evidence should show piano scroll or zoom"
);

assertRejects(
  withChange(report, (draft) => {
    const evidence = draft.checks.find((check) => check.name === "manualPianoRollParity").evidence;
    evidence.after.transportPositionBeats = evidence.before.transportPositionBeats;
    evidence.after.loopBeats = evidence.before.loopBeats;
  }),
  "manualPianoRollParity evidence should show seek or loop-boundary movement"
);

assertRejects(
  withChange(report, (draft) => {
    const evidence = draft.checks.find((check) => check.name === "manualPianoRollParity").evidence;
    evidence.after.pianoRollHeight = evidence.before.pianoRollHeight;
    evidence.after.rightPanelWidth = evidence.before.rightPanelWidth;
  }),
  "manualPianoRollParity evidence should show workspace panel resizing"
);

assertRejects(
  withChange(report, (draft) => {
    draft.checks.find(
      (check) => check.name === "manualPianoRollParity"
    ).evidence.after.pianoGridWidth = 0;
  }),
  "manualPianoRollParity.after.pianoGridWidth should be a positive number"
);

console.log("manual web device report validator behavior ok");

function assertRejects(candidate, message) {
  assert.throws(() => validateManualDeviceReport(candidate), {
    message: new RegExp(escapeRegExp(message)),
  });
}

function withChange(source, change) {
  const draft = structuredClone(source);
  change(draft);
  return draft;
}

function validManualReport() {
  const generatedAt = "2026-05-18T12:00:00.000Z";
  const targetUrl = "https://andrewp2.github.io/Orbifold/";
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
      before: {
        lastMidiStatus: 0,
        lastMidiNote: -1,
      },
      after: {
        lastMidiStatus: 144,
        lastMidiNote: 60,
      },
    },
  });
  checks.push({
    name: "manualRealMidiRecording",
    pass: true,
    evidence: {
      beforeNoteCount: 2,
      afterNoteCount: 3,
    },
  });
  checks.push({
    name: "manualBrowserFileFlows",
    pass: true,
    evidence: {
      downloadFileName: "project.orbifold",
      downloadSize: 128,
      downloadFile: downloadedProjectFile(),
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
      afterImport: fileFlowState({
        navigationType: "navigate",
        timeOrigin: 1000,
        frameCount: 11,
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
    host: {
      platform: "linux",
      arch: "x64",
      release: "test",
    },
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
      runtime: {
        hasGpu: true,
        canvasWidth: 1600,
        canvasHeight: 1000,
      },
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
      afterBrowserFileFlowsBeforeReload: fileFlowState({
        navigationType: "navigate",
        timeOrigin: 1000,
        frameCount: 11,
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

function downloadedProjectFile() {
  return {
    fileName: "project.orbifold",
    bytes: 128,
    sha256: "a".repeat(64),
    projectMarker: true,
    noteCount: 0,
  };
}

function fileFlowState({ navigationType, timeOrigin, frameCount }) {
  return {
    frameCount,
    locationHref: "https://andrewp2.github.io/Orbifold/",
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

function click(name, x, y, at) {
  return {
    name,
    point: { x, y },
    at,
  };
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

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
