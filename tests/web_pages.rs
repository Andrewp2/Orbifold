#[test]
fn web_shell_loads_generated_wasm_and_icons() {
    let html = include_str!("../web/index.html");

    for required in [
        "orbifold-canvas",
        "orbifold-status",
        "<title>Orbifold</title>",
        "./pkg/orbifold_web.js",
        "start_orbifold",
        "favicon.ico",
        "orbifold_icon.png",
        "const startupTimeoutMs = 12000",
        "window.setTimeout",
        "runtime-failed",
        "runtime-ready",
        "orbifoldRuntimeReady",
        "orbifoldFrameCount",
        "waiting for first frame",
        "runtime is still starting or unavailable",
    ] {
        assert!(
            html.contains(required),
            "web/index.html should mention {required}"
        );
    }
}

#[test]
fn web_build_script_targets_the_wasm_example() {
    let script = include_str!("../scripts/build-web.sh");

    for required in [
        "wasm32-unknown-unknown",
        "orbifold_web",
        "--no-default-features",
        "--features web-app",
        "wasm-bindgen",
        "web/index.html",
        "favicon.ico",
        "orbifold_icon.png",
        ".nojekyll",
    ] {
        assert!(
            script.contains(required),
            "scripts/build-web.sh should mention {required}"
        );
    }
}

#[test]
fn pages_workflow_builds_and_deploys_dist() {
    let workflow = include_str!("../.github/workflows/pages.yml");

    for required in [
        "wasm32-unknown-unknown",
        "FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true",
        "cargo install wasm-bindgen-cli --version 0.2.121 --locked",
        "./scripts/build-web.sh dist",
        "actions/setup-node@v4",
        "node-version: 22",
        "python3 -m http.server 4173 --directory dist",
        "./scripts/check-web-dist.mjs dist",
        "if ./scripts/check-web-layout.mjs http://127.0.0.1:4173/; then",
        "./scripts/check-web-layout.mjs http://127.0.0.1:4173/",
        "if ./scripts/check-web-smoke.mjs http://127.0.0.1:4173/; then",
        "./scripts/check-web-smoke.mjs http://127.0.0.1:4173/",
        "if ./scripts/capture-web-visuals.mjs http://127.0.0.1:4173/ --out screenshots/web-local; then",
        "./scripts/capture-web-visuals.mjs http://127.0.0.1:4173/ --out screenshots/web-local",
        "actions/upload-artifact@v4",
        "orbifold-web-visuals-local",
        "for _ in {1..3}; do",
        "actions/configure-pages@v5",
        "actions/upload-pages-artifact@v4",
        "actions/deploy-pages@v4",
        "./scripts/check-web-live.mjs \"${{ steps.deployment.outputs.page_url }}\"",
        "./scripts/check-web-layout.mjs \"${{ steps.deployment.outputs.page_url }}\"",
        "./scripts/check-web-smoke.mjs \"${{ steps.deployment.outputs.page_url }}\"",
        "./scripts/capture-web-visuals.mjs \"${{ steps.deployment.outputs.page_url }}\" --out screenshots/web-deployed",
        "orbifold-web-visuals-deployed",
        "path: dist",
    ] {
        assert!(
            workflow.contains(required),
            ".github/workflows/pages.yml should mention {required}"
        );
    }
}

#[test]
fn ci_workflow_opts_github_javascript_actions_into_node24() {
    let workflow = include_str!("../.github/workflows/ci.yml");

    for required in [
        "FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true",
        "actions/checkout@v4",
        "actions/setup-node@v4",
        "node-version: 22",
        "cargo fmt --check",
        "cargo test",
        "node scripts/test-web-artifact-checks.mjs",
        "node scripts/test-web-artifact-fingerprint.mjs",
        "node scripts/test-web-manual-devices.mjs",
        "node scripts/test-web-manual-report-validator.mjs",
        "node scripts/test-web-parity-gate.mjs",
        "node scripts/test-web-visual-layout-helpers.mjs",
        "cargo clippy --all-targets -- -D warnings",
    ] {
        assert!(
            workflow.contains(required),
            ".github/workflows/ci.yml should mention {required}"
        );
    }
}

#[test]
fn web_layout_check_script_verifies_multi_viewport_canvas_geometry() {
    let script = include_str!("../scripts/check-web-layout.mjs");
    let behavior_test = include_str!("../scripts/test-web-visual-layout-helpers.mjs");
    let readme = include_str!("../README.md");
    let audit = include_str!("../docs/web_parity_audit.md");
    let checklist = include_str!("../docs/manual_qa_checklist.md");

    for required in [
        "usage: scripts/check-web-layout.mjs <url>",
        "ORBIFOLD_CHROME_DEVTOOLS_TIMEOUT_MS",
        "--enable-unsafe-webgpu",
        "--ignore-gpu-blocklist",
        "--disable-dev-shm-usage",
        "isCliEntrypoint",
        "export function urlForViewport",
        "export function isReadyForLayoutCheck",
        "export function layoutFailures",
        "export function browserFailures",
        "compact-1200x760",
        "desktop-1600x1000",
        "hidpi-1920x1080-dpr2",
        "wide-3840x2160",
        "orbifold_layout",
        "checkViewportInFreshTarget",
        "Target.closeTarget",
        "runtime-ready",
        "runtime-failed",
        "canvasClientWidth",
        "canvasClientHeight",
        "canvasWidth",
        "canvasHeight",
        "canvasRectWidth",
        "canvasRectHeight",
        "documentScrollWidth",
        "documentScrollHeight",
        "bodyScrollWidth",
        "bodyScrollHeight",
        "orbifoldPianoGridWidth",
        "orbifoldPianoGridHeight",
        "orbifoldPianoRollHeight",
        "orbifoldRightPanelWidth",
        "orbifoldTextAuditReady",
        "orbifoldTextAuditCount",
        "orbifoldTextAuditIssueCount",
        "orbifoldTextAuditNonFiniteCount",
        "orbifoldTextAuditSampleIssue",
        "requireExact",
    ] {
        assert!(
            script.contains(required),
            "scripts/check-web-layout.mjs should verify browser layout geometry: {required}"
        );
    }

    for required in [
        "layoutFailures(goodLayout, viewport)",
        "isReadyForLayoutCheck(goodLayout, viewport)",
        "canvasClientWidth 400 < 1598",
        "textAuditSampleIssue top bar overlap",
        "browserFailures([",
        "web visual layout helper behavior ok",
    ] {
        assert!(
            behavior_test.contains(required),
            "scripts/test-web-visual-layout-helpers.mjs should behavior-test {required}"
        );
    }

    for docs in [readme, audit, checklist] {
        assert!(
            docs.contains("./scripts/check-web-layout.mjs"),
            "web layout check workflow should be documented"
        );
    }
}

