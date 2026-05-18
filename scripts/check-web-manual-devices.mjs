#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import readline from "node:readline/promises";
import { spawn } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { validateManualDeviceReport } from "./check-web-manual-report.mjs";
import { fetchWebArtifactFingerprint } from "./web-artifact-fingerprint.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..");
const manualEvidenceRequirements =
  "real Web Audio output, Web MIDI hardware, file-flow, shortcut, and piano-roll checks";

let stdout = "";
let stderr = "";
let options = null;
let timeoutMs = numberFromEnv("ORBIFOLD_WEB_MANUAL_TIMEOUT_MS", 60_000);
let devtoolsTimeoutMs = numberFromEnv("ORBIFOLD_CHROME_DEVTOOLS_TIMEOUT_MS", 20_000);
let profile = "";
let rl = null;
let report = null;
let chrome = null;
let pageSession = null;
let send = null;

if (isCliEntrypoint()) {
  try {
    await runManualDeviceCli(process.argv.slice(2));
  } catch (error) {
    console.error(`manual web device runner failed: ${error.message ?? error}`);
    process.exit(1);
  }
}

export async function runManualDeviceCli(args) {
  options = parseManualDeviceArgs(args);
  if (!options.url) {
    console.error(
      "usage: scripts/check-web-manual-devices.mjs <url> [--out reports] [--keep-open] [--preflight] [--finalize]"
    );
    process.exit(2);
  }

  if (options.preflight) {
    const preflight = await runManualDevicePreflight(options.url);
    printManualDevicePreflight(preflight);
    if (!preflight.passed) {
      process.exitCode = 1;
    }
    return;
  }

  if (!process.stdin.isTTY) {
    console.error("This manual device check requires an interactive terminal.");
    process.exit(2);
  }

  if (typeof WebSocket !== "function") {
    console.error("Node.js with a global WebSocket implementation is required.");
    process.exit(2);
  }

  const chromePath = findChrome();
  if (!chromePath) {
    console.error("Could not find Chrome. Set CHROME_BIN or install google-chrome/chromium.");
    process.exit(2);
  }

  profile = fs.mkdtempSync(path.join(os.tmpdir(), "orbifold-web-manual-"));
  rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  report = createManualDeviceReport(options.url, chromePath);
  stdout = "";
  stderr = "";
  let manualPassed = false;

  chrome = spawn(
    chromePath,
    [
      "--remote-debugging-port=0",
      "--enable-unsafe-webgpu",
      "--ignore-gpu-blocklist",
      "--disable-dev-shm-usage",
      "--no-first-run",
      "--no-default-browser-check",
      "--window-size=1600,1000",
      `--user-data-dir=${profile}`,
      "about:blank",
    ],
    { stdio: ["ignore", "pipe", "pipe"] }
  );

  chrome.stdout.on("data", (chunk) => {
    stdout += chunk;
  });
  chrome.stderr.on("data", (chunk) => {
    stderr += chunk;
  });

  try {
    const browserWsUrl = await waitForDevtoolsEndpoint();
    ({ send, pageSession } = await connectToPage(browserWsUrl));
    await runManualDeviceCheck();
    manualPassed = true;
  } catch (error) {
    report.error = String(error?.stack || error?.message || error);
    setCheck("manualDeviceVerifierCompleted", false, { error: report.error });
    process.exitCode = 1;
  } finally {
    const reportPath = writeReport(report, options.outDir);
    console.log(`\nManual device parity report: ${reportPath}`);
    rl.close();
    if (options.keepOpen) {
      console.log("Chrome left open because --keep-open was supplied.");
    } else {
      await terminateChrome(chrome);
      await removeProfile(profile);
    }
    if (manualPassed && options.finalize) {
      await runManualDeviceFinalizers(options.url, reportPath);
    } else if (manualPassed) {
      printManualDeviceNextSteps(options.url, reportPath);
    }
  }
}

