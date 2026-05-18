#[test]
fn first_run_guide_covers_core_tester_workflows() {
    let guide = include_str!("../docs/first_run.md");

    for required in [
        "# First Run Guide",
        "cargo run",
        "Audio Setup",
        "MIDI Setup",
        "Tuning And Key Maps",
        "docs/lumatone_setup.md",
        "Record And Edit",
        "Save And Recover",
        "Visual Size",
        "Troubleshooting",
        "SETUP REQUIRED",
        "orbifold_autosave.orbifold",
        "cargo run -- --screenshot-size=1200x760",
        "cargo run -- --screenshot-size=3840x2160",
        "docs/troubleshooting.md",
    ] {
        assert!(
            guide.contains(required),
            "first run guide should mention {required}"
        );
    }
}

#[test]
fn readme_links_first_run_guide() {
    let readme = include_str!("../README.md");

    assert!(
        readme.contains("docs/first_run.md"),
        "README should link the first run guide"
    );
}

#[test]
fn file_format_doc_covers_persistence_compatibility_hooks() {
    let doc = include_str!("../docs/file_formats.md");

    for required in [
        "# Orbifold File Formats",
        "orbifold_settings.txt",
        "microtonal_daw_settings.txt",
        "orbifold_project=1",
        "microtonal_daw_project=1",
        "sample_instrument_path",
        "note\t",
        "orbifold_autosave.orbifold",
        ".project.orbifold.<pid>.tmp",
        "project.orbifold.bak",
        "layout_left_width",
        "recent_project",
        "Sample missing",
    ] {
        assert!(
            doc.contains(required),
            "file format doc should mention {required}"
        );
    }
}

#[test]
fn readme_links_file_format_doc() {
    let readme = include_str!("../README.md");

    assert!(
        readme.contains("docs/file_formats.md"),
        "README should link the file format guide"
    );
}

#[test]
fn architecture_doc_covers_major_code_boundaries() {
    let doc = include_str!("../docs/architecture.md");

    for required in [
        "# Architecture Overview",
        "src/main.rs",
        "AppState",
        "SharedMusicProject",
        "SynthHandle",
        "src/audio.rs",
        "src/midi.rs",
        "src/project.rs",
        "src/settings.rs",
        "src/ui/native.rs",
        "native/piano_interaction.rs",
        "ui/actions.rs",
        "Action Flow",
        "Testing",
    ] {
        assert!(
            doc.contains(required),
            "architecture doc should mention {required}"
        );
    }
}

#[test]
fn readme_links_architecture_doc() {
    let readme = include_str!("../README.md");

    assert!(
        readme.contains("docs/architecture.md"),
        "README should link the architecture overview"
    );
}

#[test]
fn audio_midi_threading_doc_covers_callback_boundaries() {
    let doc = include_str!("../docs/audio_midi_threading.md");

    for required in [
        "# Audio And MIDI Threading Model",
        "UI/Main Thread",
        "Audio Callback",
        "MIDI Callback",
        "AppState",
        "SynthEngine",
        "SynthHandle",
        "AudioCommand",
        "SharedMusicProject",
        "SharedMidiLast",
        "SharedLumatoneMap",
        "try_iter",
        "Device Lifecycle",
        "Failure Policy",
        "Rules For New Work",
    ] {
        assert!(
            doc.contains(required),
            "audio/MIDI threading doc should mention {required}"
        );
    }
}

#[test]
fn readme_and_architecture_link_audio_midi_threading_doc() {
    let readme = include_str!("../README.md");
    let architecture = include_str!("../docs/architecture.md");

    for (name, doc) in [("README", readme), ("architecture overview", architecture)] {
        assert!(
            doc.contains("docs/audio_midi_threading.md"),
            "{name} should link the audio/MIDI threading guide"
        );
    }
}

#[test]
fn operad_integration_doc_covers_native_ui_boundaries() {
    let doc = include_str!("../docs/operad_integration.md");

    for required in [
        "# Operad Integration Model",
        "native-window",
        "NativeOperadApp",
        "NativeWindowHooks",
        "build_surface_document",
        "widgets::scene",
        "native/controls.rs",
        "native/surfaces.rs",
        "native/piano_interaction.rs",
        "ui/actions.rs",
        "canonical_action_name",
        "Screenshot Mode",
        "Testing Expectations",
        "Rules For New UI Work",
    ] {
        assert!(
            doc.contains(required),
            "Operad integration doc should mention {required}"
        );
    }
}

