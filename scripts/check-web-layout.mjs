#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import { spawn } from "node:child_process";

const url = process.argv[2];
const timeoutMs = numberFromEnv("ORBIFOLD_WEB_LAYOUT_TIMEOUT_MS", 20_000);

const viewports = [
  { label: "compact-1200x760", width: 1200, height: 760, deviceScaleFactor: 1 },
  { label: "desktop-1600x1000", width: 1600, height: 1000, deviceScaleFactor: 1 },
  { label: "hidpi-1920x1080-dpr2", width: 1920, height: 1080, deviceScaleFactor: 2 },
  { label: "wide-3840x2160", width: 3840, height: 2160, deviceScaleFactor: 1 },
];

if (!url) {
  console.error("usage: scripts/check-web-layout.mjs <url>");
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

const profile = fs.mkdtempSync(`${os.tmpdir()}/orbifold-web-layout-`);
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
    "--window-size=3840,2160",
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
  const results = await checkLayouts(browserWsUrl);
  for (const result of results) {
    console.log(
      `- ${result.label}: canvas ${result.canvasClientWidth}x${result.canvasClientHeight}, ` +
        `grid ${Math.round(result.pianoGridWidth)}x${Math.round(result.pianoGridHeight)}, ` +
        `piano ${Math.round(result.pianoRollHeight)} high`
    );
  }
  console.log(`Orbifold web layout checks passed for ${url}`);
} catch (error) {
  console.error(`Orbifold web layout check failed: ${error.message ?? error}`);
  process.exitCode = 1;
} finally {
  await terminateChrome(chrome);
  await removePath(profile);
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
    const deadline = Date.now() + 10_000;
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

async function checkLayouts(browserWsUrl) {
  const ws = new WebSocket(browserWsUrl);
  let id = 0;
  const pending = new Map();
  const requestUrls = new Map();
  const events = [];

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
      events.push(summarizeEvent(message, requestUrls));
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

    const results = [];
    for (const viewport of viewports) {
      results.push(await checkViewport(send, sessionId, viewport));
    }
    const failures = browserFailures(events);
    if (failures.length > 0) {
      throw new Error(failures.join("; "));
    }
    return results;
  } finally {
    ws.close();
  }
}

async function checkViewport(send, sessionId, viewport) {
  await send(
    "Emulation.setDeviceMetricsOverride",
    {
      width: viewport.width,
      height: viewport.height,
      deviceScaleFactor: viewport.deviceScaleFactor,
      mobile: false,
    },
    sessionId
  );
  await send("Page.navigate", { url: urlForViewport(url, viewport.label) }, sessionId);
  const state = await waitForReadyState(send, sessionId, viewport);
  const failures = layoutFailures(state, viewport);
  if (failures.length > 0) {
    throw new Error(`${viewport.label}: ${failures.join("; ")}; state=${JSON.stringify(state)}`);
  }
  return { label: viewport.label, ...state };
}

function urlForViewport(targetUrl, label) {
  const parsed = new URL(targetUrl);
  parsed.searchParams.set("orbifold_layout", label);
  return parsed.href;
}

async function waitForReadyState(send, sessionId, viewport) {
  const deadline = Date.now() + timeoutMs;
  let state = {};
  while (Date.now() <= deadline) {
    state = await evaluateWebState(send, sessionId);
    if (String(state.className ?? "").includes("runtime-failed")) {
      throw new Error(`${viewport.label} runtime failed: ${state.status}`);
    }
    if (isReadyForLayoutCheck(state, viewport)) {
      return state;
    }
    await delay(250);
  }
  throw new Error(`${viewport.label} did not become layout-ready: ${JSON.stringify(state)}`);
}

function isReadyForLayoutCheck(state, viewport) {
  const className = String(state.className ?? "");
  const dpr = viewport.deviceScaleFactor;
  return (
    className.includes("runtime-ready") &&
    Number(state.frameCount) >= 2 &&
    Number(state.canvasClientWidth) >= viewport.width - 2 &&
    Number(state.canvasClientHeight) >= viewport.height - 2 &&
    Number(state.canvasWidth) >= viewport.width * dpr - 2 &&
    Number(state.canvasHeight) >= viewport.height * dpr - 2
  );
}

async function evaluateWebState(send, sessionId) {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `(() => {
        const canvas = document.getElementById("orbifold-canvas");
        const status = document.getElementById("orbifold-status");
        const body = document.body;
        const html = document.documentElement;
        const dataset = body?.dataset ?? {};
        const rect = canvas?.getBoundingClientRect();
        return {
          title: document.title,
          className: body?.className ?? "",
          status: status?.textContent ?? "",
          frameCount: Number(dataset.orbifoldFrameCount ?? 0),
          viewportWidth: Number(dataset.orbifoldViewportWidth ?? 0),
          viewportHeight: Number(dataset.orbifoldViewportHeight ?? 0),
          uiScale: Number(dataset.orbifoldUiScale ?? 0),
          devicePixelRatio: window.devicePixelRatio,
          innerWidth: window.innerWidth,
          innerHeight: window.innerHeight,
          documentClientWidth: html.clientWidth,
          documentClientHeight: html.clientHeight,
          documentScrollWidth: html.scrollWidth,
          documentScrollHeight: html.scrollHeight,
          bodyScrollWidth: body?.scrollWidth ?? 0,
          bodyScrollHeight: body?.scrollHeight ?? 0,
          canvasLeft: rect?.left ?? 0,
          canvasTop: rect?.top ?? 0,
          canvasRectWidth: rect?.width ?? 0,
          canvasRectHeight: rect?.height ?? 0,
          canvasClientWidth: canvas?.clientWidth ?? 0,
          canvasClientHeight: canvas?.clientHeight ?? 0,
          canvasWidth: canvas?.width ?? 0,
          canvasHeight: canvas?.height ?? 0,
          pianoGridWidth: Number(dataset.orbifoldPianoGridWidth ?? 0),
          pianoGridHeight: Number(dataset.orbifoldPianoGridHeight ?? 0),
          pianoRollHeight: Number(dataset.orbifoldPianoRollHeight ?? 0),
          rightPanelWidth: Number(dataset.orbifoldRightPanelWidth ?? 0),
          lastStatus: dataset.orbifoldLastStatus ?? ""
        };
      })()`,
      returnByValue: true,
    },
    sessionId
  );
  return result.result.value ?? {};
}