async function runManualDeviceCheck() {
  const browserVersion = await send("Browser.getVersion");
  report.chrome.version = browserVersion.product;
  report.chrome.userAgent = browserVersion.userAgent;
  report.chrome.protocolVersion = browserVersion.protocolVersion;
  report.artifact = await fetchWebArtifactFingerprint(options.url);

  await send("Page.navigate", { url: options.url }, pageSession);
  await waitForOrbifoldReady();
  report.states.runtime = await evaluateRuntimeState();
  report.states.initial = await evaluateProjectState();
  addCheck("browserRuntimeReady", true, pickRuntimeEvidence(report.states.runtime));

  console.log("\nA Chrome window is open for manual web parity checks.");
  console.log("This is not a CI check: it requires real audio, real Web MIDI, and your ears.");
  report.states.beforeVisualInspection = await evaluateManualVisualState();
  await promptEnter(
    "Inspect the UI for full-window canvas coverage and obvious text overlap, then press Enter."
  );
  const initialVisualOk = await confirm("Does the browser UI look usable at this size?");
  await promptEnter(
    "Resize Chrome or fullscreen it to the largest/high-DPI or 4K-like size available, inspect layout scale, canvas coverage, and text overlap again, then press Enter."
  );
  await delay(1000);
  report.states.afterLargeVisualInspection = await evaluateManualVisualState();
  const largeVisualOk = await confirm("Does the resized/high-DPI browser UI still look usable?");
  const visualOk =
    initialVisualOk &&
    largeVisualOk &&
    manualVisualStateLooksClean(report.states.beforeVisualInspection) &&
    manualVisualStateLooksClean(report.states.afterLargeVisualInspection) &&
    manualVisualStateShowsResize(
      report.states.beforeVisualInspection,
      report.states.afterLargeVisualInspection
    );
  report.userConfirmations.visualInspection = visualOk;
  addCheck("manualVisualInspection", visualOk, {
    initial: report.states.beforeVisualInspection,
    inspectedLarge: report.states.afterLargeVisualInspection,
  });

  await clickManualControl("viewDevices");
  await delay(500);

  await clickManualControl("audioRefresh");
  await promptEnter(
    "If Chrome asks for audio-output permission, grant it. Select the desired audio output in Orbifold if needed, then press Enter."
  );
  report.states.afterAudioRefresh = await evaluateProjectState();
  const audioOutputsDiscovered =
    report.states.afterAudioRefresh.audioOutputCount > 0 &&
    report.states.afterAudioRefresh.browserAudioOutputNames.length > 0;
  addCheck("webAudioOutputsDiscovered", audioOutputsDiscovered, {
    audioOutputCount: report.states.afterAudioRefresh.audioOutputCount,
    browserAudioOutputNames: report.states.afterAudioRefresh.browserAudioOutputNames,
    browserAudioDiagnostic: report.states.afterAudioRefresh.browserAudioDiagnostic,
    lastStatus: report.states.afterAudioRefresh.lastStatus,
  });

  await clickManualControl("audioConnect");
  await delay(1200);
  report.states.afterAudioConnect = await evaluateProjectState();
  const audioConnected =
    report.states.afterAudioConnect.audioStreamConnected &&
    report.states.afterAudioConnect.audioContextCreated &&
    report.states.afterAudioConnect.audioProcessorAttached &&
    report.states.afterAudioConnect.audioResumeRequested &&
    report.states.afterAudioConnect.audioResumeResolved;
  addCheck("webAudioConnectedState", audioConnected, {
    connectedAudioOutput: report.states.afterAudioConnect.connectedAudioOutput,
    browserAudioDiagnostic: report.states.afterAudioConnect.browserAudioDiagnostic,
    audioContextCreated: report.states.afterAudioConnect.audioContextCreated,
    audioProcessorAttached: report.states.afterAudioConnect.audioProcessorAttached,
    audioResumeRequested: report.states.afterAudioConnect.audioResumeRequested,
    audioResumeResolved: report.states.afterAudioConnect.audioResumeResolved,
    lastStatus: report.states.afterAudioConnect.lastStatus,
  });

  await clickManualControl("audioTestA4");
  await delay(1500);
  report.states.afterAudioTest = await evaluateProjectState();
  const heardA4 = await confirm("Did you hear the A4 test tone from the selected browser output?");
  report.userConfirmations.audibleA4 = heardA4;
  const audibleWebAudio =
    heardA4 &&
    report.states.afterAudioTest.audioNonzero &&
    report.states.afterAudioTest.audioCallbackCount > 0 &&
    report.states.afterAudioTest.audioFrameCount > 0 &&
    report.states.afterAudioTest.audioPeak > 0;
  addCheck("manualAudibleWebAudio", audibleWebAudio, {
    audioCallbackCount: report.states.afterAudioTest.audioCallbackCount,
    audioFrameCount: report.states.afterAudioTest.audioFrameCount,
    audioPeak: report.states.afterAudioTest.audioPeak,
    audioNonzero: report.states.afterAudioTest.audioNonzero,
    lastStatus: report.states.afterAudioTest.lastStatus,
  });

  await clickManualControl("midiRefresh");
  await promptEnter(
    "Grant Web MIDI permission if prompted, connect a real MIDI device, then press Enter."
  );
  await clickManualControl("midiRefresh");
  await delay(1000);
  report.states.afterMidiRefresh = await evaluateProjectState();
  const midiInputsDiscovered =
    report.states.afterMidiRefresh.midiInputCount > 0 &&
    report.states.afterMidiRefresh.browserMidiInputNames.length > 0;
  addCheck("webMidiInputsDiscovered", midiInputsDiscovered, {
    midiInputCount: report.states.afterMidiRefresh.midiInputCount,
    browserMidiInputNames: report.states.afterMidiRefresh.browserMidiInputNames,
    browserMidiDiagnostic: report.states.afterMidiRefresh.browserMidiDiagnostic,
    lastStatus: report.states.afterMidiRefresh.lastStatus,
  });

  await clickManualControl("midiConnect");
  await delay(1000);
  report.states.afterMidiConnect = await evaluateProjectState();
  const midiConnected =
    report.states.afterMidiConnect.connectedMidiInput.length > 0 &&
    report.states.afterMidiConnect.midiInputConnection.length > 0;
  addCheck("webMidiConnectedState", midiConnected, {
    connectedMidiInput: report.states.afterMidiConnect.connectedMidiInput,
    midiInputConnection: report.states.afterMidiConnect.midiInputConnection,
    browserMidiDiagnostic: report.states.afterMidiConnect.browserMidiDiagnostic,
    lastStatus: report.states.afterMidiConnect.lastStatus,
  });

  const beforeMidiNote = await evaluateProjectState();
  await promptEnter(
    "Play and release a note on the real MIDI device. Wait until Orbifold's MIDI status changes, then press Enter."
  );
  report.states.afterRealMidiNote = await evaluateProjectState();
  const midiStatusChanged =
    report.states.afterRealMidiNote.lastMidiStatus !== beforeMidiNote.lastMidiStatus ||
    report.states.afterRealMidiNote.lastMidiNote !== beforeMidiNote.lastMidiNote;
  const userSawMidi = await confirm("Did Orbifold visibly report the real MIDI note?");
  report.userConfirmations.realMidiNoteVisible = userSawMidi;
  addCheck("manualRealMidiInput", userSawMidi && midiStatusChanged, {
    before: {
      lastMidiStatus: beforeMidiNote.lastMidiStatus,
      lastMidiNote: beforeMidiNote.lastMidiNote,
    },
    after: {
      lastMidiStatus: report.states.afterRealMidiNote.lastMidiStatus,
      lastMidiNote: report.states.afterRealMidiNote.lastMidiNote,
      lastStatus: report.states.afterRealMidiNote.lastStatus,
    },
  });

  const beforeRecording = await evaluateProjectState();
  await clickManualControl("record");
  await promptEnter(
    "Recording is armed. Play and release one MIDI note, wait a moment, then press Enter."
  );
  await clickManualControl("record");
  await delay(800);
  report.states.afterMidiRecording = await evaluateProjectState();
  const recordedNote =
    report.states.afterMidiRecording.noteCount > beforeRecording.noteCount ||
    persistedNoteCount(report.states.afterMidiRecording.project) >
      persistedNoteCount(beforeRecording.project);
  const userSawRecording = await confirm("Did a real MIDI note appear in the clip?");
  report.userConfirmations.realMidiRecordingVisible = userSawRecording;
  addCheck("manualRealMidiRecording", userSawRecording && recordedNote, {
    beforeNoteCount: beforeRecording.noteCount,
    afterNoteCount: report.states.afterMidiRecording.noteCount,
    lastStatus: report.states.afterMidiRecording.lastStatus,
  });

  await promptEnter(
    "Use the browser UI with real file pickers: save/open a .orbifold project, load a Scala .scl scale, load a Lumatone .ltn key map, import a supported WAV asset, reload the page, and confirm the state restores. Press Enter when finished."
  );
  report.states.afterBrowserFileFlows = await evaluateProjectState();
  const browserFileFlows = await confirm(
    "Did browser project/scale/key-map/asset file flows and reload persistence work?"
  );
  report.userConfirmations.browserFileFlows = browserFileFlows;
  addCheck("manualBrowserFileFlows", browserFileFlows, {
    title: report.states.afterBrowserFileFlows.title,
    lastStatus: report.states.afterBrowserFileFlows.lastStatus,
    noteCount: report.states.afterBrowserFileFlows.noteCount,
    assetCount: report.states.afterBrowserFileFlows.assetCount,
    project: report.states.afterBrowserFileFlows.project,
    scaleDescription: report.states.afterBrowserFileFlows.scaleDescription,
    scalaPath: report.states.afterBrowserFileFlows.scalaPath,
    lumatonePath: report.states.afterBrowserFileFlows.lumatonePath,
    lumatoneLoaded: report.states.afterBrowserFileFlows.lumatoneLoaded,
    downloadFileName: report.states.afterBrowserFileFlows.downloadFileName,
    downloadSize: report.states.afterBrowserFileFlows.downloadSize,
  });

  report.states.beforeShortcutParity = await evaluateProjectState();
  await promptEnter(
    "Spot-check browser shortcuts against native behavior for transport, editing, file commands, help, and UI zoom. Leave at least one concrete browser shortcut change visible, such as a note edit, project download, transport toggle, or UI scale change. Press Enter when finished."
  );
  report.states.afterShortcutParity = await evaluateProjectState();
  const shortcutParity = await confirm("Did the browser keyboard shortcuts match native behavior?");
  report.userConfirmations.shortcutParity = shortcutParity;
  addCheck("manualShortcutParity", shortcutParity, {
    requiredWorkflows: ["transport", "editing", "file", "help", "uiZoom"],
    before: pickShortcutParityEvidence(report.states.beforeShortcutParity),
    after: pickShortcutParityEvidence(report.states.afterShortcutParity),
  });

  report.states.beforePianoRollParity = await evaluateProjectState();
  await promptEnter(
    "Compare browser and native piano-roll/workspace behavior: create/select/move/resize a note, edit velocity, scroll or zoom the piano view, seek or drag a loop boundary, and resize the right or bottom workspace panel. Leave those browser changes visible. Press Enter when finished."
  );
  report.states.afterPianoRollParity = await evaluateProjectState();
  const pianoRollParity = await confirm(
    "Did browser piano-roll and workspace interactions match native behavior?"
  );
  report.userConfirmations.pianoRollParity = pianoRollParity;
  addCheck("manualPianoRollParity", pianoRollParity, {
    requiredWorkflows: ["noteEdit", "velocityEdit", "scrollOrZoom", "seekOrLoop", "panelResize"],
    before: pickPianoRollParityEvidence(report.states.beforePianoRollParity),
    after: pickPianoRollParityEvidence(report.states.afterPianoRollParity),
  });

  const passed = report.checks.every((check) => check.pass);
  addCheck("manualDeviceVerifierCompleted", passed, {
    passedChecks: report.checks.filter((check) => check.pass).length,
    totalChecks: report.checks.length,
  });

  if (!passed) {
    throw new Error("Manual device parity checks did not all pass.");
  }
  validateManualDeviceReport(report);

  console.log("\nManual web device parity checks passed and report evidence validated.");
}