#[test]
fn web_dist_check_script_verifies_pages_artifact_shape() {
    let script = include_str!("../scripts/check-web-dist.mjs");
    let behavior_test = include_str!("../scripts/test-web-artifact-checks.mjs");

    for required in [
        "export async function checkWebDist",
        "export function requireWebIndexHtml",
        "pkg/orbifold_web.js",
        "pkg/orbifold_web_bg.wasm",
        "favicon.ico",
        "orbifold_icon.png",
        ".nojekyll",
        "window.orbifoldRuntimeReady",
        "runtime-ready",
        "runtime-failed",
        "href=\"/",
        "src=\"/",
        "from \"/",
    ] {
        assert!(
            script.contains(required),
            "scripts/check-web-dist.mjs should verify {required}"
        );
    }

    for required in [
        "await checkWebDist(distDir)",
        "requireWebIndexHtml(webIndexHtml())",
        "missing pkg\\/orbifold_web_bg\\.wasm",
        "index\\.html should not contain href=\"\\/",
        "web artifact checks behavior ok",
    ] {
        assert!(
            behavior_test.contains(required),
            "scripts/test-web-artifact-checks.mjs should behavior-test {required}"
        );
    }
}

#[test]
fn web_visual_capture_script_records_browser_layout_evidence() {
    let script = include_str!("../scripts/capture-web-visuals.mjs");
    let behavior_test = include_str!("../scripts/test-web-visual-layout-helpers.mjs");
    let readme = include_str!("../README.md");
    let audit = include_str!("../docs/web_parity_audit.md");
    let checklist = include_str!("../docs/manual_qa_checklist.md");

    for required in [
        "usage: scripts/capture-web-visuals.mjs <url> [--out screenshots/web]",
        "ORBIFOLD_CHROME_DEVTOOLS_TIMEOUT_MS",
        "--enable-unsafe-webgpu",
        "--ignore-gpu-blocklist",
        "--disable-dev-shm-usage",
        "isCliEntrypoint",
        "export function parseArgs",
        "export function timestampForPath",
        "export function screenshotFallbackReason",
        "export function isReadyForCapture",
        "export function visualCaptureFailures",
        "export function pngStats",
        "compact-1200x760",
        "desktop-1600x1000",
        "hidpi-1920x1080-dpr2",
        "wide-3840x2160",
        "Page.captureScreenshot",
        "fromSurface: false",
        "screenshotAttempts",
        "paint-snapshot-svg",
        "readVisualSnapshot",
        "Target.closeTarget",
        "orbifoldVisualSnapshotReady",
        "orbifoldVisualSnapshotBytes",
        "orbifoldVisualSnapshotItemCount",
        "manifest.json",
        "visualCaptureFailures",
        "browser errors were recorded during visual capture",
        "Runtime.exceptionThrown",
        "Runtime.consoleAPICalled",
        "Network.loadingFailed",
        "Log.entryAdded",
        "inflateSync",
        "screenshot is blank/transparent",
        "nonTransparentPixels",
        "paethPredictor",
        "runtime-ready",
        "runtime-failed",
        "canvasClientWidth",
        "canvasClientHeight",
        "canvasWidth",
        "canvasHeight",
        "devicePixelRatio",
        "orbifoldPianoGridWidth",
        "orbifoldPianoRollHeight",
        "orbifoldRightPanelWidth",
    ] {
        assert!(
            script.contains(required),
            "scripts/capture-web-visuals.mjs should capture web visual parity evidence: {required}"
        );
    }

    for required in [
        "parseVisualArgs([\"https://example.invalid/Orbifold\"])",
        "timestampForPath()",
        "isReadyForCapture(captureState, viewport)",
        "screenshotFallbackReason(null)",
        "pngStats(",
        "makeRgbaPng(2, 1",
        "visualCaptureFailures([",
        "web visual layout helper behavior ok",
    ] {
        assert!(
            behavior_test.contains(required),
            "scripts/test-web-visual-layout-helpers.mjs should behavior-test {required}"
        );
    }

    for docs in [readme, audit, checklist] {
        assert!(
            docs.contains("./scripts/capture-web-visuals.mjs"),
            "web visual capture workflow should be documented"
        );
    }
}

#[test]
fn web_manual_device_script_records_real_browser_device_evidence() {
    let script = include_str!("../scripts/check-web-manual-devices.mjs");
    let behavior_test = include_str!("../scripts/test-web-manual-devices.mjs");
    let readme = include_str!("../README.md");
    let audit = include_str!("../docs/web_parity_audit.md");
    let checklist = include_str!("../docs/manual_qa_checklist.md");

    for required in [
        "usage: scripts/check-web-manual-devices.mjs <url>",
        "orbifold.web_manual_device_parity.v1",
        "This manual device check requires an interactive terminal",
        "validateManualDeviceReport",
        "fetchWebArtifactFingerprint",
        "report.artifact",
        "report evidence validated",
        "isCliEntrypoint",
        "export async function runManualDeviceCli",
        "export function parseManualDeviceArgs",
        "export function createManualDeviceReport",
        "export function persistedNoteCount",
        "--enable-unsafe-webgpu",
        "--ignore-gpu-blocklist",
        "Input.dispatchMouseEvent",
        "clickManualControl",
        "prefix + \"X\"",
        "orbifoldManualAudioRefresh",
        "orbifoldManualMidiRefresh",
        "manualVisualInspection",
        "audioOutputsDiscovered",
        "webAudioOutputsDiscovered",
        "audioConnected",
        "webAudioConnectedState",
        "audibleWebAudio",
        "manualAudibleWebAudio",
        "midiInputsDiscovered",
        "webMidiInputsDiscovered",
        "midiConnected",
        "webMidiConnectedState",
        "manualRealMidiInput",
        "manualRealMidiRecording",
        "web-manual-devices-",
        "reports",
    ] {
        assert!(
            script.contains(required),
            "scripts/check-web-manual-devices.mjs should capture manual device evidence: {required}"
        );
    }

    for required in [
        "parseManualDeviceArgs",
        "createManualDeviceReport",
        "persistedNoteCount",
        "Unknown argument: --bogus",
        "manual web device runner behavior ok",
    ] {
        assert!(
            behavior_test.contains(required),
            "scripts/test-web-manual-devices.mjs should behavior-test {required}"
        );
    }

    for docs in [readme, audit, checklist] {
        assert!(
            docs.contains("./scripts/check-web-manual-devices.mjs"),
            "manual web device parity workflow should be documented"
        );
    }
}

