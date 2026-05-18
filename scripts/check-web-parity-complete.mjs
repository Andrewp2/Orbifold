#!/usr/bin/env node

import { readdir, readFile, stat } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { validateManualDeviceReport } from "./check-web-manual-report.mjs";
import {
  compareWebArtifactFingerprints,
  requireArtifactFingerprint,
} from "./web-artifact-fingerprint.mjs";

const requiredSteps = [
  "manualDeviceReport",
  "manualReportTarget",
  "manualReportArtifact",
  "deployedArtifact",
  "deployedLayout",
  "deployedSmoke",
  "deployedVisualCapture",
];

const requiredVisualCaptures = [
  { label: "compact-1200x760", width: 1200, height: 760, deviceScaleFactor: 1 },
  { label: "desktop-1600x1000", width: 1600, height: 1000, deviceScaleFactor: 1 },
  { label: "hidpi-1920x1080-dpr2", width: 1920, height: 1080, deviceScaleFactor: 2 },
  { label: "wide-3840x2160", width: 3840, height: 2160, deviceScaleFactor: 1 },
];

if (isCliEntrypoint()) {
  try {
    const options = parseParityCompletionArgs(process.argv.slice(2));
    if (options.help) {
      console.error(
        "usage: scripts/check-web-parity-complete.mjs <reports/web-parity-gate-*.json|reports-dir> [--url https://pages-url/]"
      );
      process.exit(2);
    }
    const reportPath = await resolveParityCompletionReportPath(options.target);
    await validateParityCompletionReportFile(reportPath, { expectedUrl: options.expectedUrl });
    console.log(`web parity completion evidence ok: ${reportPath}`);
  } catch (error) {
    console.error(`web parity completion evidence failed: ${error.message ?? error}`);
    process.exit(1);
  }
}

export function parseParityCompletionArgs(args) {
  const parsed = { target: "reports", expectedUrl: "", help: false };
  let targetSeen = false;
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--url") {
      const value = args[++index];
      if (!value || value.startsWith("--")) {
        throw new Error("--url requires a value");
      }
      parsed.expectedUrl = value;
    } else if (arg.startsWith("--url=")) {
      const value = arg.slice("--url=".length);
      if (!value) {
        throw new Error("--url requires a value");
      }
      parsed.expectedUrl = value;
    } else if (arg === "--help" || arg === "-h") {
      parsed.help = true;
      return parsed;
    } else if (!arg.startsWith("--") && !targetSeen) {
      parsed.target = arg;
      targetSeen = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return parsed;
}

export function validateParityCompletionReport(report, options = {}) {
  requireEqual(report.schema, "orbifold.web_parity_gate.v1", "schema");
  requireTruthy(report.generatedAt, "generatedAt");
  requireIsoDate(report.generatedAt, "generatedAt");
  requireTruthy(report.targetUrl, "targetUrl");
  if (options.expectedUrl) {
    requireEqual(
      normalizeCompletionUrl(report.targetUrl),
      normalizeCompletionUrl(options.expectedUrl),
      "targetUrl"
    );
  }
  requireEqual(report.passed, true, "passed");
  requireEqual(report.skippedVisualCapture, false, "skippedVisualCapture");
  requireArray(report.steps, "steps");
  requireArtifactFingerprint(report.liveArtifact, "liveArtifact");
  requireTruthy(report.manualReportPath, "manualReportPath");
  requireTruthy(report.visualOut, "visualOut");

  requireUniqueStepNames(report.steps);
  requireEveryStepPassed(report.steps);

  const stepsByName = new Map(report.steps.map((step) => [step.name, step]));
  for (const name of requiredSteps) {
    const step = stepsByName.get(name);
    requireObject(step, `steps.${name}`);
    requireEqual(step.exitCode, 0, `steps.${name}.exitCode`);
    requireIsoDate(step.startedAt, `steps.${name}.startedAt`);
    requireIsoDate(step.endedAt, `steps.${name}.endedAt`);
    if (Array.isArray(step.command) && step.command.includes("skipped")) {
      throw new Error(`steps.${name}.command should not be skipped`);
    }
  }

  const artifactStep = stepsByName.get("manualReportArtifact");
  if (!String(artifactStep.stdoutTail ?? "").includes("manual report artifact matches live")) {
    throw new Error("manualReportArtifact should confirm the manual artifact matches live");
  }

  requireTruthy(options.manualReport, "manualReport");
  validateManualDeviceReport(options.manualReport);
  const differences = compareWebArtifactFingerprints(
    report.liveArtifact,
    options.manualReport.artifact
  );
  if (differences.length > 0) {
    throw new Error(`manual report artifact should match live artifact: ${differences[0]}`);
  }

  requireTruthy(options.visualManifest, "visualManifest");
  validateVisualCaptureManifest(options.visualManifest, report);
}