export function parseManualDeviceArgs(args) {
  const parsed = {
    url: null,
    outDir: "reports",
    keepOpen: false,
    preflight: false,
    finalize: false,
  };
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--out") {
      const value = args[++index];
      if (!value || value.startsWith("--")) {
        throw new Error("--out requires a value");
      }
      parsed.outDir = value;
    } else if (arg.startsWith("--out=")) {
      const value = arg.slice("--out=".length);
      if (!value) {
        throw new Error("--out requires a value");
      }
      parsed.outDir = value;
    } else if (arg === "--keep-open") {
      parsed.keepOpen = true;
    } else if (arg === "--preflight") {
      parsed.preflight = true;
    } else if (arg === "--finalize") {
      parsed.finalize = true;
    } else if (arg === "--help" || arg === "-h") {
      parsed.url = null;
      return parsed;
    } else if (!arg.startsWith("--") && !parsed.url) {
      parsed.url = arg;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return parsed;
}

export async function runManualDevicePreflight(url) {
  const checks = [];
  checks.push({
    name: "node-websocket",
    passed: typeof WebSocket === "function",
    detail:
      typeof WebSocket === "function"
        ? "Node.js global WebSocket is available"
        : "Node.js with a global WebSocket implementation is required",
  });

  const chromePath = findChrome();
  checks.push({
    name: "chrome",
    passed: Boolean(chromePath),
    detail: chromePath || "Chrome not found; set CHROME_BIN or install google-chrome/chromium",
  });

  try {
    const artifact = await fetchWebArtifactFingerprint(url);
    checks.push({
      name: "deployed-artifact",
      passed: true,
      detail: `${artifact.files.wasm.bytes} byte wasm artifact at ${artifact.rootUrl}`,
    });
    return {
      url,
      chromePath,
      artifact,
      checks,
      passed: checks.every((check) => check.passed),
    };
  } catch (error) {
    checks.push({
      name: "deployed-artifact",
      passed: false,
      detail: String(error?.message ?? error),
    });
    return {
      url,
      chromePath,
      artifact: null,
      checks,
      passed: false,
    };
  }
}