#[test]
fn web_manual_report_validator_requires_real_device_evidence() {
    let script = include_str!("../scripts/check-web-manual-report.mjs");
    let behavior_test = include_str!("../scripts/test-web-manual-report-validator.mjs");
    let readme = include_str!("../README.md");
    let audit = include_str!("../docs/web_parity_audit.md");
    let checklist = include_str!("../docs/manual_qa_checklist.md");
    let release_checklist = include_str!("../docs/release_checklist.md");
    let release_workflow = include_str!("../docs/release_workflow.md");
    let limitations = include_str!("../docs/known_limitations.md");

    for required in [
        "usage: scripts/check-web-manual-report.mjs <reports/web-manual-devices-*.json|reports-dir>",
        "orbifold.web_manual_device_parity.v1",
        "manualDeviceVerifierCompleted",
        "manualAudibleWebAudio",
        "manualRealMidiInput",
        "manualRealMidiRecording",
        "host.platform",
        "host.arch",
        "host.release",
        "chrome.path",
        "chrome.userAgent",
        "chrome.protocolVersion",
        "browserEvents",
        "manualReportBrowserFailures",
        "browserEvents should not contain runtime errors",
        "Runtime.exceptionThrown",
        "Runtime.consoleAPICalled",
        "Network.loadingFailed",
        "Log.entryAdded",
        "clicks.${name}.at",
        "requireIsoDate",
        "audioResumeResolved",
        "audioNonzero",
        "midiInputCount",
        "connectedMidiInput",
        "requireArtifactFingerprint",
        "normalizeWebRootHref",
        "artifact.rootUrl",
        "artifact",
        "beforeNoteCount",
        "afterNoteCount",
        "manualRealMidiInput evidence should show a changed MIDI status or note",
        "requiredClickCounts",
        "audioTestA4",
        "midiRefresh",
        "manual web device report ok",
        "export function validateManualDeviceReport",
        "isCliEntrypoint",
    ] {
        assert!(
            script.contains(required),
            "scripts/check-web-manual-report.mjs should validate manual report evidence: {required}"
        );
    }

    for required in [
        "validateManualDeviceReport(report)",
        "browserEvents should not contain runtime errors",
        "clicks.record expected at least 2",
        "artifact.rootUrl expected",
        "states.afterAudioTest.audioNonzero expected true",
        "manualRealMidiInput evidence should show a changed MIDI status or note",
        "manualRealMidiRecording evidence should show a new recorded note",
        "manual web device report validator behavior ok",
    ] {
        assert!(
            behavior_test.contains(required),
            "scripts/test-web-manual-report-validator.mjs should behavior-test {required}"
        );
    }

    for docs in [
        readme,
        audit,
        checklist,
        release_checklist,
        release_workflow,
        limitations,
    ] {
        assert!(
            docs.contains("./scripts/check-web-manual-report.mjs reports/"),
            "manual web device report validation workflow should be documented"
        );
    }
}

#[test]
fn web_parity_gate_ties_deployed_and_manual_evidence_together() {
    let script = include_str!("../scripts/check-web-parity-gate.mjs");
    let behavior_test = include_str!("../scripts/test-web-parity-gate.mjs");
    let readme = include_str!("../README.md");
    let audit = include_str!("../docs/web_parity_audit.md");
    let checklist = include_str!("../docs/manual_qa_checklist.md");
    let release_checklist = include_str!("../docs/release_checklist.md");
    let release_workflow = include_str!("../docs/release_workflow.md");

    for required in [
        "usage: scripts/check-web-parity-gate.mjs <https://pages-url/>",
        "orbifold.web_parity_gate.v1",
        "check-web-live.mjs",
        "check-web-layout.mjs",
        "check-web-smoke.mjs",
        "capture-web-visuals.mjs",
        "check-web-manual-report.mjs",
        "web-artifact-fingerprint.mjs",
        "fetchWebArtifactFingerprint",
        "compareWebArtifactFingerprints",
        "web-parity-gate-",
        "--report",
        "--visual-out",
        "--skip-visual-capture",
        "visual capture was skipped; rerun without --skip-visual-capture for parity",
        "manualDeviceReport",
        "manualReportTarget",
        "manualReportArtifact",
        "manual report target",
        "manual report artifact matches live",
        "deployedVisualCapture",
        "Orbifold web parity gate passed",
        "isCliEntrypoint",
        "export async function runParityGate",
        "export function parseParityGateArgs",
        "export function createParityGateReport",
        "export function normalizeParityGateUrl",
    ] {
        assert!(
            script.contains(required),
            "scripts/check-web-parity-gate.mjs should tie web parity evidence together: {required}"
        );
    }

    for required in [
        "parseParityGateArgs",
        "normalizeParityGateUrl",
        "createParityGateReport",
        "compareWebArtifactFingerprints",
        "Unknown argument: --bogus",
        "screenshots/final",
        "rootUrl expected https://example.invalid/Orbifold/, got https://example.invalid/Other/",
        "wasm sha256 expected",
        "web parity gate behavior ok",
    ] {
        assert!(
            behavior_test.contains(required),
            "scripts/test-web-parity-gate.mjs should behavior-test {required}"
        );
    }

    for docs in [
        readme,
        audit,
        checklist,
        release_checklist,
        release_workflow,
    ] {
        assert!(
            docs.contains("./scripts/check-web-parity-gate.mjs https://<user>.github.io/<repo>/ --report reports/"),
            "web parity gate workflow should be documented"
        );
    }
}

#[test]
fn web_artifact_fingerprint_script_hashes_deployed_files() {
    let script = include_str!("../scripts/web-artifact-fingerprint.mjs");
    let behavior_test = include_str!("../scripts/test-web-artifact-fingerprint.mjs");

    for required in [
        "orbifold.web_artifact_fingerprint.v1",
        "createHash",
        "sha256",
        "cache: \"no-store\"",
        "redirect: \"follow\"",
        "pkg/orbifold_web.js",
        "pkg/orbifold_web_bg.wasm",
        "favicon.ico",
        "orbifold_icon.png",
        "fetchWebArtifactFingerprint",
        "compareWebArtifactFingerprints",
        "requireArtifactFingerprint",
        "normalizeWebRootHref",
    ] {
        assert!(
            script.contains(required),
            "scripts/web-artifact-fingerprint.mjs should fingerprint deployed web artifacts: {required}"
        );
    }

    for required in [
        "requireArtifactFingerprint(artifact)",
        "normalizeWebRootHref",
        "compareWebArtifactFingerprints",
        "rootUrl expected https://example.invalid/Orbifold/, got https://example.invalid/Other/",
        "artifact.schema should be orbifold.web_artifact_fingerprint.v1",
        "artifact.files.wasm should be an object",
        "artifact.files.icon.sha256 should be a sha256 hex digest",
        "artifact.files.favicon.bytes should be positive",
        "web artifact fingerprint behavior ok",
    ] {
        assert!(
            behavior_test.contains(required),
            "scripts/test-web-artifact-fingerprint.mjs should behavior-test {required}"
        );
    }
}