export async function validateParityCompletionReportFile(reportPath, options = {}) {
  const report = JSON.parse(await readFile(reportPath, "utf8"));
  const reportDir = path.dirname(reportPath);
  const manualReportPath = resolveManualReportPath(report.manualReportPath, reportDir);
  const manualReport = JSON.parse(await readFile(manualReportPath, "utf8"));
  const visualManifestPath = await resolveVisualManifestPath(report, reportDir);
  const visualManifest = JSON.parse(await readFile(visualManifestPath, "utf8"));
  validateParityCompletionReport(report, {
    reportDir,
    manualReport,
    visualManifest,
    expectedUrl: options.expectedUrl,
  });
  await requireExistingVisualCaptureFiles(visualManifest, visualManifestPath);
}

export function validateVisualCaptureManifest(manifest, report) {
  requireObject(manifest, "visualManifest");
  requireTruthy(manifest.target, "visualManifest.target");
  requireEqual(
    normalizeCompletionUrl(manifest.target),
    normalizeCompletionUrl(report.targetUrl),
    "visualManifest.target"
  );
  requireIsoDate(manifest.capturedAt, "visualManifest.capturedAt");
  requireArray(manifest.captures, "visualManifest.captures");
  if (Array.isArray(manifest.failures) && manifest.failures.length > 0) {
    throw new Error(`visualManifest.failures should be empty: ${manifest.failures[0]}`);
  }

  const capturesByLabel = new Map(manifest.captures.map((capture) => [capture.label, capture]));
  for (const expected of requiredVisualCaptures) {
    const capture = capturesByLabel.get(expected.label);
    requireObject(capture, `visualManifest.captures.${expected.label}`);
    requireEqual(capture.width, expected.width, `visualManifest.captures.${expected.label}.width`);
    requireEqual(capture.height, expected.height, `visualManifest.captures.${expected.label}.height`);
    requireEqual(
      capture.deviceScaleFactor,
      expected.deviceScaleFactor,
      `visualManifest.captures.${expected.label}.deviceScaleFactor`
    );
    requireReadyVisualState(capture.state, expected);
    requireVisualEvidence(capture, expected.label);
  }
}

export async function resolveParityCompletionReportPath(targetPath) {
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
    .filter((entry) => /^web-parity-gate-.+\.json$/.test(entry.name))
    .map((entry) => path.join(targetPath, entry.name))
    .sort();

  if (candidates.length === 0) {
    throw new Error(`${targetPath} does not contain web-parity-gate-*.json reports`);
  }

  return candidates[candidates.length - 1];
}

export async function resolveVisualManifestPath(report, reportDir = ".") {
  const step = Array.isArray(report.steps)
    ? report.steps.find((candidate) => candidate.name === "deployedVisualCapture")
    : null;
  const stdoutTail = String(step?.stdoutTail ?? "");
  const manifestMatch = stdoutTail.match(/(?:^|\n)- manifest: ([^\n]+manifest\.json)(?:\n|$)/);
  if (manifestMatch) {
    return resolveEvidencePath(manifestMatch[1].trim(), reportDir);
  }

  requireTruthy(report.visualOut, "visualOut");
  const visualOut = resolveEvidencePath(report.visualOut, reportDir);
  const visualOutStat = await stat(visualOut);
  if (visualOutStat.isFile()) {
    return visualOut;
  }
  if (!visualOutStat.isDirectory()) {
    throw new Error(`${visualOut} is neither a visual manifest nor a directory`);
  }

  const directManifest = path.join(visualOut, "manifest.json");
  try {
    const directManifestStat = await stat(directManifest);
    if (directManifestStat.isFile()) {
      return directManifest;
    }
  } catch (_error) {
    // Fall through to timestamped run directories.
  }

  const entries = await readdir(visualOut, { withFileTypes: true });
  const candidates = entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(visualOut, entry.name, "manifest.json"))
    .sort();

  for (let index = candidates.length - 1; index >= 0; index -= 1) {
    try {
      const candidateStat = await stat(candidates[index]);
      if (candidateStat.isFile()) {
        return candidates[index];
      }
    } catch (_error) {
      // Keep looking for an existing manifest.
    }
  }

  throw new Error(`${visualOut} does not contain a visual capture manifest`);
}