export function printManualDevicePreflight(preflight) {
  console.log(`manual web device preflight: ${preflight.passed ? "ok" : "failed"}`);
  console.log(`target: ${preflight.url}`);
  for (const check of preflight.checks) {
    console.log(`[${check.passed ? "ok" : "fail"}] ${check.name}: ${check.detail}`);
  }
  if (preflight.passed) {
    console.log(
      `preflight ok; rerun without --preflight in an interactive terminal with ${manualEvidenceRequirements}`
    );
  }
}

export function createManualDeviceReport(
  targetUrl,
  chromePath,
  generatedAt = new Date().toISOString()
) {
  return {
    schema: "orbifold.web_manual_device_parity.v1",
    generatedAt,
    targetUrl,
    host: {
      platform: process.platform,
      arch: process.arch,
      release: os.release(),
    },
    chrome: {
      path: chromePath,
    },
    checks: [],
    clicks: [],
    states: {},
    userConfirmations: {},
    browserEvents: [],
  };
}

function numberFromEnv(name, fallback) {
  const raw = process.env[name];
  if (!raw) return fallback;
  const parsed = Number(raw);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function findChrome() {
  const candidates = [
    process.env.CHROME_BIN,
    "/usr/bin/google-chrome",
    "/usr/bin/google-chrome-stable",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ].filter(Boolean);
  return candidates.find((candidate) => fs.existsSync(candidate));
}

function waitForDevtoolsEndpoint() {
  return new Promise((resolve, reject) => {
    const deadline = Date.now() + devtoolsTimeoutMs;
    const timer = setInterval(() => {
      const text = `${stderr}\n${stdout}`;
      const match = text.match(/DevTools listening on (ws:\/\/[^\s]+)/);
      if (match) {
        clearInterval(timer);
        resolve(match[1]);
      } else if (Date.now() > deadline) {
        clearInterval(timer);
        reject(
          new Error(
            `Chrome did not publish a DevTools endpoint. Recent output:\n${text.slice(-2000)}`
          )
        );
      }
    }, 50);
  });
}

async function connectToPage(browserWsUrl) {
  const ws = new WebSocket(browserWsUrl);
  let id = 0;
  const pending = new Map();
  const requestUrls = new Map();

  function sendMessage(method, params = {}, sessionId = undefined) {
    const message = { id: ++id, method, params };
    if (sessionId) message.sessionId = sessionId;
    ws.send(JSON.stringify(message));
    return new Promise((resolve, reject) => {
      pending.set(message.id, { method, resolve, reject });
    });
  }

  ws.onmessage = (messageEvent) => {
    const message = JSON.parse(messageEvent.data);
    if (message.id && pending.has(message.id)) {
      const pendingMessage = pending.get(message.id);
      pending.delete(message.id);
      if (message.error) {
        pendingMessage.reject(
          new Error(`${pendingMessage.method}: ${JSON.stringify(message.error)}`)
        );
      } else {
        pendingMessage.resolve(message.result);
      }
      return;
    }

    if (message.method === "Network.requestWillBeSent") {
      requestUrls.set(message.params.requestId, message.params.request.url);
    }
    if (
      message.method === "Runtime.consoleAPICalled" ||
      message.method === "Runtime.exceptionThrown" ||
      message.method === "Log.entryAdded" ||
      message.method === "Network.loadingFailed"
    ) {
      report.browserEvents.push({
        ...message,
        url: requestUrls.get(message.params?.requestId),
      });
    }
  };

  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = reject;
  });

  const { targetId } = await sendMessage("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await sendMessage("Target.attachToTarget", { targetId, flatten: true });
  await sendMessage("Runtime.enable", {}, sessionId);
  await sendMessage("Log.enable", {}, sessionId);
  await sendMessage("Network.enable", {}, sessionId);
  await sendMessage("Page.enable", {}, sessionId);
  await sendMessage("Page.bringToFront", {}, sessionId);
  return { send: sendMessage, pageSession: sessionId };
}