#[test]
fn web_live_check_script_verifies_deployed_pages_artifact_shape() {
    let script = include_str!("../scripts/check-web-live.mjs");
    let behavior_test = include_str!("../scripts/test-web-artifact-checks.mjs");

    for required in [
        "usage: scripts/check-web-live.mjs <https://pages-url/>",
        "export async function checkWebLive",
        "export function normalizeWebLiveUrl",
        "requireWebIndexHtml(index.text)",
        "fetchRequired",
        "cache: \"no-store\"",
        "redirect: \"follow\"",
        "content-type",
        "pkg/orbifold_web.js",
        "pkg/orbifold_web_bg.wasm",
        "favicon.ico",
        "orbifold_icon.png",
        "static fallback",
        "orbifold_web_bg.wasm",
        "start_orbifold",
        "is not a wasm binary",
    ] {
        assert!(
            script.contains(required),
            "scripts/check-web-live.mjs should verify {required}"
        );
    }

    for required in [
        "await checkWebLive(\"https://example.invalid/Orbifold\", mockFetch())",
        "normalizeWebLiveUrl(\"https://example.invalid/Orbifold?old=1#section\")",
        "is not a wasm binary",
        "returned content-type text\\/plain, expected text\\/html",
        "returned HTTP 404",
        "web artifact checks behavior ok",
    ] {
        assert!(
            behavior_test.contains(required),
            "scripts/test-web-artifact-checks.mjs should behavior-test {required}"
        );
    }
}

