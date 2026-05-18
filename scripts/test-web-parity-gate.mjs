#!/usr/bin/env node

import assert from "node:assert/strict";
import {
  createParityGateReport,
  normalizeParityGateUrl,
  parseParityGateArgs,
} from "./check-web-parity-gate.mjs";
import { compareWebArtifactFingerprints } from "./web-artifact-fingerprint.mjs";

assert.deepEqual(parseParityGateArgs(["https://example.invalid/Orbifold"]), {
  url: "https://example.invalid/Orbifold",
  report: "reports",
  visualOut: "reports/web-visuals",
  skipVisualCapture: false,
});

assert.deepEqual(
  parseParityGateArgs([
    "https://example.invalid/Orbifold/",
    "--report",
    "reports/manual",
    "--visual-out=screenshots/final",
    "--skip-visual-capture",
  ]),
  {
    url: "https://example.invalid/Orbifold/",
    report: "reports/manual",
    visualOut: "screenshots/final",
    skipVisualCapture: true,
  }
);

assert.throws(() => parseParityGateArgs(["https://example.invalid/Orbifold/", "--bogus"]), {
  message: /Unknown argument: --bogus/,
});
assert.throws(() => parseParityGateArgs(["https://example.invalid/Orbifold/", "--report"]), {
  message: /--report requires a value/,
});
assert.throws(() => parseParityGateArgs(["https://example.invalid/Orbifold/", "--report="]), {
  message: /--report requires a value/,
});
assert.throws(() => parseParityGateArgs(["https://example.invalid/Orbifold/", "--visual-out"]), {
  message: /--visual-out requires a value/,
});
assert.throws(() => parseParityGateArgs(["https://example.invalid/Orbifold/", "--visual-out="]), {
  message: /--visual-out requires a value/,
});

assert.equal(
  normalizeParityGateUrl("https://example.invalid/Orbifold?cache=old#section"),
  "https://example.invalid/Orbifold/"
);

assert.deepEqual(
  createParityGateReport(
    parseParityGateArgs([
      "https://example.invalid/Orbifold/",
      "--report=reports/manual",
      "--visual-out",
      "screenshots/final",
      "--skip-visual-capture",
    ]),
    "2026-05-18T12:00:00.000Z"
  ),
  {
    schema: "orbifold.web_parity_gate.v1",
    generatedAt: "2026-05-18T12:00:00.000Z",
    targetUrl: "https://example.invalid/Orbifold/",
    manualReport: "reports/manual",
    visualOut: "screenshots/final",
    skippedVisualCapture: true,
    steps: [],
  }
);

const artifact = artifactFingerprint("https://example.invalid/Orbifold/");
assert.deepEqual(
  compareWebArtifactFingerprints(
    artifactFingerprint("https://example.invalid/Orbifold?fresh=1#hash"),
    artifact
  ),
  []
);

assert.deepEqual(
  compareWebArtifactFingerprints(
    withArtifactChange(artifact, (draft) => {
      draft.rootUrl = "https://example.invalid/Other/";
    }),
    artifact
  ),
  ["rootUrl expected https://example.invalid/Orbifold/, got https://example.invalid/Other/"]
);

assert.deepEqual(
  compareWebArtifactFingerprints(
    withArtifactChange(artifact, (draft) => {
      draft.files.wasm.sha256 = "f".repeat(64);
      draft.files.wasm.bytes += 1;
    }),
    artifact
  ),
  [
    `wasm sha256 expected ${artifact.files.wasm.sha256}, got ${"f".repeat(64)}`,
    `wasm bytes expected ${artifact.files.wasm.bytes}, got ${artifact.files.wasm.bytes + 1}`,
  ]
);

console.log("web parity gate behavior ok");

function withArtifactChange(source, change) {
  const draft = structuredClone(source);
  change(draft);
  return draft;
}

function artifactFingerprint(rootUrl) {
  return {
    schema: "orbifold.web_artifact_fingerprint.v1",
    rootUrl,
    generatedAt: "2026-05-18T12:00:00.000Z",
    files: Object.fromEntries(
      ["index", "js", "wasm", "favicon", "icon"].map((name, index) => [
        name,
        {
          url: `${rootUrl}${name}`,
          bytes: index + 1,
          sha256: "abcdef0123456789".repeat(4),
        },
      ])
    ),
  };
}
