#[test]
fn changelog_mentions_current_package_version() {
    let changelog = include_str!("../CHANGELOG.md");
    let version_heading = format!("## {}", env!("CARGO_PKG_VERSION"));

    assert!(
        changelog.contains(&version_heading),
        "CHANGELOG.md should contain {version_heading}"
    );
}

#[test]
fn release_checklist_mentions_required_gates() {
    let checklist = include_str!("../docs/release_checklist.md");

    for required in [
        "cargo fmt --check",
        "cargo test",
        "cargo clippy --all-targets -- -D warnings",
        "./scripts/build-web.sh dist",
        "./scripts/check-web-dist.mjs dist",
        "./scripts/check-web-live.mjs https://<user>.github.io/<repo>/",
        "cargo run -- --screenshot-size=1200x760",
        "cargo run -- --screenshot-size=3840x2160",
        "CHANGELOG.md",
        "LICENSE",
        "packaging/linux/orbifold.desktop",
        "web/index.html",
        ".github/workflows/pages.yml",
        "docs/web_parity_audit.md",
        "docs/release_workflow.md",
    ] {
        assert!(
            checklist.contains(required),
            "release checklist should mention {required}"
        );
    }
}

#[test]
fn release_workflow_mentions_release_candidate_evidence() {
    let workflow = include_str!("../docs/release_workflow.md");

    for required in [
        "# Release Workflow",
        "Cargo.toml",
        "CHANGELOG.md",
        "docs/known_limitations.md",
        "docs/orbifold_usability_gap_analysis.md",
        "cargo test -q --test release_docs",
        "cargo test -q --test desktop_metadata",
        "cargo fmt --check",
        "cargo test",
        "cargo clippy --all-targets -- -D warnings",
        "./scripts/build-web.sh dist",
        "./scripts/check-web-dist.mjs dist",
        "cargo run -- --startup-probe",
        "cargo run -- --screenshot-size=1200x760",
        "cargo run -- --screenshot-size=3840x2160",
        "screenshots/latest.png",
        "docs/ui_testing_workflow.md",
        "docs/manual_qa_checklist.md",
        "docs/web_parity_audit.md",
        "v0.1.0",
    ] {
        assert!(
            workflow.contains(required),
            "release workflow should mention {required}"
        );
    }
}

#[test]
fn gitignore_excludes_local_runtime_state() {
    let gitignore = include_str!("../.gitignore");

    for required in [
        "/dist/",
        "/orbifold_settings.txt",
        "/microtonal_daw_settings.txt",
        "/orbifold_autosave.orbifold",
        "/screenshots/*.png",
    ] {
        assert!(
            gitignore.lines().any(|line| line.trim() == required),
            ".gitignore should contain {required}"
        );
    }
}
