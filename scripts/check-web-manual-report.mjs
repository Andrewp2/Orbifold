#!/usr/bin/env node

import { readdir, readFile, stat } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import {
  normalizeWebRootHref,
  requireArtifactFingerprint,
} from "./web-artifact-fingerprint.mjs";

const requiredChecks = [
  "browserRuntimeReady",
  "manualVisualInspection",
  "webAudioOutputsDiscovered",
  "webAudioConnectedState",
  "manualAudibleWebAudio",
  "webMidiInputsDiscovered",
  "webMidiConnectedState",
  "manualRealMidiInput",
  "manualRealMidiRecording",
  "manualBrowserFileFlows",
  "manualShortcutParity",
  "manualPianoRollParity",
  "manualDeviceVerifierCompleted",
];

const requiredClickCounts = {
  viewDevices: 1,
  audioRefresh: 1,
  audioConnect: 1,
  audioTestA4: 1,
  midiRefresh: 2,
  midiConnect: 1,
  record: 2,
};

if (isCliEntrypoint()) {
  const target = process.argv[2] ?? "reports";

  if (process.argv.includes("--help") || process.argv.includes("-h")) {
    console.error(
      "usage: scripts/check-web-manual-report.mjs <reports/web-manual-devices-*.json|reports-dir>"
    );
    process.exit(2);
  }

  try {
    const reportPath = await resolveReportPath(target);
    const report = JSON.parse(await readFile(reportPath, "utf8"));
    validateManualDeviceReport(report);
    console.log(`manual web device report ok: ${reportPath}`);
  } catch (error) {
    console.error(`manual web device report failed: ${error.message ?? error}`);
    process.exit(1);
  }
}

