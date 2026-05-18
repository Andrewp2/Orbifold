#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";

const url = process.argv[2];
if (!url) {
  console.error("usage: check-web-smoke.mjs <url>");
  process.exit(2);
}

if (typeof WebSocket !== "function") {
  console.error("Node.js with a global WebSocket implementation is required.");
  process.exit(2);
}

const timeoutMs = numberFromEnv("ORBIFOLD_WEB_SMOKE_TIMEOUT_MS", 15_000);
const devtoolsTimeoutMs = numberFromEnv("ORBIFOLD_CHROME_DEVTOOLS_TIMEOUT_MS", 20_000);
const settleMs = numberFromEnv("ORBIFOLD_WEB_SMOKE_SETTLE_MS", 1_000);
const smokeScaleDescription = "Browser 5-EDO";
const smokeScalaProjectLine = "scala_path=browser_5_edo.scl";
const smokeLumatoneProjectLine = "lumatone_path=classic.ltn";
const smokeSampleInstrumentProjectLine =
  "sample_instrument_path=browser_assets/samples/smoke_sample.wav";
const chromePath = findChrome();
if (!chromePath) {
  console.error("Could not find Chrome. Set CHROME_BIN or install google-chrome/chromium.");
  process.exit(2);
}

const profile = fs.mkdtempSync(path.join(os.tmpdir(), "orbifold-web-smoke-"));
const chrome = spawn(
  chromePath,
  [
    "--headless=new",
    "--remote-debugging-port=0",
    "--enable-unsafe-webgpu",
    "--ignore-gpu-blocklist",
    "--disable-dev-shm-usage",
    "--no-first-run",
    "--no-default-browser-check",
    "--no-sandbox",
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

try {
  const browserWsUrl = await waitForDevtoolsEndpoint();
  const events = await runSmoke(browserWsUrl);
  const failures = smokeFailures(events);
  if (failures.length > 0) {
    console.error("Orbifold web smoke failed:");
    for (const failure of failures) {
      console.error(`- ${failure}`);
    }
    process.exitCode = 1;
  } else {
    console.log(`Orbifold web smoke passed for ${url}`);
  }
} catch (error) {
  console.error(`Orbifold web smoke failed: ${error.message ?? error}`);
  process.exitCode = 1;
} finally {
  await terminateChrome(chrome);
  await removeProfile(profile);
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

async function runSmoke(browserWsUrl) {
  const ws = new WebSocket(browserWsUrl);
  let id = 0;
  const pending = new Map();
  const events = [];
  const requestUrls = new Map();
  const artifactsDir = fs.mkdtempSync(path.join(os.tmpdir(), "orbifold-web-smoke-files-"));

  function send(method, params = {}, sessionId = undefined) {
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
      events.push({ ...message, url: requestUrls.get(message.params?.requestId) });
    }
  };

  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = reject;
  });

  try {
    const { targetId } = await send("Target.createTarget", { url: "about:blank" });
    const { sessionId } = await send("Target.attachToTarget", {
      targetId,
      flatten: true,
    });
    await send("Runtime.enable", {}, sessionId);
    await send("Log.enable", {}, sessionId);
    await send("Network.enable", {}, sessionId);
    await send("Page.enable", {}, sessionId);
    await send("DOM.enable", {}, sessionId);
    await send("Browser.setDownloadBehavior", {
      behavior: "allow",
      downloadPath: artifactsDir,
    });
    await verifyBrowserStartupStorageGuards(send);
    await installMockBrowserDevices(send, sessionId);
    await send(
      "Emulation.setDeviceMetricsOverride",
      {
        width: 1600,
        height: 1000,
        deviceScaleFactor: 1,
        mobile: false,
      },
      sessionId
    );
    await send("Page.navigate", { url }, sessionId);
    await waitForOrbifoldReady(send, sessionId);
    await verifyHighDpiCanvasScale(send, sessionId);
    await verifyToolbarButtonClicks(send, sessionId);
    await verifyActionDispatch(send, sessionId);
    await verifyBrowserShortcutMapping(send, sessionId);
    await verifyKeyboardShortcut(send, sessionId);
    await verifyBrowserTextEditActions(send, sessionId);
    await verifyPianoGridDoubleClick(send, sessionId);
    await verifyPianoNoteDrag(send, sessionId);
    await verifyPianoNoteResize(send, sessionId);
    await verifyPianoVelocityDrag(send, sessionId);
    await verifyBrowserClipEditActions(send, sessionId);
    await verifyTimelineAndLoopGestures(send, sessionId);
    await verifyPianoWheelNavigation(send, sessionId);
    await verifyWorkspaceResizeGestures(send, sessionId);
    await verifyBrowserFileFlows(send, sessionId, artifactsDir);
    await verifyBrowserMidiFlow(send, sessionId);
    await verifyBrowserMidiFailureFlows(send);
    await verifyBrowserAudioFlow(send, sessionId);
    await verifyBrowserAudioFallbackFlow(send);
    await verifyBrowserPersistenceAfterReload(send, sessionId);
    await delay(settleMs);
    return events;
  } finally {
    ws.close();
    await removeProfile(artifactsDir);
  }
}

async function installMockBrowserDevices(send, sessionId) {
  await send(
    "Page.addScriptToEvaluateOnNewDocument",
    {
      source: `(() => {
        const audioOutputs = [
          {
            kind: "audiooutput",
            deviceId: "default",
            label: "Default Smoke Output",
          },
          {
            kind: "audiooutput",
            deviceId: "orbifold-smoke-speakers",
            label: "Orbifold Smoke Speakers",
          },
        ];
        const mediaDevices = navigator.mediaDevices || {};
        Object.defineProperty(navigator, "mediaDevices", {
          configurable: true,
          value: {
            ...mediaDevices,
            enumerateDevices: async () => audioOutputs,
          },
        });
        const installAudioSinkStub = () => {
          const AudioContextCtor = window.AudioContext || window.webkitAudioContext;
          if (!AudioContextCtor || !AudioContextCtor.prototype) {
            return;
          }
          Object.defineProperty(AudioContextCtor.prototype, "setSinkId", {
            configurable: true,
            value: async function setSinkId(deviceId) {
              document.body.dataset.orbifoldSmokeSinkId = String(deviceId || "");
              this.__orbifoldSmokeSinkId = String(deviceId || "");
            },
          });
        };
        installAudioSinkStub();
        Object.defineProperty(window, "__orbifoldSmokeAudioOutputs", {
          configurable: true,
          value: audioOutputs,
        });
        const input = {
          id: "orbifold-smoke-midi",
          name: "Orbifold Smoke MIDI",
          manufacturer: "Orbifold",
          type: "input",
          state: "connected",
          connection: "closed",
          onmidimessage: null,
          open: async () => {
            input.connection = "open";
            return input;
          },
        };
        const access = {
          inputs: new Map([[input.id, input]]),
          outputs: new Map(),
          sysexEnabled: false,
          onstatechange: null,
        };
        Object.defineProperty(navigator, "requestMIDIAccess", {
          configurable: true,
          value: async () => access,
        });
        Object.defineProperty(window, "__orbifoldSmokeMidiInput", {
          configurable: true,
          value: input,
        });
      })();`,
    },
    sessionId
  );
}

async function verifyHighDpiCanvasScale(send, sessionId) {
  await send(
    "Emulation.setDeviceMetricsOverride",
    {
      width: 1920,
      height: 1080,
      deviceScaleFactor: 2,
      mobile: false,
    },
    sessionId
  );
  await waitForOrbifoldState(
    send,
    sessionId,
    (state) =>
      state.canvasClientWidth >= 1900 &&
      state.canvasClientHeight >= 1000 &&
      state.canvasWidth >= 3800 &&
      state.canvasHeight >= 2100 &&
      state.devicePixelRatio >= 2,
    "high-DPI browser canvas did not scale to the full viewport"
  );
  await send(
    "Emulation.setDeviceMetricsOverride",
    {
      width: 1600,
      height: 1000,
      deviceScaleFactor: 1,
      mobile: false,
    },
    sessionId
  );
  await waitForOrbifoldState(
    send,
    sessionId,
    (state) =>
      state.canvasClientWidth >= 1500 &&
      state.canvasClientHeight >= 900 &&
      state.canvasWidth >= 1200 &&
      state.canvasHeight >= 760,
    "browser canvas did not recover after high-DPI resize"
  );
}

async function verifyBrowserStartupStorageGuards(send) {
  const badSettings = "not browser settings";
  const badProject = "not an orbifold project";
  const { targetId } = await send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await send("Target.attachToTarget", {
    targetId,
    flatten: true,
  });
  try {
    await send("Page.enable", {}, sessionId);
    await send(
      "Page.addScriptToEvaluateOnNewDocument",
      {
        source: `
          localStorage.setItem("orbifold.settings.v1", ${JSON.stringify(badSettings)});
          localStorage.setItem("orbifold.project.v1", ${JSON.stringify(badProject)});
        `,
      },
      sessionId
    );
    await send(
      "Emulation.setDeviceMetricsOverride",
      {
        width: 1200,
        height: 760,
        deviceScaleFactor: 1,
        mobile: false,
      },
      sessionId
    );
    await send("Page.navigate", { url: urlForSmokeVariant("startup-storage-guard") }, sessionId);
    await waitForOrbifoldReady(send, sessionId);

    const state = await evaluateProjectState(send, sessionId);
    if (state.settings !== badSettings || state.project !== badProject) {
      throw new Error(
        `browser startup invalid localStorage was overwritten: ${JSON.stringify(state)}`
      );
    }
    if (
      !state.lastStatus.includes("Project parse error (browser session):") &&
      !state.lastStatus.includes("Browser settings load error:")
    ) {
      throw new Error(
        `browser startup invalid localStorage did not leave a visible error: ${JSON.stringify(
          state
        )}`
      );
    }
  } finally {
    await send(
      "Runtime.evaluate",
      {
        expression: `
          localStorage.removeItem("orbifold.settings.v1");
          localStorage.removeItem("orbifold.project.v1");
        `,
        returnByValue: true,
      },
      sessionId
    ).catch(() => {});
    await send("Target.closeTarget", { targetId }).catch(() => {});
  }
}

async function verifyToolbarButtonClicks(send, sessionId) {
  await clickCanvasPoint(send, sessionId, 570, 33);
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastAction === "transport.play_stop" &&
      state.transportPlaying &&
      state.lastStatus.includes("Transport playing"),
    "toolbar Play button click did not start transport through canvas hit testing"
  );

  await clickCanvasPoint(send, sessionId, 628, 33);
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastAction === "transport.stop" &&
      !state.transportPlaying &&
      state.lastStatus.includes("Transport stopped"),
    "toolbar Stop button click did not stop transport through canvas hit testing"
  );
}

