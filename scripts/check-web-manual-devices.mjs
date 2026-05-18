#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import readline from "node:readline/promises";
import { spawn } from "node:child_process";

const options = parseArgs(process.argv.slice(2));
if (!options.url) {
  console.error(
    "usage: scripts/check-web-manual-devices.mjs <url> [--out reports] [--keep-open]"
  );
  process.exit(2);
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

const timeoutMs = numberFromEnv("ORBIFOLD_WEB_MANUAL_TIMEOUT_MS", 60_000);
const devtoolsTimeoutMs = numberFromEnv("ORBIFOLD_CHROME_DEVTOOLS_TIMEOUT_MS", 20_000);
const profile = fs.mkdtempSync(path.join(os.tmpdir(), "orbifold-web-manual-"));
const rl = readline.createInterface({ input: process.stdin, output: process.stdout });

const report = {
  schema: "orbifold.web_manual_device_parity.v1",
  generatedAt: new Date().toISOString(),
  targetUrl: options.url,
  host: {
    platform: process.platform,
    arch: process.arch,
    release: os.release(),
  },
  chrome: {
    path: chromePath,
  },
  checks: [],
  states: {},
  userConfirmations: {},
  browserEvents: [],
};

const chrome = spawn(
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

let stdout = "";
let stderr = "";
chrome.stdout.on("data", (chunk) => {
  stdout += chunk;
});
chrome.stderr.on("data", (chunk) => {
  stderr += chunk;
});

let pageSession = null;
let send = null;

try {
  const browserWsUrl = await waitForDevtoolsEndpoint();
  ({ send, pageSession } = await connectToPage(browserWsUrl));
  await runManualDeviceCheck();
} catch (error) {
  report.error = String(error?.stack || error?.message || error);
  addCheck("manualDeviceVerifierCompleted", false, { error: report.error });
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
}

async function runManualDeviceCheck() {
  const browserVersion = await send("Browser.getVersion");
  report.chrome.version = browserVersion.product;
  report.chrome.userAgent = browserVersion.userAgent;
  report.chrome.protocolVersion = browserVersion.protocolVersion;

  await send("Page.navigate", { url: options.url }, pageSession);
  await waitForOrbifoldReady();
  report.states.runtime = await evaluateRuntimeState();
  report.states.initial = await evaluateProjectState();
  addCheck("browserRuntimeReady", true, pickRuntimeEvidence(report.states.runtime));

  console.log("\nA Chrome window is open for manual web parity checks.");
  console.log("This is not a CI check: it requires real audio, real Web MIDI, and your ears.");
  await promptEnter(
    "Inspect the UI for full-window canvas coverage and obvious text overlap, then press Enter."
  );
  const visualOk = await confirm("Does the browser UI look usable at this size?");
  report.userConfirmations.visualInspection = visualOk;
  addCheck("manualVisualInspection", visualOk, pickRuntimeEvidence(await evaluateRuntimeState()));

  await dispatchAction("view.devices");
  await delay(500);

  await dispatchAction("audio.refresh");
  await promptEnter(
    "If Chrome asks for audio-output permission, grant it. Select the desired audio output in Orbifold if needed, then press Enter."
  );
  report.states.afterAudioRefresh = await evaluateProjectState();
  addCheck("webAudioOutputsDiscovered", report.states.afterAudioRefresh.audioOutputCount > 0, {
    audioOutputCount: report.states.afterAudioRefresh.audioOutputCount,
    browserAudioOutputNames: report.states.afterAudioRefresh.browserAudioOutputNames,
    browserAudioDiagnostic: report.states.afterAudioRefresh.browserAudioDiagnostic,
    lastStatus: report.states.afterAudioRefresh.lastStatus,
  });

  await dispatchAction("audio.connect");
  await delay(1200);
  report.states.afterAudioConnect = await evaluateProjectState();
  addCheck("webAudioConnectedState", report.states.afterAudioConnect.audioStreamConnected, {
    connectedAudioOutput: report.states.afterAudioConnect.connectedAudioOutput,
    browserAudioDiagnostic: report.states.afterAudioConnect.browserAudioDiagnostic,
    audioContextCreated: report.states.afterAudioConnect.audioContextCreated,
    audioProcessorAttached: report.states.afterAudioConnect.audioProcessorAttached,
    audioResumeRequested: report.states.afterAudioConnect.audioResumeRequested,
    audioResumeResolved: report.states.afterAudioConnect.audioResumeResolved,
    lastStatus: report.states.afterAudioConnect.lastStatus,
  });

  await dispatchAction("audio.test_a4");
  await delay(1500);
  report.states.afterAudioTest = await evaluateProjectState();
  const heardA4 = await confirm("Did you hear the A4 test tone from the selected browser output?");
  report.userConfirmations.audibleA4 = heardA4;
  addCheck("manualAudibleWebAudio", heardA4 && report.states.afterAudioTest.audioNonzero, {
    audioCallbackCount: report.states.afterAudioTest.audioCallbackCount,
    audioFrameCount: report.states.afterAudioTest.audioFrameCount,
    audioPeak: report.states.afterAudioTest.audioPeak,
    audioNonzero: report.states.afterAudioTest.audioNonzero,
    lastStatus: report.states.afterAudioTest.lastStatus,
  });

  await dispatchAction("midi.refresh");
  await promptEnter(
    "Grant Web MIDI permission if prompted, connect a real MIDI device, then press Enter."
  );
  await dispatchAction("midi.refresh");
  await delay(1000);
  report.states.afterMidiRefresh = await evaluateProjectState();
  addCheck("webMidiInputsDiscovered", report.states.afterMidiRefresh.midiInputCount > 0, {
    midiInputCount: report.states.afterMidiRefresh.midiInputCount,
    browserMidiInputNames: report.states.afterMidiRefresh.browserMidiInputNames,
    browserMidiDiagnostic: report.states.afterMidiRefresh.browserMidiDiagnostic,
    lastStatus: report.states.afterMidiRefresh.lastStatus,
  });

  await dispatchAction("midi.connect");
  await delay(1000);
  report.states.afterMidiConnect = await evaluateProjectState();
  addCheck("webMidiConnectedState", report.states.afterMidiConnect.connectedMidiInput.length > 0, {
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
  await dispatchAction("transport.record");
  await promptEnter(
    "Recording is armed. Play and release one MIDI note, wait a moment, then press Enter."
  );
  await dispatchAction("transport.record");
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

  const passed = report.checks.every((check) => check.pass);
  addCheck("manualDeviceVerifierCompleted", passed, {
    passedChecks: report.checks.filter((check) => check.pass).length,
    totalChecks: report.checks.length,
  });

  if (!passed) {
    throw new Error("Manual device parity checks did not all pass.");
  }

  console.log("\nManual web device parity checks passed.");
}

function parseArgs(args) {
  const parsed = { url: null, outDir: "reports", keepOpen: false };
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--out") {
      parsed.outDir = args[++index];
    } else if (arg === "--keep-open") {
      parsed.keepOpen = true;
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
        project: localStorage.getItem("orbifold.project.v1") || "",
        lastStatus: document.body.dataset.orbifoldLastStatus ?? "",
        frameCount: Number(document.body.dataset.orbifoldFrameCount ?? 0),
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

async function dispatchAction(action) {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `window.orbifoldDispatchAction(${JSON.stringify(action)})`,
      returnByValue: true,
    },
    pageSession
  );
  if (result.result.value !== true) {
    throw new Error(`browser action dispatch hook rejected ${action}`);
  }
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

function persistedNoteCount(projectText) {
  return (String(projectText || "").match(/\nnote\t/g) ?? []).length;
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
