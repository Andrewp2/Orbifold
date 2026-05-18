# Release Workflow

Use this workflow before tagging a version, publishing a build, or sharing an
Orbifold build with testers. The shorter gate list lives in
`docs/release_checklist.md`; this document explains the order and evidence
expected for a release candidate.

## 1. Define The Release Candidate

- Decide the package version in `Cargo.toml`.
- Confirm `CHANGELOG.md` has a heading for that version or an explicit
  `Unreleased` section for an untaged tester build.
- Confirm the intended user-visible scope: features added, known limitations,
  and any workflows that are intentionally disabled.
- Review `docs/known_limitations.md` and
  `docs/orbifold_usability_gap_analysis.md` for issues that should be called
  out to testers.
- Review `docs/web_parity_audit.md` before making any browser parity claim.

Do not tag a release while the changelog and known-limitation story disagree
with the actual product behavior.

## 2. Check Metadata

Verify release-facing metadata before running the long checks:

```sh
cargo test -q --test release_docs
cargo test -q --test desktop_metadata
```

These tests cover the changelog version heading, required release gates, Linux
desktop launcher fields, and the scalable icon asset. They do not replace manual
inspection of the release notes.

Manual metadata checks:

- `Cargo.toml` package name and version are correct.
- `LICENSE` is present.
- `README.md` describes the current prototype honestly.
- `packaging/linux/orbifold.desktop` points at the installed `orbifold` binary.
- `packaging/linux/icons/hicolor/scalable/apps/orbifold.svg` is present.
- `packaging/linux/icons/hicolor/64x64/apps/orbifold.png` matches
  `orbifold_icon.png`.
- `favicon.ico` contains 16px, 32px, 48px, and 64px browser icon entries.
- `web/index.html` loads `./pkg/orbifold_web.js` and references the favicon
  assets.
- `.github/workflows/pages.yml` builds `dist/` with `./scripts/build-web.sh`
  checks it with `./scripts/check-web-dist.mjs`, and uploads it as a Pages
  artifact.
- `docs/web_parity_audit.md` separates automated web evidence from manual
  browser, deployed Pages, Web MIDI, Web Audio, and manual report validation.

## 3. Run Automated Gates

Run the required release gates from the repo root:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
./scripts/build-web.sh dist
./scripts/check-web-dist.mjs dist
cargo run -- --startup-probe
cargo run -- --screenshot-size=1200x760
cargo run -- --screenshot-size=3840x2160
```

The screenshot commands should run without probing audio or MIDI hardware. If a
gate fails, fix the failure and restart the release workflow from the relevant
step.

The web build command should produce `dist/index.html`, `dist/pkg/`,
`dist/.nojekyll`, and copied favicon assets. The artifact checker verifies that
shape plus relative asset references before Pages upload. It runs the shared
Orbifold wasm app when WebGPU is available, including the browser Web Audio
callback and nonzero test-tone sample path, Web MIDI, browser file,
MIDI recording, browser file, project-session, browser-imported asset, and
shared Operad UI paths. It is still not a substitute for native hardware smoke
testing because browser device permission, WebGPU, Web MIDI, and audio-output
routing depend on the tester's browser and platform.
Run `./scripts/check-web-smoke.mjs` against the served `dist/` output for the
automated web parity gate, then use `docs/web_parity_audit.md` for the manual
web evidence that CI cannot provide.
When the manual browser/device pass writes `reports/web-manual-devices-*.json`,
run `./scripts/check-web-manual-report.mjs reports/` so the release evidence is
checked for passed Web Audio, Web MIDI, real-click, and user-confirmation
fields, including real file-picker, shortcut, and piano-roll parity
confirmations, plus the deployed artifact fingerprint, instead of only
preserving an unchecked JSON file. You can also run the manual browser/device command with
`--finalize` to run this validator and the remaining parity gates immediately
after the hardware prompts pass.
Then run
`./scripts/check-web-parity-gate.mjs https://<user>.github.io/<repo>/ --report reports/`
to tie the deployed live/layout/smoke checks, visual capture, manual report
validation, and live-vs-manual artifact fingerprint comparison into one final
pass/fail artifact.
Finally run
`./scripts/check-web-parity-complete.mjs reports/ --url https://<user>.github.io/<repo>/`
over the saved reports so the release evidence has a simple offline completion
check for the deployed target, including the visual manifest and saved viewport
artifact files.
After deployment, run `./scripts/check-web-live.mjs` against the Pages URL to
verify the published site is serving the expected wasm loader, wasm binary,
icons, relative paths, and runtime hooks. The Pages workflow also runs
`./scripts/check-web-smoke.mjs` against the deployed URL for CI runtime evidence
before the manual browser pass.