async function waitForOrbifoldReady() {
  const deadline = Date.now() + timeoutMs;
  let lastState = null;
  while (Date.now() <= deadline) {
    lastState = await evaluateRuntimeState();
    if (
      lastState.className.includes("runtime-ready") &&
      Number(lastState.frameCount) >= 2 &&
      lastState.keyboardShortcuts === "installed" &&
      lastState.canvasWidth > 0 &&
      lastState.canvasHeight > 0 &&
      lastState.hasGpu
    ) {
      return;
    }
    if (lastState.className.includes("runtime-failed")) {
      throw new Error(`runtime failed: ${lastState.status}`);
    }
    await delay(250);
  }
  throw new Error(
    `runtime did not become ready within ${timeoutMs}ms; last state: ${JSON.stringify(
      lastState
    )}`
  );
}

async function evaluateRuntimeState() {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `({
        className: document.body?.className ?? "",
        status: document.getElementById("orbifold-status")?.textContent ?? "",
        frameCount: Number(document.body?.dataset.orbifoldFrameCount ?? 0),
        keyboardShortcuts: document.body?.dataset.orbifoldKeyboardShortcuts ?? "",
        canvasWidth: document.getElementById("orbifold-canvas")?.width ?? 0,
        canvasHeight: document.getElementById("orbifold-canvas")?.height ?? 0,
        canvasClientWidth: document.getElementById("orbifold-canvas")?.clientWidth ?? 0,
        canvasClientHeight: document.getElementById("orbifold-canvas")?.clientHeight ?? 0,
        devicePixelRatio: window.devicePixelRatio || 1,
        hasGpu: !!navigator.gpu
      })`,
      returnByValue: true,
    },
    pageSession
  );
  return result.result.value;
}

