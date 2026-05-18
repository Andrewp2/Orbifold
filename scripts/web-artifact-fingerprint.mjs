import { createHash } from "node:crypto";

export async function fetchWebArtifactFingerprint(target) {
  const rootUrl = normalizeWebRootUrl(target);
  const index = await fetchRequired("./", rootUrl, { text: true, contentType: "text/html" });
  const js = await fetchRequired("./pkg/orbifold_web.js", rootUrl, { text: true });
  const wasm = await fetchRequired("./pkg/orbifold_web_bg.wasm", rootUrl);
  const favicon = await fetchRequired("./favicon.ico", rootUrl);
  const icon = await fetchRequired("./orbifold_icon.png", rootUrl);

  return {
    schema: "orbifold.web_artifact_fingerprint.v1",
    rootUrl: rootUrl.href,
    generatedAt: new Date().toISOString(),
    files: {
      index: fileFingerprint(index),
      js: fileFingerprint(js),
      wasm: fileFingerprint(wasm),
      favicon: fileFingerprint(favicon),
      icon: fileFingerprint(icon),
    },
  };
}

export function compareWebArtifactFingerprints(actual, expected) {
  requireArtifactFingerprint(actual, "actual");
  requireArtifactFingerprint(expected, "expected");

  const differences = [];
  if (normalizeWebRootHref(actual.rootUrl) !== normalizeWebRootHref(expected.rootUrl)) {
    differences.push(`rootUrl expected ${expected.rootUrl}, got ${actual.rootUrl}`);
  }
  for (const fileName of ["index", "js", "wasm", "favicon", "icon"]) {
    const actualFile = actual.files[fileName];
    const expectedFile = expected.files[fileName];
    if (actualFile.sha256 !== expectedFile.sha256) {
      differences.push(
        `${fileName} sha256 expected ${expectedFile.sha256}, got ${actualFile.sha256}`
      );
    }
    if (actualFile.bytes !== expectedFile.bytes) {
      differences.push(`${fileName} bytes expected ${expectedFile.bytes}, got ${actualFile.bytes}`);
    }
  }
  return differences;
}

export function requireArtifactFingerprint(fingerprint, label = "artifact") {
  if (!fingerprint || typeof fingerprint !== "object" || Array.isArray(fingerprint)) {
    throw new Error(`${label} should be an object`);
  }
  if (fingerprint.schema !== "orbifold.web_artifact_fingerprint.v1") {
    throw new Error(`${label}.schema should be orbifold.web_artifact_fingerprint.v1`);
  }
  if (!fingerprint.rootUrl) {
    throw new Error(`${label}.rootUrl should be present`);
  }
  if (!fingerprint.generatedAt) {
    throw new Error(`${label}.generatedAt should be present`);
  }
  if (!fingerprint.files || typeof fingerprint.files !== "object") {
    throw new Error(`${label}.files should be an object`);
  }

  for (const fileName of ["index", "js", "wasm", "favicon", "icon"]) {
    const file = fingerprint.files[fileName];
    if (!file || typeof file !== "object" || Array.isArray(file)) {
      throw new Error(`${label}.files.${fileName} should be an object`);
    }
    if (!file.url) {
      throw new Error(`${label}.files.${fileName}.url should be present`);
    }
    if (!/^[0-9a-f]{64}$/.test(String(file.sha256))) {
      throw new Error(`${label}.files.${fileName}.sha256 should be a sha256 hex digest`);
    }
    if (!(Number(file.bytes) > 0)) {
      throw new Error(`${label}.files.${fileName}.bytes should be positive`);
    }
  }
}

export function normalizeWebRootHref(value) {
  return normalizeWebRootUrl(value).href;
}

function normalizeWebRootUrl(value) {
  const url = new URL(value);
  url.hash = "";
  url.search = "";
  if (!url.pathname.endsWith("/")) {
    url.pathname = `${url.pathname}/`;
  }
  return url;
}

async function fetchRequired(relativePath, rootUrl, options = {}) {
  const url = new URL(relativePath, rootUrl);
  const response = await fetch(url, {
    cache: "no-store",
    redirect: "follow",
  });
  if (!response.ok) {
    throw new Error(`${url.href} returned HTTP ${response.status}`);
  }

  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length === 0) {
    throw new Error(`${url.href} returned an empty response`);
  }

  const contentType = response.headers.get("content-type") ?? "";
  if (options.contentType && !contentType.includes(options.contentType)) {
    throw new Error(
      `${url.href} returned content-type ${contentType}, expected ${options.contentType}`
    );
  }

  return {
    bytes,
    contentType,
    text: options.text ? new TextDecoder().decode(bytes) : "",
    url,
  };
}

function fileFingerprint(file) {
  return {
    url: file.url.href,
    bytes: file.bytes.length,
    sha256: createHash("sha256").update(file.bytes).digest("hex"),
    contentType: file.contentType,
  };
}