async function clickCanvasPoint(send, sessionId, x, y) {
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mousePressed",
      x,
      y,
      button: "left",
      clickCount: 1,
    },
    sessionId
  );
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mouseReleased",
      x,
      y,
      button: "left",
      clickCount: 1,
    },
    sessionId
  );
  await delay(120);
}

async function verifyActionDispatch(send, sessionId) {
  const dispatchResult = await send(
    "Runtime.evaluate",
    {
      expression: `window.orbifoldDispatchAction("clip.add_note")`,
      returnByValue: true,
    },
    sessionId
  );
  if (dispatchResult.result.value !== true) {
    throw new Error("runtime did not expose the browser action dispatch hook");
  }

  const deadline = Date.now() + timeoutMs;
  let lastState = null;
  while (Date.now() <= deadline) {
    lastState = await evaluateProjectState(send, sessionId);
    if (
      lastState.lastAction === "clip.add_note" &&
      lastState.noteCount >= 1 &&
      lastState.project.includes("\nnote\t") &&
      lastState.project.includes("orbifold_project=1")
    ) {
      return;
    }
    await delay(250);
  }
  throw new Error(
    `browser action dispatch did not persist a clip note; last state: ${JSON.stringify(lastState)}`
  );
}

async function verifyPianoNoteResize(send, sessionId) {
  const geometry = await waitForPianoGeometry(send, sessionId, (geometry) => {
    return (
      geometry.resizeStartX > 0 &&
      geometry.resizeStartY > 0 &&
      geometry.resizeEndX > 0 &&
      geometry.resizeEndY > 0
    );
  });

  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mousePressed",
      x: geometry.resizeStartX,
      y: geometry.resizeStartY,
      button: "left",
      clickCount: 1,
    },
    sessionId
  );
  await delay(120);
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mouseMoved",
      x: geometry.resizeStartX + 12,
      y: geometry.resizeStartY,
      button: "left",
      buttons: 1,
    },
    sessionId
  );
  await delay(120);
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mouseMoved",
      x: geometry.resizeEndX,
      y: geometry.resizeEndY,
      button: "left",
      buttons: 1,
    },
    sessionId
  );
  await delay(120);
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mouseReleased",
      x: geometry.resizeEndX,
      y: geometry.resizeEndY,
      button: "left",
      clickCount: 1,
    },
    sessionId
  );

  const deadline = Date.now() + timeoutMs;
  let lastState = null;
  while (Date.now() <= deadline) {
    lastState = await evaluateProjectState(send, sessionId);
    if (thirdNoteDurationBeat(lastState.project) >= 2.5) {
      return;
    }
    await delay(250);
  }
  throw new Error(
    `piano note resize did not change the double-click note length; last state: ${JSON.stringify(
      lastState
    )}`
  );
}

async function verifyPianoNoteDrag(send, sessionId) {
  const geometry = await evaluatePianoGeometry(send, sessionId);
  if (
    geometry.dragStartX <= 0 ||
    geometry.dragStartY <= 0 ||
    geometry.dragEndX <= 0 ||
    geometry.dragEndY <= 0
  ) {
    throw new Error(`piano drag automation geometry unavailable: ${JSON.stringify(geometry)}`);
  }

  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mousePressed",
      x: geometry.dragStartX,
      y: geometry.dragStartY,
      button: "left",
      clickCount: 1,
    },
    sessionId
  );
  await delay(120);
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mouseMoved",
      x: geometry.dragStartX + 12,
      y: geometry.dragStartY,
      button: "left",
      buttons: 1,
    },
    sessionId
  );
  await delay(120);
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mouseMoved",
      x: geometry.dragEndX,
      y: geometry.dragEndY,
      button: "left",
      buttons: 1,
    },
    sessionId
  );
  await delay(120);
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mouseReleased",
      x: geometry.dragEndX,
      y: geometry.dragEndY,
      button: "left",
      clickCount: 1,
    },
    sessionId
  );

  const deadline = Date.now() + timeoutMs;
  let lastState = null;
  while (Date.now() <= deadline) {
    lastState = await evaluateProjectState(send, sessionId);
    if (thirdNoteStartBeat(lastState.project) >= 5.9) {
      return;
    }
    await delay(250);
  }
  throw new Error(
    `piano note drag did not move the double-click note; last state: ${JSON.stringify(lastState)}`
  );
}

async function verifyPianoVelocityDrag(send, sessionId) {
  const geometry = await waitForPianoGeometry(send, sessionId, (geometry) => {
    return (
      geometry.velocityStartX > 0 &&
      geometry.velocityStartY > 0 &&
      geometry.velocityEndX > 0 &&
      geometry.velocityEndY > 0
    );
  });
  const before = await evaluateProjectState(send, sessionId);
  const noteBefore = projectNoteById(before.project, 3);
  if (!noteBefore) {
    throw new Error(`piano velocity drag precondition found no note 3: ${before.project}`);
  }

  await dragPointer(send, sessionId, {
    startX: geometry.velocityStartX,
    startY: geometry.velocityStartY,
    endX: geometry.velocityEndX,
    endY: geometry.velocityEndY,
  });

  await waitForProjectState(
    send,
    sessionId,
    (state) => {
      const note = projectNoteById(state.project, 3);
      return (
        note &&
        note.velocity > noteBefore.velocity &&
        note.velocity >= 120 &&
        state.lastPointerAction === "note.velocity.3" &&
        state.lastPointerPhase === "commit"
      );
    },
    "browser piano velocity drag did not update the note velocity"
  );
}

async function verifyBrowserClipEditActions(send, sessionId) {
  const before = await evaluateProjectState(send, sessionId);
  const sourceNote = projectNoteById(before.project, 3);
  if (!sourceNote) {
    throw new Error(`browser clip edit precondition found no note 3: ${before.project}`);
  }
  const beforeCount = persistedNoteCount(before.project);

  await dispatchBrowserAction(send, sessionId, "clip.copy_note");
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastAction === "clip.copy_note" &&
      state.noteCount === before.noteCount &&
      state.lastStatus.startsWith("Copied note"),
    "browser clip copy did not copy the selected note without changing the project"
  );

  await dispatchBrowserAction(send, sessionId, "clip.paste_note");
  const pasted = await waitForProjectState(
    send,
    sessionId,
    (state) => {
      const note = latestProjectNote(state.project);
      return (
        state.lastAction === "clip.paste_note" &&
        persistedNoteCount(state.project) === beforeCount + 1 &&
        note &&
        note.id > sourceNote.id &&
        note.velocity === sourceNote.velocity &&
        note.durationBeat === sourceNote.durationBeat
      );
    },
    "browser clip paste did not create a copied note through the shared action path"
  );
  const pastedNote = latestProjectNote(pasted.project);

  await dispatchBrowserAction(send, sessionId, "clip.duplicate_note");
  const duplicated = await waitForProjectState(
    send,
    sessionId,
    (state) => {
      const note = latestProjectNote(state.project);
      return (
        state.lastAction === "clip.duplicate_note" &&
        persistedNoteCount(state.project) === beforeCount + 2 &&
        state.lastStatus.includes("Duplicated clip note") &&
        note &&
        pastedNote &&
        note.id > pastedNote.id &&
        note.velocity === pastedNote.velocity
      );
    },
    "browser clip duplicate did not duplicate the selected note through the shared action path"
  );
  const duplicatedNote = latestProjectNote(duplicated.project);

  await dispatchBrowserAction(send, sessionId, "clip.delete_note");
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastAction === "clip.delete_note" &&
      persistedNoteCount(state.project) === beforeCount + 1 &&
      duplicatedNote &&
      !projectNoteById(state.project, duplicatedNote.id) &&
      state.lastStatus.includes("Deleted clip note"),
    "browser clip delete did not remove the selected note through the shared action path"
  );

  await verifyBrowserQuantizeAction(send, sessionId);
}

async function verifyBrowserQuantizeAction(send, sessionId) {
  await dispatchBrowserAction(send, sessionId, "transport.snap");
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.lastAction === "transport.snap" && state.lastStatus.includes("Snap off"),
    "browser snap toggle did not turn snap off before quantize setup"
  );

  const geometry = await waitForPianoGeometry(send, sessionId, (geometry) => {
    return geometry.gridX > 0 && geometry.gridWidth > 200 && geometry.addY > 0;
  });
  const offGridPoint = {
    x: geometry.gridX + geometry.gridWidth * 0.3337,
    y: geometry.addY,
  };
  const before = await evaluateProjectState(send, sessionId);
  await doubleClickCanvasPoint(send, sessionId, offGridPoint.x, offGridPoint.y);
  const added = await waitForProjectState(
    send,
    sessionId,
    (state) => {
      const note = latestProjectNote(state.project);
      return (
        persistedNoteCount(state.project) === persistedNoteCount(before.project) + 1 &&
        note &&
        !isQuantizedToSixteenth(note.startBeat)
      );
    },
    "browser off-grid double-click did not create an unsnapped note with snap disabled"
  );
  const offGridNote = latestProjectNote(added.project);

  await dispatchBrowserAction(send, sessionId, "transport.snap");
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.lastAction === "transport.snap" && state.lastStatus.includes("Snap on"),
    "browser snap toggle did not restore snap before quantize"
  );

  await dispatchBrowserAction(send, sessionId, "clip.quantize");
  await waitForProjectState(
    send,
    sessionId,
    (state) => {
      const note = offGridNote ? projectNoteById(state.project, offGridNote.id) : null;
      return (
        state.lastAction === "clip.quantize" &&
        note &&
        isQuantizedToSixteenth(note.startBeat) &&
        Math.abs(note.startBeat - offGridNote.startBeat) > 0.0001 &&
        state.lastStatus.startsWith("Quantized note")
      );
    },
    "browser clip quantize did not snap the selected off-grid note"
  );
}

async function verifyPianoGridDoubleClick(send, sessionId) {
  const geometry = await evaluatePianoGeometry(send, sessionId);
  if (geometry.addX <= 0 || geometry.addY <= 0) {
    throw new Error(`piano automation geometry unavailable: ${JSON.stringify(geometry)}`);
  }

  for (const clickCount of [1, 2]) {
    await send(
      "Input.dispatchMouseEvent",
      {
        type: "mousePressed",
        x: geometry.addX,
        y: geometry.addY,
        button: "left",
        clickCount,
      },
      sessionId
    );
    await send(
      "Input.dispatchMouseEvent",
      {
        type: "mouseReleased",
        x: geometry.addX,
        y: geometry.addY,
        button: "left",
        clickCount,
      },
      sessionId
    );
    await delay(120);
  }

  const deadline = Date.now() + timeoutMs;
  let lastState = null;
  while (Date.now() <= deadline) {
    lastState = await evaluateProjectState(send, sessionId);
    if (persistedNoteCount(lastState.project) >= 3) {
      return;
    }
    await delay(250);
  }
  throw new Error(
    `piano grid double-click did not persist a third note; last state: ${JSON.stringify(
      lastState
    )}`
  );
}