async function evaluateProjectState() {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `({
        title: document.title,
        className: document.body?.className ?? "",
        lastAction: document.body.dataset.orbifoldLastAction ?? "",
        noteCount: Number(document.body.dataset.orbifoldProjectNoteCount ?? 0),
        assetCount: Number(document.body.dataset.orbifoldAudioAssetCount ?? 0),
        project: localStorage.getItem("orbifold.project.v1") || "",
        settings: localStorage.getItem("orbifold.settings.v1") || "",
        lastStatus: document.body.dataset.orbifoldLastStatus ?? "",
        frameCount: Number(document.body.dataset.orbifoldFrameCount ?? 0),
        transportPlaying: document.body.dataset.orbifoldTransportPlaying === "1",
        transportPositionBeats: Number(document.body.dataset.orbifoldTransportPositionBeats ?? 0),
        loopBeats: Number(document.body.dataset.orbifoldLoopBeats ?? 0),
        uiScale: Number(document.body.dataset.orbifoldUiScale ?? 0),
        showAssetBrowser: document.body.dataset.orbifoldShowAssetBrowser === "1",
        showScaleBrowser: document.body.dataset.orbifoldShowScaleBrowser === "1",
        showClipPanel: document.body.dataset.orbifoldShowClipPanel === "1",
        scaleDescription: document.body.dataset.orbifoldScaleDescription ?? "",
        scalaPath: document.body.dataset.orbifoldScalaPath ?? "",
        lumatonePath: document.body.dataset.orbifoldLumatonePath ?? "",
        lumatoneLoaded: document.body.dataset.orbifoldLumatoneLoaded === "1",
        downloadFileName: document.body.dataset.orbifoldLastDownloadFileName ?? "",
        downloadSize: Number(document.body.dataset.orbifoldLastDownloadSize ?? 0),
        pianoGridWidth: Number(document.body.dataset.orbifoldPianoGridWidth ?? 0),
        pianoGridHeight: Number(document.body.dataset.orbifoldPianoGridHeight ?? 0),
        pianoViewStart: Number(document.body.dataset.orbifoldPianoViewStart ?? 0),
        pianoViewBeats: Number(document.body.dataset.orbifoldPianoViewBeats ?? 0),
        pianoRollHeight: Number(document.body.dataset.orbifoldPianoRollHeight ?? 0),
        rightPanelWidth: Number(document.body.dataset.orbifoldRightPanelWidth ?? 0),
        midiInputCount: Number(document.body.dataset.orbifoldMidiInputCount ?? 0),
        connectedMidiInput: document.body.dataset.orbifoldConnectedMidiInput ?? "",
        browserMidiInputNames: document.body.dataset.orbifoldBrowserMidiInputNames ?? "",
        browserMidiDiagnostic: document.body.dataset.orbifoldBrowserMidiDiagnostic ?? "",
        midiInputConnection: document.body.dataset.orbifoldMidiInputConnection ?? "",
        lastMidiStatus: Number(document.body.dataset.orbifoldLastMidiStatus ?? 0),
        lastMidiNote: Number(document.body.dataset.orbifoldLastMidiNote ?? -1),
        audioOutputCount: Number(document.body.dataset.orbifoldAudioOutputCount ?? 0),
        connectedAudioOutput: document.body.dataset.orbifoldConnectedAudioOutput ?? "",
        browserAudioOutputNames: document.body.dataset.orbifoldBrowserAudioOutputNames ?? "",
        browserAudioDiagnostic: document.body.dataset.orbifoldBrowserAudioDiagnostic ?? "",
        audioSinkRequested: document.body.dataset.orbifoldAudioSinkRequested ?? "",
        audioSinkResolved: document.body.dataset.orbifoldAudioSinkResolved ?? "",
        audioStreamConnected: document.body.dataset.orbifoldAudioStreamConnected === "1",
        audioContextCreated: document.body.dataset.orbifoldAudioContextCreated === "1",
        audioProcessorAttached: document.body.dataset.orbifoldAudioProcessorAttached === "1",
        audioResumeRequested: document.body.dataset.orbifoldAudioResumeRequested === "1",
        audioResumeResolved: document.body.dataset.orbifoldAudioResumeResolved === "1",
        audioCallbackCount: Number(document.body.dataset.orbifoldAudioCallbackCount ?? 0),
        audioFrameCount: Number(document.body.dataset.orbifoldAudioFrameCount ?? 0),
        audioPeak: Number(document.body.dataset.orbifoldAudioPeak ?? 0),
        audioNonzero: document.body.dataset.orbifoldAudioNonzero === "1"
      })`,
      returnByValue: true,
    },
    pageSession
  );
  return result.result.value ?? {};
}

async function evaluateManualVisualState() {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `(() => {
        const canvas = document.getElementById("orbifold-canvas");
        const body = document.body;
        const html = document.documentElement;
        const dataset = body?.dataset ?? {};
        const rect = canvas?.getBoundingClientRect();
        return {
          className: body?.className ?? "",
          frameCount: Number(dataset.orbifoldFrameCount ?? 0),
          viewportWidth: Number(dataset.orbifoldViewportWidth ?? 0),
          viewportHeight: Number(dataset.orbifoldViewportHeight ?? 0),
          uiScale: Number(dataset.orbifoldUiScale ?? 0),
          devicePixelRatio: window.devicePixelRatio || 1,
          innerWidth: window.innerWidth,
          innerHeight: window.innerHeight,
          documentScrollWidth: html.scrollWidth,
          documentScrollHeight: html.scrollHeight,
          canvasClientWidth: canvas?.clientWidth ?? 0,
          canvasClientHeight: canvas?.clientHeight ?? 0,
          canvasWidth: canvas?.width ?? 0,
          canvasHeight: canvas?.height ?? 0,
          canvasLeft: rect?.left ?? 0,
          canvasTop: rect?.top ?? 0,
          canvasRectWidth: rect?.width ?? 0,
          canvasRectHeight: rect?.height ?? 0,
          textAuditReady: dataset.orbifoldTextAuditReady ?? "",
          textAuditCount: Number(dataset.orbifoldTextAuditCount ?? 0),
          textAuditIssueCount: Number(dataset.orbifoldTextAuditIssueCount ?? 0),
          textAuditNonFiniteCount: Number(dataset.orbifoldTextAuditNonFiniteCount ?? 0),
          textAuditSampleIssue: dataset.orbifoldTextAuditSampleIssue ?? ""
        };
      })()`,
      returnByValue: true,
    },
    pageSession
  );
  return result.result.value ?? {};
}