export function validateManualDeviceReport(report) {
  requireEqual(report.schema, "orbifold.web_manual_device_parity.v1", "schema");
  requireTruthy(report.generatedAt, "generatedAt");
  requireIsoDate(report.generatedAt, "generatedAt");
  requireTruthy(report.targetUrl, "targetUrl");
  requireNoError(report);
  requireArray(report.checks, "checks");
  requireArray(report.clicks, "clicks");
  requireArray(report.browserEvents, "browserEvents");
  const browserFailures = manualReportBrowserFailures(report.browserEvents);
  if (browserFailures.length > 0) {
    throw new Error(
      `browserEvents should not contain runtime errors: ${browserFailures.join("; ")}`
    );
  }
  requireObject(report.host, "host");
  requireTruthy(report.host.platform, "host.platform");
  requireTruthy(report.host.arch, "host.arch");
  requireTruthy(report.host.release, "host.release");
  requireObject(report.states, "states");
  requireObject(report.userConfirmations, "userConfirmations");
  requireObject(report.chrome, "chrome");
  requireTruthy(report.chrome.path, "chrome.path");
  requireTruthy(report.chrome.version, "chrome.version");
  requireTruthy(report.chrome.userAgent, "chrome.userAgent");
  requireTruthy(report.chrome.protocolVersion, "chrome.protocolVersion");
  requireArtifactFingerprint(report.artifact, "artifact");
  requireIsoDate(report.artifact.generatedAt, "artifact.generatedAt");
  requireEqual(
    normalizeWebRootHref(report.artifact.rootUrl),
    normalizeWebRootHref(report.targetUrl),
    "artifact.rootUrl"
  );

  const checksByName = new Map(report.checks.map((check) => [check.name, check]));
  const requirePassedCheck = (name) => {
    const check = checksByName.get(name);
    requireTruthy(check, `checks.${name}`);
    requireEqual(check.pass, true, `checks.${name}.pass`);
    return check;
  };
  const requirePassedCheckEvidence = (name) => {
    const evidence = requirePassedCheck(name).evidence;
    requireObject(evidence, `checks.${name}.evidence`);
    return evidence;
  };

  for (const name of requiredChecks) {
    requirePassedCheck(name);
  }

  for (const [name, expectedCount] of Object.entries(requiredClickCounts)) {
    const clicks = report.clicks.filter((click) => click.name === name);
    if (clicks.length < expectedCount) {
      throw new Error(`clicks.${name} expected at least ${expectedCount}, found ${clicks.length}`);
    }
    for (const click of clicks) {
      requirePositiveNumber(click.point?.x, `clicks.${name}.point.x`);
      requirePositiveNumber(click.point?.y, `clicks.${name}.point.y`);
      requireIsoDate(click.at, `clicks.${name}.at`);
    }
  }

  requireEqual(report.userConfirmations.visualInspection, true, "visualInspection");
  requireEqual(report.userConfirmations.audibleA4, true, "audibleA4");
  requireEqual(report.userConfirmations.realMidiNoteVisible, true, "realMidiNoteVisible");
  requireEqual(
    report.userConfirmations.realMidiRecordingVisible,
    true,
    "realMidiRecordingVisible"
  );
  requireEqual(report.userConfirmations.browserFileFlows, true, "browserFileFlows");
  requireEqual(report.userConfirmations.shortcutParity, true, "shortcutParity");
  requireEqual(report.userConfirmations.pianoRollParity, true, "pianoRollParity");

  requireObject(report.states.runtime, "states.runtime");
  requireEqual(report.states.runtime.hasGpu, true, "states.runtime.hasGpu");
  requirePositiveNumber(report.states.runtime.canvasWidth, "states.runtime.canvasWidth");
  requirePositiveNumber(report.states.runtime.canvasHeight, "states.runtime.canvasHeight");

  requireObject(report.states.afterAudioRefresh, "states.afterAudioRefresh");
  requirePositiveNumber(
    report.states.afterAudioRefresh.audioOutputCount,
    "states.afterAudioRefresh.audioOutputCount"
  );
  requireTruthy(
    report.states.afterAudioRefresh.browserAudioOutputNames,
    "states.afterAudioRefresh.browserAudioOutputNames"
  );

  requireObject(report.states.afterAudioConnect, "states.afterAudioConnect");
  requireEqual(
    report.states.afterAudioConnect.audioStreamConnected,
    true,
    "states.afterAudioConnect.audioStreamConnected"
  );
  requireEqual(
    report.states.afterAudioConnect.audioContextCreated,
    true,
    "states.afterAudioConnect.audioContextCreated"
  );
  requireEqual(
    report.states.afterAudioConnect.audioProcessorAttached,
    true,
    "states.afterAudioConnect.audioProcessorAttached"
  );
  requireEqual(
    report.states.afterAudioConnect.audioResumeRequested,
    true,
    "states.afterAudioConnect.audioResumeRequested"
  );
  requireEqual(
    report.states.afterAudioConnect.audioResumeResolved,
    true,
    "states.afterAudioConnect.audioResumeResolved"
  );

  requireObject(report.states.afterAudioTest, "states.afterAudioTest");
  requireEqual(report.states.afterAudioTest.audioNonzero, true, "states.afterAudioTest.audioNonzero");
  requirePositiveNumber(
    report.states.afterAudioTest.audioCallbackCount,
    "states.afterAudioTest.audioCallbackCount"
  );
  requirePositiveNumber(
    report.states.afterAudioTest.audioFrameCount,
    "states.afterAudioTest.audioFrameCount"
  );
  requirePositiveNumber(report.states.afterAudioTest.audioPeak, "states.afterAudioTest.audioPeak");

  requireObject(report.states.afterMidiRefresh, "states.afterMidiRefresh");
  requirePositiveNumber(
    report.states.afterMidiRefresh.midiInputCount,
    "states.afterMidiRefresh.midiInputCount"
  );
  requireTruthy(
    report.states.afterMidiRefresh.browserMidiInputNames,
    "states.afterMidiRefresh.browserMidiInputNames"
  );

  requireObject(report.states.afterMidiConnect, "states.afterMidiConnect");
  requireTruthy(
    report.states.afterMidiConnect.connectedMidiInput,
    "states.afterMidiConnect.connectedMidiInput"
  );
  requireTruthy(
    report.states.afterMidiConnect.midiInputConnection,
    "states.afterMidiConnect.midiInputConnection"
  );

  requireObject(report.states.afterRealMidiNote, "states.afterRealMidiNote");
  if (
    !(Number(report.states.afterRealMidiNote.lastMidiStatus) > 0) &&
    !(Number(report.states.afterRealMidiNote.lastMidiNote) >= 0)
  ) {
    throw new Error("states.afterRealMidiNote should show a real MIDI status or note");
  }
  const realMidiEvidence = requirePassedCheck("manualRealMidiInput").evidence ?? {};
  if (
    Number(realMidiEvidence.before?.lastMidiStatus) ===
      Number(realMidiEvidence.after?.lastMidiStatus) &&
    Number(realMidiEvidence.before?.lastMidiNote) === Number(realMidiEvidence.after?.lastMidiNote)
  ) {
    throw new Error("manualRealMidiInput evidence should show a changed MIDI status or note");
  }

  requireObject(report.states.afterMidiRecording, "states.afterMidiRecording");
  requirePositiveNumber(
    report.states.afterMidiRecording.noteCount,
    "states.afterMidiRecording.noteCount"
  );
  const recordingEvidence = requirePassedCheck("manualRealMidiRecording").evidence ?? {};
  if (!(Number(recordingEvidence.afterNoteCount) > Number(recordingEvidence.beforeNoteCount))) {
    throw new Error("manualRealMidiRecording evidence should show a new recorded note");
  }

  requireObject(report.states.afterBrowserFileFlows, "states.afterBrowserFileFlows");
  requirePositiveNumber(
    report.states.afterBrowserFileFlows.frameCount,
    "states.afterBrowserFileFlows.frameCount"
  );
  const browserFileEvidence = requirePassedCheckEvidence("manualBrowserFileFlows");
  requirePositiveNumber(browserFileEvidence.downloadSize, "manualBrowserFileFlows.downloadSize");
  requirePositiveNumber(browserFileEvidence.assetCount, "manualBrowserFileFlows.assetCount");
  requireTruthy(browserFileEvidence.scaleDescription, "manualBrowserFileFlows.scaleDescription");
  requireTruthy(browserFileEvidence.lumatonePath, "manualBrowserFileFlows.lumatonePath");
  requireEqual(browserFileEvidence.lumatoneLoaded, true, "manualBrowserFileFlows.lumatoneLoaded");

  requireObject(report.states.afterShortcutParity, "states.afterShortcutParity");
  requirePositiveNumber(
    report.states.afterShortcutParity.frameCount,
    "states.afterShortcutParity.frameCount"
  );
  const shortcutEvidence = requirePassedCheckEvidence("manualShortcutParity");
  requireTruthy(shortcutEvidence.lastAction, "manualShortcutParity.lastAction");
  requirePositiveNumber(shortcutEvidence.noteCount, "manualShortcutParity.noteCount");
  requirePositiveNumber(shortcutEvidence.uiScale, "manualShortcutParity.uiScale");

  requireObject(report.states.afterPianoRollParity, "states.afterPianoRollParity");
  requirePositiveNumber(
    report.states.afterPianoRollParity.frameCount,
    "states.afterPianoRollParity.frameCount"
  );
  const pianoEvidence = requirePassedCheckEvidence("manualPianoRollParity");
  requirePositiveNumber(pianoEvidence.noteCount, "manualPianoRollParity.noteCount");
  requirePositiveNumber(pianoEvidence.pianoViewBeats, "manualPianoRollParity.pianoViewBeats");
  requirePositiveNumber(pianoEvidence.pianoGridWidth, "manualPianoRollParity.pianoGridWidth");
  requirePositiveNumber(pianoEvidence.pianoGridHeight, "manualPianoRollParity.pianoGridHeight");
  requirePositiveNumber(pianoEvidence.pianoRollHeight, "manualPianoRollParity.pianoRollHeight");
  requirePositiveNumber(pianoEvidence.rightPanelWidth, "manualPianoRollParity.rightPanelWidth");
}