#[test]
fn readme_and_architecture_link_operad_integration_doc() {
    let readme = include_str!("../README.md");
    let architecture = include_str!("../docs/architecture.md");

    for (name, doc) in [("README", readme), ("architecture overview", architecture)] {
        assert!(
            doc.contains("docs/operad_integration.md"),
            "{name} should link the Operad integration guide"
        );
    }
}

#[test]
fn ui_testing_workflow_doc_covers_verification_paths() {
    let doc = include_str!("../docs/ui_testing_workflow.md");

    for required in [
        "# UI Testing Workflow",
        "cargo fmt --check",
        "cargo test",
        "cargo clippy --all-targets -- -D warnings",
        "cargo run -- --startup-probe",
        "cargo run -- --screenshot-size=1200x760",
        "cargo run -- --screenshot-size=3840x2160",
        "screenshots/latest.png",
        "src/ui/native/tests.rs",
        "docs/manual_qa_checklist.md",
        "text-overlap",
        "cursor-shape",
        "disabled controls do not dispatch",
        "visual inspection",
    ] {
        assert!(
            doc.contains(required),
            "UI testing workflow should mention {required}"
        );
    }
}

#[test]
fn readme_architecture_and_operad_link_ui_testing_workflow_doc() {
    let readme = include_str!("../README.md");
    let architecture = include_str!("../docs/architecture.md");
    let operad = include_str!("../docs/operad_integration.md");

    for (name, doc) in [
        ("README", readme),
        ("architecture overview", architecture),
        ("Operad integration guide", operad),
    ] {
        assert!(
            doc.contains("docs/ui_testing_workflow.md"),
            "{name} should link the UI testing workflow"
        );
    }
}

#[test]
fn web_parity_audit_covers_automated_and_manual_evidence() {
    let doc = include_str!("../docs/web_parity_audit.md");

    for required in [
        "# Web Parity Audit",
        "Parity Definition",
        "Automated Evidence",
        "Manual Evidence",
        "Non-Parity Signals",
        "./scripts/build-web.sh dist",
        "./scripts/check-web-smoke.mjs http://127.0.0.1:4173/",
        "./scripts/check-web-live.mjs https://<user>.github.io/<repo>/",
        "./scripts/check-web-smoke.mjs https://<user>.github.io/<repo>/",
        "./scripts/check-web-manual-report.mjs reports/",
        "GitHub Pages",
        "Web MIDI",
        "Web Audio",
        "high-DPI",
        "real MIDI device",
        "audible output",
        "static fallback shell",
        "CDP screenshot",
    ] {
        assert!(
            doc.contains(required),
            "web parity audit should mention {required}"
        );
    }
}

#[test]
fn readme_release_and_testing_docs_link_web_parity_audit() {
    for (name, doc) in [
        ("README", include_str!("../README.md")),
        (
            "architecture overview",
            include_str!("../docs/architecture.md"),
        ),
        (
            "known limitations",
            include_str!("../docs/known_limitations.md"),
        ),
        (
            "manual QA checklist",
            include_str!("../docs/manual_qa_checklist.md"),
        ),
        (
            "release checklist",
            include_str!("../docs/release_checklist.md"),
        ),
        (
            "release workflow",
            include_str!("../docs/release_workflow.md"),
        ),
        (
            "UI testing workflow",
            include_str!("../docs/ui_testing_workflow.md"),
        ),
    ] {
        assert!(
            doc.contains("docs/web_parity_audit.md"),
            "{name} should link the web parity audit"
        );
    }
}

#[test]
fn add_ui_control_doc_covers_action_and_test_workflow() {
    let doc = include_str!("../docs/add_ui_control.md");

    for required in [
        "# Adding A UI Control Or Action",
        "src/ui/native/top_bar.rs",
        "src/ui/native/browser.rs",
        "src/ui/native/editor_panels.rs",
        "src/ui/native/control_panel.rs",
        "src/ui/native/devices.rs",
        "src/ui/native/controls.rs",
        "add_button_at",
        "add_toggle_button_at",
        "add_selectable_at",
        "src/ui/actions.rs",
        "dispatch_action",
        "canonical_action_name",
        "handle_key",
        "docs/keyboard_shortcuts.md",
        "src/ui/accessibility.rs",
        "src/ui/native/tests.rs",
    ] {
        assert!(
            doc.contains(required),
            "add UI control doc should mention {required}"
        );
    }
}

