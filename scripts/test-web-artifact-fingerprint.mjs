#!/usr/bin/env node

import assert from "node:assert/strict";
import {
  compareWebArtifactFingerprints,
  normalizeWebRootHref,
  requireArtifactFingerprint,
} from "./web-artifact-fingerprint.mjs";

const artifact = artifactFingerprint("https://example.invalid/Orbifold/");

assert.doesNotThrow(() => requireArtifactFingerprint(artifact));

assert.equal(
  normalizeWebRootHref("https://example.invalid/Orbifold?cache=old#fragment"),
  "https://example.invalid/Orbifold/"
);

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
      draft.files.index.bytes += 1;
      draft.files.index.sha256 = "f".repeat(64);
    }),
    artifact
  ),
  [
    `index sha256 expected ${artifact.files.index.sha256}, got ${"f".repeat(64)}`,
    `index bytes expected ${artifact.files.index.bytes}, got ${artifact.files.index.bytes + 1}`,
  ]
);

assertRejects(
  withArtifactChange(artifact, (draft) => {
    draft.schema = "wrong";
  }),
  "artifact.schema should be orbifold.web_artifact_fingerprint.v1"
);

assertRejects(
  withArtifactChange(artifact, (draft) => {
    delete draft.files.wasm;
  }),
  "artifact.files.wasm should be an object"
);

assertRejects(
  withArtifactChange(artifact, (draft) => {
    draft.files.icon.sha256 = "not-a-sha";
  }),
  "artifact.files.icon.sha256 should be a sha256 hex digest"
);

assertRejects(
  withArtifactChange(artifact, (draft) => {
    draft.files.favicon.bytes = 0;
  }),
  "artifact.files.favicon.bytes should be positive"
);

console.log("web artifact fingerprint behavior ok");

function assertRejects(candidate, message) {
  assert.throws(() => requireArtifactFingerprint(candidate), {
    message: new RegExp(escapeRegExp(message)),
  });
}

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
          sha256: "0123456789abcdef".repeat(4),
        },
      ])
    ),
  };
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
