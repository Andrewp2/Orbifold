#!/usr/bin/env node

import assert from "node:assert/strict";
import {
  browserShortcutMappingCases,
  findBlankPianoGridPoint,
  isQuantizedToSixteenth,
  latestProjectNote,
  parseSmokeArgs,
  persistedNoteCount,
  projectIncludesLoopBeats,
  projectNoteById,
  projectNotes,
  smokeFailures,
  thirdNoteDurationBeat,
  thirdNoteStartBeat,
  urlForSmokeVariant,
} from "./check-web-smoke.mjs";

const project = [
  "orbifold_project=1",
  "loop_beats=16",
  "note\t1\t1.125\t0.25\t0\t60\t0\t60\t100",
  "note\t2\t2.5\t0.5\t0\t62\t0\t62\t88",
  "note\t3\t6\t2.75\t0\t64\t0\t64\t96",
  "",
].join("\n");

assert.deepEqual(parseSmokeArgs(["https://example.invalid/Orbifold"]), {
  url: "https://example.invalid/Orbifold",
  help: false,
});
assert.deepEqual(parseSmokeArgs(["--help"]), {
  url: "",
  help: true,
});
assert.throws(() => parseSmokeArgs(["https://example.invalid/Orbifold", "--bogus"]), {
  message: /Unknown argument: --bogus/,
});

assert.equal(persistedNoteCount(project), 3);
assert.deepEqual(projectNotes(project), [
  {
    id: 1,
    startBeat: 1.125,
    durationBeat: 0.25,
    keyIndex: 0,
    musicalNote: 60,
    rawChannel: 0,
    rawNote: 60,
    velocity: 100,
  },
  {
    id: 2,
    startBeat: 2.5,
    durationBeat: 0.5,
    keyIndex: 0,
    musicalNote: 62,
    rawChannel: 0,
    rawNote: 62,
    velocity: 88,
  },
  {
    id: 3,
    startBeat: 6,
    durationBeat: 2.75,
    keyIndex: 0,
    musicalNote: 64,
    rawChannel: 0,
    rawNote: 64,
    velocity: 96,
  },
]);
assert.equal(latestProjectNote(project).id, 3);
assert.equal(projectNoteById(project, 2).velocity, 88);
assert.equal(projectNoteById(project, 99), null);
assert.equal(thirdNoteStartBeat(project), 6);
assert.equal(thirdNoteDurationBeat(project), 2.75);
assert.equal(projectIncludesLoopBeats({ project, loopBeats: 16 }), true);
assert.equal(projectIncludesLoopBeats({ project, loopBeats: 8 }), false);
assert.equal(isQuantizedToSixteenth(1.125), true);
assert.equal(isQuantizedToSixteenth(1.13), false);

assert.deepEqual(
  findBlankPianoGridPoint(
    {
      gridX: 100,
      gridY: 200,
      gridWidth: 700,
      gridHeight: 310,
      viewStart: 0,
      viewBeats: 16,
      minPitch: 50,
      maxPitch: 80,
    },
    { project, loopBeats: 16 }
  ),
  { x: 737, y: 205 }
);
assert.equal(
  urlForSmokeVariant("reload", "https://example.invalid/Orbifold/?old=1"),
  "https://example.invalid/Orbifold/?old=1&orbifold_smoke=reload"
);

const shortcutCases = browserShortcutMappingCases();
assert.equal(shortcutCases.find((item) => item.label === "Ctrl+S").action, "file.save");
assert.equal(shortcutCases.find((item) => item.label === "Shift+ArrowRight").action, "clip.longer");
assert.equal(shortcutCases.find((item) => item.label === "Alt+R").action, "");
assert.equal(shortcutCases.find((item) => item.label === "Repeat ArrowRight").event.repeat, true);

assert.deepEqual(
  smokeFailures([
    {
      method: "Runtime.consoleAPICalled",
      params: { type: "error", args: [{ value: "renderer failed" }] },
    },
    {
      method: "Runtime.consoleAPICalled",
      params: { type: "warning", args: [{ value: "ignored" }] },
    },
    {
      method: "Runtime.exceptionThrown",
      params: { exceptionDetails: { text: "panic" } },
    },
    {
      method: "Network.loadingFailed",
      url: "https://example.invalid/pkg/orbifold_web_bg.wasm",
      params: { requestId: "1" },
    },
    {
      method: "Log.entryAdded",
      params: { entry: { level: "error", text: "webgpu failed" } },
    },
  ]),
  [
    "console error: renderer failed",
    "exception: panic",
    "network load failed: https://example.invalid/pkg/orbifold_web_bg.wasm",
    "browser log error: webgpu failed",
  ]
);

console.log("web smoke helper behavior ok");
