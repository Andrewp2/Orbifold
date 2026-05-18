#!/usr/bin/env node

import assert from "node:assert/strict";
import { validateManualDeviceReport } from "./check-web-manual-report.mjs";

const report = validManualReport();

assert.doesNotThrow(() => validateManualDeviceReport(report));

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
    },
    states: {
      runtime: {
        hasGpu: true,
        canvasWidth: 1600,
        canvasHeight: 1000,
      },
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
    },
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