async function doubleClickCanvasPoint(send, sessionId, x, y) {
  for (const clickCount of [1, 2]) {
    await send(
      "Input.dispatchMouseEvent",
      {
        type: "mousePressed",
        x,
        y,
        button: "left",
        clickCount,
      },
      sessionId
    );
    await send(
      "Input.dispatchMouseEvent",
      {
        type: "mouseReleased",
        x,
        y,
        button: "left",
        clickCount,
      },
      sessionId
    );
    await delay(120);
  }
}

async function evaluatePianoGeometry(send, sessionId) {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `({
        gridX: Number(document.body.dataset.orbifoldPianoGridX ?? 0),
        gridY: Number(document.body.dataset.orbifoldPianoGridY ?? 0),
        gridWidth: Number(document.body.dataset.orbifoldPianoGridWidth ?? 0),
        gridHeight: Number(document.body.dataset.orbifoldPianoGridHeight ?? 0),
        addX: Number(document.body.dataset.orbifoldPianoAddX ?? 0),
        addY: Number(document.body.dataset.orbifoldPianoAddY ?? 0),
        dragStartX: Number(document.body.dataset.orbifoldPianoDragStartX ?? 0),
        dragStartY: Number(document.body.dataset.orbifoldPianoDragStartY ?? 0),
        dragEndX: Number(document.body.dataset.orbifoldPianoDragEndX ?? 0),
        dragEndY: Number(document.body.dataset.orbifoldPianoDragEndY ?? 0),
        resizeStartX: Number(document.body.dataset.orbifoldPianoResizeStartX ?? 0),
        resizeStartY: Number(document.body.dataset.orbifoldPianoResizeStartY ?? 0),
        resizeEndX: Number(document.body.dataset.orbifoldPianoResizeEndX ?? 0),
        resizeEndY: Number(document.body.dataset.orbifoldPianoResizeEndY ?? 0),
        velocityStartX: Number(document.body.dataset.orbifoldPianoVelocityStartX ?? 0),
        velocityStartY: Number(document.body.dataset.orbifoldPianoVelocityStartY ?? 0),
        velocityEndX: Number(document.body.dataset.orbifoldPianoVelocityEndX ?? 0),
        velocityEndY: Number(document.body.dataset.orbifoldPianoVelocityEndY ?? 0),
        viewStart: Number(document.body.dataset.orbifoldPianoViewStart ?? 0),
        viewBeats: Number(document.body.dataset.orbifoldPianoViewBeats ?? 0),
        minPitch: Number(document.body.dataset.orbifoldPianoMinPitch ?? 0),
        maxPitch: Number(document.body.dataset.orbifoldPianoMaxPitch ?? 0)
      })`,
      returnByValue: true,
    },
    sessionId
  );
  return result.result.value;
}

async function verifyPianoWheelNavigation(send, sessionId) {
  const geometry = await waitForPianoGeometry(send, sessionId, (geometry) => {
    return geometry.gridWidth > 200 && geometry.gridHeight > 200 && geometry.viewBeats > 0;
  });
  const wheelPoint = {
    x: geometry.gridX + geometry.gridWidth * 0.5,
    y: geometry.gridY + geometry.gridHeight * 0.5,
  };

  const initial = await evaluatePianoView(send, sessionId);
  const zoomedTime = await dispatchWheelUntilViewChanges(
    send,
    sessionId,
    wheelPoint,
    [{ deltaY: -420, modifiers: 2 }],
    initial,
    (view, before) => Math.abs(view.viewBeats - before.viewBeats) > 0.01,
    "Ctrl+wheel did not zoom the piano-roll time view"
  );

  await dispatchWheelUntilViewChanges(
    send,
    sessionId,
    wheelPoint,
    [
      { deltaY: -900, modifiers: 8 },
      { deltaY: 900, modifiers: 8 },
    ],
    zoomedTime,
    (view, before) => Math.abs(view.viewStart - before.viewStart) > 0.01,
    "Shift+wheel did not scroll the piano-roll time view"
  );

  const beforePitchZoom = await evaluatePianoView(send, sessionId);
  const zoomedPitch = await dispatchWheelUntilViewChanges(
    send,
    sessionId,
    wheelPoint,
    [{ deltaY: -420, modifiers: 1 }],
    beforePitchZoom,
    (view, before) => view.pitchRows !== before.pitchRows,
    "Alt+wheel did not zoom the piano-roll pitch view"
  );

  await dispatchWheelUntilViewChanges(
    send,
    sessionId,
    wheelPoint,
    [
      { deltaY: -900, modifiers: 0 },
      { deltaY: 900, modifiers: 0 },
    ],
    zoomedPitch,
    (view, before) => view.pitchCenter !== before.pitchCenter,
    "wheel did not scroll the piano-roll pitch view"
  );
}

async function verifyWorkspaceResizeGestures(send, sessionId) {
  let layout = await waitForLayoutGeometry(send, sessionId, (layout) => {
    return layout.rightResizeX > 0 && layout.rightPanelWidth > 0;
  });
  const beforeRight = layout.rightPanelWidth;
  await dragPointer(send, sessionId, {
    startX: layout.rightResizeX,
    startY: layout.rightResizeY,
    endX: layout.rightResizeEndX,
    endY: layout.rightResizeEndY,
  });
  layout = await waitForLayoutGeometry(
    send,
    sessionId,
    (layout) => Math.abs(layout.rightPanelWidth - beforeRight) > 20
  );

  const beforeBottom = layout.pianoRollHeight;
  await dragPointer(send, sessionId, {
    startX: layout.bottomResizeX,
    startY: layout.bottomResizeY,
    endX: layout.bottomResizeEndX,
    endY: layout.bottomResizeEndY,
  });
  await waitForLayoutGeometry(
    send,
    sessionId,
    (layout) => Math.abs(layout.pianoRollHeight - beforeBottom) > 20
  );
}

async function verifyTimelineAndLoopGestures(send, sessionId) {
  let layout = await waitForLayoutGeometry(send, sessionId, (layout) => {
    return (
      layout.arrangementSeekStartX > 0 &&
      layout.arrangementSeekEndX > 0 &&
      layout.pianoSeekStartX > 0 &&
      layout.pianoSeekEndX > 0 &&
      layout.arrangementLoopEndStartX > 0 &&
      layout.arrangementLoopEndTargetX > 0
    );
  });
  const initialLoopState = await evaluateProjectState(send, sessionId);

  await dragPointer(send, sessionId, {
    startX: layout.arrangementLoopEndStartX,
    startY: layout.arrangementLoopEndStartY,
    endX: layout.arrangementLoopEndTargetX,
    endY: layout.arrangementLoopEndTargetY,
  });
  const arrangementLoopState = await waitForProjectState(
    send,
    sessionId,
    (state) =>
      Math.abs(state.loopBeats - initialLoopState.loopBeats) > 0.01 &&
      projectIncludesLoopBeats(state) &&
      state.lastStatus.includes("Loop length"),
    `browser arrangement loop-end drag did not resize loop length; layout: ${JSON.stringify(
      layout
    )}`
  );

  layout = await waitForLayoutGeometry(send, sessionId, (layout) => {
    return (
      layout.pianoLoopEndStartX > 0 &&
      layout.pianoLoopEndTargetX > 0 &&
      Math.abs(layout.arrangementLoopEndStartX - layout.arrangementLoopEndTargetX) > 1
    );
  });
  await dragPointer(send, sessionId, {
    startX: layout.pianoLoopEndStartX,
    startY: layout.pianoLoopEndStartY,
    endX: layout.pianoLoopEndTargetX,
    endY: layout.pianoLoopEndTargetY,
  });
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      Math.abs(state.loopBeats - arrangementLoopState.loopBeats) > 0.01 &&
      projectIncludesLoopBeats(state) &&
      state.lastStatus.includes("Loop length"),
    "browser piano loop-end drag did not resize loop length"
  );

  layout = await waitForLayoutGeometry(send, sessionId, (layout) => {
    return (
      layout.arrangementSeekStartX > 0 &&
      layout.arrangementSeekEndX > 0 &&
      layout.pianoSeekStartX > 0 &&
      layout.pianoSeekEndX > 0
    );
  });
  await dragPointer(send, sessionId, {
    startX: layout.arrangementSeekStartX,
    startY: layout.arrangementSeekStartY,
    endX: layout.arrangementSeekEndX,
    endY: layout.arrangementSeekEndY,
  });
  const arrangementSeekState = await waitForProjectState(
    send,
    sessionId,
    (state) => state.transportPositionBeats > 1 && state.lastStatus.includes("Seek"),
    "browser arrangement ruler drag did not seek transport"
  );

  await dragPointer(send, sessionId, {
    startX: layout.pianoSeekEndX,
    startY: layout.pianoSeekEndY,
    endX: layout.pianoSeekStartX,
    endY: layout.pianoSeekStartY,
  });
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      Math.abs(state.transportPositionBeats - arrangementSeekState.transportPositionBeats) > 0.5 &&
      state.lastStatus.includes("Seek"),
    "browser piano ruler drag did not seek transport"
  );
}