#[test]
fn add_project_command_doc_covers_mutation_dirty_undo_and_persistence() {
    let doc = include_str!("../docs/add_project_command.md");

    for required in [
        "# Adding A Project Command",
        "MusicProject",
        "src/project.rs",
        "AppState",
        "src/app.rs",
        "push_project_history",
        "push_project_history_attempt",
        "discard_project_history_attempt",
        "mark_project_dirty",
        "autosave",
        "undo/redo",
        "ScaleState::note_info",
        "ClipNote",
        "ProjectSnapshot",
        "ProjectFile",
        "docs/file_formats.md",
        "src/ui/native/tests.rs",
    ] {
        assert!(
            doc.contains(required),
            "add project command doc should mention {required}"
        );
    }
}

#[test]
fn readme_architecture_and_operad_link_command_pattern_docs() {
    let readme = include_str!("../README.md");
    let architecture = include_str!("../docs/architecture.md");
    let operad = include_str!("../docs/operad_integration.md");

    for doc_path in ["docs/add_ui_control.md", "docs/add_project_command.md"] {
        assert!(readme.contains(doc_path), "README should link {doc_path}");
        assert!(
            architecture.contains(doc_path),
            "architecture overview should link {doc_path}"
        );
        assert!(
            operad.contains(doc_path),
            "Operad integration guide should link {doc_path}"
        );
    }
}

#[test]
fn troubleshooting_doc_covers_audio_midi_and_diagnostics() {
    let doc = include_str!("../docs/troubleshooting.md");

    for required in [
        "# Troubleshooting",
        "SETUP REQUIRED",
        "cargo run -- --startup-probe",
        "RUST_LOG=info cargo run -- --startup-probe",
        "orbifold::alsa=trace",
        "orbifold::jack=trace",
        "AUDIO OUTPUTS",
        "MIDI INPUTS",
        "Ch All",
        "orbifold_settings.txt",
        "orbifold_autosave.orbifold",
        "docs/file_formats.md",
    ] {
        assert!(
            doc.contains(required),
            "troubleshooting doc should mention {required}"
        );
    }
}

#[test]
fn readme_links_troubleshooting_doc() {
    let readme = include_str!("../README.md");

    assert!(
        readme.contains("docs/troubleshooting.md"),
        "README should link the troubleshooting guide"
    );
}

#[test]
fn lumatone_setup_doc_covers_current_workflow_and_limits() {
    let doc = include_str!("../docs/lumatone_setup.md");

    for required in [
        "# Lumatone Setup",
        "Scale Versus Key Map",
        "lumatone_factory_presets/",
        "8. 31 EDO.ltn",
        "Keys",
        "Loaded key map:",
        "Key map inactive",
        "Ch All",
        "Mapping Capture",
        "Capture",
        "Stop",
        "Clear",
        "does not yet save",
        "docs/troubleshooting.md",
    ] {
        assert!(
            doc.contains(required),
            "Lumatone setup doc should mention {required}"
        );
    }
}

#[test]
fn readme_links_lumatone_setup_doc() {
    let readme = include_str!("../README.md");

    assert!(
        readme.contains("docs/lumatone_setup.md"),
        "README should link the Lumatone setup guide"
    );
}

#[test]
fn lumatone_troubleshooting_doc_covers_manual_validation() {
    let doc = include_str!("../docs/lumatone_troubleshooting.md");

    for required in [
        "# Lumatone Troubleshooting And Manual Validation",
        "automatically validate",
        "Ch All",
        "Loaded key map:",
        "8. 31 EDO.ltn",
        "Normal MIDI keyboards and Lumatone input are treated differently",
        "Mapping Capture",
        "No Key/Chan pairs found in key map",
        "Invalid MIDI channel",
        "Invalid MIDI note",
        "RUST_LOG=info cargo run -- --startup-probe",
        "docs/troubleshooting.md",
    ] {
        assert!(
            doc.contains(required),
            "Lumatone troubleshooting doc should mention {required}"
        );
    }
}

