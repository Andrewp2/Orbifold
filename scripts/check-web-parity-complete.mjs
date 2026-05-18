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

if (isCliEntrypoint()) {
  const target = process.argv[2] ?? "reports";

  if (process.argv.includes("--help") || process.argv.includes("-h")) {
    console.error("usage: scripts/check-web-parity-complete.mjs <reports/web-parity-gate-*.json|reports-dir>");
    process.exit(2);
  }

  try {
    const reportPath = await resolveParityCompletionReportPath(target);
    await validateParityCompletionReportFile(reportPath);
    console.log(`web parity completion evidence ok: ${reportPath}`);
  } catch (error) {
    console.error(`web parity completion evidence failed: ${error.message ?? error}`);
    process.exit(1);
  }
}

export function validateParityCompletionReport(report, options = {}) {
  requireEqual(report.schema, "orbifold.web_parity_gate.v1", "schema");
  requireTruthy(report.generatedAt, "generatedAt");
  requireIsoDate(report.generatedAt, "generatedAt");
  requireTruthy(report.targetUrl, "targetUrl");
  requireEqual(report.passed, true, "passed");
  requireEqual(report.skippedVisualCapture, false, "skippedVisualCapture");
  requireArray(report.steps, "steps");
  requireArtifactFingerprint(report.liveArtifact, "liveArtifact");
  requireTruthy(report.manualReportPath, "manualReportPath");

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

  if (options.manualReport) {
    validateManualDeviceReport(options.manualReport);
    const differences = compareWebArtifactFingerprints(
      report.liveArtifact,
      options.manualReport.artifact
    );
    if (differences.length > 0) {
      throw new Error(`manual report artifact should match live artifact: ${differences[0]}`);
    }
  }
}

export async function validateParityCompletionReportFile(reportPath) {
  const report = JSON.parse(await readFile(reportPath, "utf8"));
  const reportDir = path.dirname(reportPath);
  const manualReportPath = resolveManualReportPath(report.manualReportPath, reportDir);
  const manualReport = JSON.parse(await readFile(manualReportPath, "utf8"));
  validateParityCompletionReport(report, { reportDir, manualReport });
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

function resolveManualReportPath(manualReportPath, reportDir) {
  if (path.isAbsolute(manualReportPath)) {
    return manualReportPath;
  }
  if (manualReportPath.includes(path.sep)) {
    return path.resolve(manualReportPath);
  }
  return path.resolve(reportDir, manualReportPath);
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

function requireIsoDate(value, label) {
  if (!value || Number.isNaN(Date.parse(value))) {
    throw new Error(`${label} should be an ISO date, got ${JSON.stringify(value)}`);
  }
}

function isCliEntrypoint() {
  return process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
}