async function verifyBrowserFileFlows(send, sessionId, artifactsDir) {
  const downloadedProject = await verifyBrowserProjectDownload(send, sessionId, artifactsDir);
  await verifyBrowserProjectSaveAsShortcut(send, sessionId, artifactsDir);

  const openProjectPath = path.join(artifactsDir, "browser_open_test.orbifold");
  fs.writeFileSync(openProjectPath, projectWithFirstNoteOnly(downloadedProject), "utf8");
  await dispatchBrowserAction(send, sessionId, "clip.add_note");
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastAction === "clip.add_note" &&
      persistedNoteCount(state.project) > persistedNoteCount(downloadedProject) &&
      state.title.includes("Orbifold - project *"),
    "browser dirty-project setup did not add a note before open confirmation"
  );
  await dispatchBrowserAction(send, sessionId, "file.open");
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.lastStatus.includes("Unsaved changes: click Open again to discard"),
    "browser project open did not stop for dirty-project confirmation"
  );
  await assertNoFileInput(send, sessionId, "dirty browser project open created a file input");
  await chooseFileForBrowserAction(send, sessionId, "file.open", openProjectPath);
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      persistedNoteCount(state.project) === 1 &&
      state.lastStatus.includes("Loaded browser project: browser_open_test.orbifold") &&
      state.title === "Orbifold - browser_open_test",
    "browser project file picker did not load the selected project"
  );

  await verifyInvalidBrowserProjectOpen(send, sessionId, artifactsDir);

  const scalePath = path.join(artifactsDir, "browser_5_edo.scl");
  fs.writeFileSync(scalePath, "Browser 5-EDO\n5\n240\n480\n720\n960\n2/1\n", "utf8");
  await chooseFileForBrowserAction(send, sessionId, "scale.open", scalePath);
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastStatus.includes("Loaded browser Scala file") &&
      state.scaleDescription === smokeScaleDescription &&
      state.scalaPath === "browser_5_edo.scl" &&
      state.project.includes(smokeScalaProjectLine),
    "browser scale file picker did not load the selected Scala file"
  );

  await verifyInvalidBrowserScaleOpen(send, sessionId, artifactsDir);

  const keymapPath = path.join(artifactsDir, "classic.ltn");
  fs.copyFileSync(path.resolve("lumatone_factory_presets/1. Classic Mode.ltn"), keymapPath);
  await chooseFileForBrowserAction(send, sessionId, "keymap.open", keymapPath);
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastStatus.includes("Loaded browser key map: classic.ltn") &&
      state.lumatonePath === "classic.ltn" &&
      state.lumatoneLoaded &&
      state.project.includes(smokeLumatoneProjectLine),
    "browser key-map file picker did not load the selected Lumatone map"
  );

  await verifyInvalidBrowserKeymapOpen(send, sessionId, artifactsDir);

  const beforeAsset = await evaluateProjectState(send, sessionId);
  const wavPath = path.join(artifactsDir, "smoke_sample.wav");
  fs.writeFileSync(wavPath, pcm16WavBuffer());
  await chooseFileForBrowserAction(send, sessionId, "asset.import", wavPath);
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.assetCount > beforeAsset.assetCount &&
      state.lastStatus.includes("Imported browser sample: smoke_sample.wav"),
    "browser asset file picker did not import the selected WAV sample"
  );
  await dispatchBrowserAction(send, sessionId, "asset.use_sample");
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastStatus.includes("Loaded sample instrument: smoke_sample.wav") &&
      state.project.includes(smokeSampleInstrumentProjectLine),
    "browser imported WAV could not be assigned as the project sample instrument"
  );

  await verifyUnsupportedBrowserAssetImport(send, sessionId, artifactsDir);
}

async function verifyInvalidBrowserProjectOpen(send, sessionId, artifactsDir) {
  const before = await evaluateProjectState(send, sessionId);
  const invalidProjectPath = path.join(artifactsDir, "bad_project.orbifold");
  fs.writeFileSync(invalidProjectPath, "not an orbifold project", "utf8");

  await chooseFileForBrowserAction(send, sessionId, "file.open", invalidProjectPath);
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastStatus.startsWith("Project parse error (bad_project.orbifold):") &&
      state.title === before.title &&
      state.project === before.project &&
      persistedNoteCount(state.project) === persistedNoteCount(before.project),
    "invalid browser project open did not preserve the current project with a visible parse error"
  );
}

async function verifyInvalidBrowserScaleOpen(send, sessionId, artifactsDir) {
  const before = await evaluateProjectState(send, sessionId);
  const invalidScalePath = path.join(artifactsDir, "bad_scale.scl");
  fs.writeFileSync(invalidScalePath, "not a scala file", "utf8");

  await chooseFileForBrowserAction(send, sessionId, "scale.open", invalidScalePath);
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastStatus.startsWith("Scala parse error (bad_scale.scl):") &&
      state.scaleDescription === before.scaleDescription &&
      state.scalaPath === before.scalaPath &&
      state.project === before.project,
    "invalid browser scale open did not preserve the current scale with a visible parse error"
  );
}

async function verifyInvalidBrowserKeymapOpen(send, sessionId, artifactsDir) {
  const before = await evaluateProjectState(send, sessionId);
  const invalidKeymapPath = path.join(artifactsDir, "bad_keymap.ltn");
  fs.writeFileSync(invalidKeymapPath, "not a key map", "utf8");

  await chooseFileForBrowserAction(send, sessionId, "keymap.open", invalidKeymapPath);
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastStatus.startsWith("Key map parse error (bad_keymap.ltn):") &&
      state.lumatonePath === before.lumatonePath &&
      state.lumatoneLoaded === before.lumatoneLoaded &&
      state.project === before.project,
    "invalid browser key-map open did not preserve the current key map with a visible parse error"
  );
}

async function verifyUnsupportedBrowserAssetImport(send, sessionId, artifactsDir) {
  const before = await evaluateProjectState(send, sessionId);
  const unsupportedAssetPath = path.join(artifactsDir, "not_audio.txt");
  fs.writeFileSync(unsupportedAssetPath, "not audio", "utf8");

  await chooseFileForBrowserAction(send, sessionId, "asset.import", unsupportedAssetPath);
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastStatus.includes("Asset import error: unsupported sample") &&
      state.assetCount === before.assetCount &&
      state.project === before.project,
    "unsupported browser asset import did not preserve assets with a visible error"
  );
}

async function verifyBrowserMidiFlow(send, sessionId) {
  await dispatchBrowserAction(send, sessionId, "midi.refresh");
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.midiInputCount >= 1 &&
      state.browserMidiInputNames.includes("Orbifold Smoke MIDI") &&
      state.lastStatus.includes("Refreshed MIDI inputs: 1 MIDI input"),
    "browser MIDI refresh did not list the mocked MIDI input"
  );

  await dispatchBrowserAction(send, sessionId, "midi.connect");
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.connectedMidiInput === "Orbifold Smoke MIDI" &&
      state.midiInputConnection === "open" &&
      state.browserMidiDiagnostic.includes("Web MIDI: open") &&
      state.browserMidiDiagnostic.includes("1 input") &&
      state.lastStatus.includes("Connected browser MIDI input: Orbifold Smoke MIDI"),
    "browser MIDI connect did not connect the mocked MIDI input"
  );

  await sendBrowserMidiMessage(send, sessionId, [0x90, 60, 100]);
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.lastMidiStatus === 0x90 && state.lastMidiNote === 60,
    "browser MIDI message did not reach Orbifold's shared MIDI handling path"
  );
  await sendBrowserMidiMessage(send, sessionId, [0x80, 60, 0]);
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.lastMidiStatus === 0x80 && state.lastMidiNote === 60,
    "browser MIDI note-off did not reach Orbifold's shared MIDI handling path"
  );

  await dispatchBrowserAction(send, sessionId, "transport.record");
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.lastStatus.includes("Recording:"),
    "browser recording did not start before mocked MIDI input"
  );
  await sendBrowserMidiMessage(send, sessionId, [0x90, 64, 96]);
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.lastMidiStatus === 0x90 && state.lastMidiNote === 64,
    "browser MIDI note-on did not update last MIDI state while recording"
  );
  await delay(150);
  await sendBrowserMidiMessage(send, sessionId, [0x80, 64, 0]);
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.lastMidiStatus === 0x80 && state.lastMidiNote === 64,
    "browser MIDI note-off did not update last MIDI state while recording"
  );
  await dispatchBrowserAction(send, sessionId, "transport.record");
  await waitForProjectState(
    send,
    sessionId,
    (state) => {
      const notes = projectNotes(state.project);
      return (
        state.noteCount >= 1 &&
        persistedNoteCount(state.project) >= 1 &&
        state.lastStatus.includes("Recording stopped: 1 note") &&
        notes.some((note) => note.rawNote === 64 && note.velocity === 96)
      );
    },
    "browser MIDI recording did not persist a note-on/note-off clip note"
  );
}

async function sendBrowserMidiMessage(send, sessionId, data) {
  await send(
    "Runtime.evaluate",
    {
      expression: `window.__orbifoldSmokeMidiInput.onmidimessage({ data: ${JSON.stringify(data)} })`,
      returnByValue: true,
    },
    sessionId
  );
}

async function verifyBrowserMidiFailureFlows(send) {
  await verifyBrowserMidiFailureFlow(send, {
    label: "midi-unavailable",
    setupSource: `(() => {
      Object.defineProperty(navigator, "requestMIDIAccess", {
        configurable: true,
        value: undefined,
      });
    })();`,
    expectedDiagnostic: "Web MIDI: unavailable",
    refreshStatus: "MIDI refresh error: Web MIDI is not available in this browser",
    connectStatus: "MIDI connection error: Web MIDI is not available in this browser",
  });
  await verifyBrowserMidiFailureFlow(send, {
    label: "midi-denied",
    setupSource: `(() => {
      Object.defineProperty(navigator, "requestMIDIAccess", {
        configurable: true,
        value: async () => {
          throw new DOMException("Permission denied by smoke test", "NotAllowedError");
        },
      });
    })();`,
    expectedDiagnostic: "Web MIDI: permission denied",
    refreshStatus: "MIDI refresh error: Web MIDI request failed: Permission denied by smoke test",
    connectStatus: "MIDI connection error: Web MIDI request failed: Permission denied by smoke test",
  });
}

async function verifyBrowserMidiFailureFlow(
  send,
  { label, setupSource, expectedDiagnostic, refreshStatus, connectStatus }
) {
  const { targetId } = await send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await send("Target.attachToTarget", {
    targetId,
    flatten: true,
  });
  try {
    await send("Runtime.enable", {}, sessionId);
    await send("Log.enable", {}, sessionId);
    await send("Network.enable", {}, sessionId);
    await send("Page.enable", {}, sessionId);
    await send(
      "Page.addScriptToEvaluateOnNewDocument",
      { source: setupSource },
      sessionId
    );
    await send(
      "Emulation.setDeviceMetricsOverride",
      {
        width: 1200,
        height: 760,
        deviceScaleFactor: 1,
        mobile: false,
      },
      sessionId
    );
    await send("Page.navigate", { url: urlForSmokeVariant(label) }, sessionId);
    await waitForOrbifoldReady(send, sessionId);

    await dispatchBrowserAction(send, sessionId, "midi.refresh");
    await waitForProjectState(
      send,
      sessionId,
      (state) =>
        state.midiInputCount === 0 &&
        state.connectedMidiInput === "" &&
        state.browserMidiInputNames === "" &&
        state.browserMidiDiagnostic.includes(expectedDiagnostic) &&
        state.lastStatus.includes(refreshStatus),
      `browser MIDI ${label} refresh did not surface a visible unavailable/permission error`
    );

    await dispatchBrowserAction(send, sessionId, "midi.connect");
    await waitForProjectState(
      send,
      sessionId,
      (state) =>
        state.midiInputCount === 0 &&
        state.connectedMidiInput === "" &&
        state.browserMidiInputNames === "" &&
        state.browserMidiDiagnostic.includes(expectedDiagnostic) &&
        state.lastStatus.includes(connectStatus),
      `browser MIDI ${label} connect did not stay disconnected with a visible error`
    );
  } finally {
    await send("Target.closeTarget", { targetId }).catch(() => {});
  }
}

