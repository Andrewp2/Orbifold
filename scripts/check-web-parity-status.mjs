#!/usr/bin/env node

import { pathToFileURL } from "node:url";
import { readFile } from "node:fs/promises";
import { resolveReportPath, validateManualDeviceReport } from "./check-web-manual-report.mjs";
import {
  resolveParityCompletionReportPath,
  validateParityCompletionReportFile,
} from "./check-web-parity-complete.mjs";
import { normalizeWebRootHref } from "./web-artifact-fingerprint.mjs";

if (isCliEntrypoint()) {
  try {
    const options = parseWebParityStatusArgs(process.argv.slice(2));
    if (options.help) {
      console.error("usage: scripts/check-web-parity-status.mjs [reports-dir] [--url https://pages-url/]");
      process.exit(2);
    }
    const status = await inspectWebParityStatus(options.target, {
      expectedUrl: options.expectedUrl,
    });
    printWebParityStatus(status);
    process.exit(status.complete ? 0 : 1);
  } catch (error) {
    console.error(`web parity status failed: ${error.message ?? error}`);
    process.exit(2);
  }
}

export function parseWebParityStatusArgs(args) {
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
      parsed.expectedUrl = arg.slice("--url=".length);
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

export async function inspectWebParityStatus(target = "reports", options = {}) {
  const status = {
    target,
    expectedUrl: options.expectedUrl ?? "",
    complete: false,
    manualReport: await inspectManualReport(target, options),
    completionReport: await inspectCompletionReport(target, options),
  };
  status.complete = status.manualReport.ok && status.completionReport.ok;
  return status;
}

async function inspectManualReport(target, options) {
  try {
    const path = await resolveReportPath(target);
    const report = JSON.parse(await readFile(path, "utf8"));
    validateManualDeviceReport(report);
    if (
      options.expectedUrl &&
      normalizeWebRootHref(report.targetUrl) !== normalizeWebRootHref(options.expectedUrl)
    ) {
      throw new Error(
        `manual report target ${normalizeWebRootHref(
          report.targetUrl
        )} does not match expected ${normalizeWebRootHref(options.expectedUrl)}`
      );
    }
    return {
      ok: true,
      path,
      targetUrl: report.targetUrl,
      generatedAt: report.generatedAt,
      checkCount: report.checks.length,
    };
  } catch (error) {
    return { ok: false, error: String(error?.message ?? error) };
  }
}

async function inspectCompletionReport(target, options) {
  try {
    const path = await resolveParityCompletionReportPath(target);
    await validateParityCompletionReportFile(path, { expectedUrl: options.expectedUrl });
    return { ok: true, path };
  } catch (error) {
    return { ok: false, error: String(error?.message ?? error) };
  }
}

export function printWebParityStatus(status) {
  console.log(`web parity status: ${status.complete ? "complete" : "incomplete"}`);
  console.log(`reports target: ${status.target}`);
  if (status.expectedUrl) {
    console.log(`expected URL: ${status.expectedUrl}`);
  }
  printItem("manual device report", status.manualReport, formatManualReport);
  printItem("completion gate report", status.completionReport, (item) => item.path);
  if (!status.complete) {
    console.log("");
    console.log("next required evidence:");
    if (!status.manualReport.ok) {
      console.log(`- run ${manualDeviceCommand(status)} with real Web Audio and Web MIDI hardware`);
      console.log("- then run scripts/check-web-manual-report.mjs reports/");
      return;
    }
    console.log(`- run ${parityGateCommand(status)}`);
    console.log(`- then run ${parityCompleteCommand(status)}`);
  }
}

function printItem(label, item, format) {
  if (item.ok) {
    console.log(`[ok] ${label}: ${format(item)}`);
  } else {
    console.log(`[missing] ${label}: ${item.error}`);
  }
}

function formatManualReport(item) {
  return `${item.path} (${item.checkCount} checks, target ${item.targetUrl}, ${item.generatedAt})`;
}

function manualDeviceCommand(status) {
  if (status.expectedUrl) {
    return `scripts/check-web-manual-devices.mjs ${status.expectedUrl}`;
  }
  return "scripts/check-web-manual-devices.mjs against the deployed Pages URL";
}

function parityGateCommand(status) {
  if (status.expectedUrl) {
    return `scripts/check-web-parity-gate.mjs ${status.expectedUrl} --report reports/`;
  }
  return "scripts/check-web-parity-gate.mjs against the deployed Pages URL with --report reports/";
}

function parityCompleteCommand(status) {
  if (status.expectedUrl) {
    return `scripts/check-web-parity-complete.mjs reports/ --url ${status.expectedUrl}`;
  }
  return "scripts/check-web-parity-complete.mjs reports/";
}

function isCliEntrypoint() {
  return process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
}