async function evaluateManualControlGeometry() {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `(() => {
        const pointFromDataset = (prefix) => ({
          x: Number(document.body.dataset[prefix + "X"] ?? 0),
          y: Number(document.body.dataset[prefix + "Y"] ?? 0)
        });
        return {
          viewDevices: pointFromDataset("orbifoldManualViewDevices"),
          audioRefresh: pointFromDataset("orbifoldManualAudioRefresh"),
          audioConnect: pointFromDataset("orbifoldManualAudioConnect"),
          audioTestA4: pointFromDataset("orbifoldManualAudioTestA4"),
          midiRefresh: pointFromDataset("orbifoldManualMidiRefresh"),
          midiConnect: pointFromDataset("orbifoldManualMidiConnect"),
          record: pointFromDataset("orbifoldManualRecord")
        };
      })()`,
      returnByValue: true,
    },
    pageSession
  );
  return result.result.value ?? {};
}

async function waitForManualControl(name) {
  const deadline = Date.now() + timeoutMs;
  let geometry = null;
  while (Date.now() <= deadline) {
    geometry = await evaluateManualControlGeometry();
    const point = geometry[name];
    if (point && point.x > 0 && point.y > 0) {
      return point;
    }
    await delay(250);
  }
  throw new Error(`manual control ${name} was not visible; geometry: ${JSON.stringify(geometry)}`);
}

async function clickManualControl(name) {
  const point = await waitForManualControl(name);
  report.clicks.push({ name, point, at: new Date().toISOString() });
  await send("Input.dispatchMouseEvent", {
    type: "mouseMoved",
    x: point.x,
    y: point.y,
    button: "none",
  }, pageSession);
  await send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: point.x,
    y: point.y,
    button: "left",
    buttons: 1,
    clickCount: 1,
  }, pageSession);
  await delay(80);
  await send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    x: point.x,
    y: point.y,
    button: "left",
    buttons: 0,
    clickCount: 1,
  }, pageSession);
}

async function promptEnter(message) {
  await rl.question(`\n${message}\nPress Enter to continue.`);
}

async function confirm(message) {
  while (true) {
    const answer = (await rl.question(`${message} [y/N] `)).trim().toLowerCase();
    if (answer === "y" || answer === "yes") return true;
    if (answer === "" || answer === "n" || answer === "no") return false;
  }
}

function addCheck(name, pass, evidence = {}) {
  if (report.checks.some((check) => check.name === name)) return;
  setCheck(name, pass, evidence);
}

function setCheck(name, pass, evidence = {}) {
  const existing = report.checks.find((check) => check.name === name);
  if (existing) {
    existing.pass = Boolean(pass);
    existing.evidence = evidence;
    return;
  }
  report.checks.push({ name, pass: Boolean(pass), evidence });
}

function pickRuntimeEvidence(state) {
  return {
    frameCount: state.frameCount,
    className: state.className,
    canvas: {
      width: state.canvasWidth,
      height: state.canvasHeight,
      clientWidth: state.canvasClientWidth,
      clientHeight: state.canvasClientHeight,
      devicePixelRatio: state.devicePixelRatio,
    },
    hasGpu: state.hasGpu,
  };
}

function manualVisualStateLooksClean(state) {
  const dpr = Math.max(1, Number(state.devicePixelRatio) || 1);
  return (
    String(state.className ?? "").includes("runtime-ready") &&
    Number(state.frameCount) > 0 &&
    Number(state.canvasClientWidth) > 0 &&
    Number(state.canvasClientHeight) > 0 &&
    Number(state.canvasWidth) >= Number(state.canvasClientWidth) * dpr - 2 &&
    Number(state.canvasHeight) >= Number(state.canvasClientHeight) * dpr - 2 &&
    Number(state.canvasRectWidth) >= Number(state.canvasClientWidth) - 2 &&
    Number(state.canvasRectHeight) >= Number(state.canvasClientHeight) - 2 &&
    state.textAuditReady === "1" &&
    Number(state.textAuditCount) > 0 &&
    Number(state.textAuditIssueCount) === 0 &&
    Number(state.textAuditNonFiniteCount) === 0 &&
    !state.textAuditSampleIssue
  );
}