async function verifyBrowserAudioFlow(send, sessionId) {
  await dispatchBrowserAction(send, sessionId, "audio.refresh");
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.audioOutputCount >= 2 &&
      state.audioOutputSelectionSupported &&
      state.browserAudioOutputNames.includes("Default Smoke Output") &&
      state.browserAudioOutputNames.includes("Orbifold Smoke Speakers") &&
      /^Refreshed audio outputs: \d+ audio outputs?/.test(state.lastStatus),
    "browser audio refresh did not expose mocked Web Audio outputs"
  );

  await dispatchBrowserAction(send, sessionId, "audio.connect");
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.audioStreamConnected &&
      state.connectedAudioOutput === "Default Smoke Output" &&
      state.audioSinkRequested === "Default Smoke Output" &&
      state.audioSinkResolved === "Default Smoke Output" &&
      state.audioSinkDeviceId === "default" &&
      state.smokeSinkId === "default" &&
      state.browserAudioDiagnostic.includes("Web Audio: sink") &&
      state.browserAudioDiagnostic.includes("2 outs") &&
      state.browserAudioDiagnostic.includes("routed") &&
      state.audioContextCreated &&
      state.audioProcessorAttached &&
      state.audioResumeRequested,
    "browser audio connect did not create and route the named Web Audio stream"
  );

  await dispatchBrowserAction(send, sessionId, "audio.test_a4");
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastStatus.includes("Test tone A4") &&
      state.audioCallbackCount > 0 &&
      state.audioFrameCount > 0 &&
      state.audioNonzero &&
      state.audioPeak > 0.0001,
    "browser audio test tone did not produce nonzero Web Audio samples"
  );
}

async function verifyBrowserAudioFallbackFlow(send) {
  const { targetId } = await send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await send("Target.attachToTarget", {
    targetId,
    flatten: true,
  });
  try {
    await send("Runtime.enable", {}, sessionId);
    await send("Log.enable", {}, sessionId);
    await send("Network.enable", {}, sessionId);
    await send("Page.enable", {}, sessionId);
    await send(
      "Page.addScriptToEvaluateOnNewDocument",
      {
        source: `(() => {
          for (const name of ["AudioContext", "webkitAudioContext"]) {
            const Ctor = window[name];
            if (!Ctor || !Ctor.prototype) {
              continue;
            }
            try {
              Object.defineProperty(Ctor.prototype, "setSinkId", {
                configurable: true,
                value: undefined,
              });
            } catch (_error) {
              try {
                delete Ctor.prototype.setSinkId;
              } catch (_deleteError) {}
            }
          }
        })();`,
      },
      sessionId
    );
    await send(
      "Emulation.setDeviceMetricsOverride",
      {
        width: 1200,
        height: 760,
        deviceScaleFactor: 1,
        mobile: false,
      },
      sessionId
    );
    await send("Page.navigate", { url: urlForSmokeVariant("audio-fallback") }, sessionId);
    await waitForOrbifoldReady(send, sessionId);

    await dispatchBrowserAction(send, sessionId, "audio.refresh");
    await waitForProjectState(
      send,
      sessionId,
      (state) =>
        state.audioOutputCount === 1 &&
        !state.audioOutputSelectionSupported &&
        state.browserAudioOutputNames === "Browser audio" &&
        state.browserAudioDiagnostic.includes("Web Audio: default") &&
        state.lastStatus.includes("Refreshed audio outputs: 1 audio output"),
      "browser audio refresh did not expose the default fallback output when sink selection is unavailable"
    );

    await activateBrowserDocument(send, sessionId);
    await dispatchBrowserAction(send, sessionId, "audio.connect");
    await waitForProjectState(
      send,
      sessionId,
      (state) =>
        state.audioStreamConnected &&
        state.connectedAudioOutput === "Browser audio" &&
        state.audioSinkRequested === "Browser audio" &&
        state.audioSinkSelectionUnsupported &&
        state.audioContextCreated &&
        state.audioProcessorAttached &&
        state.audioResumeRequested,
      "browser audio fallback connect did not create a default Web Audio stream"
    );

    await activateBrowserDocument(send, sessionId);
    await dispatchBrowserAction(send, sessionId, "audio.test_a4");
    await waitForProjectState(
      send,
      sessionId,
      (state) =>
        state.lastStatus.includes("Test tone A4") &&
        state.audioCallbackCount > 0 &&
        state.audioFrameCount > 0 &&
        state.audioNonzero &&
        state.audioPeak > 0.0001,
      "browser audio fallback test tone did not produce nonzero Web Audio samples"
    );
  } finally {
    await send("Target.closeTarget", { targetId }).catch(() => {});
  }
}

async function activateBrowserDocument(send, sessionId) {
  const point = { x: 12, y: 12 };
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mouseMoved",
      x: point.x,
      y: point.y,
      button: "none",
    },
    sessionId
  );
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mousePressed",
      x: point.x,
      y: point.y,
      button: "left",
      buttons: 1,
      clickCount: 1,
    },
    sessionId
  );
  await delay(80);
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mouseReleased",
      x: point.x,
      y: point.y,
      button: "left",
      buttons: 0,
      clickCount: 1,
    },
    sessionId
  );
}

async function verifyBrowserPersistenceAfterReload(send, sessionId) {
  await verifyBrowserUiScaleReload(send, sessionId);
  await setBrowserPanelVisibility(send, sessionId, {
    showAssetBrowser: false,
    showScaleBrowser: true,
    showClipPanel: true,
  });

  const beforeReload = await evaluateProjectState(send, sessionId);
  const expectedNotes = persistedNoteCount(beforeReload.project);
  if (
    expectedNotes < 1 ||
    beforeReload.assetCount < 1 ||
    beforeReload.scaleDescription !== smokeScaleDescription ||
    beforeReload.scalaPath !== "browser_5_edo.scl" ||
    beforeReload.lumatonePath !== "classic.ltn" ||
    !beforeReload.lumatoneLoaded ||
    !beforeReload.project.includes(smokeScalaProjectLine) ||
    !beforeReload.project.includes(smokeLumatoneProjectLine) ||
    !beforeReload.project.includes(smokeSampleInstrumentProjectLine)
  ) {
    throw new Error(
      `browser persistence precondition missing project, text-resource, asset, or sample-instrument state: ${JSON.stringify(
        beforeReload
      )}`
    );
  }
  if (
    !beforeReload.settings.includes("show_asset_browser=false") ||
    !beforeReload.settings.includes("show_scale_browser=true") ||
    !beforeReload.settings.includes("show_clip_panel=true")
  ) {
    throw new Error(
      `browser settings precondition missing persisted panel visibility: ${JSON.stringify(
        beforeReload
      )}`
    );
  }

  await send("Page.reload", { ignoreCache: true }, sessionId);
  await waitForOrbifoldReady(send, sessionId);
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      persistedNoteCount(state.project) === expectedNotes &&
      state.assetCount >= beforeReload.assetCount &&
      state.scaleDescription === smokeScaleDescription &&
      state.scalaPath === "browser_5_edo.scl" &&
      state.lumatonePath === "classic.ltn" &&
      state.lumatoneLoaded &&
      state.project.includes(smokeScalaProjectLine) &&
      state.project.includes(smokeLumatoneProjectLine) &&
      state.project.includes(smokeSampleInstrumentProjectLine) &&
      state.showAssetBrowser === false &&
      state.showScaleBrowser === true &&
      state.showClipPanel === true &&
      state.settings.includes("show_asset_browser=false") &&
      state.settings.includes("show_scale_browser=true") &&
      state.settings.includes("show_clip_panel=true") &&
      state.lastStatus.includes("Restored browser project session"),
    "browser reload did not restore the saved project session, browser text resources, imported sample instrument, asset, and panel settings"
  );
}

async function verifyBrowserUiScaleReload(send, sessionId) {
  const beforeZoom = await evaluateProjectState(send, sessionId);
  await dispatchBrowserAction(send, sessionId, "ui.scale_up");
  await delay(1000);
  await waitForOrbifoldReady(send, sessionId);
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.uiScale > beforeZoom.uiScale &&
      state.settings.includes("ui_scale=1.1") &&
      state.lastStatus.includes("Restored browser project session"),
    "browser UI scale action did not persist settings and reload the web runtime"
  );
}

async function setBrowserPanelVisibility(send, sessionId, desired) {
  let state = await evaluateProjectState(send, sessionId);
  if (state.showAssetBrowser !== desired.showAssetBrowser) {
    await dispatchBrowserAction(send, sessionId, "view.assets");
    state = await waitForProjectState(
      send,
      sessionId,
      (state) => state.showAssetBrowser === desired.showAssetBrowser,
      "browser asset-panel visibility setting did not change"
    );
  }
  if (state.showScaleBrowser !== desired.showScaleBrowser) {
    await dispatchBrowserAction(send, sessionId, "view.scales");
    state = await waitForProjectState(
      send,
      sessionId,
      (state) => state.showScaleBrowser === desired.showScaleBrowser,
      "browser scale-panel visibility setting did not change"
    );
  }
  if (state.showClipPanel !== desired.showClipPanel) {
    await dispatchBrowserAction(send, sessionId, "view.clip");
    await waitForProjectState(
      send,
      sessionId,
      (state) => state.showClipPanel === desired.showClipPanel,
      "browser clip-panel visibility setting did not change"
    );
  }
}

async function verifyBrowserProjectDownload(send, sessionId, artifactsDir) {
  await dispatchBrowserAction(send, sessionId, "file.save");
  const state = await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastAction === "file.save" &&
      state.downloadFileName.endsWith(".orbifold") &&
      state.downloadSize > 0 &&
      state.downloadText.includes("orbifold_project=1") &&
      persistedNoteCount(state.downloadText) >= 3,
    "browser project save did not publish a downloadable project file"
  );
  await waitForDownloadedFile(artifactsDir, state.downloadFileName, state.downloadSize);
  return state.downloadText;
}

async function verifyBrowserProjectSaveAsShortcut(send, sessionId, artifactsDir) {
  const before = await evaluateProjectState(send, sessionId);
  removeDownloadedFile(artifactsDir, before.downloadFileName);

  await pressKey(send, sessionId, {
    key: "s",
    code: "KeyS",
    windowsVirtualKeyCode: 83,
    modifiers: 10,
  });

  const state = await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastAction === "file.save_as" &&
      state.downloadFileName.endsWith(".orbifold") &&
      state.downloadSize > 0 &&
      state.downloadText.includes("orbifold_project=1") &&
      persistedNoteCount(state.downloadText) >= 3,
    "browser Ctrl+Shift+S shortcut did not publish a downloadable project file"
  );
  await waitForDownloadedFile(artifactsDir, state.downloadFileName, state.downloadSize);
}

function removeDownloadedFile(downloadDir, fileName) {
  if (!fileName) {
    return;
  }
  const filePath = path.join(downloadDir, path.basename(fileName));
  fs.rmSync(filePath, { force: true });
}

async function chooseFileForBrowserAction(send, sessionId, action, filePath) {
  await dispatchBrowserAction(send, sessionId, action);
  const nodeId = await waitForFileInput(send, sessionId);
  await send("DOM.setFileInputFiles", { nodeId, files: [filePath] }, sessionId);
  await delay(250);
}