## 4. Inspect Visual Output

Open `screenshots/latest.png` after each screenshot command, or preserve both
images with their timestamped names from `screenshots/`.

Use `docs/ui_testing_workflow.md` and `docs/manual_qa_checklist.md` for the
inspection criteria. At minimum, verify:

- the UI fills the full rendered image,
- no obvious text overlaps exist,
- minimum-size controls remain targetable,
- 4K scale is readable,
- device setup, asset browser, clip panel, arrangement, piano roll, and status
  bar are visually coherent,
- incomplete workflows are labeled honestly.

Screenshot rendering only proves that pixels were written. Visual inspection is
part of the release gate.

## 5. Manual Smoke Test

Run a normal interactive build:

```sh
cargo run
```

Smoke-test the user workflows that make an alpha build usable:

- launch without required devices and verify Devices/Setup mode is clear,
- refresh and connect available MIDI and audio devices,
- play the A4 test tone when audio is connected,
- load a scale and key map,
- add or record a short clip,
- move, resize, duplicate, delete, undo, and redo a piano-roll note edit,
- save, quit, reopen, and confirm the clip is restored,
- verify layout reset and UI zoom still work,
- confirm the status bar names failures and shows useful context.

Record any unchecked hardware path in the release notes or handoff. For example,
do not imply Lumatone hardware was validated if no Lumatone was connected.

Run a browser smoke test from the generated Pages output:

```sh
python3 -m http.server 4173 --directory dist
```

Open `http://127.0.0.1:4173/` in a WebGPU-capable browser and verify:

- the live wasm UI replaces the static fallback shell,
- the browser tab title follows clean/dirty project state,
- project open/save/download uses `.orbifold` text and dirty-open confirmation,
- Scala `.scl` and Lumatone `.ltn` browser opens use the same parsers as native,
- `Ctrl`/`Cmd+S`, `Ctrl`/`Cmd+O`, `?`, arrows, and note-edit shortcuts work,
- BPM, root, base-frequency, scale-search, and asset-search text edits work,
- piano-roll note add, move, resize, velocity, wheel zoom/pan, viewport drags,
  and loop-end drags behave like native,
- workspace splitters resize the same panels as native,
- browser WAV import can preview/use a sample, reload, and keep the sample
  instrument available within browser storage quota,
- Web MIDI refresh/connect and note input work in a browser that exposes Web
  MIDI.

If the environment can only show the static fallback shell, record that as
fallback-only evidence. Do not claim live web UI parity from fallback screenshots
alone.

After GitHub Pages deploys, open the deployed Pages URL and repeat the live-UI,
favicon/assets, file, MIDI, audio, layout, and piano-roll checks that are
available in that browser. A local `dist/` pass is not proof that the deployed
site is serving the expected artifact. Start by running:

```sh
./scripts/check-web-live.mjs https://<user>.github.io/<repo>/
./scripts/check-web-smoke.mjs https://<user>.github.io/<repo>/
./scripts/check-web-parity-gate.mjs https://<user>.github.io/<repo>/ --report reports/
./scripts/check-web-parity-complete.mjs reports/ --url https://<user>.github.io/<repo>/
```

## 6. Prepare Handoff Notes

A release handoff should include:

- package version or release-candidate identifier,
- changelog section used,
- exact automated commands run,
- screenshot sizes captured and inspected,
- manual workflows checked,
- hardware connected during testing,
- known limitations worth calling out,
- any skipped checks with reasons.

If publishing to GitHub, create the tag only after the release candidate evidence
is complete. The tag name should match the package version, for example
`v0.1.0`.

## 7. After Release

After a shared build or tag:

- keep the release notes and known limitations aligned with tester feedback,
- add newly discovered issues to `docs/orbifold_usability_gap_analysis.md`,
- start the next changelog section before beginning new release-bound work,
- avoid changing an existing tag; make a new patch version instead.