#[test]
fn web_smoke_script_checks_headless_runtime_readiness() {
    let script = include_str!("../scripts/check-web-smoke.mjs");

    for required in [
        "ORBIFOLD_CHROME_DEVTOOLS_TIMEOUT_MS",
        "--enable-unsafe-webgpu",
        "--ignore-gpu-blocklist",
        "--disable-dev-shm-usage",
        "Runtime.exceptionThrown",
        "Runtime.consoleAPICalled",
        "Network.loadingFailed",
        "waitForOrbifoldReady",
        "runtime-ready",
        "orbifoldFrameCount",
        "Number(lastState.frameCount) >= 2",
        "keyboardShortcuts === \"installed\"",
        "verifyBrowserStartupStorageGuards",
        "Page.addScriptToEvaluateOnNewDocument",
        "orbifold_smoke",
        "browser startup invalid localStorage was overwritten",
        "browser startup invalid localStorage did not leave a visible error",
        "verifyHighDpiCanvasScale",
        "deviceScaleFactor: 2",
        "high-DPI browser canvas did not scale to the full viewport",
        "browser canvas did not recover after high-DPI resize",
        "verifyToolbarButtonClicks",
        "toolbar Play button click did not start transport through canvas hit testing",
        "toolbar Stop button click did not stop transport through canvas hit testing",
        "window.orbifoldDispatchAction(\"clip.add_note\")",
        "verifyBrowserShortcutMapping",
        "__orbifoldShortcutActionForTest",
        "browser shortcut mapping mismatch for ${shortcut.label}",
        "Ctrl+Shift+S",
        "Cmd+S",
        "Ctrl+Shift+Z",
        "Shift+ArrowRight",
        "Repeat ArrowRight",
        "Ctrl+Alt+S",
        "verifyKeyboardShortcut",
        "verifyKeyboardNoteEditShortcuts",
        "verifyBrowserTextEditActions",
        "orbifoldDispatchTextInput",
        "orbifoldDispatchTextKey",
        "transport.bpm_input",
        "scale.root_input",
        "scale.base_input",
        "scale.search",
        "asset.search",
        "bpm=144",
        "root_midi=60",
        "base_freq=432",
        "Scale search: edo",
        "Asset search: kick",
        "verifyPianoGridDoubleClick",
        "verifyPianoNoteDrag",
        "verifyPianoNoteResize",
        "verifyPianoVelocityDrag",
        "verifyBrowserClipEditActions",
        "verifyBrowserQuantizeAction",
        "verifyPianoWheelNavigation",
        "verifyTimelineAndLoopGestures",
        "orbifoldTransportPositionBeats",
        "orbifoldLoopBeats",
        "orbifoldArrangementSeekStartX",
        "orbifoldPianoSeekStartX",
        "orbifoldArrangementLoopEndStartX",
        "orbifoldPianoLoopEndStartX",
        "browser arrangement ruler drag did not seek transport",
        "browser piano ruler drag did not seek transport",
        "browser arrangement loop-end drag did not resize loop length",
        "browser piano loop-end drag did not resize loop length",
        "projectIncludesLoopBeats",
        "verifyWorkspaceResizeGestures",
        "verifyBrowserFileFlows",
        "verifyBrowserProjectSaveAsShortcut",
        "verifyInvalidBrowserProjectOpen",
        "verifyInvalidBrowserScaleOpen",
        "verifyInvalidBrowserKeymapOpen",
        "verifyUnsupportedBrowserAssetImport",
        "Browser.setDownloadBehavior",
        "DOM.setFileInputFiles",
        "file.open",
        "file.save",
        "file.save_as",
        "scale.open",
        "keymap.open",
        "asset.import",
        "installMockBrowserDevices",
        "enumerateDevices: async () => audioOutputs",
        "setSinkId(deviceId)",
        "Default Smoke Output",
        "Orbifold Smoke Speakers",
        "open: async ()",
        "verifyBrowserMidiFlow",
        "navigator, \"requestMIDIAccess\"",
        "midi.refresh",
        "midi.connect",
        "browserMidiInputNames.includes(\"Orbifold Smoke MIDI\")",
        "orbifoldBrowserMidiDiagnostic",
        "browserMidiDiagnostic.includes(\"Web MIDI: open\")",
        "midiInputConnection === \"open\"",
        "browser MIDI refresh did not list the mocked MIDI input",
        "browser MIDI connect did not connect the mocked MIDI input",
        "browser MIDI message did not reach Orbifold's shared MIDI handling path",
        "browser MIDI note-off did not reach Orbifold's shared MIDI handling path",
        "browser recording did not start before mocked MIDI input",
        "browser MIDI note-on did not update last MIDI state while recording",
        "browser MIDI note-off did not update last MIDI state while recording",
        "browser MIDI recording did not persist a note-on/note-off clip note",
        "transport.record",
        "sendBrowserMidiMessage",
        "verifyBrowserMidiFailureFlows",
        "verifyBrowserMidiFailureFlow",
        "midi-unavailable",
        "midi-denied",
        "Web MIDI is not available in this browser",
        "Web MIDI request failed: Permission denied by smoke test",
        "Web MIDI: unavailable",
        "Web MIDI: permission denied",
        "browser MIDI ${label} refresh did not surface a visible unavailable/permission error",
        "browser MIDI ${label} connect did not stay disconnected with a visible error",
        "projectNotes",
        "latestProjectNote",
        "projectNoteById",
        "rawNote === 64",
        "velocity === 96",
        "verifyBrowserAudioFlow",
        "audio.refresh",
        "audio.connect",
        "audioOutputSelectionSupported",
        "orbifoldAudioOutputSelectionSupported",
        "orbifoldBrowserAudioOutputNames",
        "orbifoldBrowserAudioDiagnostic",
        "browserAudioDiagnostic.includes(\"Web Audio: sink\")",
        "state.audioOutputCount >= 2",
        "state.connectedAudioOutput === \"Default Smoke Output\"",
        "state.audioSinkResolved === \"Default Smoke Output\"",
        "state.audioSinkDeviceId === \"default\"",
        "browser audio refresh did not expose mocked Web Audio outputs",
        "browser audio connect did not create and route the named Web Audio stream",
        "audio.test_a4",
        "browser audio test tone did not produce nonzero Web Audio samples",
        "verifyBrowserAudioFallbackFlow",
        "activateBrowserDocument",
        "orbifold_smoke",
        "audioSinkSelectionUnsupported",
        "Web Audio: default",
        "Browser audio",
        "browser audio refresh did not expose the default fallback output when sink selection is unavailable",
        "browser audio fallback connect did not create a default Web Audio stream",
        "browser audio fallback test tone did not produce nonzero Web Audio samples",
        "verifyBrowserAudioUnavailableFlow",
        "audio-unavailable",
        "Web Audio: unavailable",
        "Audio refresh error: Web Audio is not available in this browser",
        "No audio output selected",
        "browser audio refresh did not surface a visible unavailable error",
        "browser audio connect did not stay disconnected when Web Audio is unavailable",
        "audioCallbackCount > 0",
        "audioFrameCount > 0",
        "audioPeak > 0.0001",
        "audioNonzero",
        "verifyBrowserPersistenceAfterReload",
        "verifyBrowserUiScaleReload",
        "ui.scale_up",
        "ui_scale=1.1",
        "browser UI scale action did not persist settings and reload the web runtime",
        "Page.reload",
        "setBrowserPanelVisibility",
        "show_asset_browser=false",
        "show_scale_browser=true",
        "show_clip_panel=true",
        "scaleDescription === smokeScaleDescription",
        "scala_path=browser_5_edo.scl",
        "lumatone_path=classic.ltn",
        "lumatoneLoaded",
        "sample_instrument_path=browser_assets/samples/smoke_sample.wav",
        "browser imported WAV could not be assigned as the project sample instrument",
        "browser reload did not restore the saved project session, browser text resources, imported sample instrument, asset, and panel settings",
        "browser asset-panel visibility setting did not change",
        "browser scale-panel visibility setting did not change",
        "browser clip-panel visibility setting did not change",
        "browser project file picker did not load the selected project",
        "browser dirty-project setup did not add a note before open confirmation",
        "browser project open did not stop for dirty-project confirmation",
        "dirty browser project open created a file input",
        "Unsaved changes: click Open again to discard",
        "state.title.includes(\"Orbifold - project *\")",
        "state.title === \"Orbifold - browser_open_test\"",
        "assertNoFileInput",
        "fileInputNodeId",
        "browser Ctrl+Shift+S shortcut did not publish a downloadable project file",
        "invalid browser project open did not preserve the current project with a visible parse error",
        "invalid browser scale open did not preserve the current scale with a visible parse error",
        "invalid browser key-map open did not preserve the current key map with a visible parse error",
        "unsupported browser asset import did not preserve assets with a visible error",
        "Project parse error (bad_project.orbifold):",
        "Scala parse error (bad_scale.scl):",
        "Key map parse error (bad_keymap.ltn):",
        "Asset import error: unsupported sample",
        "browser scale file picker did not load the selected Scala file",
        "browser key-map file picker did not load the selected Lumatone map",
        "browser asset file picker did not import the selected WAV sample",
        "Input.dispatchKeyEvent",
        "pressKey",
        "Input.dispatchMouseEvent",
        "browser keyboard ArrowRight did not nudge the selected note",
        "browser keyboard Shift+ArrowRight did not lengthen the selected note",
        "browser keyboard ArrowUp did not transpose the selected note",
        "browser keyboard Shift+ArrowUp did not raise the selected note velocity",
        "browser keyboard shortcut did not route Shift+/ to shortcut help",
        "browser keyboard shortcut did not route Space to transport play",
        "browser keyboard shortcut did not route Space to transport stop",
        "browser clip copy did not copy the selected note without changing the project",
        "browser clip paste did not create a copied note through the shared action path",
        "browser clip duplicate did not duplicate the selected note through the shared action path",
        "browser clip delete did not remove the selected note through the shared action path",
        "browser off-grid double-click did not create an unsnapped note with snap disabled",
        "browser clip quantize did not snap the selected off-grid note",
        "isQuantizedToSixteenth",
        "browser shortcut mapping probe was not installed",
        "type: \"mouseWheel\"",
        "Ctrl+wheel did not zoom the piano-roll time view",
        "Shift+wheel did not scroll the piano-roll time view",
        "Alt+wheel did not zoom the piano-roll pitch view",
        "wheel did not scroll the piano-roll pitch view",
        "type: \"rawKeyDown\"",
        "type: \"mousePressed\"",
        "type: \"mouseMoved\"",
        "code: \"KeyN\"",
        "lastAction === \"clip.add_note\"",
        "noteCount >= 1",
        "noteCount >= 2",
        "persistedNoteCount(lastState.project) >= 3",
        "thirdNoteStartBeat(lastState.project) >= 5.9",
        "thirdNoteDurationBeat(lastState.project) >= 2.5",
        "browser piano velocity drag did not update the note velocity",
        "orbifold.project.v1",
        "\\nnote\\t",
        "orbifoldPianoAddX",
        "orbifoldPianoAddY",
        "orbifoldPianoDragStartX",
        "orbifoldPianoDragEndX",
        "orbifoldPianoResizeStartX",
        "orbifoldPianoResizeEndX",
        "orbifoldPianoVelocityStartX",
        "orbifoldPianoVelocityEndX",
        "orbifoldPianoViewStart",
        "orbifoldPianoViewBeats",
        "orbifoldPianoMinPitch",
        "orbifoldPianoMaxPitch",
        "orbifoldRightResizeX",
        "orbifoldBottomResizeY",
        "orbifoldRightPanelWidth",
        "orbifoldPianoRollHeight",
        "navigator.gpu",
        "window.__orbifoldLastDownloadText",
        "audioStreamConnected",
        "transportPlaying",
        "canvasWidth >= 1200",
        "canvasHeight >= 760",
        "canvasClientWidth",
        "devicePixelRatio",
    ] {
        assert!(
            script.contains(required),
            "scripts/check-web-smoke.mjs should verify {required}"
        );
    }
}