#[test]
fn readme_and_first_run_link_lumatone_troubleshooting_doc() {
    let readme = include_str!("../README.md");
    let first_run = include_str!("../docs/first_run.md");
    let setup = include_str!("../docs/lumatone_setup.md");

    for (name, doc) in [
        ("README", readme),
        ("first run guide", first_run),
        ("Lumatone setup guide", setup),
    ] {
        assert!(
            doc.contains("docs/lumatone_troubleshooting.md"),
            "{name} should link the Lumatone troubleshooting guide"
        );
    }
}

#[test]
fn asset_browser_doc_covers_import_workflow_and_limits() {
    let doc = include_str!("../docs/asset_browser.md");

    for required in [
        "# Asset Browser",
        "audio_assets/samples/",
        "audio_assets/instruments/",
        "audio_assets/presets/",
        "audio_assets/impulses/",
        "wav",
        "sfz",
        "ron",
        "Import",
        "kick_2.wav",
        "Imported sample as kick_2.wav",
        "visible `Clear` button",
        "WAV samples can be previewed",
        "Only WAV samples can be previewed",
        "WAV required for preview/use",
        "project sample instrument",
        "do not package or relink",
        "docs/asset_to_sound.md",
    ] {
        assert!(
            doc.contains(required),
            "asset browser doc should mention {required}"
        );
    }
}

#[test]
fn readme_links_asset_browser_doc() {
    let readme = include_str!("../README.md");

    assert!(
        readme.contains("docs/asset_browser.md"),
        "README should link the asset browser guide"
    );
}

#[test]
fn asset_to_sound_doc_covers_current_sound_workflow_and_limits() {
    let doc = include_str!("../docs/asset_to_sound.md");

    for required in [
        "# Asset-To-Sound Workflows",
        "What Produces Sound Today",
        "built-in synth",
        "audio_assets/",
        "library only",
        "WAV preview and project sample instrument available",
        "project sample instrument",
        "selected sample path is saved",
        "Non-WAV sample files can be imported and selected",
        "instrument playback is not available yet",
        "synth preset loading is not available yet",
        "effects or convolution loading is not available",
        "no instrument playback yet",
        "Target Workflow Not Yet Implemented",
        "docs/asset_browser.md",
    ] {
        assert!(
            doc.contains(required),
            "asset-to-sound doc should mention {required}"
        );
    }
}

#[test]
fn readme_links_asset_to_sound_doc() {
    let readme = include_str!("../README.md");

    assert!(
        readme.contains("docs/asset_to_sound.md"),
        "README should link the asset-to-sound guide"
    );
}

#[test]
fn known_limitations_doc_covers_current_product_limits() {
    let doc = include_str!("../docs/known_limitations.md");

    for required in [
        "# Known Limitations",
        "single current clip",
        "arrangement model is not implemented yet",
        "audio_assets/",
        "WAV samples can be previewed",
        "not yet assignable",
        "does not validate",
        "does not program Lumatone hardware",
        "packaging or relink workflow",
        "release packaging",
        "cargo run -- --screenshot-size=1200x760",
        "docs/troubleshooting.md",
    ] {
        assert!(
            doc.contains(required),
            "known limitations doc should mention {required}"
        );
    }
}

#[test]
fn readme_links_known_limitations_doc() {
    let readme = include_str!("../README.md");

    assert!(
        readme.contains("docs/known_limitations.md"),
        "README should link the known limitations guide"
    );
}

#[test]
fn readme_and_architecture_link_release_workflow_doc() {
    let readme = include_str!("../README.md");
    let architecture = include_str!("../docs/architecture.md");

    for (name, doc) in [("README", readme), ("architecture overview", architecture)] {
        assert!(
            doc.contains("docs/release_workflow.md"),
            "{name} should link the release workflow"
        );
    }
}

#[test]
fn keyboard_shortcuts_doc_includes_workflow_examples() {
    let doc = include_str!("../docs/keyboard_shortcuts.md");

    for required in [
        "Workflow Examples",
        "Record A Phrase",
        "Add And Shape One Note",
        "Recover From An Edit",
        "Copy Material At The Playhead",
        "Make The View Readable",
        "Save Or Open Without Losing Work",
        "Arrow-key note edits may repeat",
        "`Alt` is held",
        "Ctrl`/`Cmd+Wheel",
        "Shift+Wheel",
        "Alt+Wheel",
        "whole-clip quantize",
    ] {
        assert!(
            doc.contains(required),
            "keyboard shortcuts doc should mention {required}"
        );
    }
}
