#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import zlib from "node:zlib";
import { spawn } from "node:child_process";

const args = process.argv.slice(2);
const parsedArgs = parseArgs(args);
const target = parsedArgs.target;
const outDir = parsedArgs.outDir;
const timeoutMs = numberFromEnv("ORBIFOLD_WEB_VISUAL_TIMEOUT_MS", 20_000);
const devtoolsTimeoutMs = numberFromEnv("ORBIFOLD_CHROME_DEVTOOLS_TIMEOUT_MS", 20_000);

const viewports = [
  { label: "compact-1200x760", width: 1200, height: 760, deviceScaleFactor: 1 },
  { label: "desktop-1600x1000", width: 1600, height: 1000, deviceScaleFactor: 1 },
  { label: "hidpi-1920x1080-dpr2", width: 1920, height: 1080, deviceScaleFactor: 2 },
  { label: "wide-3840x2160", width: 3840, height: 2160, deviceScaleFactor: 1 },
];

if (!target) {
  console.error("usage: scripts/capture-web-visuals.mjs <url> [--out screenshots/web]");
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

const runDir = path.join(outDir, timestampForPath());
fs.mkdirSync(runDir, { recursive: true });

const profile = fs.mkdtempSync(path.join(os.tmpdir(), "orbifold-web-visuals-"));
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
  const result = await captureVisuals(browserWsUrl);
  const manifestPath = path.join(runDir, "manifest.json");
  fs.writeFileSync(manifestPath, `${JSON.stringify(result, null, 2)}\n`);
  console.log(`Orbifold web visual captures wrote ${runDir}`);
  for (const capture of result.captures) {
    console.log(`- ${capture.label}: ${capture.screenshot ?? capture.snapshot}`);
  }
  console.log(`- manifest: ${manifestPath}`);
} catch (error) {
  console.error(`Orbifold web visual capture failed: ${error.message ?? error}`);
  process.exitCode = 1;
} finally {
  await terminateChrome(chrome);
  await removePath(profile);
}

function parseArgs(rawArgs) {
  let parsedTarget = "";
  let parsedOutDir = "screenshots/web";
  for (let index = 0; index < rawArgs.length; index += 1) {
    const arg = rawArgs[index];
    if (arg === "--out") {
      parsedOutDir = rawArgs[index + 1] ?? parsedOutDir;
      index += 1;
    } else if (arg.startsWith("--out=")) {
      parsedOutDir = arg.slice("--out=".length);
    } else if (!arg.startsWith("--") && !parsedTarget) {
      parsedTarget = arg;
    }
  }
  return {
    target: parsedTarget,
    outDir: parsedOutDir,
  };
}

function numberFromEnv(name, fallback) {
  const raw = process.env[name];
  if (!raw) return fallback;
  const parsed = Number(raw);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function timestampForPath() {
  return new Date().toISOString().replace(/\.\d+Z$/, "Z").replace(/[:]/g, "");
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

async function captureVisuals(browserWsUrl) {
  const ws = new WebSocket(browserWsUrl);
  let id = 0;
  const pending = new Map();
  const events = [];
  const requestUrls = new Map();

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
    const captures = [];
    for (const viewport of viewports) {
      captures.push(await captureViewportInFreshTarget(send, viewport));
    }

    return {
      target,
      capturedAt: new Date().toISOString(),
      chrome: chromePath,
      captures,
      events,
    };
  } finally {
    ws.close();
  }
}

async function captureViewportInFreshTarget(send, viewport) {
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
    return await captureViewport(send, sessionId, viewport);
  } finally {
    await send("Target.closeTarget", { targetId }).catch(() => {});
  }
}

async function captureViewport(send, sessionId, viewport) {
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
  await send("Page.navigate", { url: urlForViewport(target, viewport.label) }, sessionId);

  const state = await waitForReadyState(send, sessionId, viewport);
  const screenshot = await send(
    "Page.captureScreenshot",
    {
      format: "png",
      fromSurface: true,
      captureBeyondViewport: false,
    },
    sessionId
  );
  const screenshotBytes = Buffer.from(screenshot.data, "base64");
  const imageStats = pngStats(screenshotBytes);
  const screenshotUsable =
    imageStats && imageStats.nonTransparentPixels > 0 && imageStats.rgbRange > 0;
  const screenshotFallback = screenshotFallbackReason(imageStats);
  if (screenshotUsable) {
    const screenshotPath = path.join(runDir, `${viewport.label}.png`);
    fs.writeFileSync(screenshotPath, screenshotBytes);

    return {
      ...viewport,
      mode: "page-screenshot",
      screenshot: screenshotPath,
      imageStats,
      state,
    };
  }

  const snapshot = await readVisualSnapshot(send, sessionId, viewport);
  const snapshotPath = path.join(runDir, `${viewport.label}.svg`);
  fs.writeFileSync(snapshotPath, snapshot.svg);

  return {
    ...viewport,
    mode: "paint-snapshot-svg",
    snapshot: snapshotPath,
    screenshotFallback,
    imageStats,
    snapshotStats: snapshot.stats,
    state,
  };
}