#[test]
fn cargo_manifest_keeps_web_build_separate_from_native_app() {
    let manifest = include_str!("../Cargo.toml");

    for required in [
        "native-app",
        "web-app",
        "operad/web-runtime",
        "[[bin]]",
        "required-features = [\"native-app\"]",
        "[[example]]",
        "name = \"orbifold_web\"",
        "required-features = [\"web-app\"]",
    ] {
        assert!(
            manifest.contains(required),
            "Cargo.toml should mention {required}"
        );
    }
}

#[test]
fn web_keyboard_shortcuts_use_browser_specific_file_and_device_paths() {
    let source = include_str!("../src/ui/web.rs");

    for required in [
        "for action in browser_drain_keyboard_actions()",
        "self.handle_browser_activate_action(&action)",
        "fn handle_browser_activate_action(&mut self, action_name: &str)",
        "\"file.open\" => self.begin_browser_project_open()",
        "\"file.save\" | \"file.save_as\" => self.download_browser_project()",
        "self.app.browser_project_download_payload()",
        "self.app.mark_browser_project_downloaded(&file_name)",
        "Browser project download error",
        "download_text_file_js(file_name, text).map_err(js_error_message)",
        "fn download_text_file_js(file_name: &str, text: &str) -> Result<(), JsValue>",
        "\"scale.open\" | \"scale.import\" => self.begin_browser_text_open(",
        "\"keymap.open\" => self.begin_browser_text_open(",
        "\"asset.import\" => self.begin_browser_asset_import()",
        "\"midi.refresh\" => self.begin_browser_midi_refresh()",
        "\"midi.connect\" => self.begin_browser_midi_connect()",
        "Ok(js_string_array_lossy(value))",
        "document.body.dataset.orbifoldBrowserMidiInputNames",
        "document.body.dataset.orbifoldMidiAccessState = \"permission requested\"",
        "document.body.dataset.orbifoldMidiAccessState = \"permission denied\"",
        "Web MIDI request failed",
        "orbifoldBrowserErrorMessage",
        "if (typeof input.open === \"function\")",
        "await input.open()",
        "document.body.dataset.orbifoldMidiInputConnection",
        "orbifoldMidiAccess.onstatechange",
        "throw `Browser MIDI input not found: ${selectedName}`",
        "if (orbifoldMidiInput)",
    ] {
        assert!(
            source.contains(required),
            "src/ui/web.rs should route browser keyboard actions through {required}"
        );
    }
}

#[test]
fn web_project_open_keeps_dirty_confirmation_gate() {
    let source = include_str!("../src/ui/web.rs");
    let app_source = include_str!("../src/app.rs");

    for required in [
        "fn begin_browser_project_open(&mut self)",
        "if !self.app.request_open_project()",
        "BrowserTextFileKind::Project",
        "BrowserTextFileKind::Scale",
        "BrowserTextFileKind::KeyMap",
        "\"Opening browser project...\"",
        "if app.load_browser_project_text_with_resources(",
        "app.set_status_preserving_error(\"Restored browser project session\")",
        "should_persist_project =",
        "self.app.load_browser_project_text_with_resources(",
        "self.app.load_browser_scale_text(&file.text, &file.name)",
        "self.app.load_browser_lumatone_text(&file.text, &file.name)",
        "browser_save_text_resource(\"scale\", &file.name, &file.text)",
        "browser_save_text_resource(\"keymap\", &file.name, &file.text)",
        "browser_project_resources_for_text(&text)",
    ] {
        assert!(
            source.contains(required),
            "src/ui/web.rs should keep dirty-open confirmation before browser picker: {required}"
        );
    }

    assert!(
        app_source.contains("pub(crate) fn load_browser_project_text")
            && app_source.contains("pub(crate) fn load_browser_project_text_with_resources")
            && app_source.contains("pub(crate) fn load_browser_scale_text")
            && app_source.contains("pub(crate) fn load_browser_lumatone_text")
            && app_source.contains("-> bool"),
        "browser file loads should report success so web persistence cannot hide parse failures"
    );
}

#[test]
fn web_startup_does_not_overwrite_invalid_browser_settings() {
    let source = include_str!("../src/ui/web.rs");

    for required in [
        "should_write_initial_settings",
        "(app, false)",
        "Browser settings load error",
        "Browser settings unavailable",
        "if should_write_initial_settings",
        "browser_save_settings_text(&app.browser_settings_text())",
        "app.set_status_preserving_error(format!(\"Restored {restored} browser assets\"))",
    ] {
        assert!(
            source.contains(required),
            "src/ui/web.rs should avoid overwriting invalid or unavailable browser settings: {required}"
        );
    }
}

#[test]
fn web_shortcut_bridge_matches_native_repeat_and_help_basics() {
    let source = include_str!("../src/ui/web.rs");

    for required in [
        "event.repeat && !orbifoldShortcutAllowsRepeat(event)",
        "[\"ArrowLeft\", \"ArrowRight\", \"ArrowDown\", \"ArrowUp\"]",
        "return \"help.shortcuts\"",
        "lower === \"c\" && !event.shiftKey",
        "lower === \"v\" && !event.shiftKey",
        "lower === \"n\" && !event.shiftKey",
        "event.preventDefault();",
        "window.__orbifoldShortcutActionForTest",
        "eventInit || {}",
        "{ passive: false }",
    ] {
        assert!(
            source.contains(required),
            "src/ui/web.rs should preserve web shortcut parity detail: {required}"
        );
    }
}

