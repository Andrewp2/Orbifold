#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { resolveReportPath } from "./check-web-manual-report.mjs";
import {
  compareWebArtifactFingerprints,
  fetchWebArtifactFingerprint,
} from "./web-artifact-fingerprint.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..");
const options = parseArgs(process.argv.slice(2));

if (!options.url) {
  console.error(
    "usage: scripts/check-web-parity-gate.mjs <https://pages-url/> [--report reports] [--visual-out screenshots/web-parity] [--skip-visual-capture]"
  );
  process.exit(2);
}

const gateReport = {
  schema: "orbifold.web_parity_gate.v1",
  generatedAt: new Date().toISOString(),
  targetUrl: options.url,
  manualReport: options.report,
  visualOut: options.visualOut,
  skippedVisualCapture: options.skipVisualCapture,
  steps: [],
};

try {
  if (options.skipVisualCapture) {
    recordSkippedVisualCapture();
    throw new Error("visual capture was skipped; rerun without --skip-visual-capture for parity");
  }
  await runStep("manualDeviceReport", ["check-web-manual-report.mjs", options.report]);
  await verifyManualReportTarget(options.report, options.url);
  await verifyManualReportArtifact(options.report, options.url);
  await runStep("deployedArtifact", ["check-web-live.mjs", options.url]);
  await runStep("deployedLayout", ["check-web-layout.mjs", options.url]);
  await runStep("deployedSmoke", ["check-web-smoke.mjs", options.url]);
  await runStep("deployedVisualCapture", [
    "capture-web-visuals.mjs",
    options.url,
    "--out",
    options.visualOut,
  ]);
  gateReport.passed = true;
  console.log("\nOrbifold web parity gate passed.");
} catch (error) {
  gateReport.passed = false;
  gateReport.error = String(error?.stack || error?.message || error);
  console.error(`\nOrbifold web parity gate failed: ${error?.message ?? error}`);
  process.exitCode = 1;
} finally {
  const reportPath = writeGateReport(gateReport);
  console.log(`Web parity gate report: ${reportPath}`);
}

function parseArgs(args) {
  const parsed = {
    url: "",
    report: "reports",
    visualOut: "screenshots/web-parity",
    skipVisualCapture: false,
  };
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--report") {
      parsed.report = args[++index] ?? parsed.report;
    } else if (arg.startsWith("--report=")) {
      parsed.report = arg.slice("--report=".length);
    } else if (arg === "--visual-out") {
      parsed.visualOut = args[++index] ?? parsed.visualOut;
    } else if (arg.startsWith("--visual-out=")) {
      parsed.visualOut = arg.slice("--visual-out=".length);
    } else if (arg === "--skip-visual-capture") {
      parsed.skipVisualCapture = true;
    } else if (arg === "--help" || arg === "-h") {
      parsed.url = "";
      return parsed;
    } else if (!arg.startsWith("--") && !parsed.url) {
      parsed.url = arg;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return parsed;
}

function runStep(name, scriptAndArgs) {
  const scriptPath = path.join(scriptDir, scriptAndArgs[0]);
  const args = [scriptPath, ...scriptAndArgs.slice(1)];
  const startedAt = new Date();
  const command = [process.execPath, ...args];
  console.log(`\n[${name}] ${command.map(shellQuote).join(" ")}`);

  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, args, {
      cwd: repoRoot,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      const text = String(chunk);
      stdout += text;
      process.stdout.write(text);
    });
    child.stderr.on("data", (chunk) => {
      const text = String(chunk);
      stderr += text;
      process.stderr.write(text);
    });
    child.on("error", reject);
    child.on("close", (exitCode, signal) => {
      const step = {
        name,
        command,
        startedAt: startedAt.toISOString(),
        endedAt: new Date().toISOString(),
        exitCode,
        signal,
        stdoutTail: stdout.slice(-4000),
        stderrTail: stderr.slice(-4000),
      };
      gateReport.steps.push(step);
      if (exitCode === 0) {
        resolve(step);
      } else {
        reject(new Error(`${name} failed with exit ${exitCode}${signal ? ` signal ${signal}` : ""}`));
      }
    });
  });
}

function recordSkippedVisualCapture() {
  gateReport.steps.push({
    name: "deployedVisualCapture",
    command: ["skipped"],
    startedAt: new Date().toISOString(),
    endedAt: new Date().toISOString(),
    exitCode: 1,
    signal: null,
    stdoutTail: "",
    stderrTail: "visual capture skipped; this is not complete web parity evidence",
  });
}

function writeGateReport(report) {
  const outDir = "reports";
  fs.mkdirSync(outDir, { recursive: true });
  const stamp = report.generatedAt.replace(/[:.]/g, "-");
  const reportPath = path.join(outDir, `web-parity-gate-${stamp}.json`);
  fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  return reportPath;
}

async function verifyManualReportTarget(reportTarget, expectedUrl) {
  const reportPath = await resolveReportPath(reportTarget);
  const report = JSON.parse(await fs.promises.readFile(reportPath, "utf8"));
  const actual = normalizeUrl(report.targetUrl);
  const expected = normalizeUrl(expectedUrl);
  const step = {
    name: "manualReportTarget",
    command: ["read", reportPath],
    startedAt: new Date().toISOString(),
    endedAt: new Date().toISOString(),
    exitCode: actual === expected ? 0 : 1,
    signal: null,
    stdoutTail: `manual report target ${actual}`,
    stderrTail: "",
  };
  gateReport.steps.push(step);
  gateReport.manualReportPath = reportPath;
  if (actual !== expected) {
    throw new Error(`manual report target ${actual} does not match gate target ${expected}`);
  }
}

async function verifyManualReportArtifact(reportTarget, expectedUrl) {
  const reportPath = await resolveReportPath(reportTarget);
  const report = JSON.parse(await fs.promises.readFile(reportPath, "utf8"));
  const actual = await fetchWebArtifactFingerprint(expectedUrl);
  const differences = compareWebArtifactFingerprints(actual, report.artifact);
  const step = {
    name: "manualReportArtifact",
    command: ["fetch", expectedUrl],
    startedAt: new Date().toISOString(),
    endedAt: new Date().toISOString(),
    exitCode: differences.length === 0 ? 0 : 1,
    signal: null,
    stdoutTail:
      differences.length === 0
        ? `manual report artifact matches live ${normalizeUrl(expectedUrl)}`
        : "",
    stderrTail: differences.slice(0, 10).join("\n"),
  };
  gateReport.steps.push(step);
  gateReport.liveArtifact = actual;
  if (differences.length > 0) {
    throw new Error(
      `manual report artifact does not match the current deployed artifact: ${differences[0]}`
    );
  }
}

function normalizeUrl(value) {
  const url = new URL(value);
  url.hash = "";
  url.search = "";
  if (!url.pathname.endsWith("/")) {
    url.pathname = `${url.pathname}/`;
  }
  return url.href;
}

function shellQuote(value) {
  if (/^[A-Za-z0-9_./:=@+-]+$/.test(value)) return value;
  return `'${value.replaceAll("'", "'\\''")}'`;
}