export async function resolveReportPath(targetPath) {
  const targetStat = await stat(targetPath);
  if (targetStat.isFile()) {
    return targetPath;
  }
  if (!targetStat.isDirectory()) {
    throw new Error(`${targetPath} is neither a file nor a directory`);
  }

  const entries = await readdir(targetPath, { withFileTypes: true });
  const candidates = entries
    .filter((entry) => entry.isFile())
    .filter((entry) => /^web-manual-devices-.+\.json$/.test(entry.name))
    .map((entry) => path.join(targetPath, entry.name))
    .sort();

  if (candidates.length === 0) {
    throw new Error(`${targetPath} does not contain web-manual-devices-*.json reports`);
  }

  return candidates[candidates.length - 1];
}

function requireNoError(data) {
  if (data.error) {
    throw new Error(`report contains error: ${data.error}`);
  }
}

function manualReportBrowserFailures(events) {
  const failures = [];
  for (const event of events) {
    if (event.method === "Runtime.exceptionThrown") {
      failures.push(`exception: ${event.params?.exceptionDetails?.text ?? "unknown"}`);
    } else if (
      event.method === "Runtime.consoleAPICalled" &&
      ["error", "assert"].includes(event.params?.type)
    ) {
      failures.push(`console ${event.params.type}: ${consoleArgs(event.params.args ?? [])}`);
    } else if (event.method === "Network.loadingFailed") {
      failures.push(`network load failed: ${event.url ?? event.params?.errorText ?? "unknown"}`);
    } else if (
      event.method === "Log.entryAdded" &&
      event.params?.entry?.level === "error"
    ) {
      failures.push(`browser log error: ${event.params.entry.text}`);
    }
  }
  return failures;
}

function consoleArgs(args) {
  return args.map((arg) => arg.value ?? arg.description ?? arg.type ?? "").join(" ");
}

function requireArray(value, label) {
  if (!Array.isArray(value)) {
    throw new Error(`${label} should be an array`);
  }
}

function requireObject(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} should be an object`);
  }
}

function requireTruthy(value, label) {
  if (!value) {
    throw new Error(`${label} should be present`);
  }
}

function requireEqual(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(`${label} expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

function requirePositiveNumber(value, label) {
  if (!(Number(value) > 0)) {
    throw new Error(`${label} should be a positive number, got ${JSON.stringify(value)}`);
  }
}

function requireIsoDate(value, label) {
  if (!value || Number.isNaN(Date.parse(value))) {
    throw new Error(`${label} should be an ISO date, got ${JSON.stringify(value)}`);
  }
}

function isCliEntrypoint() {
  return process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
}