#[test]
fn web_timeline_seeking_keeps_active_drag_capture() {
    let source = include_str!("../src/ui/web.rs");

    for required in [
        "timeline_drag: Option<WebTimelineDragMode>",
        "enum WebTimelineDragMode",
        "self.timeline_drag.is_some()",
        "web_timeline_drag_mode_from_action(action)",
        "\"transport.seek\" => Some(WebTimelineDragMode::Arrangement)",
        "\"piano.seek\" => Some(WebTimelineDragMode::Piano)",
        "fn handle_timeline_drag_action",
        "fn seek_timeline",
        "layout.arrangement_beat_at(point)",
        "layout.piano_ruler_beat_at(point)",
        "fn handle_browser_workspace_pointer_event",
        "layout.loop_end_drag_action_at_point(event.position)",
        "layout.timeline_drag_action_at_point(event.position)",
        "self.handle_active_pointer_drag(event.phase, event.position, layout)",
        "action == \"active.drag_capture\" && self.workspace_pointer_bridge_installed",
    ] {
        assert!(
            source.contains(required),
            "src/ui/web.rs should keep browser timeline seeking captured during long drags: {required}"
        );
    }
}

#[test]
fn web_runtime_handles_shared_text_edit_actions() {
    let web_source = include_str!("../src/ui/web.rs");
    let actions_source = include_str!("../src/ui/actions.rs");
    let native_source = include_str!("../src/ui/native.rs");

    for required in [
        "WidgetActionKind::TextEdit(edit)",
        "handle_text_edit_action(&mut self.app, &action_name, edit)",
        "browser_drain_text_edits()",
        "drain_text_edit_actions_js",
        "orbifoldTextEditQueue",
        "window.orbifoldDispatchTextInput",
        "window.orbifoldDispatchTextKey",
        "self.persist_browser_settings()",
        "self.persist_browser_project_snapshot()",
    ] {
        assert!(
            web_source.contains(required),
            "src/ui/web.rs should route browser text edits through shared handling: {required}"
        );
    }

    for required in [
        "pub(super) fn handle_text_edit_action",
        "\"scale.search\" => handle_scale_search_text_edit",
        "\"scale.root_input\" => handle_root_midi_text_edit",
        "\"scale.base_input\" => handle_base_freq_text_edit",
        "\"asset.search\" => handle_asset_search_text_edit",
        "\"transport.bpm_input\" => handle_bpm_text_edit",
    ] {
        assert!(
            actions_source.contains(required),
            "src/ui/actions.rs should keep text edit behavior shared: {required}"
        );
    }

    assert!(
        native_source.contains("handle_text_edit_action(&mut self.app, &action_name, edit)"),
        "native text edit dispatch should keep using the shared handler"
    );
}

#[test]
fn web_runtime_surfaces_async_browser_audio_errors() {
    let web_source = include_str!("../src/ui/web.rs");
    let audio_source = include_str!("../src/audio.rs");

    for required in [
        "for err in crate::audio::drain_browser_audio_errors()",
        "self.app.set_error_status(err)",
    ] {
        assert!(
            web_source.contains(required),
            "src/ui/web.rs should surface async browser audio errors: {required}"
        );
    }

    for required in [
        "browser_audio_available_js",
        "document.body.dataset.orbifoldAudioContextCreated",
        "document.body.dataset.orbifoldAudioProcessorAttached",
        "document.body.dataset.orbifoldAudioResumeRequested",
        "orbifoldAudioOutputForName",
        "document.body.dataset.orbifoldAudioSinkRequested",
        "document.body.dataset.orbifoldAudioSinkResolved",
        "Web Audio sink selection failed",
        "recordOrbifoldAudioOutput",
        "document.body.dataset.orbifoldAudioCallbackCount",
        "document.body.dataset.orbifoldAudioFrameCount",
        "document.body.dataset.orbifoldAudioPeak",
        "document.body.dataset.orbifoldAudioNonzero",
        "return Vec::new()",
        "pushOrbifoldAudioError",
        "Web Audio resume failed",
        "drain_orbifold_audio_errors_js",
        "pub(crate) fn drain_browser_audio_errors() -> Vec<String>",
    ] {
        assert!(
            audio_source.contains(required),
            "src/audio.rs should queue async browser audio errors for the web UI: {required}"
        );
    }
}