function screenshotFallbackReason(imageStats) {
  if (!imageStats) {
    return "screenshot statistics unavailable";
  }
  if (imageStats.nonTransparentPixels === 0) {
    return "screenshot is blank/transparent";
  }
  if (imageStats.rgbRange === 0) {
    return "screenshot has no color variation";
  }
  return "screenshot could not be used as visual evidence";
}

function urlForViewport(url, label) {
  const parsed = new URL(url);
  parsed.searchParams.set("orbifold_visual", label);
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
    if (isReadyForCapture(state, viewport)) {
      return state;
    }
    await delay(250);
  }
  throw new Error(`${viewport.label} did not become capture-ready: ${JSON.stringify(state)}`);
}

function isReadyForCapture(state, viewport) {
  const className = String(state.className ?? "");
  const dpr = viewport.deviceScaleFactor;
  const minClientWidth = viewport.width - 2;
  const minClientHeight = viewport.height - 2;
  const minCanvasWidth = viewport.width * dpr - 2;
  const minCanvasHeight = viewport.height * dpr - 2;
  return (
    className.includes("runtime-ready") &&
    Number(state.frameCount) >= 2 &&
    Number(state.canvasClientWidth) >= minClientWidth &&
    Number(state.canvasClientHeight) >= minClientHeight &&
    Number(state.canvasWidth) >= minCanvasWidth &&
    Number(state.canvasHeight) >= minCanvasHeight &&
    Number(state.viewportWidth) >= minClientWidth &&
    Number(state.viewportHeight) >= minClientHeight &&
    state.visualSnapshotReady === "1" &&
    Number(state.visualSnapshotBytes) > 1000
  );
}

async function readVisualSnapshot(send, sessionId, viewport) {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `(() => ({
        svg: String(window.__orbifoldVisualSnapshotSvg || ""),
        stats: {
          bytes: Number(document.body.dataset.orbifoldVisualSnapshotBytes || 0),
          itemCount: Number(document.body.dataset.orbifoldVisualSnapshotItemCount || 0),
          unsupportedCount: Number(document.body.dataset.orbifoldVisualSnapshotUnsupportedCount || 0)
        }
      }))()`,
      returnByValue: true,
    },
    sessionId
  );
  const snapshot = result.result.value ?? {};
  const svg = String(snapshot.svg ?? "");
  const stats = snapshot.stats ?? {};
  if (!svg.includes("<svg") || svg.length < 1000 || Number(stats.itemCount ?? 0) < 10) {
    throw new Error(
      `${viewport.label} screenshot was unusable and no usable SVG paint snapshot was available`
    );
  }
  return { svg, stats };
}