async function waitForFileInput(send, sessionId) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() <= deadline) {
    const nodeId = await fileInputNodeId(send, sessionId);
    if (nodeId) {
      return nodeId;
    }
    await delay(100);
  }
  throw new Error("browser action did not open a file input");
}

async function assertNoFileInput(send, sessionId, failureMessage) {
  await delay(500);
  const nodeId = await fileInputNodeId(send, sessionId);
  if (nodeId) {
    throw new Error(failureMessage);
  }
}

async function fileInputNodeId(send, sessionId) {
  const document = await send("DOM.getDocument", { depth: 1 }, sessionId);
  const result = await send(
    "DOM.querySelector",
    { nodeId: document.root.nodeId, selector: "input[type=file]" },
    sessionId
  );
  return result.nodeId || 0;
}

async function dispatchBrowserAction(send, sessionId, action) {
  const dispatchResult = await send(
    "Runtime.evaluate",
    {
      expression: `window.orbifoldDispatchAction(${JSON.stringify(action)})`,
      returnByValue: true,
    },
    sessionId
  );
  if (dispatchResult.result.value !== true) {
    throw new Error(`browser action dispatch hook rejected ${action}`);
  }
}

async function verifyBrowserTextEditActions(send, sessionId) {
  await dispatchBrowserTextInput(send, sessionId, "transport.bpm_input", "144");
  await dispatchBrowserTextKey(send, sessionId, "transport.bpm_input", "Enter");
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.project.includes("\nbpm=144\n"),
    "browser BPM text edit did not commit through the shared text edit path"
  );

  await dispatchBrowserTextInput(send, sessionId, "scale.root_input", "60");
  await dispatchBrowserTextKey(send, sessionId, "scale.root_input", "Enter");
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.project.includes("\nroot_midi=60\n") && state.settings.includes("root_midi=60\n"),
    "browser root text edit did not persist through the shared text edit path"
  );

  await dispatchBrowserTextInput(send, sessionId, "scale.base_input", "432");
  await dispatchBrowserTextKey(send, sessionId, "scale.base_input", "Enter");
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.project.includes("\nbase_freq=432\n") && state.settings.includes("base_freq=432\n"),
    "browser base-frequency text edit did not persist through the shared text edit path"
  );

  await dispatchBrowserTextInput(send, sessionId, "scale.search", "edo");
  await dispatchBrowserTextKey(send, sessionId, "scale.search", "Enter");
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.lastStatus.includes("Scale search: edo"),
    "browser scale search text edit did not update status through the shared text edit path"
  );

  await dispatchBrowserTextInput(send, sessionId, "asset.search", "kick");
  await dispatchBrowserTextKey(send, sessionId, "asset.search", "Enter");
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.lastStatus.includes("Asset search: kick"),
    "browser asset search text edit did not update status through the shared text edit path"
  );
}

async function dispatchBrowserTextInput(send, sessionId, action, text) {
  await dispatchBrowserTextEdit(send, sessionId, "orbifoldDispatchTextInput", action, text);
}

async function dispatchBrowserTextKey(send, sessionId, action, key) {
  await dispatchBrowserTextEdit(send, sessionId, "orbifoldDispatchTextKey", action, key);
}

async function dispatchBrowserTextEdit(send, sessionId, hook, action, value) {
  const dispatchResult = await send(
    "Runtime.evaluate",
    {
      expression: `window.${hook}(${JSON.stringify(action)}, ${JSON.stringify(value)})`,
      returnByValue: true,
    },
    sessionId
  );
  if (dispatchResult.result.value !== true) {
    throw new Error(`browser text edit dispatch hook rejected ${action}`);
  }
}

async function waitForProjectState(send, sessionId, predicate, failureMessage) {
  const deadline = Date.now() + timeoutMs;
  let lastState = null;
  while (Date.now() <= deadline) {
    lastState = await evaluateProjectState(send, sessionId);
    if (predicate(lastState)) {
      return lastState;
    }
    await delay(250);
  }
  throw new Error(`${failureMessage}; last state: ${JSON.stringify(lastState)}`);
}

async function waitForDownloadedFile(downloadDir, fileName, minimumBytes) {
  const filePath = path.join(downloadDir, fileName);
  const deadline = Date.now() + timeoutMs;
  while (Date.now() <= deadline) {
    if (fs.existsSync(filePath) && fs.statSync(filePath).size >= minimumBytes) {
      return;
    }
    await delay(100);
  }
  throw new Error(`browser download did not create ${fileName} in ${downloadDir}`);
}

function projectWithFirstNoteOnly(projectText) {
  let keptNote = false;
  const lines = projectText.split("\n").filter((line) => {
    if (!line.startsWith("note\t")) {
      return true;
    }
    if (keptNote) {
      return false;
    }
    keptNote = true;
    return true;
  });
  if (!keptNote) {
    throw new Error("cannot build one-note project fixture from project with no notes");
  }
  return lines.join("\n");
}

function pcm16WavBuffer() {
  const sampleRate = 48_000;
  const sampleCount = 256;
  const bytesPerSample = 2;
  const dataSize = sampleCount * bytesPerSample;
  const buffer = Buffer.alloc(44 + dataSize);
  buffer.write("RIFF", 0);
  buffer.writeUInt32LE(36 + dataSize, 4);
  buffer.write("WAVE", 8);
  buffer.write("fmt ", 12);
  buffer.writeUInt32LE(16, 16);
  buffer.writeUInt16LE(1, 20);
  buffer.writeUInt16LE(1, 22);
  buffer.writeUInt32LE(sampleRate, 24);
  buffer.writeUInt32LE(sampleRate * bytesPerSample, 28);
  buffer.writeUInt16LE(bytesPerSample, 32);
  buffer.writeUInt16LE(16, 34);
  buffer.write("data", 36);
  buffer.writeUInt32LE(dataSize, 40);
  for (let i = 0; i < sampleCount; i += 1) {
    const sample = Math.round(Math.sin((i / sampleCount) * Math.PI * 2) * 12_000);
    buffer.writeInt16LE(sample, 44 + i * bytesPerSample);
  }
  return buffer;
}

async function dragPointer(send, sessionId, drag) {
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mousePressed",
      x: drag.startX,
      y: drag.startY,
      button: "left",
      clickCount: 1,
    },
    sessionId
  );
  await delay(120);
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mouseMoved",
      x: drag.startX + (drag.endX - drag.startX) * 0.35,
      y: drag.startY + (drag.endY - drag.startY) * 0.35,
      button: "left",
      buttons: 1,
    },
    sessionId
  );
  await delay(120);
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mouseMoved",
      x: drag.endX,
      y: drag.endY,
      button: "left",
      buttons: 1,
    },
    sessionId
  );
  await delay(120);
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mouseReleased",
      x: drag.endX,
      y: drag.endY,
      button: "left",
      clickCount: 1,
    },
    sessionId
  );
}

async function dispatchWheelUntilViewChanges(
  send,
  sessionId,
  point,
  attempts,
  baseline,
  predicate,
  failureMessage
) {
  let lastView = baseline;
  for (const attempt of attempts) {
    await dispatchMouseWheel(send, sessionId, point, attempt);
    const deadline = Date.now() + 1_500;
    while (Date.now() <= deadline) {
      lastView = await evaluatePianoView(send, sessionId);
      if (predicate(lastView, baseline)) {
        return lastView;
      }
      await delay(100);
    }
  }
  throw new Error(
    `${failureMessage}; before=${JSON.stringify(baseline)} after=${JSON.stringify(lastView)}`
  );
}

async function dispatchMouseWheel(send, sessionId, point, attempt) {
  await send(
    "Input.dispatchMouseEvent",
    {
      type: "mouseWheel",
      x: point.x,
      y: point.y,
      deltaX: attempt.deltaX ?? 0,
      deltaY: attempt.deltaY ?? 0,
      modifiers: attempt.modifiers ?? 0,
    },
    sessionId
  );
  await delay(120);
}

async function evaluatePianoView(send, sessionId) {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `(() => {
        const minPitch = Number(document.body.dataset.orbifoldPianoMinPitch ?? 0);
        const maxPitch = Number(document.body.dataset.orbifoldPianoMaxPitch ?? 0);
        return {
          viewStart: Number(document.body.dataset.orbifoldPianoViewStart ?? 0),
          viewBeats: Number(document.body.dataset.orbifoldPianoViewBeats ?? 0),
          minPitch,
          maxPitch,
          pitchCenter: (minPitch + maxPitch) / 2,
          pitchRows: maxPitch - minPitch + 1
        };
      })()`,
      returnByValue: true,
    },
    sessionId
  );
  return result.result.value ?? {};
}

async function waitForLayoutGeometry(send, sessionId, predicate) {
  const deadline = Date.now() + timeoutMs;
  let layout = null;
  while (Date.now() <= deadline) {
    layout = await evaluateLayoutGeometry(send, sessionId);
    if (predicate(layout)) {
      return layout;
    }
    await delay(250);
  }
  throw new Error(`layout automation geometry unavailable: ${JSON.stringify(layout)}`);
}

async function evaluateLayoutGeometry(send, sessionId) {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `({
        rightResizeX: Number(document.body.dataset.orbifoldRightResizeX ?? 0),
        rightResizeY: Number(document.body.dataset.orbifoldRightResizeY ?? 0),
        rightResizeEndX: Number(document.body.dataset.orbifoldRightResizeEndX ?? 0),
        rightResizeEndY: Number(document.body.dataset.orbifoldRightResizeEndY ?? 0),
        bottomResizeX: Number(document.body.dataset.orbifoldBottomResizeX ?? 0),
        bottomResizeY: Number(document.body.dataset.orbifoldBottomResizeY ?? 0),
        bottomResizeEndX: Number(document.body.dataset.orbifoldBottomResizeEndX ?? 0),
        bottomResizeEndY: Number(document.body.dataset.orbifoldBottomResizeEndY ?? 0),
        rightPanelWidth: Number(document.body.dataset.orbifoldRightPanelWidth ?? 0),
        pianoRollHeight: Number(document.body.dataset.orbifoldPianoRollHeight ?? 0),
        pianoViewStart: Number(document.body.dataset.orbifoldPianoViewStart ?? 0),
        pianoViewBeats: Number(document.body.dataset.orbifoldPianoViewBeats ?? 0),
        arrangementSeekStartX: Number(document.body.dataset.orbifoldArrangementSeekStartX ?? 0),
        arrangementSeekStartY: Number(document.body.dataset.orbifoldArrangementSeekStartY ?? 0),
        arrangementSeekEndX: Number(document.body.dataset.orbifoldArrangementSeekEndX ?? 0),
        arrangementSeekEndY: Number(document.body.dataset.orbifoldArrangementSeekEndY ?? 0),
        pianoSeekStartX: Number(document.body.dataset.orbifoldPianoSeekStartX ?? 0),
        pianoSeekStartY: Number(document.body.dataset.orbifoldPianoSeekStartY ?? 0),
        pianoSeekEndX: Number(document.body.dataset.orbifoldPianoSeekEndX ?? 0),
        pianoSeekEndY: Number(document.body.dataset.orbifoldPianoSeekEndY ?? 0),
        arrangementLoopEndStartX: Number(document.body.dataset.orbifoldArrangementLoopEndStartX ?? 0),
        arrangementLoopEndStartY: Number(document.body.dataset.orbifoldArrangementLoopEndStartY ?? 0),
        arrangementLoopEndTargetX: Number(document.body.dataset.orbifoldArrangementLoopEndTargetX ?? 0),
        arrangementLoopEndTargetY: Number(document.body.dataset.orbifoldArrangementLoopEndTargetY ?? 0),
        pianoLoopEndStartX: Number(document.body.dataset.orbifoldPianoLoopEndStartX ?? 0),
        pianoLoopEndStartY: Number(document.body.dataset.orbifoldPianoLoopEndStartY ?? 0),
        pianoLoopEndTargetX: Number(document.body.dataset.orbifoldPianoLoopEndTargetX ?? 0),
        pianoLoopEndTargetY: Number(document.body.dataset.orbifoldPianoLoopEndTargetY ?? 0)
      })`,
      returnByValue: true,
    },
    sessionId
  );
  return result.result.value ?? {};
}