#[test]
fn web_runtime_marks_ready_after_orbifold_frame_preparation() {
    let source = include_str!("../src/ui/web.rs");
    let html = include_str!("../web/index.html");

    for required in [
        "frame_count: u64",
        "runtime_ready_published: bool",
        "fn prepare_browser_frame",
        "self.app.update_music_playback()",
        "mark_browser_runtime_ready(self.frame_count, viewport)",
        "export function mark_runtime_ready_js",
        "function queueOrbifoldAction(action)",
        "function orbifoldActionQueue()",
        "window.__orbifoldActionQueue",
        "window.orbifoldDispatchAction = queueOrbifoldAction",
        "export function publish_action_result_js",
        "export function publish_runtime_state_js",
        "export function publish_automation_geometry_js",
        "export function publish_text_audit_js",
        "export function visual_snapshot_requested_js",
        "export function publish_visual_snapshot_svg_js",
        "export function publish_layout_automation_js",
        "export function install_browser_workspace_pointer_bridge_js",
        "export function drain_workspace_pointer_events_js",
        "install_browser_workspace_pointer_bridge_js(\"orbifold-canvas\")",
        "drain_workspace_pointer_events_js",
        "fn publish_browser_action_result",
        "fn publish_browser_automation_geometry",
        "fn publish_browser_text_audit",
        "fn publish_browser_visual_snapshot",
        "text_audit_summary(&document)",
        "visual_snapshot_svg(&document",
        "fn handle_browser_workspace_pointer_event",
        "document.body.dataset.orbifoldLastAction",
        "document.body.dataset.orbifoldProjectNoteCount",
        "document.body.dataset.orbifoldAudioAssetCount",
        "document.body.dataset.orbifoldMidiInputCount",
        "document.body.dataset.orbifoldConnectedMidiInput",
        "document.body.dataset.orbifoldLastMidiStatus",
        "document.body.dataset.orbifoldLastMidiNote",
        "document.body.dataset.orbifoldAudioOutputCount",
        "document.body.dataset.orbifoldConnectedAudioOutput",
        "document.body.dataset.orbifoldAudioStreamConnected",
        "document.body.dataset.orbifoldTransportPlaying",
        "document.body.dataset.orbifoldTransportPositionBeats",
        "document.body.dataset.orbifoldLoopBeats",
        "document.body.dataset.orbifoldUiScale",
        "document.body.dataset.orbifoldShowAssetBrowser",
        "document.body.dataset.orbifoldShowScaleBrowser",
        "document.body.dataset.orbifoldShowClipPanel",
        "document.body.dataset.orbifoldScaleDescription",
        "document.body.dataset.orbifoldScalaPath",
        "document.body.dataset.orbifoldLumatonePath",
        "document.body.dataset.orbifoldLumatoneLoaded",
        "document.body.dataset.orbifoldLastDownloadFileName",
        "document.body.dataset.orbifoldPianoAddX",
        "document.body.dataset.orbifoldPianoDragStartX",
        "document.body.dataset.orbifoldPianoResizeStartX",
        "document.body.dataset.orbifoldPianoVelocityStartX",
        "document.body.dataset.orbifoldPianoViewStart",
        "document.body.dataset.orbifoldPianoViewBeats",
        "document.body.dataset.orbifoldRightResizeX",
        "document.body.dataset.orbifoldPianoRollHeight",
        "document.body.dataset.orbifoldTextAuditCount",
        "document.body.dataset.orbifoldTextAuditIssueCount",
        "document.body.dataset.orbifoldTextAuditNonFiniteCount",
        "document.body.dataset.orbifoldVisualSnapshotReady",
        "window.__orbifoldVisualSnapshotSvg",
        "document.body.dataset.orbifoldKeyboardShortcuts = \"installed\"",
        "window.orbifoldRuntimeReady",
    ] {
        assert!(
            source.contains(required),
            "web runtime should publish readiness from frame preparation: {required}"
        );
    }

    assert!(
        !html.contains("document.body.classList.add(\"runtime-ready\");\n        document.body.classList.remove(\"runtime-failed\");\n        if (status)"),
        "web shell should not mark runtime-ready immediately after start_orbifold returns"
    );
    assert!(
        html.contains("Orbifold web runtime started; waiting for first frame"),
        "web shell should distinguish startup from first frame readiness"
    );
}

#[test]
fn web_runtime_uses_wasm_safe_app_clock() {
    let time_source = include_str!("../src/time.rs");

    assert!(
        time_source.contains("pub(crate) struct AppInstant")
            && time_source.contains("js_sys::Date::now()"),
        "shared app time should have a wasm-safe clock"
    );

    for (path, source) in [
        ("src/app.rs", include_str!("../src/app.rs")),
        ("src/project.rs", include_str!("../src/project.rs")),
        ("src/midi.rs", include_str!("../src/midi.rs")),
        ("src/ui/actions.rs", include_str!("../src/ui/actions.rs")),
        (
            "src/ui/native/surfaces.rs",
            include_str!("../src/ui/native/surfaces.rs"),
        ),
        (
            "src/ui/native/top_bar.rs",
            include_str!("../src/ui/native/top_bar.rs"),
        ),
    ] {
        assert!(
            !source.contains("std::time::Instant::now()"),
            "{path} should not call std::time::Instant::now(), which panics in wasm"
        );
    }
}

#[test]
fn web_runtime_uses_wasm_safe_delayed_note_offs() {
    let source = include_str!("../src/app.rs");

    for required in [
        "fn schedule_synth_note_off",
        "schedule_timeout_js",
        "window, js_name = setTimeout",
        "Audio test tone",
        "Metronome",
        "Audition",
    ] {
        assert!(
            source.contains(required),
            "src/app.rs should schedule web note-offs without spawning threads: {required}"
        );
    }

    for forbidden in [
        "std::thread::spawn(move || {\n            std::thread::sleep(std::time::Duration::from_millis(300))",
        "std::thread::spawn(move || {\n            std::thread::sleep(std::time::Duration::from_millis(45))",
        "std::thread::spawn(move || {\n            std::thread::sleep(std::time::Duration::from_millis(140))",
    ] {
        assert!(
            !source.contains(forbidden),
            "web-reachable audition/test/metronome note-offs should not directly spawn threads"
        );
    }
}

#[test]
fn web_runtime_uses_shared_dynamic_project_title() {
    let web_source = include_str!("../src/ui/web.rs");
    let app_source = include_str!("../src/app.rs");
    let native_window_source = include_str!("../src/ui/native/windowing.rs");

    assert!(
        web_source.contains(".with_title(|state: &WebOrbifoldApp| state.app.window_title())"),
        "web runtime should publish the same dynamic project/dirty title as native"
    );
    assert!(
        app_source.contains("pub(crate) fn window_title(&self) -> String"),
        "AppState should own title formatting so native and web share it"
    );
    assert!(
        native_window_source.contains("app.window_title()"),
        "native window title should use the shared AppState title formatter"
    );
}

#[test]
fn web_asset_bytes_use_indexeddb_with_legacy_localstorage_fallback() {
    let source = include_str!("../src/ui/web.rs");
    let readme = include_str!("../README.md");
    let asset_doc = include_str!("../docs/asset_browser.md");
    let fallback_html = include_str!("../web/index.html");

    for required in [
        "const ORBIFOLD_ASSET_DB_NAME",
        "window.indexedDB.open",
        "loadBrowserAssetStorageRecordsFromIndexedDb",
        "saveBrowserAssetStorageRecordToIndexedDb",
        "legacyAssetStorageRecords",
        "safeLegacyAssetStorageRecords",
        "Legacy browser asset storage error",
        "IndexedDB asset load error",
        "migrateMissingLegacyAssetRecordsToIndexedDb",
        "mergeBrowserAssetStorageRecords",
        "browserAssetStoragePathSet",
        "saveLegacyBrowserAssetStorageRecord",
        "async fn load_browser_asset_storage_records_js",
        "async fn save_browser_asset_storage_record_js",
    ] {
        assert!(
            source.contains(required),
            "src/ui/web.rs should persist browser asset bytes through IndexedDB with fallback: {required}"
        );
    }

    assert!(
        readme.contains(
            "asset bytes in IndexedDB with a legacy `localStorage`\nmigration/merge/fallback"
        ),
        "README should describe IndexedDB asset persistence"
    );
    assert!(
        asset_doc.contains("Browser-imported asset bytes are stored in IndexedDB"),
        "asset browser docs should describe IndexedDB asset persistence"
    );
    assert!(
        fallback_html.contains("WAV sample imports persist through IndexedDB"),
        "web fallback shell should describe IndexedDB asset persistence"
    );
}
