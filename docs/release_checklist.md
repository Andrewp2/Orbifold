# Release Checklist

Use this checklist before tagging or sharing an Orbifold build with testers.
For the full sequence, evidence expectations, and post-release handoff, see
`docs/release_workflow.md`.

## Version And Metadata

- Confirm `Cargo.toml` contains the intended package version.
- Confirm the status bar shows the same package version.
- Update `CHANGELOG.md` for the release.
- Check `packaging/linux/orbifold.desktop`.
- Check `packaging/linux/icons/hicolor/scalable/apps/orbifold.svg`.
- Check `packaging/linux/icons/hicolor/64x64/apps/orbifold.png`.
- Check `favicon.ico`.
- Check `web/index.html`.
- Check `.github/workflows/pages.yml`.
- Check `docs/web_parity_audit.md`.
- Confirm `LICENSE` is present.

## Verification

Run:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
./scripts/build-web.sh dist
./scripts/check-web-dist.mjs dist
./scripts/check-web-smoke.mjs http://127.0.0.1:4173/
./scripts/check-web-live.mjs https://<user>.github.io/<repo>/
cargo run -- --screenshot-size=1200x760
cargo run -- --screenshot-size=3840x2160
```

Inspect the generated screenshots after each UI-affecting change. Passing tests
or successfully writing an image does not prove the UI is visually usable.
For web parity claims, also complete the manual browser, deployed Pages, Web
MIDI, and audible Web Audio checks in `docs/web_parity_audit.md`, then validate
the generated manual-device report with
`./scripts/check-web-manual-report.mjs reports/` and run the final gate with
`./scripts/check-web-parity-gate.mjs https://<user>.github.io/<repo>/ --report reports/`.
Then run `./scripts/check-web-parity-complete.mjs reports/` over the saved
reports before making a parity claim. This also validates the saved visual
manifest and viewport artifact files.
The final gate compares the manual report's deployed artifact fingerprint with
the live Pages site, so rerun the manual device pass after deploying a changed
web artifact.

## Manual Smoke Test

- Launch a normal interactive build.
- Confirm missing audio or MIDI opens Devices/Setup mode instead of crashing.
- Refresh and connect available MIDI and audio devices.
- Play the A4 test tone when audio is connected.
- Record a short clip.
- Move, resize, duplicate, delete, and undo a piano-roll note edit.
- Save, quit, reopen, and confirm the clip is restored.
- Confirm the status bar shows the version and useful failure messages.