async function waitForPianoGeometry(send, sessionId, predicate) {
  const deadline = Date.now() + timeoutMs;
  let geometry = null;
  while (Date.now() <= deadline) {
    geometry = await evaluatePianoGeometry(send, sessionId);
    if (predicate(geometry)) {
      return geometry;
    }
    await delay(250);
  }
  throw new Error(`piano automation geometry unavailable: ${JSON.stringify(geometry)}`);
}

async function verifyBrowserShortcutMapping(send, sessionId) {
  const probeResult = await send(
    "Runtime.evaluate",
    {
      expression: `typeof window.__orbifoldShortcutActionForTest === "function"`,
      returnByValue: true,
    },
    sessionId
  );
  if (probeResult.result.value !== true) {
    throw new Error("browser shortcut mapping probe was not installed");
  }

  for (const shortcut of browserShortcutMappingCases()) {
    const actual = await evaluateShortcutAction(send, sessionId, shortcut.event);
    if (actual !== shortcut.action) {
      const expected = shortcut.action || "(none)";
      const observed = actual || "(none)";
      throw new Error(
        `browser shortcut mapping mismatch for ${shortcut.label}: expected ${expected}, got ${observed}`
      );
    }
  }
}

function browserShortcutMappingCases() {
  return [
    shortcutCase("Ctrl+N", { key: "n", ctrlKey: true }, "file.new"),
    shortcutCase("Ctrl+O", { key: "o", ctrlKey: true }, "file.open"),
    shortcutCase("Ctrl+S", { key: "s", ctrlKey: true }, "file.save"),
    shortcutCase("Ctrl+Shift+S", { key: "s", ctrlKey: true, shiftKey: true }, "file.save_as"),
    shortcutCase("Cmd+S", { key: "s", metaKey: true }, "file.save"),
    shortcutCase("Ctrl+Z", { key: "z", ctrlKey: true }, "edit.undo"),
    shortcutCase("Ctrl+Y", { key: "y", ctrlKey: true }, "edit.redo"),
    shortcutCase("Ctrl+Shift+Z", { key: "z", ctrlKey: true, shiftKey: true }, "edit.redo"),
    shortcutCase("Ctrl+C", { key: "c", ctrlKey: true }, "clip.copy_note"),
    shortcutCase("Ctrl+V", { key: "v", ctrlKey: true }, "clip.paste_note"),
    shortcutCase("Ctrl++", { key: "+", ctrlKey: true }, "ui.scale_up"),
    shortcutCase("Ctrl+=", { key: "=", ctrlKey: true }, "ui.scale_up"),
    shortcutCase("Ctrl+-", { key: "-", ctrlKey: true }, "ui.scale_down"),
    shortcutCase("Ctrl+0", { key: "0", ctrlKey: true }, "ui.scale_reset"),
    shortcutCase("Space", { key: " " }, "transport.play_stop"),
    shortcutCase("Home", { key: "Home" }, "transport.prev"),
    shortcutCase("Escape", { key: "Escape" }, "edit.escape"),
    shortcutCase("?", { key: "?" }, "help.shortcuts"),
    shortcutCase("Shift+/", { key: "/", shiftKey: true }, "help.shortcuts"),
    shortcutCase("R", { key: "r" }, "transport.record"),
    shortcutCase("M", { key: "m" }, "transport.metronome"),
    shortcutCase("Shift+Q", { key: "q", shiftKey: true }, "transport.record_quantize"),
    shortcutCase("Q", { key: "q" }, "clip.quantize"),
    shortcutCase("G", { key: "g" }, "transport.snap"),
    shortcutCase("P", { key: "p" }, "audio.all_off"),
    shortcutCase("N", { key: "n" }, "clip.add_note"),
    shortcutCase("D", { key: "d" }, "clip.duplicate_note"),
    shortcutCase("Delete", { key: "Delete" }, "clip.delete_note"),
    shortcutCase("Backspace", { key: "Backspace" }, "clip.delete_note"),
    shortcutCase("ArrowLeft", { key: "ArrowLeft" }, "clip.nudge_left"),
    shortcutCase("ArrowRight", { key: "ArrowRight" }, "clip.nudge_right"),
    shortcutCase("ArrowUp", { key: "ArrowUp" }, "clip.pitch_up"),
    shortcutCase("ArrowDown", { key: "ArrowDown" }, "clip.pitch_down"),
    shortcutCase("Shift+ArrowLeft", { key: "ArrowLeft", shiftKey: true }, "clip.shorter"),
    shortcutCase("Shift+ArrowRight", { key: "ArrowRight", shiftKey: true }, "clip.longer"),
    shortcutCase("Shift+ArrowUp", { key: "ArrowUp", shiftKey: true }, "clip.velocity_up"),
    shortcutCase("Shift+ArrowDown", { key: "ArrowDown", shiftKey: true }, "clip.velocity_down"),
    shortcutCase("+", { key: "+" }, "piano.zoom_in"),
    shortcutCase("=", { key: "=" }, "piano.zoom_in"),
    shortcutCase("-", { key: "-" }, "piano.zoom_out"),
    shortcutCase("Alt+R", { key: "r", altKey: true }, ""),
    shortcutCase("Ctrl+Alt+S", { key: "s", ctrlKey: true, altKey: true }, ""),
    shortcutCase("Repeat Space", { key: " ", repeat: true }, ""),
    shortcutCase("Repeat ArrowRight", { key: "ArrowRight", repeat: true }, "clip.nudge_right"),
    shortcutCase("Shift+N", { key: "n", shiftKey: true }, ""),
    shortcutCase("Ctrl+Shift+C", { key: "c", ctrlKey: true, shiftKey: true }, ""),
  ];
}

function shortcutCase(label, event, action) {
  return { label, event, action };
}

async function evaluateShortcutAction(send, sessionId, event) {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `window.__orbifoldShortcutActionForTest(${JSON.stringify(event)})`,
      returnByValue: true,
    },
    sessionId
  );
  return result.result.value ?? "";
}

async function verifyKeyboardShortcut(send, sessionId) {
  await send(
    "Runtime.evaluate",
    {
      expression: `document.getElementById("orbifold-canvas")?.focus()`,
      returnByValue: true,
    },
    sessionId
  );
  await pressKey(send, sessionId, {
    key: "n",
    code: "KeyN",
    windowsVirtualKeyCode: 78,
    text: "n",
  });

  await waitForProjectState(
    send,
    sessionId,
    (state) => {
      const persistedNotes = persistedNoteCount(state.project);
      return (
        state.lastAction === "clip.add_note" && state.noteCount >= 2 && persistedNotes >= 2
      );
    },
    "browser keyboard shortcut did not persist a second clip note"
  );
  await verifyKeyboardNoteEditShortcuts(send, sessionId);

  await pressKey(send, sessionId, {
    key: "?",
    code: "Slash",
    windowsVirtualKeyCode: 191,
    text: "?",
    modifiers: 8,
  });
  await waitForProjectState(
    send,
    sessionId,
    (state) =>
      state.lastAction === "help.shortcuts" && state.lastStatus.includes("Shortcuts:"),
    "browser keyboard shortcut did not route Shift+/ to shortcut help"
  );

  await pressKey(send, sessionId, {
    key: " ",
    code: "Space",
    windowsVirtualKeyCode: 32,
    text: " ",
  });
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.lastAction === "transport.play_stop" && state.transportPlaying,
    "browser keyboard shortcut did not route Space to transport play"
  );
  await pressKey(send, sessionId, {
    key: " ",
    code: "Space",
    windowsVirtualKeyCode: 32,
    text: " ",
  });
  await waitForProjectState(
    send,
    sessionId,
    (state) => state.lastAction === "transport.play_stop" && !state.transportPlaying,
    "browser keyboard shortcut did not route Space to transport stop"
  );
}

async function verifyKeyboardNoteEditShortcuts(send, sessionId) {
  const before = await evaluateProjectState(send, sessionId);
  const editedNote = latestProjectNote(before.project);
  if (!editedNote) {
    throw new Error(`browser keyboard note-edit precondition found no notes: ${before.project}`);
  }

  await pressKey(send, sessionId, {
    key: "ArrowRight",
    code: "ArrowRight",
    windowsVirtualKeyCode: 39,
  });
  const nudged = await waitForProjectState(
    send,
    sessionId,
    (state) => {
      const note = projectNoteById(state.project, editedNote.id);
      return (
        state.lastAction === "clip.nudge_right" &&
        note &&
        note.startBeat > editedNote.startBeat
      );
    },
    "browser keyboard ArrowRight did not nudge the selected note"
  );

  await pressKey(send, sessionId, {
    key: "ArrowRight",
    code: "ArrowRight",
    windowsVirtualKeyCode: 39,
    modifiers: 8,
  });
  const lengthened = await waitForProjectState(
    send,
    sessionId,
    (state) => {
      const note = projectNoteById(state.project, editedNote.id);
      const nudgedNote = projectNoteById(nudged.project, editedNote.id);
      return (
        state.lastAction === "clip.longer" &&
        note &&
        nudgedNote &&
        note.durationBeat > nudgedNote.durationBeat
      );
    },
    "browser keyboard Shift+ArrowRight did not lengthen the selected note"
  );

  await pressKey(send, sessionId, {
    key: "ArrowUp",
    code: "ArrowUp",
    windowsVirtualKeyCode: 38,
  });
  const transposed = await waitForProjectState(
    send,
    sessionId,
    (state) => {
      const note = projectNoteById(state.project, editedNote.id);
      const lengthenedNote = projectNoteById(lengthened.project, editedNote.id);
      return (
        state.lastAction === "clip.pitch_up" &&
        note &&
        lengthenedNote &&
        note.musicalNote > lengthenedNote.musicalNote
      );
    },
    "browser keyboard ArrowUp did not transpose the selected note"
  );

  await pressKey(send, sessionId, {
    key: "ArrowUp",
    code: "ArrowUp",
    windowsVirtualKeyCode: 38,
    modifiers: 8,
  });
  await waitForProjectState(
    send,
    sessionId,
    (state) => {
      const note = projectNoteById(state.project, editedNote.id);
      const transposedNote = projectNoteById(transposed.project, editedNote.id);
      return (
        state.lastAction === "clip.velocity_up" &&
        note &&
        transposedNote &&
        note.velocity > transposedNote.velocity
      );
    },
    "browser keyboard Shift+ArrowUp did not raise the selected note velocity"
  );
}

