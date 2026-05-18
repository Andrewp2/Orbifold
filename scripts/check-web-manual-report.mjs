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
  try {
    const options = parseManualReportArgs(process.argv.slice(2));
    if (options.help) {
      console.error(
        "usage: scripts/check-web-manual-report.mjs <reports/web-manual-devices-*.json|reports-dir>"
      );
      process.exit(2);
    }
    const reportPath = await resolveReportPath(options.target);
    const report = JSON.parse(await readFile(reportPath, "utf8"));
    validateManualDeviceReport(report);
    console.log(`manual web device report ok: ${reportPath}`);
  } catch (error) {
    console.error(`manual web device report failed: ${error.message ?? error}`);
    process.exit(1);
  }
}

export function parseManualReportArgs(args) {
  const parsed = { target: "reports", help: false };
  let targetSeen = false;
  for (const arg of args) {
    if (arg === "--help" || arg === "-h") {
      parsed.help = true;
      return parsed;
    }
    if (!arg.startsWith("--") && !targetSeen) {
      parsed.target = arg;
      targetSeen = true;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }
  return parsed;
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

  requireUniqueCheckNames(report.checks);
  requireEveryCheckPassed(report.checks);

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
  requireObject(report.states.beforeVisualInspection, "states.beforeVisualInspection");
  requireObject(report.states.afterLargeVisualInspection, "states.afterLargeVisualInspection");
  const visualEvidence = requirePassedCheckEvidence("manualVisualInspection");
  requireObject(visualEvidence.initial, "manualVisualInspection.initial");
  requireObject(visualEvidence.inspectedLarge, "manualVisualInspection.inspectedLarge");
  requireManualVisualState(visualEvidence.initial, "manualVisualInspection.initial");
  requireManualVisualState(
    visualEvidence.inspectedLarge,
    "manualVisualInspection.inspectedLarge"
  );
  if (!manualVisualStateShowsResize(visualEvidence.initial, visualEvidence.inspectedLarge)) {
    throw new Error(
      "manualVisualInspection evidence should show a resize or high-DPI checkpoint"
    );
  }

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

  requireObject(report.states.beforeBrowserFileFlows, "states.beforeBrowserFileFlows");
  requireObject(report.states.afterBrowserFileFlows, "states.afterBrowserFileFlows");
  requirePositiveNumber(
    report.states.afterBrowserFileFlows.frameCount,
    "states.afterBrowserFileFlows.frameCount"
  );
  const browserFileEvidence = requirePassedCheckEvidence("manualBrowserFileFlows");
  requireObject(browserFileEvidence.before, "manualBrowserFileFlows.before");
  requireObject(browserFileEvidence.after, "manualBrowserFileFlows.after");
  requireBrowserFileFlowCheckpoint(browserFileEvidence.before, "manualBrowserFileFlows.before");
  requireBrowserFileFlowCheckpoint(browserFileEvidence.after, "manualBrowserFileFlows.after");
  if (
    !(
      Number(browserFileEvidence.after.timeOrigin) >
      Number(browserFileEvidence.before.timeOrigin)
    )
  ) {
    throw new Error("manualBrowserFileFlows evidence should show a browser reload checkpoint");
  }
  requireEqual(
    browserFileEvidence.after.navigationType,
    "reload",
    "manualBrowserFileFlows.after.navigationType"
  );
  requireTruthy(
    browserFileEvidence.downloadFileName,
    "manualBrowserFileFlows.downloadFileName"
  );
  if (!String(browserFileEvidence.downloadFileName).endsWith(".orbifold")) {
    throw new Error(
      `manualBrowserFileFlows.downloadFileName should end with .orbifold, got ${JSON.stringify(
        browserFileEvidence.downloadFileName
      )}`
    );
  }
  requirePositiveNumber(browserFileEvidence.downloadSize, "manualBrowserFileFlows.downloadSize");
  requireTruthy(browserFileEvidence.project, "manualBrowserFileFlows.project");
  if (!String(browserFileEvidence.project).includes("orbifold_project=1")) {
    throw new Error("manualBrowserFileFlows.project should contain an orbifold project marker");
  }
  requirePositiveNumber(browserFileEvidence.assetCount, "manualBrowserFileFlows.assetCount");
  requireTruthy(browserFileEvidence.scaleDescription, "manualBrowserFileFlows.scaleDescription");
  requireTruthy(browserFileEvidence.scalaPath, "manualBrowserFileFlows.scalaPath");
  requireTruthy(browserFileEvidence.lumatonePath, "manualBrowserFileFlows.lumatonePath");
  requireEqual(browserFileEvidence.lumatoneLoaded, true, "manualBrowserFileFlows.lumatoneLoaded");

  requireObject(report.states.beforeShortcutParity, "states.beforeShortcutParity");
  requireObject(report.states.afterShortcutParity, "states.afterShortcutParity");
  requirePositiveNumber(
    report.states.afterShortcutParity.frameCount,
    "states.afterShortcutParity.frameCount"
  );
  const shortcutEvidence = requirePassedCheckEvidence("manualShortcutParity");
  requireRequiredWorkflows(shortcutEvidence, "manualShortcutParity", [
    "transport",
    "editing",
    "file",
    "help",
    "uiZoom",
  ]);
  requireObject(shortcutEvidence.before, "manualShortcutParity.before");
  requireObject(shortcutEvidence.after, "manualShortcutParity.after");
  requireFiniteNumber(shortcutEvidence.before.noteCount, "manualShortcutParity.before.noteCount");
  requirePositiveNumber(shortcutEvidence.before.uiScale, "manualShortcutParity.before.uiScale");
  requireFiniteNumber(
    shortcutEvidence.before.downloadSize,
    "manualShortcutParity.before.downloadSize"
  );
  requireTruthy(shortcutEvidence.before.project, "manualShortcutParity.before.project");
  requireTruthy(shortcutEvidence.after.lastAction, "manualShortcutParity.after.lastAction");
  requirePositiveNumber(shortcutEvidence.after.noteCount, "manualShortcutParity.after.noteCount");
  requirePositiveNumber(shortcutEvidence.after.uiScale, "manualShortcutParity.after.uiScale");
  requireFiniteNumber(
    shortcutEvidence.after.downloadSize,
    "manualShortcutParity.after.downloadSize"
  );
  requireTruthy(shortcutEvidence.after.project, "manualShortcutParity.after.project");
  if (!shortcutEvidenceHasConcreteChange(shortcutEvidence.before, shortcutEvidence.after)) {
    throw new Error(
      "manualShortcutParity evidence should show a concrete shortcut state change"
    );
  }

  requireObject(report.states.beforePianoRollParity, "states.beforePianoRollParity");
  requireObject(report.states.afterPianoRollParity, "states.afterPianoRollParity");
  requirePositiveNumber(
    report.states.afterPianoRollParity.frameCount,
    "states.afterPianoRollParity.frameCount"
  );
  const pianoEvidence = requirePassedCheckEvidence("manualPianoRollParity");
  requireRequiredWorkflows(pianoEvidence, "manualPianoRollParity", [
    "noteEdit",
    "velocityEdit",
    "scrollOrZoom",
    "seekOrLoop",
    "panelResize",
  ]);
  requireObject(pianoEvidence.before, "manualPianoRollParity.before");
  requireObject(pianoEvidence.after, "manualPianoRollParity.after");
  requireFiniteNumber(pianoEvidence.before.noteCount, "manualPianoRollParity.before.noteCount");
  requireTruthy(pianoEvidence.before.project, "manualPianoRollParity.before.project");
  requireFiniteNumber(
    pianoEvidence.before.transportPositionBeats,
    "manualPianoRollParity.before.transportPositionBeats"
  );
  requirePositiveNumber(pianoEvidence.before.loopBeats, "manualPianoRollParity.before.loopBeats");
  requireFiniteNumber(
    pianoEvidence.before.pianoViewStart,
    "manualPianoRollParity.before.pianoViewStart"
  );
  requirePositiveNumber(
    pianoEvidence.before.pianoViewBeats,
    "manualPianoRollParity.before.pianoViewBeats"
  );
  requirePositiveNumber(
    pianoEvidence.before.pianoGridWidth,
    "manualPianoRollParity.before.pianoGridWidth"
  );
  requirePositiveNumber(
    pianoEvidence.before.pianoGridHeight,
    "manualPianoRollParity.before.pianoGridHeight"
  );
  requirePositiveNumber(
    pianoEvidence.before.pianoRollHeight,
    "manualPianoRollParity.before.pianoRollHeight"
  );
  requirePositiveNumber(
    pianoEvidence.before.rightPanelWidth,
    "manualPianoRollParity.before.rightPanelWidth"
  );
  requirePositiveNumber(pianoEvidence.after.noteCount, "manualPianoRollParity.after.noteCount");
  requireTruthy(pianoEvidence.after.project, "manualPianoRollParity.after.project");
  requireFiniteNumber(
    pianoEvidence.after.transportPositionBeats,
    "manualPianoRollParity.after.transportPositionBeats"
  );
  requirePositiveNumber(pianoEvidence.after.loopBeats, "manualPianoRollParity.after.loopBeats");
  requireFiniteNumber(
    pianoEvidence.after.pianoViewStart,
    "manualPianoRollParity.after.pianoViewStart"
  );
  requirePositiveNumber(
    pianoEvidence.after.pianoViewBeats,
    "manualPianoRollParity.after.pianoViewBeats"
  );
  requirePositiveNumber(
    pianoEvidence.after.pianoGridWidth,
    "manualPianoRollParity.after.pianoGridWidth"
  );
  requirePositiveNumber(
    pianoEvidence.after.pianoGridHeight,
    "manualPianoRollParity.after.pianoGridHeight"
  );
  requirePositiveNumber(
    pianoEvidence.after.pianoRollHeight,
    "manualPianoRollParity.after.pianoRollHeight"
  );
  requirePositiveNumber(
    pianoEvidence.after.rightPanelWidth,
    "manualPianoRollParity.after.rightPanelWidth"
  );
  if (String(pianoEvidence.before.project) === String(pianoEvidence.after.project)) {
    throw new Error("manualPianoRollParity evidence should show a note or velocity edit");
  }
  if (
    nearlyEqual(pianoEvidence.before.pianoViewStart, pianoEvidence.after.pianoViewStart) &&
    nearlyEqual(pianoEvidence.before.pianoViewBeats, pianoEvidence.after.pianoViewBeats)
  ) {
    throw new Error("manualPianoRollParity evidence should show piano scroll or zoom");
  }
  if (
    nearlyEqual(
      pianoEvidence.before.transportPositionBeats,
      pianoEvidence.after.transportPositionBeats
    ) &&
    nearlyEqual(pianoEvidence.before.loopBeats, pianoEvidence.after.loopBeats)
  ) {
    throw new Error("manualPianoRollParity evidence should show seek or loop-boundary movement");
  }
  if (
    nearlyEqual(pianoEvidence.before.pianoRollHeight, pianoEvidence.after.pianoRollHeight) &&
    nearlyEqual(pianoEvidence.before.rightPanelWidth, pianoEvidence.after.rightPanelWidth)
  ) {
    throw new Error("manualPianoRollParity evidence should show workspace panel resizing");
  }
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

function requireRequiredWorkflows(evidence, label, requiredWorkflows) {
  requireArray(evidence.requiredWorkflows, `${label}.requiredWorkflows`);
  for (const workflow of requiredWorkflows) {
    if (!evidence.requiredWorkflows.includes(workflow)) {
      throw new Error(`${label}.requiredWorkflows should include ${workflow}`);
    }
  }
}

function requireUniqueCheckNames(checks) {
  const seen = new Set();
  for (let index = 0; index < checks.length; index += 1) {
    const name = checks[index]?.name;
    requireTruthy(name, `checks.${index}.name`);
    if (seen.has(name)) {
      throw new Error(`checks.${name} should appear exactly once`);
    }
    seen.add(name);
  }
}

function requireEveryCheckPassed(checks) {
  for (const check of checks) {
    requireEqual(check.pass, true, `checks.${check.name}.pass`);
  }
}

function requireManualVisualState(state, label) {
  requireTruthy(state.className, `${label}.className`);
  if (!String(state.className).includes("runtime-ready")) {
    throw new Error(`${label}.className should include runtime-ready`);
  }
  requirePositiveNumber(state.frameCount, `${label}.frameCount`);
  requirePositiveNumber(state.viewportWidth, `${label}.viewportWidth`);
  requirePositiveNumber(state.viewportHeight, `${label}.viewportHeight`);
  requirePositiveNumber(state.uiScale, `${label}.uiScale`);
  requirePositiveNumber(state.devicePixelRatio, `${label}.devicePixelRatio`);
  requirePositiveNumber(state.canvasClientWidth, `${label}.canvasClientWidth`);
  requirePositiveNumber(state.canvasClientHeight, `${label}.canvasClientHeight`);
  requirePositiveNumber(state.canvasWidth, `${label}.canvasWidth`);
  requirePositiveNumber(state.canvasHeight, `${label}.canvasHeight`);
  requireFiniteNumber(state.canvasLeft, `${label}.canvasLeft`);
  requireFiniteNumber(state.canvasTop, `${label}.canvasTop`);
  requirePositiveNumber(state.canvasRectWidth, `${label}.canvasRectWidth`);
  requirePositiveNumber(state.canvasRectHeight, `${label}.canvasRectHeight`);
  requirePositiveNumber(state.textAuditCount, `${label}.textAuditCount`);
  requireEqual(state.textAuditReady, "1", `${label}.textAuditReady`);
  requireEqual(Number(state.textAuditIssueCount), 0, `${label}.textAuditIssueCount`);
  requireEqual(Number(state.textAuditNonFiniteCount), 0, `${label}.textAuditNonFiniteCount`);
  if (state.textAuditSampleIssue) {
    throw new Error(`${label}.textAuditSampleIssue should be empty`);
  }

  const dpr = Math.max(1, Number(state.devicePixelRatio));
  if (Number(state.canvasWidth) < Number(state.canvasClientWidth) * dpr - 2) {
    throw new Error(`${label}.canvasWidth should cover client width at devicePixelRatio`);
  }
  if (Number(state.canvasHeight) < Number(state.canvasClientHeight) * dpr - 2) {
    throw new Error(`${label}.canvasHeight should cover client height at devicePixelRatio`);
  }
  if (Number(state.canvasRectWidth) < Number(state.canvasClientWidth) - 2) {
    throw new Error(`${label}.canvasRectWidth should cover canvas client width`);
  }
  if (Number(state.canvasRectHeight) < Number(state.canvasClientHeight) - 2) {
    throw new Error(`${label}.canvasRectHeight should cover canvas client height`);
  }
}

function requireBrowserFileFlowCheckpoint(state, label) {
  requirePositiveNumber(state.frameCount, `${label}.frameCount`);
  requireTruthy(state.locationHref, `${label}.locationHref`);
  requireTruthy(state.navigationType, `${label}.navigationType`);
  requirePositiveNumber(state.timeOrigin, `${label}.timeOrigin`);
}

function manualVisualStateShowsResize(initial, inspectedLarge) {
  return (
    Math.abs(Number(initial.canvasClientWidth) - Number(inspectedLarge.canvasClientWidth)) >= 16 ||
    Math.abs(Number(initial.canvasClientHeight) - Number(inspectedLarge.canvasClientHeight)) >= 16 ||
    Math.abs(Number(initial.devicePixelRatio) - Number(inspectedLarge.devicePixelRatio)) >= 0.1 ||
    Math.abs(Number(initial.uiScale) - Number(inspectedLarge.uiScale)) >= 0.01
  );
}

function shortcutEvidenceHasConcreteChange(before, after) {
  return (
    Number(before.noteCount) !== Number(after.noteCount) ||
    String(before.project ?? "") !== String(after.project ?? "") ||
    Boolean(before.transportPlaying) !== Boolean(after.transportPlaying) ||
    !nearlyEqual(before.uiScale, after.uiScale) ||
    Number(after.downloadSize) > Number(before.downloadSize ?? 0)
  );
}

function nearlyEqual(left, right) {
  return Math.abs(Number(left) - Number(right)) < 0.0001;
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

function requireFiniteNumber(value, label) {
  if (!Number.isFinite(Number(value))) {
    throw new Error(`${label} should be a finite number, got ${JSON.stringify(value)}`);
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