function manualVisualStateShowsResize(initial, inspectedLarge) {
  return (
    Math.abs(Number(initial.canvasClientWidth) - Number(inspectedLarge.canvasClientWidth)) >= 16 ||
    Math.abs(Number(initial.canvasClientHeight) - Number(inspectedLarge.canvasClientHeight)) >= 16 ||
    Math.abs(Number(initial.devicePixelRatio) - Number(inspectedLarge.devicePixelRatio)) >= 0.1 ||
    Math.abs(Number(initial.uiScale) - Number(inspectedLarge.uiScale)) >= 0.01
  );
}

function pickShortcutParityEvidence(state) {
  return {
    frameCount: state.frameCount,
    lastAction: state.lastAction,
    lastStatus: state.lastStatus,
    noteCount: state.noteCount,
    transportPlaying: state.transportPlaying,
    uiScale: state.uiScale,
    downloadFileName: state.downloadFileName,
    downloadSize: state.downloadSize,
    project: state.project,
  };
}

function pickPianoRollParityEvidence(state) {
  return {
    frameCount: state.frameCount,
    noteCount: state.noteCount,
    project: state.project,
    transportPositionBeats: state.transportPositionBeats,
    loopBeats: state.loopBeats,
    pianoViewStart: state.pianoViewStart,
    pianoViewBeats: state.pianoViewBeats,
    pianoGridWidth: state.pianoGridWidth,
    pianoGridHeight: state.pianoGridHeight,
    pianoRollHeight: state.pianoRollHeight,
    rightPanelWidth: state.rightPanelWidth,
  };
}

export function persistedNoteCount(projectText) {
  return (String(projectText || "").match(/\nnote\t/g) ?? []).length;
}

export function manualDeviceFinalizerCommands(url, reportPath) {
  return [
    {
      name: "manualReport",
      command: [process.execPath, path.join(scriptDir, "check-web-manual-report.mjs"), reportPath],
    },
    {
      name: "parityGate",
      command: [
        process.execPath,
        path.join(scriptDir, "check-web-parity-gate.mjs"),
        url,
        "--report",
        reportPath,
      ],
    },
    {
      name: "parityComplete",
      command: [
        process.execPath,
        path.join(scriptDir, "check-web-parity-complete.mjs"),
        "reports",
        "--url",
        url,
      ],
    },
  ];
}

export function manualDeviceNextStepLines(url, reportPath) {
  return [
    `./scripts/check-web-manual-report.mjs ${shellQuote(reportPath)}`,
    `./scripts/check-web-parity-gate.mjs ${shellQuote(url)} --report ${shellQuote(reportPath)}`,
    `./scripts/check-web-parity-complete.mjs reports/ --url ${shellQuote(url)}`,
  ];
}

function printManualDeviceNextSteps(url, reportPath) {
  console.log("\nManual device report passed. Next parity commands:");
  for (const line of manualDeviceNextStepLines(url, reportPath)) {
    console.log(`- ${line}`);
  }
  console.log("Or rerun the manual device command with --finalize to run these automatically.");
}

async function runManualDeviceFinalizers(url, reportPath) {
  console.log("\nFinalizing web parity evidence from the manual device report.");
  for (const step of manualDeviceFinalizerCommands(url, reportPath)) {
    await runFinalizerStep(step);
  }
}

function runFinalizerStep(step) {
  console.log(`\n[${step.name}] ${step.command.map(shellQuote).join(" ")}`);
  return new Promise((resolve, reject) => {
    const child = spawn(step.command[0], step.command.slice(1), {
      cwd: repoRoot,
      env: process.env,
      stdio: "inherit",
    });
    child.on("error", reject);
    child.on("close", (exitCode, signal) => {
      if (exitCode === 0) {
        resolve();
      } else {
        reject(
          new Error(
            `${step.name} failed with exit ${exitCode}${signal ? ` signal ${signal}` : ""}`
          )
        );
      }
    });
  });
}

function writeReport(data, outDir) {
  fs.mkdirSync(outDir, { recursive: true });
  const stamp = data.generatedAt.replace(/[:.]/g, "-");
  const reportPath = path.join(outDir, `web-manual-devices-${stamp}.json`);
  fs.writeFileSync(reportPath, `${JSON.stringify(data, null, 2)}\n`);
  return reportPath;
}

async function terminateChrome(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  child.kill("SIGTERM");
  await Promise.race([
    new Promise((resolve) => child.once("exit", resolve)),
    delay(1500).then(() => {
      if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
    }),
  ]);
}

async function removeProfile(profilePath) {
  await fs.promises.rm(profilePath, { recursive: true, force: true });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function shellQuote(value) {
  if (/^[A-Za-z0-9_./:=@+-]+$/.test(value)) return value;
  return `'${value.replaceAll("'", "'\\''")}'`;
}

function isCliEntrypoint() {
  return process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;
}