async function evaluateWebState(send, sessionId) {
  const result = await send(
    "Runtime.evaluate",
    {
      expression: `(() => {
        const canvas = document.getElementById("orbifold-canvas");
        const status = document.getElementById("orbifold-status");
        const body = document.body;
        const dataset = body?.dataset ?? {};
        return {
          title: document.title,
          className: body?.className ?? "",
          status: status?.textContent ?? "",
          frameCount: Number(dataset.orbifoldFrameCount ?? 0),
          viewportWidth: Number(dataset.orbifoldViewportWidth ?? 0),
          viewportHeight: Number(dataset.orbifoldViewportHeight ?? 0),
          uiScale: Number(dataset.orbifoldUiScale ?? 0),
          canvasClientWidth: canvas?.clientWidth ?? 0,
          canvasClientHeight: canvas?.clientHeight ?? 0,
          canvasWidth: canvas?.width ?? 0,
          canvasHeight: canvas?.height ?? 0,
          devicePixelRatio: window.devicePixelRatio,
          documentScrollWidth: document.documentElement.scrollWidth,
          documentScrollHeight: document.documentElement.scrollHeight,
          bodyScrollWidth: body?.scrollWidth ?? 0,
          bodyScrollHeight: body?.scrollHeight ?? 0,
          pianoGridWidth: Number(dataset.orbifoldPianoGridWidth ?? 0),
          pianoGridHeight: Number(dataset.orbifoldPianoGridHeight ?? 0),
          pianoRollHeight: Number(dataset.orbifoldPianoRollHeight ?? 0),
          rightPanelWidth: Number(dataset.orbifoldRightPanelWidth ?? 0),
          visualSnapshotReady: dataset.orbifoldVisualSnapshotReady ?? "",
          visualSnapshotBytes: Number(dataset.orbifoldVisualSnapshotBytes ?? 0),
          visualSnapshotItemCount: Number(dataset.orbifoldVisualSnapshotItemCount ?? 0),
          visualSnapshotUnsupportedCount: Number(dataset.orbifoldVisualSnapshotUnsupportedCount ?? 0),
          lastStatus: dataset.orbifoldLastStatus ?? ""
        };
      })()`,
      returnByValue: true,
    },
    sessionId
  );
  return result.result.value ?? {};
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

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function pngStats(bytes) {
  const signature = "89504e470d0a1a0a";
  if (bytes.subarray(0, 8).toString("hex") !== signature) {
    return null;
  }

  let offset = 8;
  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = 0;
  const idatChunks = [];

  while (offset + 12 <= bytes.length) {
    const length = bytes.readUInt32BE(offset);
    const type = bytes.subarray(offset + 4, offset + 8).toString("ascii");
    const dataStart = offset + 8;
    const dataEnd = dataStart + length;
    if (dataEnd + 4 > bytes.length) {
      return null;
    }

    if (type === "IHDR") {
      width = bytes.readUInt32BE(dataStart);
      height = bytes.readUInt32BE(dataStart + 4);
      bitDepth = bytes[dataStart + 8];
      colorType = bytes[dataStart + 9];
    } else if (type === "IDAT") {
      idatChunks.push(bytes.subarray(dataStart, dataEnd));
    } else if (type === "IEND") {
      break;
    }

    offset = dataEnd + 4;
  }

  const bytesPerPixel = colorType === 6 ? 4 : colorType === 2 ? 3 : 0;
  if (!width || !height || bitDepth !== 8 || !bytesPerPixel || idatChunks.length === 0) {
    return null;
  }

  const inflated = zlib.inflateSync(Buffer.concat(idatChunks));
  const rowBytes = width * bytesPerPixel;
  const expectedBytes = (rowBytes + 1) * height;
  if (inflated.length < expectedBytes) {
    return null;
  }

  let sourceOffset = 0;
  let previous = Buffer.alloc(rowBytes);
  let redMin = 255;
  let redMax = 0;
  let greenMin = 255;
  let greenMax = 0;
  let blueMin = 255;
  let blueMax = 0;
  let alphaMin = colorType === 6 ? 255 : 255;
  let alphaMax = colorType === 6 ? 0 : 255;
  let nonTransparentPixels = 0;

  for (let y = 0; y < height; y += 1) {
    const filter = inflated[sourceOffset];
    sourceOffset += 1;
    const row = Buffer.from(inflated.subarray(sourceOffset, sourceOffset + rowBytes));
    sourceOffset += rowBytes;
    unfilterRow(row, previous, bytesPerPixel, filter);

    for (let index = 0; index < row.length; index += bytesPerPixel) {
      const red = row[index];
      const green = row[index + 1];
      const blue = row[index + 2];
      const alpha = colorType === 6 ? row[index + 3] : 255;
      redMin = Math.min(redMin, red);
      redMax = Math.max(redMax, red);
      greenMin = Math.min(greenMin, green);
      greenMax = Math.max(greenMax, green);
      blueMin = Math.min(blueMin, blue);
      blueMax = Math.max(blueMax, blue);
      alphaMin = Math.min(alphaMin, alpha);
      alphaMax = Math.max(alphaMax, alpha);
      if (alpha > 0) {
        nonTransparentPixels += 1;
      }
    }

    previous = row;
  }

  return {
    width,
    height,
    colorType,
    bitDepth,
    nonTransparentPixels,
    alphaRange: alphaMax - alphaMin,
    rgbRange: Math.max(redMax - redMin, greenMax - greenMin, blueMax - blueMin),
  };
}

function unfilterRow(row, previous, bytesPerPixel, filter) {
  for (let index = 0; index < row.length; index += 1) {
    const left = index >= bytesPerPixel ? row[index - bytesPerPixel] : 0;
    const up = previous[index] ?? 0;
    const upperLeft = index >= bytesPerPixel ? previous[index - bytesPerPixel] ?? 0 : 0;
    if (filter === 1) {
      row[index] = (row[index] + left) & 0xff;
    } else if (filter === 2) {
      row[index] = (row[index] + up) & 0xff;
    } else if (filter === 3) {
      row[index] = (row[index] + Math.floor((left + up) / 2)) & 0xff;
    } else if (filter === 4) {
      row[index] = (row[index] + paethPredictor(left, up, upperLeft)) & 0xff;
    } else if (filter !== 0) {
      throw new Error(`Unsupported PNG filter ${filter}`);
    }
  }
}

function paethPredictor(left, up, upperLeft) {
  const estimate = left + up - upperLeft;
  const leftDistance = Math.abs(estimate - left);
  const upDistance = Math.abs(estimate - up);
  const upperLeftDistance = Math.abs(estimate - upperLeft);
  if (leftDistance <= upDistance && leftDistance <= upperLeftDistance) {
    return left;
  }
  if (upDistance <= upperLeftDistance) {
    return up;
  }
  return upperLeft;
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
  try {
    await fs.promises.rm(targetPath, { recursive: true, force: true });
  } catch {
    // Temporary browser profile cleanup is best-effort.
  }
}