function layoutFailures(state, viewport) {
  const failures = [];
  const dpr = viewport.deviceScaleFactor;
  const expectedCanvasWidth = viewport.width * dpr;
  const expectedCanvasHeight = viewport.height * dpr;
  const tolerance = 2;

  requireAtLeast(failures, "viewportWidth", state.viewportWidth, viewport.width - tolerance);
  requireAtLeast(failures, "viewportHeight", state.viewportHeight, viewport.height - tolerance);
  requireAtLeast(failures, "canvasClientWidth", state.canvasClientWidth, viewport.width - tolerance);
  requireAtLeast(
    failures,
    "canvasClientHeight",
    state.canvasClientHeight,
    viewport.height - tolerance
  );
  requireAtLeast(failures, "canvasWidth", state.canvasWidth, expectedCanvasWidth - tolerance);
  requireAtLeast(failures, "canvasHeight", state.canvasHeight, expectedCanvasHeight - tolerance);
  requireAtLeast(failures, "pianoGridWidth", state.pianoGridWidth, 320);
  requireAtLeast(failures, "pianoGridHeight", state.pianoGridHeight, 120);
  requireAtLeast(failures, "pianoRollHeight", state.pianoRollHeight, 180);
  requireAtLeast(failures, "rightPanelWidth", state.rightPanelWidth, 180);

  requireNear(failures, "canvasLeft", state.canvasLeft, 0, tolerance);
  requireNear(failures, "canvasTop", state.canvasTop, 0, tolerance);
  requireNear(failures, "canvasRectWidth", state.canvasRectWidth, viewport.width, tolerance);
  requireNear(failures, "canvasRectHeight", state.canvasRectHeight, viewport.height, tolerance);

  requireAtMost(failures, "documentScrollWidth", state.documentScrollWidth, viewport.width + 1);
  requireAtMost(failures, "documentScrollHeight", state.documentScrollHeight, viewport.height + 1);
  requireAtMost(failures, "bodyScrollWidth", state.bodyScrollWidth, viewport.width + 1);
  requireAtMost(failures, "bodyScrollHeight", state.bodyScrollHeight, viewport.height + 1);

  return failures;
}

function requireAtLeast(failures, label, value, minimum) {
  if (!Number.isFinite(value) || value < minimum) {
    failures.push(`${label} ${value} < ${minimum}`);
  }
}

function requireAtMost(failures, label, value, maximum) {
  if (!Number.isFinite(value) || value > maximum) {
    failures.push(`${label} ${value} > ${maximum}`);
  }
}

function requireNear(failures, label, value, expected, tolerance) {
  if (!Number.isFinite(value) || Math.abs(value - expected) > tolerance) {
    failures.push(`${label} ${value} not within ${tolerance} of ${expected}`);
  }
}

function summarizeEvent(message, requestUrls) {
  const params = message.params ?? {};
  if (message.method === "Runtime.consoleAPICalled") {
    return {
      method: message.method,
      type: params.type,
      text: (params.args ?? []).map((arg) => arg.value ?? arg.description ?? "").join(" "),
    };
  }
  if (message.method === "Runtime.exceptionThrown") {
    return {
      method: message.method,
      text: params.exceptionDetails?.text ?? "",
      description: params.exceptionDetails?.exception?.description ?? "",
    };
  }
  if (message.method === "Network.loadingFailed") {
    return {
      method: message.method,
      url: requestUrls.get(params.requestId) ?? "",
      errorText: params.errorText ?? "",
    };
  }
  return {
    method: message.method,
    level: params.entry?.level ?? "",
    text: params.entry?.text ?? "",
  };
}

function browserFailures(events) {
  const failures = [];
  for (const event of events) {
    if (event.method === "Runtime.exceptionThrown") {
      failures.push(`exception: ${event.text}`);
    } else if (
      event.method === "Runtime.consoleAPICalled" &&
      ["error", "assert"].includes(event.type)
    ) {
      failures.push(`console ${event.type}: ${event.text}`);
    } else if (event.method === "Network.loadingFailed") {
      failures.push(`network load failed: ${event.url || event.errorText}`);
    } else if (event.method === "Log.entryAdded" && event.level === "error") {
      failures.push(`browser log error: ${event.text}`);
    }
  }
  return failures;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function terminateChrome(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  child.kill("SIGTERM");
  await new Promise((resolve) => {
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      resolve();
    }, 2_000);
    child.once("exit", () => {
      clearTimeout(timer);
      resolve();
    });
  });
}

async function removePath(targetPath) {
  await fs.promises.rm(targetPath, { recursive: true, force: true });
}