async function requireExistingVisualCaptureFiles(manifest, manifestPath) {
  const manifestDir = path.dirname(manifestPath);
  for (const capture of manifest.captures ?? []) {
    const evidencePath = capture.screenshot ?? capture.snapshot;
    requireTruthy(evidencePath, `visualManifest.captures.${capture.label}.evidencePath`);
    const resolved = resolveEvidencePath(evidencePath, manifestDir);
    const evidenceStat = await stat(resolved);
    if (!evidenceStat.isFile() || evidenceStat.size <= 0) {
      throw new Error(`visual capture evidence should be a non-empty file: ${resolved}`);
    }
  }
}

function resolveManualReportPath(manualReportPath, reportDir) {
  return resolveEvidencePath(manualReportPath, reportDir);
}

function resolveEvidencePath(value, relativeDir) {
  if (path.isAbsolute(value)) {
    return value;
  }
  if (value.includes(path.sep)) {
    return path.resolve(value);
  }
  return path.resolve(relativeDir, value);
}

function requireReadyVisualState(state, expected) {
  requireObject(state, `visualManifest.captures.${expected.label}.state`);
  const dpr = expected.deviceScaleFactor;
  requireStringIncludes(state.className, "runtime-ready", `${expected.label}.state.className`);
  requireAtLeast(Number(state.frameCount), 2, `${expected.label}.state.frameCount`);
  requireAtLeast(
    Number(state.canvasClientWidth),
    expected.width - 2,
    `${expected.label}.state.canvasClientWidth`
  );
  requireAtLeast(
    Number(state.canvasClientHeight),
    expected.height - 2,
    `${expected.label}.state.canvasClientHeight`
  );
  requireAtLeast(
    Number(state.canvasWidth),
    expected.width * dpr - 2,
    `${expected.label}.state.canvasWidth`
  );
  requireAtLeast(
    Number(state.canvasHeight),
    expected.height * dpr - 2,
    `${expected.label}.state.canvasHeight`
  );
  requireEqual(
    String(state.visualSnapshotReady),
    "1",
    `${expected.label}.state.visualSnapshotReady`
  );
  requireAtLeast(
    Number(state.visualSnapshotBytes),
    1000,
    `${expected.label}.state.visualSnapshotBytes`
  );
}

function requireVisualEvidence(capture, label) {
  if (capture.screenshot) {
    requireObject(capture.imageStats, `${label}.imageStats`);
    requireAtLeast(
      Number(capture.imageStats.nonTransparentPixels),
      1,
      `${label}.imageStats.nonTransparentPixels`
    );
    requireAtLeast(Number(capture.imageStats.rgbRange), 1, `${label}.imageStats.rgbRange`);
    return;
  }

  requireEqual(capture.mode, "paint-snapshot-svg", `${label}.mode`);
  requireTruthy(capture.snapshot, `${label}.snapshot`);
  requireObject(capture.snapshotStats, `${label}.snapshotStats`);
  requireAtLeast(Number(capture.snapshotStats.bytes), 1000, `${label}.snapshotStats.bytes`);
  requireAtLeast(Number(capture.snapshotStats.itemCount), 10, `${label}.snapshotStats.itemCount`);
  if (Number(capture.snapshotStats.unsupportedCount) >= Number(capture.snapshotStats.itemCount)) {
    throw new Error(`${label}.snapshotStats unsupported items should not cover the whole capture`);
  }
}

function requireUniqueStepNames(steps) {
  const seen = new Set();
  for (let index = 0; index < steps.length; index += 1) {
    const name = steps[index]?.name;
    requireTruthy(name, `steps.${index}.name`);
    if (seen.has(name)) {
      throw new Error(`steps.${name} should appear exactly once`);
    }
    seen.add(name);
  }
}

function requireEveryStepPassed(steps) {
  for (const step of steps) {
    requireEqual(step.exitCode, 0, `steps.${step.name}.exitCode`);
  }
}

function normalizeCompletionUrl(value) {
  const url = new URL(value);
  url.hash = "";
  url.search = "";
  if (!url.pathname.endsWith("/")) {
    url.pathname = `${url.pathname}/`;
  }
  return url.href;
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

function requireStringIncludes(value, expected, label) {
  if (!String(value ?? "").includes(expected)) {
    throw new Error(`${label} should include ${JSON.stringify(expected)}`);
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

function requireAtLeast(actual, expected, label) {
  if (!Number.isFinite(actual) || actual < expected) {
    throw new Error(`${label} expected at least ${expected}, got ${JSON.stringify(actual)}`);
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