async function pressKey(
  send,
  sessionId,
  { key, code, windowsVirtualKeyCode, text = "", modifiers = 0 }
) {
  const event = {
    key,
    code,
    windowsVirtualKeyCode,
    nativeVirtualKeyCode: windowsVirtualKeyCode,
    modifiers,
  };
  if (text) {
    event.text = text;
    event.unmodifiedText = text;
  }
  await send("Input.dispatchKeyEvent", { ...event, type: "rawKeyDown" }, sessionId);
  await send("Input.dispatchKeyEvent", { ...event, type: "keyUp" }, sessionId);
}

async function evaluateProjectState(send, sessionId) {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `({
        project: localStorage.getItem("orbifold.project.v1") || "",
        settings: localStorage.getItem("orbifold.settings.v1") || "",
        title: document.title,
        lastAction: document.body.dataset.orbifoldLastAction ?? "",
        lastPointerAction: document.body.dataset.orbifoldLastPointerAction ?? "",
        lastPointerPhase: document.body.dataset.orbifoldLastPointerPhase ?? "",
        noteCount: Number(document.body.dataset.orbifoldProjectNoteCount ?? 0),
        lastStatus: document.body.dataset.orbifoldLastStatus ?? "",
        frameCount: Number(document.body.dataset.orbifoldFrameCount ?? 0),
        assetCount: Number(document.body.dataset.orbifoldAudioAssetCount ?? 0),
        midiInputCount: Number(document.body.dataset.orbifoldMidiInputCount ?? 0),
        connectedMidiInput: document.body.dataset.orbifoldConnectedMidiInput ?? "",
        browserMidiInputNames: document.body.dataset.orbifoldBrowserMidiInputNames ?? "",
        browserMidiDiagnostic: document.body.dataset.orbifoldBrowserMidiDiagnostic ?? "",
        midiInputState: document.body.dataset.orbifoldMidiInputState ?? "",
        midiInputConnection: document.body.dataset.orbifoldMidiInputConnection ?? "",
        lastMidiStatus: Number(document.body.dataset.orbifoldLastMidiStatus ?? 0),
        lastMidiNote: Number(document.body.dataset.orbifoldLastMidiNote ?? -1),
        audioOutputCount: Number(document.body.dataset.orbifoldAudioOutputCount ?? 0),
        connectedAudioOutput: document.body.dataset.orbifoldConnectedAudioOutput ?? "",
        audioOutputSelectionSupported: document.body.dataset.orbifoldAudioOutputSelectionSupported === "1",
        browserAudioOutputNames: document.body.dataset.orbifoldBrowserAudioOutputNames ?? "",
        browserAudioDiagnostic: document.body.dataset.orbifoldBrowserAudioDiagnostic ?? "",
        audioSinkRequested: document.body.dataset.orbifoldAudioSinkRequested ?? "",
        audioSinkResolved: document.body.dataset.orbifoldAudioSinkResolved ?? "",
        audioSinkDeviceId: document.body.dataset.orbifoldAudioSinkDeviceId ?? "",
        audioSinkSelectionUnsupported: document.body.dataset.orbifoldAudioSinkSelectionUnsupported === "1",
        smokeSinkId: document.body.dataset.orbifoldSmokeSinkId ?? "",
        audioStreamConnected: document.body.dataset.orbifoldAudioStreamConnected === "1",
        audioContextCreated: document.body.dataset.orbifoldAudioContextCreated === "1",
        audioProcessorAttached: document.body.dataset.orbifoldAudioProcessorAttached === "1",
        audioResumeRequested: document.body.dataset.orbifoldAudioResumeRequested === "1",
        audioResumeResolved: document.body.dataset.orbifoldAudioResumeResolved === "1",
        audioCallbackCount: Number(document.body.dataset.orbifoldAudioCallbackCount ?? 0),
        audioFrameCount: Number(document.body.dataset.orbifoldAudioFrameCount ?? 0),
        audioPeak: Number(document.body.dataset.orbifoldAudioPeak ?? 0),
        audioNonzero: document.body.dataset.orbifoldAudioNonzero === "1",
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
        transportPlaying: document.body.dataset.orbifoldTransportPlaying === "1",
        downloadFileName: document.body.dataset.orbifoldLastDownloadFileName ?? "",
        downloadSize: Number(document.body.dataset.orbifoldLastDownloadSize ?? 0),
        downloadText: window.__orbifoldLastDownloadText || ""
      })`,
      returnByValue: true,
    },
    sessionId
  );
  return result.result.value ?? {};
}

function persistedNoteCount(projectText) {
  return (projectText.match(/\nnote\t/g) ?? []).length;
}

function projectNotes(projectText) {
  return projectText
    .split("\n")
    .map((line) => line.split("\t"))
    .filter((parts) => parts[0] === "note")
    .map((parts) => ({
      id: Number(parts[1]),
      startBeat: Number(parts[2]),
      durationBeat: Number(parts[3]),
      keyIndex: Number(parts[4]),
      musicalNote: Number(parts[5]),
      rawChannel: Number(parts[6]),
      rawNote: Number(parts[7]),
      velocity: Number(parts[8]),
    }));
}

function latestProjectNote(projectText) {
  return projectNotes(projectText).sort((left, right) => right.id - left.id)[0] ?? null;
}

function projectNoteById(projectText, id) {
  return projectNotes(projectText).find((note) => note.id === id) ?? null;
}

function thirdNoteStartBeat(projectText) {
  for (const line of projectText.split("\n")) {
    const parts = line.split("\t");
    if (parts[0] === "note" && parts[1] === "3") {
      return Number(parts[2]);
    }
  }
  return Number.NaN;
}

function thirdNoteDurationBeat(projectText) {
  for (const line of projectText.split("\n")) {
    const parts = line.split("\t");
    if (parts[0] === "note" && parts[1] === "3") {
      return Number(parts[3]);
    }
  }
  return Number.NaN;
}

function projectIncludesLoopBeats(state) {
  return state.project.includes(`\nloop_beats=${state.loopBeats}\n`);
}

function urlForSmokeVariant(label) {
  const parsed = new URL(url);
  parsed.searchParams.set("orbifold_smoke", label);
  return parsed.href;
}

function isQuantizedToSixteenth(beat) {
  return Math.abs(beat * 16 - Math.round(beat * 16)) < 0.0001;
}

async function waitForOrbifoldReady(send, sessionId) {
  const deadline = Date.now() + timeoutMs;
  let lastState = null;
  while (Date.now() <= deadline) {
    lastState = await evaluateOrbifoldState(send, sessionId);
    if (
      lastState.className.includes("runtime-ready") &&
      lastState.status === "" &&
      Number(lastState.frameCount) >= 2 &&
      lastState.keyboardShortcuts === "installed" &&
      lastState.canvasWidth >= 1200 &&
      lastState.canvasHeight >= 760 &&
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

async function waitForOrbifoldState(send, sessionId, predicate, failureMessage) {
  const deadline = Date.now() + timeoutMs;
  let lastState = null;
  while (Date.now() <= deadline) {
    lastState = await evaluateOrbifoldState(send, sessionId);
    if (lastState.className.includes("runtime-ready") && predicate(lastState)) {
      return lastState;
    }
    await delay(250);
  }
  throw new Error(`${failureMessage}; last state: ${JSON.stringify(lastState)}`);
}

async function evaluateOrbifoldState(send, sessionId) {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `({
        title: document.title,
        className: document.body?.className ?? "",
        status: document.getElementById("orbifold-status")?.textContent ?? "",
        frameCount: document.body?.dataset.orbifoldFrameCount ?? "",
        keyboardShortcuts: document.body?.dataset.orbifoldKeyboardShortcuts ?? "",
        viewportWidth: Number(document.body?.dataset.orbifoldViewportWidth ?? 0),
        viewportHeight: Number(document.body?.dataset.orbifoldViewportHeight ?? 0),
        canvasWidth: document.getElementById("orbifold-canvas")?.width ?? 0,
        canvasHeight: document.getElementById("orbifold-canvas")?.height ?? 0,
        canvasClientWidth: document.getElementById("orbifold-canvas")?.clientWidth ?? 0,
        canvasClientHeight: document.getElementById("orbifold-canvas")?.clientHeight ?? 0,
        devicePixelRatio: window.devicePixelRatio || 1,
        hasGpu: !!navigator.gpu
      })`,
      returnByValue: true,
    },
    sessionId
  );
  return result.result.value;
}

function smokeFailures(events) {
  const failures = [];
  for (const event of events) {
    if (event.method === "Runtime.exceptionThrown") {
      failures.push(`exception: ${event.params.exceptionDetails.text}`);
    } else if (
      event.method === "Runtime.consoleAPICalled" &&
      ["error", "assert"].includes(event.params.type)
    ) {
      failures.push(`console ${event.params.type}: ${consoleArgs(event.params.args)}`);
    } else if (event.method === "Network.loadingFailed") {
      failures.push(`network load failed: ${event.url ?? event.params.requestId}`);
    } else if (event.method === "Log.entryAdded" && event.params.entry.level === "error") {
      failures.push(`browser log error: ${event.params.entry.text}`);
    }
  }
  return failures;
}

function consoleArgs(args) {
  return args.map((arg) => arg.value ?? arg.description ?? arg.type).join(" ");
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function terminateChrome(child) {
  return new Promise((resolve) => {
    if (child.exitCode !== null || child.signalCode !== null) {
      resolve();
      return;
    }
    child.once("exit", resolve);
    child.kill("SIGTERM");
    setTimeout(() => {
      if (child.exitCode === null && child.signalCode === null) {
        child.kill("SIGKILL");
      }
    }, 2_000).unref();
  });
}

async function removeProfile(profile) {
  try {
    await fs.promises.rm(profile, { recursive: true, force: true });
  } catch {
    // Temporary browser profile cleanup is best-effort.
  }
}
