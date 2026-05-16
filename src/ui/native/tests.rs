use super::*;
use crate::app::{AudioAssetItem, AudioAssetKind, ScaleLibraryItem};
use crate::audio::{AudioOutputDevice, AudioStreamInfo};
use crate::settings::AppSettings;
use crate::synth::{SynthSettings, Waveform};
use operad::{AccessibilityRole, PaintKind, PaintList};
use std::path::PathBuf;

const TEXT_OVERLAP_TOLERANCE: f32 = 1.0;

#[derive(Clone, Debug)]
struct TextBox {
    source: String,
    text: String,
    allocated: UiRect,
    visible: UiRect,
}

#[test]
fn initial_window_size_uses_monitor_space_without_exceeding_caps() {
    let four_k = initial_window_size_for_monitor(PhysicalSize::new(3840, 2160), 1.0);
    assert_eq!(four_k.width, 3200.0);
    assert_eq!(four_k.height, 1900.0);

    let scaled_four_k = initial_window_size_for_monitor(PhysicalSize::new(3840, 2160), 2.0);
    assert!((scaled_four_k.width - 1728.0).abs() < f64::EPSILON);
    assert!((scaled_four_k.height - 950.4).abs() < f64::EPSILON);

    let small = initial_window_size_for_monitor(PhysicalSize::new(1280, 720), 1.0);
    assert_eq!(small.width, 1400.0);
    assert_eq!(small.height, 760.0);
}

#[test]
fn window_title_reports_project_name_and_dirty_state() {
    let mut app = AppState::for_layout_tests();

    assert_eq!(window_title_for_app(&app), "Orbifold");

    app.project_dirty = true;
    assert_eq!(window_title_for_app(&app), "Orbifold - Untitled *");

    app.project_path = Some(PathBuf::from("sessions/float.orbifold"));
    app.project_dirty = false;
    assert_eq!(window_title_for_app(&app), "Orbifold - float");

    app.project_dirty = true;
    assert_eq!(window_title_for_app(&app), "Orbifold - float *");
}

#[test]
fn ui_scale_combines_display_density_with_user_preference() {
    let four_k = PhysicalSize::new(3840, 2160);
    assert_eq!(ui_scale_for_values(1.0, four_k, 1.0), 2.0);
    assert!((ui_scale_for_values(1.0, four_k, 1.2) - 2.4).abs() < 0.0001);
    assert_eq!(ui_scale_for_values(2.0, four_k, 1.0), 2.0);
    assert!((ui_scale_for_values(2.0, four_k, 2.0) - 2.8421052).abs() < 0.0001);
}

#[test]
fn ui_scale_preserves_minimum_layout_space() {
    let minimum = PhysicalSize::new(1200, 760);
    assert_eq!(ui_scale_for_values(2.0, minimum, 1.0), 1.0);
    assert_eq!(
        logical_size_for_window(minimum, 1.0),
        UiSize::new(1200.0, 760.0)
    );

    let wide_but_short = PhysicalSize::new(1920, 760);
    assert_eq!(ui_scale_for_values(1.0, wide_but_short, 2.0), 1.0);

    let roomy = PhysicalSize::new(2400, 1520);
    assert_eq!(ui_scale_for_values(1.0, roomy, 2.0), 2.0);
}

#[test]
fn zoom_out_expands_logical_surface_and_pointer_coordinates() {
    let minimum = PhysicalSize::new(1200, 760);
    let logical = logical_size_for_window(minimum, 0.75);
    assert!((logical.width - 1600.0).abs() < 0.001);
    assert!((logical.height - 1013.3333).abs() < 0.001);

    assert_eq!(
        point_from_position(PhysicalPosition::new(600.0, 300.0), 0.75),
        UiPoint::new(800.0, 400.0)
    );
}

#[test]
fn idle_redraw_only_runs_while_transport_is_playing() {
    let app = AppState::for_layout_tests();

    assert!(!should_redraw_when_idle(&app));

    app.music_project.lock().transport.playing = true;

    assert!(should_redraw_when_idle(&app));
}

#[test]
fn screenshot_physical_size_prefers_requested_size() {
    assert_eq!(
        screenshot_physical_size(Some((2560.0, 1440.0)), PhysicalSize::new(1920, 1045)),
        PhysicalSize::new(2560, 1440)
    );
    assert_eq!(
        screenshot_physical_size(None, PhysicalSize::new(1920, 1045)),
        PhysicalSize::new(1920, 1045)
    );
    assert_eq!(
        screenshot_physical_size(Some((1200.4, 759.6)), PhysicalSize::new(1920, 1045)),
        PhysicalSize::new(1200, 760)
    );
}

#[test]
fn requested_screenshot_size_uses_density_not_host_dpi() {
    assert_eq!(
        screenshot_ui_scale_for_values(
            2.0,
            Some((1200.0, 760.0)),
            PhysicalSize::new(1200, 760),
            1.0,
        ),
        1.0
    );
    assert_eq!(
        screenshot_ui_scale_for_values(
            2.0,
            Some((3840.0, 2160.0)),
            PhysicalSize::new(3840, 2160),
            1.0,
        ),
        2.0
    );
    assert_eq!(
        screenshot_ui_scale_for_values(2.0, None, PhysicalSize::new(1200, 760), 1.0),
        1.0
    );
}

#[test]
fn screenshot_pixel_validation_accepts_full_surface_content() {
    let mut pixels = solid_test_pixels(100, 60, [8, 12, 18, 255]);
    fill_test_rect(&mut pixels, 100, 0, 0, 100, 8, [13, 20, 29, 255]);
    fill_test_rect(&mut pixels, 100, 0, 54, 100, 6, [14, 24, 34, 255]);
    fill_test_rect(&mut pixels, 100, 0, 0, 6, 60, [70, 86, 101, 255]);
    fill_test_rect(&mut pixels, 100, 94, 0, 6, 60, [70, 86, 101, 255]);

    assert!(validate_screenshot_pixels(100, 60, &pixels, color(8, 12, 18)).is_ok());
}

#[test]
fn screenshot_pixel_validation_rejects_blank_image() {
    let pixels = solid_test_pixels(100, 60, [8, 12, 18, 255]);

    let error = validate_screenshot_pixels(100, 60, &pixels, color(8, 12, 18))
        .expect_err("blank image should fail");

    assert!(error.contains("blank"));
}

#[test]
fn screenshot_pixel_validation_rejects_corner_only_content() {
    let mut pixels = solid_test_pixels(100, 60, [8, 12, 18, 255]);
    fill_test_rect(&mut pixels, 100, 0, 0, 60, 35, [70, 86, 101, 255]);

    let error = validate_screenshot_pixels(100, 60, &pixels, color(8, 12, 18))
        .expect_err("corner-only content should fail");

    assert!(error.contains("cropped"));
}

#[test]
fn piano_pitch_label_step_preserves_readable_spacing() {
    assert_eq!(piano_pitch_label_step(25, 6.0), 4);
    assert_eq!(piano_pitch_label_step(25, 24.0), 2);
    assert_eq!(piano_pitch_label_step(8, 24.0), 1);
}

#[test]
fn piano_pitch_grid_line_step_thins_dense_rows() {
    assert_eq!(piano_pitch_grid_line_step(24.0), 1);
    assert_eq!(piano_pitch_grid_line_step(8.0), 1);
    assert_eq!(piano_pitch_grid_line_step(4.0), 2);
}

#[test]
fn midi_note_name_uses_standard_octave_labels() {
    assert_eq!(midi_note_name(60), "C4");
    assert_eq!(midi_note_name(69), "A4");
    assert_eq!(midi_note_name(70), "A#4");
    assert_eq!(midi_note_name(0), "C-1");
}

#[test]
fn piano_pitch_label_uses_note_names_for_12_tet() {
    let app = AppState::for_layout_tests();

    assert_eq!(pitch_label(&app, 60), "C4");
    assert_eq!(pitch_label(&app, 69), "A4");
}

#[test]
fn piano_pitch_label_uses_degree_and_cents_for_microtonal_scales() {
    let mut app = AppState::for_layout_tests();
    app.load_scale_path(PathBuf::from("scales/31-edo.scl"), true)
        .expect("bundled 31-EDO scale should load");

    assert_eq!(pitch_label(&app, 69), "d1 +0c");
    assert_eq!(pitch_label(&app, 100), "d1 +1200c");
}

#[test]
fn microtonal_piano_pitch_labels_include_cents_and_fit_supported_viewports() {
    let mut app = populated_layout_test_app();
    app.load_scale_path(PathBuf::from("scales/31-edo.scl"), true)
        .expect("bundled 31-EDO scale should load");

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(
        text.iter()
            .any(|item| item.text.starts_with('d') && item.text.ends_with('c'))
    );
    assert_text_overlap_free("minimum-microtonal-pitch-labels", &text);
}

#[test]
fn midi_event_label_prioritizes_note_name_and_channel() {
    let event = crate::midi::MidiEvent {
        raw_status: 0x90,
        status: 0x90,
        channel: 0,
        midi_note: 60,
        velocity: 96,
        key_index: 60,
        musical_note: 60,
        mapped_from_lumatone: false,
        freq: Some(261.63),
        scale_degree: Some(0),
        scale_octave: Some(0),
        cents_from_root: Some(0.0),
        at: std::time::Instant::now(),
    };

    assert_eq!(
        midi_event_label(&event),
        "Last MIDI ch1 note C4 (60) vel96 status 90 D0 O0 +0c"
    );
}

#[test]
fn midi_event_label_reports_control_changes_without_fake_note_names() {
    let event = crate::midi::MidiEvent {
        raw_status: 0xB0,
        status: 0xB0,
        channel: 1,
        midi_note: 64,
        velocity: 127,
        key_index: 64,
        musical_note: 64,
        mapped_from_lumatone: false,
        freq: None,
        scale_degree: None,
        scale_octave: None,
        cents_from_root: None,
        at: std::time::Instant::now(),
    };

    assert_eq!(
        midi_event_label(&event),
        "Last MIDI ch2 sustain on value127 status B0"
    );
}

#[test]
fn midi_event_label_reports_pitch_bend_policy() {
    let event = crate::midi::MidiEvent {
        raw_status: 0xE2,
        status: 0xE0,
        channel: 2,
        midi_note: 0,
        velocity: 64,
        key_index: 0,
        musical_note: 0,
        mapped_from_lumatone: false,
        freq: None,
        scale_degree: None,
        scale_octave: None,
        cents_from_root: None,
        at: std::time::Instant::now(),
    };

    assert_eq!(midi_event_label(&event), "Last MIDI ch3 bend +0 status E2");
}

#[test]
fn quantize_grid_lines_skip_unreadably_dense_subdivisions() {
    assert_eq!(
        visible_quantize_grid_step(16.0, 1600.0, QuantizeGrid::Sixteenth),
        Some(0.25)
    );
    assert_eq!(
        visible_quantize_grid_step(16.0, 640.0, QuantizeGrid::ThirtySecond),
        Some(0.25)
    );
    assert_eq!(
        visible_quantize_grid_step(16.0, 64.0, QuantizeGrid::Sixteenth),
        None
    );
    assert_eq!(
        visible_quantize_grid_step(16.0, 1600.0, QuantizeGrid::Quarter),
        None
    );
}

#[test]
fn piano_roll_claims_bottom_track_editor_space_at_minimum_size() {
    let app = AppState::for_layout_tests();
    let layout = surface_rects(&app, 1200.0, 760.0);
    let body = body_rects(1200.0, 760.0, 62.0, 26.0);

    assert_eq!(layout.piano_roll.x, body.left.x);
    assert_eq!(layout.piano_options.x, layout.piano_roll.x + 10.0);
    assert!(layout.piano_keyboard.x > layout.piano_options.right());
    assert!(layout.piano_grid.x > layout.piano_keyboard.x);
    assert!(layout.piano_roll.width > body.center.width);
    assert!(layout.piano_grid.width > body.center.width);
    assert!((layout.piano_roll.right() - (1200.0 - 8.0)).abs() < f32::EPSILON);
    assert!(layout.piano_roll.y >= body.center.bottom());
    assert!(layout.piano_roll.bottom() <= 760.0 - 26.0);
}

#[test]
fn fit_label_keeps_visible_text_inside_available_width() {
    assert_eq!(fit_label("Save Settings", 88.0, 12.0), "Save Settings");
    assert_eq!(fit_label("iii", 12.0, 12.0), "iii");

    let compact = fit_label("Connect Audio", 48.0, 12.0);
    assert!(compact.ends_with("..."));
    assert!(estimated_text_width(&compact, 12.0) <= 48.0 + 0.001);

    let tiny = fit_label("Wide", 5.0, 12.0);
    assert!(estimated_text_width(&tiny, 12.0) <= 5.0 + 0.001);
}

#[test]
fn asset_tab_labels_fit_minimum_browser_width() {
    let app = populated_layout_test_app();

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    for label in ["Samp", "Instr", "Pres", "IRs"] {
        assert!(text.iter().any(|item| item.text == label));
    }
    assert!(!text.iter().any(|item| item.text == "Sam..."));
    assert!(!text.iter().any(|item| item.text == "Pres..."));
}

#[test]
fn device_control_buttons_fit_minimum_panel_width() {
    let panel = UiRect::new(948.0, 70.0, 240.0, 650.0);
    let row = device_control_rects(panel, 600.0);
    assert!(row.prev.x >= panel.x + 12.0);
    assert!(row.connect.right() <= panel.right() - 12.0 + f32::EPSILON);
    assert!(row.refresh.width >= 0.0);
    assert!(row.connect.width >= 0.0);
}

#[test]
fn capture_control_buttons_keep_minimum_panel_margin() {
    let panel = UiRect::new(948.0, 70.0, 240.0, 650.0);
    let row = capture_control_rects(panel, 194.0);
    assert!(row.capture.x >= panel.x + 16.0);
    assert!(row.maps.right() <= panel.right() - 16.0 + f32::EPSILON);
    assert!(row.capture.width > row.stop.width);
    assert!(row.stop.width > 0.0);
}

#[test]
fn capture_action_state_only_enables_meaningful_actions() {
    assert_eq!(
        capture_action_state(false, 0),
        CaptureActionState {
            start_enabled: true,
            stop_enabled: false,
            clear_enabled: false,
        }
    );
    assert_eq!(
        capture_action_state(true, 0),
        CaptureActionState {
            start_enabled: false,
            stop_enabled: true,
            clear_enabled: false,
        }
    );
    assert_eq!(
        capture_action_state(false, 3),
        CaptureActionState {
            start_enabled: true,
            stop_enabled: false,
            clear_enabled: true,
        }
    );
}

#[test]
fn lumatone_map_label_reports_selected_map_name() {
    let mut app = AppState::for_layout_tests();

    assert_eq!(lumatone_map_label(&app), "Key map none");

    assert!(app.load_lumatone_path(PathBuf::from(
        "lumatone_factory_presets/1. Classic Mode.ltn"
    )));

    let label = lumatone_map_label(&app);
    assert!(label.starts_with("Key map 1. Classic Mode ("));
    assert!(label.ends_with(" keys)"));
    assert!(!label.contains("key0"));

    let missing = std::env::temp_dir().join(format!(
        "orbifold_missing_lumatone_label_{}.ltn",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&missing);
    app.lumatone_path = Some(missing);

    let label = lumatone_map_label(&app);
    assert!(label.starts_with("Key map missing orbifold_missing_lu"));
    assert!(label.ends_with(" keys)"));
}

#[test]
fn clip_panel_summary_reports_empty_clip_state() {
    let app = AppState::for_layout_tests();

    assert_eq!(
        clip_panel_summary(&app),
        ClipPanelSummary {
            note_total: 0,
            note_count: "0 notes".to_string(),
            loop_and_grid: "16 beats  Grid 1/16".to_string(),
            selected_note: None,
        }
    );
}

#[test]
fn clip_panel_summary_reports_selected_note_state() {
    let mut app = AppState::for_layout_tests();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);

    let summary = clip_panel_summary(&app);

    assert_eq!(summary.note_count, "1 note");
    assert_eq!(summary.loop_and_grid, "16 beats  Grid 1/16");
    assert_eq!(
        summary.selected_note,
        Some("Sel d1 o0 440.0Hz +0c b2.00 l1.00 v96".to_string())
    );
}

#[test]
fn stale_selected_note_does_not_enable_clip_note_toolbar() {
    let mut app = AppState::for_layout_tests();
    app.selected_clip_note = Some(99_999);

    for name in [
        "clip.delete_note",
        "clip.duplicate_note",
        "clip.nudge_left",
        "clip.nudge_right",
        "clip.pitch_down",
        "clip.pitch_up",
    ] {
        assert!(
            !surface_node_enabled(&app, name),
            "{name} should be disabled"
        );
    }
}

#[test]
fn operad_surface_text_does_not_overlap_supported_viewports() {
    let app = populated_layout_test_app();
    for (name, width, height) in [
        ("minimum", 1200.0, 760.0),
        ("compact-threshold-low", 1319.0, 760.0),
        ("compact-threshold-high", 1340.0, 760.0),
        ("default", 1400.0, 760.0),
        ("wide", 1920.0, 1080.0),
        ("large-monitor-logical", 2560.0, 1440.0),
    ] {
        let text = collect_surface_text_boxes(&app, width, height);
        assert_text_overlap_free(name, &text);
        assert_text_allocations_are_finite(name, &text);
    }
}

#[test]
fn operad_surface_text_does_not_overlap_with_long_runtime_names() {
    let mut app = populated_layout_test_app();
    app.connected_audio_output =
        "USB Interface 12 Channel Output With Long Device Name".to_string();
    app.midi_inputs = vec![
        "Lumatone Isomorphic Keyboard Long Virtual MIDI Port 14:0".to_string(),
        "Midi Through:Midi Through Port-0 14:0".to_string(),
    ];
    app.audio_outputs = vec![AudioOutputDevice {
        name: app.connected_audio_output.clone(),
        is_default: true,
    }];
    app.audio_assets.push(AudioAssetItem {
        name: "Very Long Granular Texture Folder Name That Should Clip".to_string(),
        path: PathBuf::from("textures"),
        kind: AudioAssetKind::Sample,
        is_dir: true,
    });
    app.last_status =
        "Connected MIDI input: Lumatone Isomorphic Keyboard Long Virtual MIDI Port 14:0"
            .to_string();

    for (name, width, height) in [
        ("minimum-long-names", 1200.0, 760.0),
        ("default-long-names", 1400.0, 760.0),
        ("wide-long-names", 1920.0, 1080.0),
    ] {
        let text = collect_surface_text_boxes(&app, width, height);
        assert_text_overlap_free(name, &text);
    }
}

#[test]
fn large_clip_surface_layout_stays_bounded() {
    let app = populated_layout_test_app();
    let root = app.scale_state.lock().root_midi;
    {
        let mut project = app.music_project.lock();
        project.transport.loop_beats = 64.0;
        project.transport.quantize_grid = QuantizeGrid::ThirtySecond;
        project.clip.notes = (0..512)
            .map(|index| {
                let pitch_offset = index as i32 % 31 - 15;
                ClipNote {
                    id: index + 1,
                    start_beats: (index % 256) as f32 * 0.25,
                    duration_beats: 0.25 + (index % 4) as f32 * 0.125,
                    key_index: pitch_offset,
                    musical_note: root + pitch_offset,
                    raw_channel: (index % 16) as u8,
                    raw_note: (48 + index % 48) as u8,
                    velocity: (64 + index % 48) as u8,
                    freq: 440.0,
                    mapped_from_lumatone: false,
                }
            })
            .collect();
    }

    let mut document = build_surface_document(&app, 1920.0, 1080.0);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(UiSize::new(1920.0, 1080.0), &mut text_measurer)
        .expect("large clip surface layout should compute");
    let paint = document.paint_list();
    let paint_items = paint_item_count(&paint);

    assert!(
        document.nodes().len() < 6_000,
        "large clip surface created too many UI nodes: {}",
        document.nodes().len()
    );
    assert!(
        paint_items < 5_000,
        "large clip surface created too many paint items: {paint_items}"
    );

    let mut text = Vec::new();
    collect_text_boxes_from_paint(&document, &paint, &mut text);
    assert_text_allocations_are_finite("large-clip", &text);
}

#[test]
fn operad_new_project_discard_confirmation_fits_minimum_layout() {
    let mut app = populated_layout_test_app();
    app.add_clip_note_at(0.0, 69);
    app.start_new_project();

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(text.iter().any(|item| item.text == "Discard?"));
    assert_text_overlap_free("minimum-new-project-confirm", &text);
}

#[test]
fn operad_open_project_discard_confirmation_fits_minimum_layout() {
    let mut app = populated_layout_test_app();
    app.add_clip_note_at(0.0, 69);
    assert!(!app.request_open_project());

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(text.iter().any(|item| item.text == "Discard?"));
    assert_text_overlap_free("minimum-open-project-confirm", &text);
}

#[test]
fn operad_autosave_recover_action_fits_minimum_layout() {
    let mut app = populated_layout_test_app();
    app.autosave_available = true;

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(text.iter().any(|item| item.text == "Recover"));
    assert_text_overlap_free("minimum-autosave-recover", &text);
}

#[test]
fn operad_autosave_and_open_recent_actions_coexist_at_minimum_layout() {
    let mut app = AppState::for_layout_tests();
    let recent = std::env::temp_dir().join(format!(
        "orbifold_recover_recent_surface_{}.orbifold",
        std::process::id()
    ));
    let backup = recent.with_file_name(format!(
        "{}.bak",
        recent
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("orbifold_recover_recent_surface.orbifold")
    ));
    let _ = std::fs::remove_file(&recent);
    let _ = std::fs::remove_file(&backup);
    app.save_project_to_path(recent.clone());
    app.start_new_project();
    app.autosave_available = true;

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(surface_node_enabled(&app, "file.recover"));
    assert!(surface_node_enabled(&app, "file.dismiss_autosave"));
    assert!(surface_node_enabled(&app, "file.open_recent"));
    assert!(text.iter().any(|item| item.text == "Recover"));
    assert!(text.iter().any(|item| item.text == "Dismiss"));
    assert!(text.iter().any(|item| item.text == "Recent"));
    assert_text_overlap_free("minimum-recover-open-recent", &text);

    app.add_clip_note_at(0.0, 69);

    assert!(!surface_node_enabled(&app, "file.recover"));
    assert!(!surface_node_enabled(&app, "file.dismiss_autosave"));
    assert!(!surface_node_enabled(&app, "file.open_recent"));

    let _ = std::fs::remove_file(recent);
    let _ = std::fs::remove_file(backup);
}

#[test]
fn operad_metronome_toggle_fits_minimum_layout() {
    let app = populated_layout_test_app();

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(text.iter().any(|item| item.text == "Metronome"));
    assert!(text.iter().any(|item| item.text == "Off"));
    assert_text_overlap_free("minimum-metronome-toggle", &text);
}

#[test]
fn operad_midi_channel_filter_fits_minimum_layout() {
    let mut app = populated_layout_test_app();
    app.cycle_midi_channel_filter();

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(text.iter().any(|item| item.text == "Ch 1"));
    assert_text_overlap_free("minimum-midi-channel-filter", &text);
}

#[test]
fn operad_record_quantize_toggle_fits_minimum_layout() {
    let app = populated_layout_test_app();

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(text.iter().any(|item| item.text == "Rec quantize"));
    assert_text_overlap_free("minimum-record-quantize-toggle", &text);
}

#[test]
fn operad_audio_mute_toggle_is_visible_when_control_panel_has_room() {
    let mut app = populated_layout_test_app();
    app.toggle_audio_mute();

    let text = collect_surface_text_boxes(&app, 1200.0, 1000.0);

    assert!(text.iter().any(|item| item.text == "Output muted"));
    assert!(text.iter().any(|item| item.text == "Muted"));
    assert_text_overlap_free("roomy-audio-mute-toggle", &text);
}

#[test]
fn operad_output_limiter_indicator_is_visible_when_control_panel_has_room() {
    let app = populated_layout_test_app();
    app.synth
        .set_settings(SynthSettings {
            master_gain: 1.0,
            attack_ms: 0.0,
            waveform: Waveform::Square,
            drive: 8.0,
            delay_mix: 0.0,
            ..SynthSettings::default()
        })
        .unwrap();
    let (mut engine, _receiver, _sender) = app.synth.make_engine(44_100.0);
    engine.handle_command(crate::synth::AudioCommand::NoteOn {
        note: 69,
        freq: 440.0,
        velocity: 1.0,
    });
    for _ in 0..512 {
        engine.next_sample();
        if app.synth.output_limited() {
            break;
        }
    }

    assert!(app.synth.output_level() > 0.0);
    assert!(app.synth.output_limited());

    let text = collect_surface_text_boxes(&app, 1200.0, 1000.0);

    assert!(text.iter().any(|item| item.text == "Output limit"));
    assert_text_overlap_free("roomy-output-limiter-indicator", &text);
}

#[test]
fn opened_saved_project_renders_loaded_clip_surface() {
    let mut source = AppState::for_layout_tests();
    let root_midi = source.scale_state.lock().root_midi;
    source.add_clip_note_at(1.0, root_midi);
    let path = std::env::temp_dir().join(format!(
        "orbifold_open_render_test_{}.orbifold",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    source.save_project_to_path(path.clone());

    let mut app = AppState::for_layout_tests();
    app.load_project_path(path.clone());

    assert_eq!(app.music_project.lock().clip.notes.len(), 1);
    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);
    assert!(text.iter().any(|item| item.text == "Current Clip"));
    assert!(text.iter().any(|item| item.text == "1 note"));
    assert!(!text.iter().any(|item| item.text == "No recorded clip"));
    assert!(!text.iter().any(|item| item.text == "No notes"));
    assert_text_overlap_free("opened-saved-project", &text);

    let _ = std::fs::remove_file(path);
}

#[test]
fn empty_piano_roll_does_not_overlay_grid_placeholder_text() {
    let mut app = AppState::for_layout_tests();

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(!text.iter().any(|item| item.text == "No notes"));
    assert_text_overlap_free("empty-piano-roll", &text);

    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(0.0, root);
    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(!text.iter().any(|item| item.text == "No notes"));
    assert_text_overlap_free("populated-piano-roll", &text);
}

#[test]
fn visible_transport_actions_update_project_state() {
    let mut app = AppState::for_layout_tests();

    dispatch_action(&mut app, "transport.bpm_up", None, None);
    dispatch_action(&mut app, "transport.loop_up", None, None);
    dispatch_action(&mut app, "transport.quantize_grid", None, None);
    dispatch_action(&mut app, "transport.loop", None, None);

    let project = app.music_project.lock();
    assert_eq!(project.transport.bpm, 121.0);
    assert_eq!(project.transport.loop_beats, 20.0);
    assert_eq!(project.transport.quantize_grid, QuantizeGrid::ThirtySecond);
    assert!(project.transport.overdub);
    assert!(app.project_dirty);
}

#[test]
fn top_bar_actions_report_expected_availability_without_audio() {
    let app = AppState::for_layout_tests();

    for name in [
        "file.open",
        "file.save",
        "file.save_as",
        "scale.open",
        "keymap.open",
        "transport.prev",
        "transport.play_stop",
        "transport.stop",
        "transport.record",
        "transport.loop",
        "transport.bpm_down",
        "transport.bpm_up",
        "transport.quantize_grid",
        "audio.all_off",
        "settings.save",
    ] {
        assert!(surface_node_enabled(&app, name), "{name} should be enabled");
    }

    assert!(!surface_node_enabled(&app, "edit.undo"));
    assert!(!surface_node_enabled(&app, "edit.redo"));
    assert!(!surface_node_enabled(&app, "audio.test_a4"));
}

#[test]
fn top_bar_undo_redo_availability_tracks_project_history() {
    let mut app = AppState::for_layout_tests();
    let root = app.scale_state.lock().root_midi;

    assert!(!surface_node_enabled(&app, "edit.undo"));
    assert!(!surface_node_enabled(&app, "edit.redo"));

    app.add_clip_note_at(0.0, root);
    assert!(surface_node_enabled(&app, "edit.undo"));
    assert!(!surface_node_enabled(&app, "edit.redo"));

    app.undo_project_edit();
    assert!(!surface_node_enabled(&app, "edit.undo"));
    assert!(surface_node_enabled(&app, "edit.redo"));
}

#[test]
fn visible_scale_and_synth_actions_update_state() {
    let mut app = AppState::for_layout_tests();

    dispatch_action(&mut app, "scale.root_up", None, None);
    dispatch_action(&mut app, "scale.base_down", None, None);
    dispatch_action(&mut app, "synth.waveform", None, None);
    dispatch_action(&mut app, "synth.gain_up", None, None);

    let scale = app.scale_state.lock();
    let synth = app.synth.settings();
    assert_eq!(scale.root_midi, 70);
    assert_eq!(scale.base_freq, 439.0);
    assert_eq!(synth.waveform, Waveform::Triangle);
    assert!((synth.master_gain - 0.40).abs() < f32::EPSILON);
    assert!(app.project_dirty);
}

#[test]
fn bounded_transport_and_scale_controls_do_not_dirty_at_limits() {
    let mut app = AppState::for_layout_tests();
    {
        let mut scale = app.scale_state.lock();
        scale.root_midi = 0;
        scale.base_freq = 8.0;
    }
    {
        let mut project = app.music_project.lock();
        project.transport.bpm = 320.0;
        project.transport.loop_beats = 128.0;
    }
    let path = std::env::temp_dir().join(format!(
        "orbifold_bounded_noop_test_{}.orbifold",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    app.save_project_to_path(path.clone());
    assert!(!app.project_dirty);

    for action in [
        "scale.root_down",
        "scale.base_down",
        "transport.bpm_up",
        "transport.loop_up",
    ] {
        dispatch_action(&mut app, action, None, None);
        assert!(!app.project_dirty, "{action} should not dirty at limit");
        assert!(
            app.last_status.contains("unchanged"),
            "{action} should report unchanged, got {}",
            app.last_status
        );
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn visible_ui_scale_actions_update_setting() {
    let mut app = AppState::for_layout_tests();

    dispatch_action(&mut app, "ui.scale_up", None, None);
    assert!((app.ui_scale() - 1.1).abs() < 0.0001);

    dispatch_action(&mut app, "ui.scale_down", None, None);
    assert!((app.ui_scale() - 1.0).abs() < 0.0001);

    dispatch_action(&mut app, "ui.scale_up", None, None);
    dispatch_action(&mut app, "ui.scale_reset", None, None);
    assert!((app.ui_scale() - 1.0).abs() < 0.0001);
}

#[test]
fn visible_synth_parameter_actions_update_state() {
    let mut app = AppState::for_layout_tests();

    dispatch_action(&mut app, "synth.attack_up", None, None);
    dispatch_action(&mut app, "synth.release_down", None, None);
    dispatch_action(&mut app, "synth.filter_down", None, None);
    dispatch_action(&mut app, "synth.delay_up", None, None);
    dispatch_action(&mut app, "synth.drive_up", None, None);

    let synth = app.synth.settings();
    assert_eq!(synth.attack_ms, 10.0);
    assert_eq!(synth.release_ms, 90.0);
    assert!((synth.filter_cutoff_hz - 16_200.0).abs() < 0.001);
    assert!((synth.delay_mix - 0.05).abs() < f32::EPSILON);
    assert!((synth.drive - 1.1).abs() < f32::EPSILON);
    assert!(app.project_dirty);
}

#[test]
fn synth_boundary_adjustment_does_not_dirty_clean_project() {
    let mut app = AppState::for_layout_tests();
    let mut settings = app.synth.settings();
    settings.master_gain = 1.0;
    app.synth.set_settings(settings).unwrap();
    let path = std::env::temp_dir().join(format!(
        "orbifold_synth_noop_test_{}.orbifold",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    app.save_project_to_path(path.clone());
    assert!(!app.project_dirty);

    dispatch_action(&mut app, "synth.gain_up", None, None);

    assert!(!app.project_dirty);
    assert_eq!(app.synth.settings().master_gain, 1.0);
    assert!(app.last_status.starts_with("Synth unchanged"));

    let _ = std::fs::remove_file(path);
}

#[test]
fn visible_synth_mute_action_updates_runtime_audio_state() {
    let mut app = AppState::for_layout_tests();

    dispatch_action(&mut app, "synth.mute", None, None);
    assert!(app.synth.muted());
    assert_eq!(app.last_status, "Audio muted");
    assert!(!app.project_dirty);

    dispatch_action(&mut app, "synth.mute", None, None);
    assert!(!app.synth.muted());
    assert_eq!(app.last_status, "Audio unmuted");
    assert!(!app.project_dirty);
}

#[test]
fn visible_clip_actions_update_project_state() {
    let mut app = AppState::for_layout_tests();
    let root_midi = app.scale_state.lock().root_midi;

    dispatch_action(&mut app, "clip.add_note", None, None);
    let note_id = app.selected_clip_note.expect("new note should be selected");
    dispatch_action(&mut app, "clip.pitch_up", None, None);
    dispatch_action(&mut app, "clip.velocity_up", None, None);

    let note = app
        .music_project
        .lock()
        .note_by_id(note_id)
        .expect("note should exist");
    assert_eq!(note.musical_note, root_midi + 1);
    assert_eq!(note.velocity, 104);

    dispatch_action(&mut app, "clip.delete_note", None, None);

    assert!(app.music_project.lock().clip.notes.is_empty());
    assert_eq!(app.selected_clip_note, None);
    assert!(app.project_dirty);
}

#[test]
fn full_width_clip_toolbar_uses_readable_action_labels() {
    let app = populated_layout_test_app();

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(text.iter().any(|item| item.text == "Pitch -"));
    assert!(text.iter().any(|item| item.text == "Pitch +"));
    assert!(text.iter().any(|item| item.text == "Quantize"));
    assert!(text.iter().any(|item| item.text == "Delete"));
    assert!(text.iter().any(|item| item.text == "Duplicate"));
    assert!(!text.iter().any(|item| item.text == "Del"));
    assert!(!text.iter().any(|item| item.text == "P-"));
    assert!(!text.iter().any(|item| item.text == "P+"));
    assert!(!text.iter().any(|item| item.text == "Clr"));
    assert_text_overlap_free("compact-clip-toolbar", &text);
}

#[test]
fn visible_file_save_new_and_edit_actions_update_project_state() {
    let mut app = AppState::for_layout_tests();
    let path = std::env::temp_dir().join(format!(
        "orbifold_dispatch_save_test_{}.orbifold",
        std::process::id()
    ));
    let backup_path = path.with_file_name(format!(
        "{}.bak",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("orbifold_dispatch_save_test.orbifold")
    ));
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&backup_path);

    app.save_project_to_path(path.clone());
    dispatch_action(&mut app, "clip.add_note", None, None);
    assert!(app.project_dirty);
    assert!(app.can_undo_project_edit());

    dispatch_action(&mut app, "edit.undo", None, None);
    assert!(app.music_project.lock().clip.notes.is_empty());
    assert!(app.can_redo_project_edit());

    dispatch_action(&mut app, "edit.redo", None, None);
    assert_eq!(app.music_project.lock().clip.notes.len(), 1);

    dispatch_action(&mut app, "file.save", None, None);
    assert!(!app.project_dirty);
    let saved = std::fs::read_to_string(&path).expect("saved project should exist");
    assert!(saved.contains("orbifold_project=1"));

    dispatch_action(&mut app, "clip.add_note", None, None);
    dispatch_action(&mut app, "file.new", None, None);
    assert!(app.new_project_confirm_pending());
    assert_eq!(app.music_project.lock().clip.notes.len(), 2);

    dispatch_action(&mut app, "file.new", None, None);
    assert!(!app.project_dirty);
    assert!(app.project_path.is_none());
    assert!(app.music_project.lock().clip.notes.is_empty());

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(backup_path);
}

#[test]
fn keymap_open_action_uses_pending_dialog_without_blocking() {
    let mut app = AppState::for_layout_tests();
    let path = PathBuf::from("lumatone_factory_presets/1. Classic Mode.ltn");

    dispatch_action(&mut app, "keymap.open", None, None);

    assert!(app.has_pending_file_dialog());
    assert_eq!(app.pending_file_dialog_label_for_tests(), Some("key map"));
    assert_eq!(app.last_status, "Opening key map dialog");

    app.complete_pending_file_dialog_for_tests(Some(path.clone()));
    app.poll_pending_file_dialog();

    assert!(!app.has_pending_file_dialog());
    assert_eq!(app.lumatone_path.as_ref(), Some(&path));
    assert!(app.lumatone_map.lock().is_some());
    assert!(app.project_dirty);
    assert!(app.last_status.starts_with("Loaded key map:"));
}

#[test]
fn cancelled_keymap_dialog_reports_cancel_without_dirtying_project() {
    let mut app = AppState::for_layout_tests();

    dispatch_action(&mut app, "keymap.open", None, None);
    app.complete_pending_file_dialog_for_tests(None);
    app.poll_pending_file_dialog();

    assert!(!app.has_pending_file_dialog());
    assert!(app.lumatone_path.is_none());
    assert!(!app.project_dirty);
    assert_eq!(app.last_status, "Key map open cancelled");
}

#[test]
fn visible_library_and_asset_actions_update_selection_state() {
    let mut app = AppState::for_layout_tests();
    app.scale_library = [("12-TET", "12.scl"), ("31-EDO", "31.scl")]
        .into_iter()
        .map(|(name, path)| ScaleLibraryItem {
            name: name.to_string(),
            path: PathBuf::from(path),
        })
        .collect();
    app.audio_assets = [
        ("Kick", AudioAssetKind::Sample),
        ("Pad", AudioAssetKind::Instrument),
        ("Hall", AudioAssetKind::Impulse),
    ]
    .into_iter()
    .map(|(name, kind)| AudioAssetItem {
        name: name.to_string(),
        path: PathBuf::from(name),
        kind,
        is_dir: false,
    })
    .collect();

    dispatch_action(&mut app, "scale.select.1", None, None);
    assert_eq!(app.selected_scale_library, 1);
    assert_eq!(app.last_status, "Selected scale: 31-EDO");

    dispatch_action(&mut app, "asset.kind.1", None, None);
    assert_eq!(app.selected_audio_asset_kind, AudioAssetKind::Instrument);
    assert_eq!(app.selected_audio_asset, Some(1));
    assert_eq!(app.last_status, "Showing Instruments");

    dispatch_action(&mut app, "asset.select.2", None, None);
    assert_eq!(app.selected_audio_asset, Some(2));
    assert_eq!(app.last_status, "Selected impulse: Hall");
}

#[test]
fn visible_library_refresh_actions_update_state() {
    let mut app = AppState::for_layout_tests();

    dispatch_action(&mut app, "scale.refresh", None, None);
    assert!(!app.scale_library.is_empty());
    assert!(app.last_status.contains("scale"));

    dispatch_action(&mut app, "scale.load_selected", None, None);
    assert!(app.scala_path.is_some());
    assert!(app.project_dirty);

    app.scale_library.insert(
        0,
        ScaleLibraryItem {
            name: "User Scale".to_string(),
            path: PathBuf::from("user-scale.scl"),
        },
    );
    app.selected_scale_library = 0;
    dispatch_action(&mut app, "scale.remove_selected", None, None);
    assert!(app.last_status.starts_with("Removed scale: "));

    dispatch_action(&mut app, "keymap.refresh", None, None);
    assert!(!app.lumatone_presets.is_empty());
    assert!(app.last_status.contains("key map"));

    dispatch_action(&mut app, "asset.refresh", None, None);
    assert!(app.last_status.contains("asset"));
}

#[test]
fn bundled_scale_remove_button_is_disabled() {
    let mut app = AppState::for_layout_tests();
    app.show_scale_browser = true;
    app.scale_library = vec![
        ScaleLibraryItem {
            name: "12-TET".to_string(),
            path: PathBuf::from("scales/12-tet.scl"),
        },
        ScaleLibraryItem {
            name: "User Scale".to_string(),
            path: PathBuf::from("user-scale.scl"),
        },
    ];
    app.selected_scale_library = 0;

    assert!(!surface_node_enabled(&app, "scale.remove_selected"));

    app.selected_scale_library = 1;

    assert!(surface_node_enabled(&app, "scale.remove_selected"));
}

#[test]
fn loaded_selected_scale_reports_loaded_button_state() {
    let mut app = AppState::for_layout_tests();
    app.show_scale_browser = true;
    let path = PathBuf::from("scales/31-edo.scl");
    app.scale_library = vec![ScaleLibraryItem {
        name: "31-EDO".to_string(),
        path: path.clone(),
    }];
    app.scala_path = Some(path);

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(!surface_node_enabled(&app, "scale.load_selected"));
    assert!(text.iter().any(|item| item.text == "Loaded"));
    assert_text_overlap_free("loaded-selected-scale", &text);
}

#[test]
fn missing_scale_library_row_is_marked_and_can_still_be_pruned_by_load() {
    let mut app = AppState::for_layout_tests();
    app.show_scale_browser = true;
    let missing = std::env::temp_dir().join(format!(
        "orbifold_missing_scale_row_{}.scl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&missing);
    app.scale_library = vec![ScaleLibraryItem {
        name: "User Scale".to_string(),
        path: missing.clone(),
    }];

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(surface_node_enabled(&app, "scale.load_selected"));
    assert!(surface_node_enabled(&app, "scale.remove_selected"));
    assert!(text.iter().any(|item| item.text.contains("Missing")));
    assert_text_overlap_free("missing-scale-row", &text);

    let action = click_surface_node(&mut app, "scale.load_selected", 1200.0, 760.0);

    assert_eq!(action.as_deref(), Some("scale.load_selected"));
    assert!(app.scale_library.is_empty());
    assert_eq!(app.last_status, "Removed missing scale: User Scale");
}

#[test]
fn selected_scale_remains_visible_in_long_library() {
    let mut app = AppState::for_layout_tests();
    app.show_scale_browser = true;
    app.scale_library = (0..18)
        .map(|idx| ScaleLibraryItem {
            name: format!("Scale {idx:02}"),
            path: PathBuf::from(format!("scale-{idx:02}.scl")),
        })
        .collect();
    app.selected_scale_library = 16;

    assert!(surface_node_exists(&app, "scale.select.16"));
    assert!(!surface_node_exists(&app, "scale.select.0"));
}

#[test]
fn selected_asset_remains_visible_in_long_kind_list() {
    let mut app = AppState::for_layout_tests();
    app.audio_assets = (0..20)
        .map(|idx| AudioAssetItem {
            name: format!("Sample {idx:02}"),
            path: PathBuf::from(format!("sample-{idx:02}.wav")),
            kind: AudioAssetKind::Sample,
            is_dir: false,
        })
        .collect();
    app.selected_audio_asset_kind = AudioAssetKind::Sample;
    app.selected_audio_asset = Some(18);

    assert!(surface_node_exists(&app, "asset.select.18"));
    assert!(!surface_node_exists(&app, "asset.select.0"));
}

#[test]
fn missing_audio_asset_row_is_marked_until_asset_refresh() {
    let mut app = AppState::for_layout_tests();
    let missing = std::env::temp_dir().join(format!(
        "orbifold_missing_audio_asset_row_{}.wav",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&missing);
    app.audio_assets = vec![AudioAssetItem {
        name: "Gone Kick".to_string(),
        path: missing,
        kind: AudioAssetKind::Sample,
        is_dir: false,
    }];

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(surface_node_enabled(&app, "asset.select.0"));
    assert!(text.iter().any(|item| item.text == "Missing Gone Kick"));
    assert_text_overlap_free("missing-audio-asset-row", &text);

    let action = click_surface_node(&mut app, "asset.select.0", 1200.0, 760.0);

    assert_eq!(action.as_deref(), Some("asset.select.0"));
    assert_eq!(app.selected_audio_asset, Some(0));
    assert_eq!(app.last_status, "Selected sample: Gone Kick");
}

#[test]
fn mouse_wheel_over_scale_panel_moves_selection() {
    let mut app = AppState::for_layout_tests();
    app.show_scale_browser = true;
    app.scale_library = (0..5)
        .map(|idx| ScaleLibraryItem {
            name: format!("Scale {idx}"),
            path: PathBuf::from(format!("scale-{idx}.scl")),
        })
        .collect();
    let layout = surface_rects(&app, 1200.0, 760.0);
    let scale_point = rect_center(left_browser_rects(&app, layout.left).scales);
    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);

    assert!(host.scroll_left_browser_list(scale_point, -24.0));
    assert_eq!(host.app.selected_scale_library, 1);
    assert_eq!(host.app.last_status, "Selected scale: Scale 1");

    for _ in 0..8 {
        host.scroll_left_browser_list(scale_point, -24.0);
    }
    assert_eq!(host.app.selected_scale_library, 4);

    assert!(host.scroll_left_browser_list(scale_point, 24.0));
    assert_eq!(host.app.selected_scale_library, 3);
}

#[test]
fn mouse_wheel_over_asset_panel_moves_selection_within_kind() {
    let mut app = AppState::for_layout_tests();
    app.audio_assets = [
        ("Kick", AudioAssetKind::Sample),
        ("Pad", AudioAssetKind::Instrument),
        ("Snare", AudioAssetKind::Sample),
    ]
    .into_iter()
    .map(|(name, kind)| AudioAssetItem {
        name: name.to_string(),
        path: PathBuf::from(name),
        kind,
        is_dir: false,
    })
    .collect();
    app.selected_audio_asset_kind = AudioAssetKind::Sample;
    app.selected_audio_asset = Some(0);
    let layout = surface_rects(&app, 1200.0, 760.0);
    let asset_point = rect_center(left_browser_rects(&app, layout.left).assets);
    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);

    assert!(host.scroll_left_browser_list(asset_point, -24.0));
    assert_eq!(host.app.selected_audio_asset, Some(2));
    assert_eq!(host.app.last_status, "Selected sample: Snare");

    assert!(host.scroll_left_browser_list(asset_point, 24.0));
    assert_eq!(host.app.selected_audio_asset, Some(0));
}

#[test]
fn visible_device_and_capture_actions_update_state() {
    let mut app = AppState::for_layout_tests();
    app.midi_inputs = vec!["Input A".to_string(), "Input B".to_string()];
    app.audio_outputs = vec![
        AudioOutputDevice {
            name: "Output A".to_string(),
            is_default: true,
        },
        AudioOutputDevice {
            name: "Output B".to_string(),
            is_default: false,
        },
    ];

    dispatch_action(&mut app, "midi.next", None, None);
    assert_eq!(app.selected_input, 1);
    assert_eq!(app.last_status, "Selected MIDI input: Input B");

    dispatch_action(&mut app, "midi.channel_filter", None, None);
    assert_eq!(app.midi_channel_filter(), Some(0));
    assert_eq!(app.last_status, "MIDI filter Ch 1");

    dispatch_action(&mut app, "audio.next", None, None);
    assert_eq!(app.selected_audio_output, 1);
    assert_eq!(app.last_status, "Selected audio output: Output B");

    dispatch_action(&mut app, "capture.start", None, None);
    assert!(app.midi_capture.lock().is_armed());

    dispatch_action(&mut app, "capture.stop", None, None);
    assert!(!app.midi_capture.lock().is_armed());

    dispatch_action(&mut app, "capture.clear", None, None);
    assert!(app.midi_capture.lock().events().is_empty());
}

#[test]
fn single_device_navigation_is_disabled_but_connect_remains_available() {
    let mut app = AppState::for_layout_tests();
    app.midi_inputs = vec!["Only Input".to_string()];
    app.audio_outputs = vec![AudioOutputDevice {
        name: "Only Output".to_string(),
        is_default: true,
    }];

    assert!(!surface_node_enabled(&app, "midi.prev"));
    assert!(!surface_node_enabled(&app, "midi.next"));
    assert!(surface_node_enabled(&app, "midi.connect"));
    assert!(!surface_node_enabled(&app, "audio.prev"));
    assert!(!surface_node_enabled(&app, "audio.next"));
    assert!(surface_node_enabled(&app, "audio.connect"));
}

#[test]
fn visible_canvas_actions_use_layout_coordinates() {
    let mut app = AppState::for_layout_tests();
    let layout = surface_rects(&app, 1200.0, 760.0);
    let seek_point = UiPoint::new(
        layout.arrangement_ruler.x + layout.arrangement_ruler.width * 0.5,
        layout.arrangement_ruler.y + layout.arrangement_ruler.height * 0.5,
    );

    dispatch_action(&mut app, "transport.seek", Some(seek_point), Some(layout));
    let position = app
        .music_project
        .lock()
        .current_position_beats(std::time::Instant::now());
    assert!((position - 8.0).abs() < 0.01);

    let piano_seek_point = UiPoint::new(
        layout.piano_ruler.x + layout.piano_ruler.width * 0.75,
        layout.piano_ruler.y + layout.piano_ruler.height * 0.5,
    );
    dispatch_action(&mut app, "piano.seek", Some(piano_seek_point), Some(layout));
    let position = app
        .music_project
        .lock()
        .current_position_beats(std::time::Instant::now());
    assert!((position - 12.0).abs() < 0.01);

    let note_point = UiPoint::new(
        layout.piano_grid.x + layout.piano_grid.width * 0.25,
        layout.piano_grid.y + layout.row_height() * 2.5,
    );
    dispatch_action(&mut app, "piano.grid", Some(note_point), Some(layout));
    let note = app
        .music_project
        .lock()
        .clip
        .notes
        .first()
        .cloned()
        .expect("piano grid click should add a note");
    assert!((note.start_beats - layout.beat_at(note_point)).abs() < 0.01);
    assert_eq!(note.musical_note, layout.pitch_at(note_point));
}

#[test]
fn pointer_clicking_visible_button_dispatches_action() {
    let mut app = AppState::for_layout_tests();

    let action = click_surface_node(&mut app, "transport.play_stop", 1200.0, 760.0);

    assert_eq!(action.as_deref(), Some("transport.play_stop"));
    assert!(app.music_project.lock().transport.playing);
}

#[test]
fn visible_controls_have_operad_action_bindings() {
    let app = AppState::for_layout_tests();
    let document = build_surface_document(&app, 1200.0, 760.0);

    for name in ["transport.play_stop", "clip.add_note", "piano.grid"] {
        let node = document
            .nodes()
            .iter()
            .find(|node| node.name == name)
            .unwrap_or_else(|| panic!("missing node {name}"));
        let action = node
            .action
            .as_ref()
            .and_then(|binding| binding.action_id())
            .map(|id| id.as_str());
        assert_eq!(action, Some(name));
    }
}

#[test]
fn coordinate_hit_targets_use_operad_pointer_edit_actions() {
    let mut app = AppState::for_layout_tests();
    app.add_clip_note_at(0.0, 69);
    let document = build_surface_document(&app, 1200.0, 760.0);

    for name in ["transport.seek", "piano.seek", "piano.grid"] {
        let node = document
            .nodes()
            .iter()
            .find(|node| node.name == name)
            .unwrap_or_else(|| panic!("missing node {name}"));
        assert_eq!(node.action_mode, WidgetActionMode::PointerEdit);
        assert!(node.input.pointer);
        assert!(!node.input.focusable);
        assert!(!node.input.keyboard);
    }

    let note_node = document
        .nodes()
        .iter()
        .find(|node| node.name.starts_with("note.select."))
        .expect("missing note hit target");
    assert_eq!(note_node.action_mode, WidgetActionMode::PointerEdit);
}

#[test]
fn coordinate_hit_targets_remain_invisible_when_hovered() {
    let app = populated_layout_test_app();
    let mut document = build_surface_document(&app, 1200.0, 760.0);
    document
        .compute_layout(UiSize::new(1200.0, 760.0), &mut ApproxTextMeasurer)
        .expect("surface layout should compute");
    let point = surface_node_center(&document, "piano.grid");

    document.handle_input(UiInputEvent::PointerMove(point));

    let node = document
        .nodes()
        .iter()
        .find(|node| node.name == "piano.grid")
        .expect("missing piano grid hit target");
    assert_eq!(node.visual.fill, ColorRgba::TRANSPARENT);
    assert_eq!(node.visual.stroke, None);
    assert!(node.interaction_visuals.is_none());
    let accessibility = node
        .accessibility
        .as_ref()
        .expect("hit target should carry hidden accessibility metadata");
    assert_eq!(accessibility.role, AccessibilityRole::Group);
    assert!(accessibility.hidden);
}

#[test]
fn disabled_controls_do_not_advertise_operad_action_bindings() {
    let app = AppState::for_layout_tests();
    let document = build_surface_document(&app, 1200.0, 760.0);
    let node = document
        .nodes()
        .iter()
        .find(|node| node.name == "audio.test_a4")
        .expect("missing disabled audio test node");

    assert!(node.action.is_none());
}

#[test]
fn pointer_clicking_top_bar_controls_updates_project_state() {
    let mut app = AppState::for_layout_tests();
    let width = 1200.0;
    let height = 760.0;

    assert_eq!(
        click_surface_node(&mut app, "transport.record", width, height).as_deref(),
        Some("transport.record")
    );
    assert!(app.music_project.lock().transport.recording);

    assert_eq!(
        click_surface_node(&mut app, "transport.loop", width, height).as_deref(),
        Some("transport.loop")
    );
    assert!(app.music_project.lock().transport.overdub);

    assert_eq!(
        click_surface_node(&mut app, "transport.bpm_up", width, height).as_deref(),
        Some("transport.bpm_up")
    );
    assert_eq!(app.music_project.lock().transport.bpm, 121.0);

    assert_eq!(
        click_surface_node(&mut app, "transport.quantize_grid", width, height).as_deref(),
        Some("transport.quantize_grid")
    );
    assert_eq!(
        app.music_project.lock().transport.quantize_grid,
        QuantizeGrid::ThirtySecond
    );

    assert_eq!(
        click_surface_node(&mut app, "audio.all_off", width, height).as_deref(),
        Some("audio.all_off")
    );
    assert_eq!(app.last_status, "All notes off");
    assert!(app.project_dirty);
}

#[test]
fn pointer_clicking_disabled_button_does_not_dispatch_action() {
    let mut app = populated_layout_test_app();
    app.audio_stream = None;
    app.last_status = "Ready".to_string();

    let action = click_surface_node(&mut app, "audio.test_a4", 1200.0, 760.0);

    assert_eq!(action, None);
    assert_eq!(app.last_status, "Ready");
}

#[test]
fn pointer_clicking_canvas_hit_target_dispatches_with_coordinates() {
    let mut app = AppState::for_layout_tests();

    let action = click_surface_node(&mut app, "piano.grid", 1200.0, 760.0);

    assert_eq!(action.as_deref(), Some("piano.grid"));
    let project = app.music_project.lock();
    assert_eq!(project.clip.notes.len(), 1);
    assert_eq!(project.clip.notes[0].duration_beats, 1.0);
    drop(project);
    assert!(app.project_dirty);
}

#[test]
fn pointer_clicking_arrangement_clip_selects_current_clip_context() {
    let mut app = AppState::for_layout_tests();
    assert!(!surface_node_exists(&app, "clip.select_current"));

    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let selected_note = app.selected_clip_note.expect("note should be selected");

    let action = click_surface_node(&mut app, "clip.select_current", 1200.0, 760.0);

    assert_eq!(action.as_deref(), Some("clip.select_current"));
    assert_ne!(app.selected_clip_note, Some(selected_note));
    assert!(app.selected_clip_note.is_none());
    assert_eq!(app.last_status, "Selected current clip: 1 note");
}

#[test]
fn pointer_clicking_rendered_note_selects_clip_note() {
    let mut app = AppState::for_layout_tests();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let note_id = app
        .selected_clip_note()
        .expect("new note should be selected")
        .id;
    app.select_clip_note(None);

    let action = click_surface_node(&mut app, &format!("note.select.{note_id}"), 1200.0, 760.0);

    let expected = format!("note.select.{note_id}");
    assert_eq!(action.as_deref(), Some(expected.as_str()));
    assert_eq!(app.selected_clip_note, Some(note_id));
    assert!(app.last_status.starts_with("Selected note"));
}

#[test]
fn pointer_dragging_rendered_note_moves_clip_note() {
    let mut app = AppState::for_layout_tests();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let note_id = app
        .selected_clip_note()
        .expect("new note should be selected")
        .id;
    app.project_dirty = false;

    let width = 1200.0;
    let height = 760.0;
    let layout = surface_rects(&app, width, height);
    let mut document = build_surface_document(&app, width, height);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(UiSize::new(width, height), &mut text_measurer)
        .expect("surface layout should compute");
    let start = surface_node_center(&document, &format!("note.select.{note_id}"));
    let target = UiPoint::new(layout.piano_grid.x + layout.piano_grid.width * 0.5, start.y);

    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);
    host.document = Some(document);
    host.press_pointer(start);
    assert!(matches!(
        host.note_drag.as_ref().map(|drag| drag.mode),
        Some(NoteDragMode::Move)
    ));

    assert!(host.drag_selected_note(target));
    host.release_pointer(target);

    let note = host
        .app
        .selected_clip_note()
        .expect("dragged note should remain selected");
    assert_eq!(note.id, note_id);
    assert!((note.start_beats - 7.5).abs() < 0.001);
    assert_eq!(note.musical_note, root);
    assert!(host.app.project_dirty);
}

#[test]
fn pointer_dragging_rendered_velocity_handle_updates_note_velocity() {
    let mut app = AppState::for_layout_tests();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let note_id = app
        .selected_clip_note()
        .expect("new note should be selected")
        .id;
    app.project_dirty = false;

    let width = 1200.0;
    let height = 760.0;
    let layout = surface_rects(&app, width, height);
    let mut document = build_surface_document(&app, width, height);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(UiSize::new(width, height), &mut text_measurer)
        .expect("surface layout should compute");
    let start = surface_node_center(&document, &format!("note.velocity.{note_id}"));
    let target = UiPoint::new(start.x, layout.velocity_lane.y);

    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);
    host.document = Some(document);
    host.press_pointer(start);
    assert!(matches!(
        host.note_drag.as_ref().map(|drag| drag.mode),
        Some(NoteDragMode::Velocity)
    ));

    assert!(host.drag_selected_note(target));
    host.release_pointer(target);

    let note = host
        .app
        .selected_clip_note()
        .expect("velocity edit should keep note selected");
    assert_eq!(note.id, note_id);
    assert_eq!(note.velocity, 127);
    assert!(host.app.project_dirty);
}

#[test]
fn velocity_lane_keeps_time_visible_notes_when_pitch_is_scrolled_away() {
    let mut app = AppState::for_layout_tests();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let note = app
        .selected_clip_note()
        .expect("new note should be selected");

    assert!(app.scroll_piano_roll(0.0, 40));
    let layout = surface_rects(&app, 1200.0, 760.0);
    assert!(piano_note_rects(note.clone(), layout).is_empty());

    let velocity_rect = piano_velocity_hit_rects(note, layout)
        .into_iter()
        .next()
        .expect("velocity bar should remain visible for time-visible note");
    assert_eq!(
        piano_cursor_shape_at(&app, Some(layout), None, rect_center(velocity_rect)),
        CursorShape::ResizeVertical
    );
}

#[test]
fn pointer_dragging_rendered_note_resize_end_updates_duration() {
    let mut app = AppState::for_layout_tests();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let note_id = app
        .selected_clip_note()
        .expect("new note should be selected")
        .id;
    assert!(app.music_project.lock().set_note_duration(note_id, 4.0));
    app.project_dirty = false;

    let width = 1200.0;
    let height = 760.0;
    let layout = surface_rects(&app, width, height);
    let mut document = build_surface_document(&app, width, height);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(UiSize::new(width, height), &mut text_measurer)
        .expect("surface layout should compute");
    let start = surface_node_center(&document, &format!("note.resize_end.{note_id}"));
    let target = UiPoint::new(
        layout.piano_grid.x + layout.piano_grid.width * 10.0 / layout.loop_beats,
        start.y,
    );

    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);
    host.document = Some(document);
    host.press_pointer(start);
    assert!(matches!(
        host.note_drag.as_ref().map(|drag| drag.mode),
        Some(NoteDragMode::ResizeEnd)
    ));

    assert!(host.drag_selected_note(target));
    host.release_pointer(target);

    let note = host
        .app
        .selected_clip_note()
        .expect("resized note should remain selected");
    assert_eq!(note.id, note_id);
    assert!((note.duration_beats - 8.0).abs() < 0.001);
    assert!(host.app.project_dirty);
}

#[test]
fn pointer_dragging_rendered_note_resize_start_updates_start_and_duration() {
    let mut app = AppState::for_layout_tests();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let note_id = app
        .selected_clip_note()
        .expect("new note should be selected")
        .id;
    assert!(app.music_project.lock().set_note_duration(note_id, 4.0));
    app.project_dirty = false;

    let width = 1200.0;
    let height = 760.0;
    let layout = surface_rects(&app, width, height);
    let mut document = build_surface_document(&app, width, height);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(UiSize::new(width, height), &mut text_measurer)
        .expect("surface layout should compute");
    let start = surface_node_center(&document, &format!("note.resize_start.{note_id}"));
    let target = UiPoint::new(
        layout.piano_grid.x + layout.piano_grid.width * 3.0 / layout.loop_beats,
        start.y,
    );

    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);
    host.document = Some(document);
    host.press_pointer(start);
    assert!(matches!(
        host.note_drag.as_ref().map(|drag| drag.mode),
        Some(NoteDragMode::ResizeStart)
    ));

    assert!(host.drag_selected_note(target));
    host.release_pointer(target);

    let note = host
        .app
        .selected_clip_note()
        .expect("resized note should remain selected");
    assert_eq!(note.id, note_id);
    assert!((note.start_beats - 3.0).abs() < 0.001);
    assert!((note.duration_beats - 3.0).abs() < 0.001);
    assert!(host.app.project_dirty);
}

#[test]
fn piano_roll_wheel_scrolls_visible_beat_and_pitch_viewports() {
    let app = AppState::for_layout_tests();
    app.music_project.lock().transport.loop_beats = 64.0;
    let width = 1200.0;
    let height = 760.0;
    let layout = surface_rects(&app, width, height);
    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);
    let point = UiPoint::new(layout.piano_grid.x + 100.0, layout.piano_grid.y + 100.0);

    assert!(host.handle_canvas_input(NativeCanvasInput {
        node: UiNodeId(0),
        key: PIANO_INPUT_CANVAS_KEY.to_string(),
        rect: layout.piano_roll,
        local_position: Some(UiPoint::new(
            point.x - layout.piano_roll.x,
            point.y - layout.piano_roll.y,
        )),
        input: RawInputEvent::Wheel(operad::RawWheelEvent::pixels(
            point,
            UiPoint::new(120.0, 0.0),
            0,
        )),
    }));
    let scrolled_horizontal = surface_rects(&host.app, width, height);
    assert!(scrolled_horizontal.view_start_beats > 0.0);
    assert_eq!(scrolled_horizontal.view_beats, 16.0);

    assert!(host.handle_canvas_input(NativeCanvasInput {
        node: UiNodeId(0),
        key: PIANO_INPUT_CANVAS_KEY.to_string(),
        rect: layout.piano_roll,
        local_position: Some(UiPoint::new(
            point.x - layout.piano_roll.x,
            point.y - layout.piano_roll.y,
        )),
        input: RawInputEvent::Wheel(operad::RawWheelEvent::pixels(
            point,
            UiPoint::new(0.0, layout.row_height() * 3.0),
            0,
        )),
    }));
    let scrolled_vertical = surface_rects(&host.app, width, height);
    assert_eq!(scrolled_vertical.min_pitch, layout.min_pitch - 3);
    assert_eq!(scrolled_vertical.max_pitch, layout.max_pitch - 3);
}

#[test]
fn piano_roll_modifier_wheel_zooms_then_pans_time_view() {
    let width = 1200.0;
    let height = 760.0;
    let app = AppState::for_layout_tests();
    let layout = surface_rects(&app, width, height);
    assert_eq!(layout.loop_beats, 16.0);
    assert_eq!(layout.view_beats, layout.loop_beats);
    let point = UiPoint::new(
        layout.piano_grid.x + layout.piano_grid.width * 0.25,
        layout.piano_grid.y + 100.0,
    );

    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);
    assert!(host.handle_canvas_input(piano_wheel_input(
        layout,
        point,
        UiPoint::new(0.0, 120.0),
        operad::KeyModifiers {
            ctrl: true,
            ..operad::KeyModifiers::NONE
        },
    )));
    let zoomed_layout = surface_rects(&host.app, width, height);
    assert!(zoomed_layout.view_beats < layout.view_beats);
    assert!(zoomed_layout.view_start_beats > layout.view_start_beats);
    assert_eq!(zoomed_layout.min_pitch, layout.min_pitch);
    assert_eq!(zoomed_layout.max_pitch, layout.max_pitch);
    assert!(host.app.last_status.starts_with("Piano zoom"));

    host.layout = Some(zoomed_layout);
    assert!(host.handle_canvas_input(piano_wheel_input(
        zoomed_layout,
        point,
        UiPoint::new(0.0, 120.0),
        operad::KeyModifiers {
            shift: true,
            ..operad::KeyModifiers::NONE
        },
    )));
    let shifted_layout = surface_rects(&host.app, width, height);
    assert!(shifted_layout.view_start_beats > zoomed_layout.view_start_beats);
    assert_eq!(shifted_layout.view_beats, zoomed_layout.view_beats);
    assert_eq!(shifted_layout.min_pitch, zoomed_layout.min_pitch);
    assert_eq!(shifted_layout.max_pitch, zoomed_layout.max_pitch);
}

#[test]
fn plus_minus_shortcuts_zoom_piano_roll_time_view() {
    let mut app = AppState::for_layout_tests();
    app.music_project.lock().transport.loop_beats = 64.0;
    app.seek_transport_to(8.0);
    let initial = surface_rects(&app, 1200.0, 760.0);

    assert!(handle_key(
        &mut app,
        &Key::Character("+".into()),
        ModifiersState::empty(),
        false,
    ));
    let zoomed = surface_rects(&app, 1200.0, 760.0);
    assert!(zoomed.view_beats < initial.view_beats);
    assert!(app.last_status.starts_with("Piano zoom"));

    assert!(handle_key(
        &mut app,
        &Key::Character("-".into()),
        ModifiersState::empty(),
        false,
    ));
    let zoomed_out = surface_rects(&app, 1200.0, 760.0);
    assert!(zoomed_out.view_beats > zoomed.view_beats);
}

#[test]
fn piano_roll_alt_wheel_zooms_pitch_view() {
    let app = AppState::for_layout_tests();
    let width = 1200.0;
    let height = 760.0;
    let layout = surface_rects(&app, width, height);
    let point = UiPoint::new(
        layout.piano_grid.x + layout.piano_grid.width * 0.5,
        layout.piano_grid.y + layout.piano_grid.height * 0.5,
    );
    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);

    assert!(host.handle_canvas_input(piano_wheel_input(
        layout,
        point,
        UiPoint::new(0.0, 120.0),
        operad::KeyModifiers {
            alt: true,
            ..operad::KeyModifiers::NONE
        },
    )));

    let zoomed_layout = surface_rects(&host.app, width, height);
    assert!(
        zoomed_layout.max_pitch - zoomed_layout.min_pitch < layout.max_pitch - layout.min_pitch
    );
    assert!(host.app.last_status.starts_with("Piano pitch zoom"));
}

#[test]
fn piano_roll_note_ruler_drag_scrolls_and_zooms_pitch_view() {
    let app = AppState::for_layout_tests();
    let width = 1200.0;
    let height = 760.0;
    let layout = surface_rects(&app, width, height);
    let start = rect_center(layout.piano_keyboard);
    let target = UiPoint::new(start.x + 80.0, start.y + layout.row_height() * 3.0);
    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);

    assert!(host.handle_canvas_input(piano_pointer_input(
        layout,
        PointerEventKind::Down(PointerButton::Primary),
        start,
        PointerButtons::PRIMARY,
        100,
    )));
    assert_eq!(host.cursor_shape, CursorShape::ResizeNorthEastSouthWest);
    assert!(host.handle_canvas_input(piano_pointer_input(
        layout,
        PointerEventKind::Move,
        target,
        PointerButtons::PRIMARY,
        140,
    )));
    assert!(host.handle_canvas_input(piano_pointer_input(
        layout,
        PointerEventKind::Up(PointerButton::Primary),
        target,
        PointerButtons::NONE,
        160,
    )));

    let edited_layout = surface_rects(&host.app, width, height);
    assert!(
        edited_layout.max_pitch - edited_layout.min_pitch < layout.max_pitch - layout.min_pitch
    );
    assert!(edited_layout.max_pitch < layout.max_pitch);
    assert!(host.piano_keyboard_drag.is_none());
}

#[test]
fn piano_roll_reports_expected_cursor_shapes_for_edit_regions() {
    let mut app = AppState::for_layout_tests();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let note_id = app
        .selected_clip_note()
        .expect("new note should be selected")
        .id;
    assert!(app.music_project.lock().set_note_duration(note_id, 4.0));
    let layout = surface_rects(&app, 1200.0, 760.0);
    let note_rect = piano_note_rects(app.selected_clip_note().unwrap(), layout)
        .into_iter()
        .next()
        .expect("note should be visible");
    let center = UiPoint::new(
        note_rect.x + note_rect.width * 0.5,
        note_rect.y + note_rect.height * 0.5,
    );
    let left_edge = UiPoint::new(note_rect.x + 2.0, center.y);
    let velocity = UiPoint::new(
        center.x,
        layout.velocity_lane.y + layout.velocity_lane.height * 0.5,
    );
    let blank_grid = UiPoint::new(layout.piano_grid.x + 8.0, layout.piano_grid.y + 8.0);
    let keyboard = rect_center(layout.piano_keyboard);

    assert_eq!(
        piano_cursor_shape_at(&app, Some(layout), None, center),
        CursorShape::Grab
    );
    assert_eq!(
        piano_cursor_shape_at(&app, Some(layout), None, left_edge),
        CursorShape::ResizeHorizontal
    );
    assert_eq!(
        piano_cursor_shape_at(&app, Some(layout), None, velocity),
        CursorShape::ResizeVertical
    );
    assert_eq!(
        piano_cursor_shape_at(&app, Some(layout), None, blank_grid),
        CursorShape::Crosshair
    );
    assert_eq!(
        piano_cursor_shape_at(&app, Some(layout), None, keyboard),
        CursorShape::ResizeNorthEastSouthWest
    );
}

#[test]
fn native_canvas_double_click_empty_piano_grid_adds_note() {
    let app = AppState::for_layout_tests();
    let width = 1200.0;
    let height = 760.0;
    let layout = surface_rects(&app, width, height);
    let point = UiPoint::new(
        layout.piano_grid.x + layout.piano_grid.width * 0.25,
        layout.piano_grid.y + layout.row_height() * 4.5,
    );
    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);

    assert!(host.handle_canvas_input(piano_pointer_input(
        layout,
        PointerEventKind::Down(PointerButton::Primary),
        point,
        PointerButtons::PRIMARY,
        100,
    )));
    assert!(host.handle_canvas_input(piano_pointer_input(
        layout,
        PointerEventKind::Up(PointerButton::Primary),
        point,
        PointerButtons::NONE,
        120,
    )));
    assert!(host.app.music_project.lock().clip.notes.is_empty());

    assert!(host.handle_canvas_input(piano_pointer_input(
        layout,
        PointerEventKind::Down(PointerButton::Primary),
        point,
        PointerButtons::PRIMARY,
        260,
    )));
    assert!(host.handle_canvas_input(piano_pointer_input(
        layout,
        PointerEventKind::Up(PointerButton::Primary),
        point,
        PointerButtons::NONE,
        280,
    )));

    let note = host
        .app
        .selected_clip_note()
        .expect("double click should create and select a note");
    assert!((note.start_beats - layout.beat_at(point)).abs() < 0.001);
    assert_eq!(note.musical_note, layout.pitch_at(point));
    assert_eq!(note.duration_beats, 1.0);
}

#[test]
fn native_canvas_dragging_note_moves_clip_note() {
    let mut app = AppState::for_layout_tests();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let note_id = app
        .selected_clip_note()
        .expect("new note should be selected")
        .id;
    app.project_dirty = false;

    let width = 1200.0;
    let height = 760.0;
    let layout = surface_rects(&app, width, height);
    let note_rect = piano_note_rects(app.selected_clip_note().unwrap(), layout)
        .into_iter()
        .next()
        .expect("note should be visible");
    let start = rect_center(note_rect);
    let target = UiPoint::new(layout.piano_grid.x + layout.piano_grid.width * 0.5, start.y);
    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);

    assert!(host.handle_canvas_input(piano_pointer_input(
        layout,
        PointerEventKind::Down(PointerButton::Primary),
        start,
        PointerButtons::PRIMARY,
        100,
    )));
    assert!(matches!(
        host.note_drag.as_ref().map(|drag| drag.mode),
        Some(NoteDragMode::Move)
    ));
    assert!(host.handle_canvas_input(piano_pointer_input(
        layout,
        PointerEventKind::Move,
        target,
        PointerButtons::PRIMARY,
        140,
    )));
    assert!(host.handle_canvas_input(piano_pointer_input(
        layout,
        PointerEventKind::Up(PointerButton::Primary),
        target,
        PointerButtons::NONE,
        160,
    )));

    let note = host
        .app
        .selected_clip_note()
        .expect("dragged note should remain selected");
    assert_eq!(note.id, note_id);
    assert!((note.start_beats - 7.5).abs() < 0.001);
    assert_eq!(note.musical_note, root);
    assert!(host.note_drag.is_none());
    assert!(host.app.project_dirty);
}

#[test]
fn native_canvas_dragging_note_edge_resizes_clip_note() {
    let mut app = AppState::for_layout_tests();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let note_id = app
        .selected_clip_note()
        .expect("new note should be selected")
        .id;
    app.project_dirty = false;

    let width = 1200.0;
    let height = 760.0;
    let layout = surface_rects(&app, width, height);
    let note_rect = piano_note_rects(app.selected_clip_note().unwrap(), layout)
        .into_iter()
        .next()
        .expect("note should be visible");
    let start = UiPoint::new(
        note_rect.right() - 2.0,
        note_rect.y + note_rect.height * 0.5,
    );
    let target = UiPoint::new(
        layout.piano_grid.x + layout.piano_grid.width * 6.0 / layout.view_beats,
        start.y,
    );
    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);

    assert!(host.handle_canvas_input(piano_pointer_input(
        layout,
        PointerEventKind::Down(PointerButton::Primary),
        start,
        PointerButtons::PRIMARY,
        100,
    )));
    assert!(matches!(
        host.note_drag.as_ref().map(|drag| drag.mode),
        Some(NoteDragMode::ResizeEnd)
    ));
    assert!(host.handle_canvas_input(piano_pointer_input(
        layout,
        PointerEventKind::Move,
        target,
        PointerButtons::PRIMARY,
        140,
    )));
    assert!(host.handle_canvas_input(piano_pointer_input(
        layout,
        PointerEventKind::Up(PointerButton::Primary),
        target,
        PointerButtons::NONE,
        160,
    )));

    let note = host
        .app
        .selected_clip_note()
        .expect("resized note should remain selected");
    assert_eq!(note.id, note_id);
    assert!((note.start_beats - 2.0).abs() < 0.001);
    assert!((note.duration_beats - 4.0).abs() < 0.001);
    assert!(host.note_drag.is_none());
    assert!(host.app.project_dirty);
}

#[test]
fn pointer_dragging_arrangement_ruler_seeks_transport() {
    let app = AppState::for_layout_tests();
    let width = 1200.0;
    let height = 760.0;
    let layout = surface_rects(&app, width, height);
    let mut document = build_surface_document(&app, width, height);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(UiSize::new(width, height), &mut text_measurer)
        .expect("surface layout should compute");
    let start = UiPoint::new(
        layout.arrangement_ruler.x + layout.arrangement_ruler.width * 0.25,
        layout.arrangement_ruler.y + layout.arrangement_ruler.height * 0.5,
    );
    let target = UiPoint::new(
        layout.arrangement_ruler.x + layout.arrangement_ruler.width * 0.75,
        start.y,
    );

    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);
    host.document = Some(document);
    host.press_pointer(start);
    assert_eq!(host.timeline_drag, Some(TimelineDragMode::Arrangement));

    assert!(host.seek_timeline(target));
    host.release_pointer(target);

    let position = host
        .app
        .music_project
        .lock()
        .current_position_beats(std::time::Instant::now());
    assert!((position - 12.0).abs() < 0.001);
    assert_eq!(host.timeline_drag, None);
}

#[test]
fn pointer_dragging_piano_ruler_seeks_transport() {
    let app = AppState::for_layout_tests();
    let width = 1200.0;
    let height = 760.0;
    let layout = surface_rects(&app, width, height);
    let mut document = build_surface_document(&app, width, height);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(UiSize::new(width, height), &mut text_measurer)
        .expect("surface layout should compute");
    let start = UiPoint::new(
        layout.piano_ruler.x + layout.piano_ruler.width * 0.25,
        layout.piano_ruler.y + layout.piano_ruler.height * 0.5,
    );
    let target = UiPoint::new(
        layout.piano_ruler.x + layout.piano_ruler.width * 0.75,
        start.y,
    );

    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);
    host.document = Some(document);
    host.press_pointer(start);
    assert_eq!(host.timeline_drag, Some(TimelineDragMode::Piano));

    assert!(host.seek_timeline(target));
    host.release_pointer(target);

    let position = host
        .app
        .music_project
        .lock()
        .current_position_beats(std::time::Instant::now());
    assert!((position - 12.0).abs() < 0.001);
    assert_eq!(host.timeline_drag, None);
}

#[test]
fn pointer_clicking_clip_toolbar_buttons_edits_selected_note() {
    let mut app = AppState::for_layout_tests();
    let width = 1920.0;
    let height = 1080.0;

    assert_eq!(
        click_surface_node(&mut app, "clip.add_note", width, height).as_deref(),
        Some("clip.add_note")
    );
    let added = app
        .selected_clip_note()
        .expect("add button should create and select a note");
    let original_pitch = added.musical_note;

    assert_eq!(
        click_surface_node(&mut app, "clip.pitch_up", width, height).as_deref(),
        Some("clip.pitch_up")
    );
    assert_eq!(
        app.selected_clip_note()
            .expect("selected note should remain selected")
            .musical_note,
        original_pitch + 1
    );

    assert_eq!(
        click_surface_node(&mut app, "clip.velocity_up", width, height).as_deref(),
        Some("clip.velocity_up")
    );
    assert_eq!(
        app.selected_clip_note()
            .expect("selected note should remain selected")
            .velocity,
        104
    );

    assert_eq!(
        click_surface_node(&mut app, "clip.duplicate_note", width, height).as_deref(),
        Some("clip.duplicate_note")
    );
    assert_eq!(app.music_project.lock().clip.notes.len(), 2);

    assert_eq!(
        click_surface_node(&mut app, "clip.delete_note", width, height).as_deref(),
        Some("clip.delete_note")
    );
    assert_eq!(app.music_project.lock().clip.notes.len(), 1);
    assert!(app.project_dirty);
}

#[test]
fn pointer_clicking_control_panel_buttons_updates_state() {
    let mut app = AppState::for_layout_tests();
    let width = 1200.0;
    let height = 1000.0;

    assert_eq!(
        click_surface_node(&mut app, "scale.root_up", width, height).as_deref(),
        Some("scale.root_up")
    );
    assert_eq!(app.scale_state.lock().root_midi, 70);

    assert_eq!(
        click_surface_node(&mut app, "scale.base_up", width, height).as_deref(),
        Some("scale.base_up")
    );
    assert!((app.scale_state.lock().base_freq - 441.0).abs() < f32::EPSILON);

    assert_eq!(
        click_surface_node(&mut app, "transport.metronome", width, height).as_deref(),
        Some("transport.metronome")
    );
    assert!(app.music_project.lock().transport.metronome_enabled);

    assert_eq!(
        click_surface_node(&mut app, "midi.channel_filter", width, height).as_deref(),
        Some("midi.channel_filter")
    );
    assert_eq!(app.midi_channel_filter(), Some(0));

    assert_eq!(
        click_surface_node(&mut app, "ui.scale_up", width, height).as_deref(),
        Some("ui.scale_up")
    );
    assert!((app.ui_scale() - 1.1).abs() < 0.0001);

    assert_eq!(
        click_surface_node(&mut app, "synth.waveform", width, height).as_deref(),
        Some("synth.waveform")
    );
    assert_eq!(app.synth.settings().waveform, Waveform::Triangle);

    assert_eq!(
        click_surface_node(&mut app, "synth.mute", width, height).as_deref(),
        Some("synth.mute")
    );
    assert!(app.synth.muted());

    assert_eq!(
        click_surface_node(&mut app, "synth.gain_up", width, height).as_deref(),
        Some("synth.gain_up")
    );
    assert!((app.synth.settings().master_gain - 0.40).abs() < f32::EPSILON);
    assert!(app.project_dirty);
}

#[test]
fn pointer_clicking_left_browser_buttons_updates_selection_state() {
    let mut app = populated_layout_test_app();
    app.show_scale_browser = true;
    let width = 1200.0;
    let height = 760.0;

    assert_eq!(
        click_surface_node(&mut app, "scale.select.1", width, height).as_deref(),
        Some("scale.select.1")
    );
    assert_eq!(app.selected_scale_library, 1);
    assert_eq!(app.last_status, "Selected scale: 19-EDO (Equal)");
    app.show_scale_browser = false;

    assert_eq!(
        click_surface_node(&mut app, "asset.kind.1", width, height).as_deref(),
        Some("asset.kind.1")
    );
    assert_eq!(app.selected_audio_asset_kind, AudioAssetKind::Instrument);
    assert_eq!(app.selected_audio_asset, Some(3));

    assert_eq!(
        click_surface_node(&mut app, "asset.select.4", width, height).as_deref(),
        Some("asset.select.4")
    );
    assert_eq!(app.selected_audio_asset, Some(4));
    assert_eq!(app.last_status, "Selected instrument: Analog");
}

#[test]
fn pointer_clicking_device_navigation_buttons_updates_selection_state() {
    let mut app = AppState::for_layout_tests();
    app.midi_inputs = vec!["Input A".to_string(), "Input B".to_string()];
    app.audio_outputs = vec![
        AudioOutputDevice {
            name: "Output A".to_string(),
            is_default: true,
        },
        AudioOutputDevice {
            name: "Output B".to_string(),
            is_default: false,
        },
    ];
    let width = 1200.0;
    let height = 760.0;

    assert_eq!(
        click_surface_node(&mut app, "midi.next", width, height).as_deref(),
        Some("midi.next")
    );
    assert_eq!(app.selected_input, 1);
    assert_eq!(app.last_status, "Selected MIDI input: Input B");

    assert_eq!(
        click_surface_node(&mut app, "midi.prev", width, height).as_deref(),
        Some("midi.prev")
    );
    assert_eq!(app.selected_input, 0);
    assert_eq!(app.last_status, "Selected MIDI input: Input A");

    assert_eq!(
        click_surface_node(&mut app, "audio.next", width, height).as_deref(),
        Some("audio.next")
    );
    assert_eq!(app.selected_audio_output, 1);
    assert_eq!(app.last_status, "Selected audio output: Output B");

    assert_eq!(
        click_surface_node(&mut app, "audio.prev", width, height).as_deref(),
        Some("audio.prev")
    );
    assert_eq!(app.selected_audio_output, 0);
    assert_eq!(app.last_status, "Selected audio output: Output A");
}

#[test]
fn pointer_clicking_recover_loads_autosave_project() {
    let path = std::env::temp_dir().join(format!(
        "orbifold_pointer_recover_test_{}.orbifold",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut source = AppState::for_layout_tests();
    let root = source.scale_state.lock().root_midi;
    source.add_clip_note_at(1.0, root);
    source.save_project_to_path(path.clone());

    let mut app = AppState::for_layout_tests();
    app.set_autosave_path_for_tests(path.clone());

    let action = click_surface_node(&mut app, "file.recover", 1200.0, 760.0);

    assert_eq!(action.as_deref(), Some("file.recover"));
    assert_eq!(app.music_project.lock().clip.notes.len(), 1);
    assert!(app.project_path.is_none());
    assert!(app.project_dirty);
    assert_eq!(app.last_status, "Recovered autosave: use Save to keep it");

    let _ = std::fs::remove_file(path);
}

#[test]
fn pointer_clicking_dismiss_autosave_removes_recovery_file() {
    let path = std::env::temp_dir().join(format!(
        "orbifold_pointer_dismiss_autosave_test_{}.orbifold",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut source = AppState::for_layout_tests();
    source.save_project_to_path(path.clone());
    let mut app = AppState::for_layout_tests();
    app.set_autosave_path_for_tests(path.clone());

    let action = click_surface_node(&mut app, "file.dismiss_autosave", 1200.0, 760.0);

    assert_eq!(action.as_deref(), Some("file.dismiss_autosave"));
    assert!(!app.autosave_available);
    assert!(!path.exists());
    assert_eq!(app.last_status, "Autosave dismissed");

    let _ = std::fs::remove_file(path);
}

#[test]
fn pointer_clicking_save_settings_writes_settings_file() {
    let path = std::env::temp_dir().join(format!(
        "orbifold_pointer_settings_test_{}.txt",
        std::process::id()
    ));
    let autosave_path = path.with_file_name(format!(
        "{}_autosave.orbifold",
        path.file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("orbifold_pointer_settings_test")
            .replace("_settings", "")
    ));
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&autosave_path);
    let mut app = AppState::for_layout_tests();
    app.set_settings_path_for_tests(path.clone(), true);
    app.scale_state.lock().root_midi = 72;

    let action = click_surface_node(&mut app, "settings.save", 1200.0, 760.0);

    assert_eq!(action.as_deref(), Some("settings.save"));
    assert_eq!(app.last_status, "Saved settings");
    let settings = AppSettings::load(&path).expect("saved settings should load");
    assert_eq!(settings.root_midi, 72);

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(autosave_path);
}

#[test]
fn all_off_remains_available_without_audio_output() {
    let app = populated_layout_test_app();

    assert!(surface_node_enabled(&app, "audio.all_off"));
    assert!(!surface_node_enabled(&app, "audio.test_a4"));
}

#[test]
fn audio_unavailable_surface_reports_no_output_and_safe_actions() {
    let mut app = populated_layout_test_app();
    app.audio_stream = None;
    app.audio_stream_info = None;
    app.audio_outputs.clear();
    app.connected_audio_output.clear();
    app.last_status = "Audio unavailable: No output device".to_string();

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(
        text.iter()
            .any(|item| item.text.contains("Audio unavailable: No output device"))
    );
    assert!(surface_node_enabled(&app, "audio.all_off"));
    assert!(surface_node_enabled(&app, "audio.refresh"));
    assert!(!surface_node_enabled(&app, "audio.prev"));
    assert!(!surface_node_enabled(&app, "audio.next"));
    assert!(!surface_node_enabled(&app, "audio.connect"));
    assert!(!surface_node_enabled(&app, "audio.test_a4"));
    assert_text_overlap_free("audio-unavailable", &text);
}

#[test]
fn midi_unavailable_surface_reports_no_input_and_safe_actions() {
    let mut app = populated_layout_test_app();
    app.midi_connection = None;
    app.connected_midi_input.clear();
    app.midi_inputs.clear();
    app.last_status = "No MIDI inputs found".to_string();

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(
        text.iter()
            .any(|item| item.text.contains("No MIDI inputs found"))
    );
    assert!(surface_node_enabled(&app, "midi.refresh"));
    assert!(!surface_node_enabled(&app, "midi.prev"));
    assert!(!surface_node_enabled(&app, "midi.next"));
    assert!(!surface_node_enabled(&app, "midi.connect"));
    assert_text_overlap_free("midi-unavailable", &text);
}

#[test]
fn last_midi_surface_label_keeps_note_name_visible() {
    let app = populated_layout_test_app();
    *app.midi_last.lock() = Some(crate::midi::MidiEvent {
        raw_status: 0x90,
        status: 0x90,
        channel: 0,
        midi_note: 60,
        velocity: 96,
        key_index: 60,
        musical_note: 60,
        mapped_from_lumatone: false,
        freq: Some(261.63),
        scale_degree: Some(0),
        scale_octave: Some(0),
        cents_from_root: Some(0.0),
        at: std::time::Instant::now(),
    });

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(text.iter().any(|item| {
        item.source == "midi.last" && item.text.starts_with("Last MIDI ch1 note C4")
    }));
    assert_text_overlap_free("last-midi-readable", &text);
}

#[test]
fn scale_root_label_reports_note_name_and_midi_number() {
    let mut app = populated_layout_test_app();
    app.scale_state.lock().root_midi = 69;

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(text.iter().any(|item| item.text == "Root A4 (69)"));

    dispatch_action(&mut app, "scale.root_up", None, None);
    assert_eq!(app.last_status, "Root A#4 (70)");
}

#[test]
fn top_transport_reports_recording_mode() {
    let app = populated_layout_test_app();

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);
    assert!(text.iter().any(|item| item.text == "Replace"));

    app.music_project.lock().transport.overdub = true;
    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);
    assert!(text.iter().any(|item| item.text == "Overdub"));
}

#[test]
fn top_transport_record_button_reports_stop_action_while_recording() {
    let mut app = populated_layout_test_app();
    app.start_recording();

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(text.iter().any(|item| item.text == "Stop Rec"));
    assert!(!text.iter().any(|item| item.text == "Record"));
    assert_text_overlap_free("recording-top-transport", &text);
}

#[test]
fn top_transport_labels_return_to_start_action_explicitly() {
    let app = populated_layout_test_app();

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(text.iter().any(|item| item.text == "Start"));
    assert!(!text.iter().any(|item| item.text == "Prev"));
    assert!(text.iter().any(|item| item.text == "Record"));
    assert!(!text.iter().any(|item| item.text == "Rec"));
}

#[test]
fn top_transport_reports_fixed_meter_when_there_is_room() {
    let app = populated_layout_test_app();

    let minimum_text = collect_surface_text_boxes(&app, 1200.0, 760.0);
    assert!(!minimum_text.iter().any(|item| item.text == "4/4"));

    let default_text = collect_surface_text_boxes(&app, 1500.0, 760.0);
    assert!(default_text.iter().any(|item| item.text == "4/4"));
}

#[test]
fn transport_position_label_reports_fixed_bar_beat_position() {
    assert_eq!(transport_position_label(0.0), "Bar 1.1");
    assert_eq!(transport_position_label(3.99), "Bar 1.4");
    assert_eq!(transport_position_label(4.0), "Bar 2.1");
    assert_eq!(transport_position_label(15.2), "Bar 4.4");
}

#[test]
fn top_transport_reports_bar_beat_position_when_there_is_room() {
    let app = populated_layout_test_app();

    let default_text = collect_surface_text_boxes(&app, 1400.0, 760.0);
    assert!(!default_text.iter().any(|item| item.text == "Bar 1.1"));

    let wide_text = collect_surface_text_boxes(&app, 1920.0, 1080.0);
    assert!(wide_text.iter().any(|item| item.text == "Bar 1.1"));
}

#[test]
fn tab_focus_skips_nonvisual_canvas_hit_targets() {
    let mut app = populated_layout_test_app();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let mut document = build_surface_document(&app, 1200.0, 760.0);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(UiSize::new(1200.0, 760.0), &mut text_measurer)
        .expect("surface layout should compute");
    let mut focused = Vec::new();

    for _ in 0..80 {
        let result = document.handle_input(UiInputEvent::Focus(FocusDirection::Next));
        if let Some(id) = result.focused {
            focused.push(document.node(id).name.clone());
        }
    }

    assert!(focused.iter().any(|name| name == "file.open"));
    assert!(focused.iter().any(|name| name == "transport.play_stop"));
    assert!(focused.iter().any(|name| name == "clip.add_note"));
    for hidden in [
        "transport.seek",
        "clip.select_current",
        "piano.seek",
        "piano.grid",
    ] {
        assert!(
            !focused.iter().any(|name| name == hidden),
            "{hidden} should not receive keyboard focus"
        );
    }
    assert!(
        !focused.iter().any(|name| name.starts_with("note.")),
        "note drag/select hit targets should not receive keyboard focus"
    );
}

#[test]
fn focusable_control_node_names_are_unique() {
    let mut app = populated_layout_test_app();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let document = build_surface_document(&app, 1200.0, 760.0);
    let mut names = std::collections::HashSet::new();

    for node in document
        .nodes()
        .iter()
        .filter(|node| node_is_focusable_action(node))
    {
        assert!(
            names.insert(node.name.clone()),
            "duplicate focusable node name: {}",
            node.name
        );
    }
}

#[test]
fn focusable_visible_controls_keep_minimum_pointer_target_size() {
    let mut app = populated_layout_test_app();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let mut document = build_surface_document(&app, MIN_LAYOUT_WIDTH, MIN_LAYOUT_HEIGHT);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(
            UiSize::new(MIN_LAYOUT_WIDTH, MIN_LAYOUT_HEIGHT),
            &mut text_measurer,
        )
        .expect("surface layout should compute");

    for node in document
        .nodes()
        .iter()
        .filter(|node| node_is_focusable_action(node))
    {
        let rect = node.layout.rect;
        assert!(
            rect.width >= MIN_POINTER_TARGET_SIZE,
            "{} width {} is below minimum pointer target size {}",
            node.name,
            rect.width,
            MIN_POINTER_TARGET_SIZE
        );
        assert!(
            rect.height >= MIN_POINTER_TARGET_SIZE,
            "{} height {} is below minimum pointer target size {}",
            node.name,
            rect.height,
            MIN_POINTER_TARGET_SIZE
        );
    }
}

#[test]
fn focusable_controls_have_meaningful_accessibility_labels() {
    let mut app = populated_layout_test_app();
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(2.0, root);
    let document = build_surface_document(&app, 1200.0, 760.0);
    let forbidden = ["", "+", "-", "<", ">", "Samp", "Instr", "Pres", "IRs"];

    for node in document
        .nodes()
        .iter()
        .filter(|node| node_is_focusable_action(node))
    {
        let label = node
            .accessibility
            .as_ref()
            .and_then(|accessibility| accessibility.label.as_deref())
            .unwrap_or("");
        assert!(
            !forbidden.contains(&label),
            "{} has weak accessibility label {:?}",
            node.name,
            label
        );
    }
}

#[test]
fn compact_button_accessibility_labels_are_descriptive() {
    assert_eq!(
        button_accessibility_label("transport.bpm_up", "+"),
        "Increase BPM"
    );
    assert_eq!(
        button_accessibility_label("midi.prev", "<"),
        "Previous MIDI input"
    );
    assert_eq!(
        button_accessibility_label("clip.nudge_right", ">"),
        "Nudge note right"
    );
    assert_eq!(
        button_accessibility_label("asset.kind.0", "Samp"),
        "Show samples"
    );
    assert_eq!(
        button_accessibility_label("synth.filter_down", "-"),
        "Decrease filter cutoff"
    );
}

#[test]
fn focus_status_uses_descriptive_accessibility_label() {
    let app = AppState::for_layout_tests();
    let mut document = build_surface_document(&app, 1200.0, 760.0);
    apply_focus_name(&mut document, Some("transport.bpm_up"));
    let focused = document.focus.focused.expect("BPM up should be focused");

    assert_eq!(
        keyboard_focus_status(&document, focused),
        "Focused Increase BPM - Enter activates"
    );
}

#[test]
fn tab_focus_reports_activation_hint_in_status_bar() {
    let app = AppState::for_layout_tests();
    let mut document = build_surface_document(&app, 1200.0, 760.0);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(UiSize::new(1200.0, 760.0), &mut text_measurer)
        .expect("surface layout should compute");
    let mut host = NativeOperadApp::new(app, false, None);
    host.document = Some(document);

    assert!(host.handle_keyboard_focus_key(
        &Key::Named(NamedKey::Tab),
        ModifiersState::empty(),
        false,
    ));

    assert_eq!(host.focused_action.as_deref(), Some("file.new"));
    assert_eq!(host.app.last_status, "Focused New - Enter activates");
    assert!(!host.app.project_dirty);
}

#[test]
fn enter_activates_focused_button() {
    let app = AppState::for_layout_tests();
    let width = 1200.0;
    let height = 760.0;
    let layout = surface_rects(&app, width, height);
    let mut document = build_surface_document(&app, width, height);
    apply_focus_name(&mut document, Some("transport.play_stop"));
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(UiSize::new(width, height), &mut text_measurer)
        .expect("surface layout should compute");
    let mut host = NativeOperadApp::new(app, false, None);
    host.layout = Some(layout);
    host.focused_action = Some("transport.play_stop".to_string());
    host.document = Some(document);

    assert!(host.handle_keyboard_focus_key(
        &Key::Named(NamedKey::Enter),
        ModifiersState::empty(),
        false,
    ));

    assert!(host.app.music_project.lock().transport.playing);
}

#[test]
fn escape_cancels_pending_discard_confirmation() {
    let mut app = AppState::for_layout_tests();
    app.add_clip_note_at(0.0, 69);
    app.start_new_project();

    assert!(handle_key(
        &mut app,
        &Key::Named(NamedKey::Escape),
        ModifiersState::empty(),
        false,
    ));

    assert!(!app.new_project_confirm_pending());
    assert_eq!(app.last_status, "Discard cancelled");
}

#[test]
fn escape_clears_selected_note_when_no_discard_confirmation_is_pending() {
    let mut app = AppState::for_layout_tests();
    app.add_clip_note_at(0.0, 69);
    assert!(app.selected_clip_note.is_some());
    let dirty = app.project_dirty;

    assert!(handle_key(
        &mut app,
        &Key::Named(NamedKey::Escape),
        ModifiersState::empty(),
        false,
    ));

    assert_eq!(app.selected_clip_note, None);
    assert_eq!(app.project_dirty, dirty);
    assert_eq!(app.last_status, "Note selection cleared");
}

#[test]
fn m_key_toggles_metronome() {
    let mut app = AppState::for_layout_tests();

    assert!(handle_key(
        &mut app,
        &Key::Character("m".into()),
        ModifiersState::empty(),
        false,
    ));

    assert!(app.music_project.lock().transport.metronome_enabled);
    assert_eq!(app.last_status, "Metronome on");
}

#[test]
fn shift_q_toggles_record_quantize() {
    let mut app = AppState::for_layout_tests();

    assert!(handle_key(
        &mut app,
        &Key::Character("Q".into()),
        ModifiersState::SHIFT,
        false,
    ));

    assert!(!app.music_project.lock().transport.quantize_on_record);
    assert_eq!(app.last_status, "Record quantize off");
}

#[test]
fn q_key_quantizes_selected_note_before_whole_clip() {
    let mut app = AppState::for_layout_tests();
    let freq = app.scale_state.lock().note_info(69).unwrap().freq;
    let (selected_id, other_id) = {
        let mut project = app.music_project.lock();
        project.transport.quantize_grid = QuantizeGrid::Eighth;
        let selected_id = project.add_note(0.3, 0.4, 69, 96, freq);
        let other_id = project.add_note(1.3, 0.4, 69, 96, freq);
        (selected_id, other_id)
    };
    app.selected_clip_note = Some(selected_id);

    assert!(handle_key(
        &mut app,
        &Key::Character("q".into()),
        ModifiersState::empty(),
        false,
    ));

    let project = app.music_project.lock();
    assert_eq!(project.note_by_id(selected_id).unwrap().start_beats, 0.5);
    assert_eq!(project.note_by_id(other_id).unwrap().start_beats, 1.3);
    assert!(app.last_status.starts_with("Quantized note d1 o0"));
}

#[test]
fn g_key_toggles_snap_without_losing_grid_value() {
    let mut app = AppState::for_layout_tests();
    app.set_quantize_grid(QuantizeGrid::Eighth);
    app.project_dirty = false;

    assert!(handle_key(
        &mut app,
        &Key::Character("g".into()),
        ModifiersState::empty(),
        false,
    ));
    assert_eq!(
        app.music_project.lock().transport.quantize_grid,
        QuantizeGrid::Off
    );
    assert_eq!(app.last_status, "Snap off");
    assert!(app.project_dirty);

    assert!(handle_key(
        &mut app,
        &Key::Character("g".into()),
        ModifiersState::empty(),
        false,
    ));
    assert_eq!(
        app.music_project.lock().transport.quantize_grid,
        QuantizeGrid::Eighth
    );
    assert_eq!(app.last_status, "Snap on 1/8");
}

#[test]
fn p_key_runs_all_notes_off() {
    let mut app = AppState::for_layout_tests();
    let event = crate::midi::MidiEvent {
        raw_status: 0x90,
        status: 0x90,
        channel: 0,
        midi_note: 60,
        velocity: 96,
        key_index: 60,
        musical_note: 60,
        mapped_from_lumatone: false,
        freq: Some(440.0),
        scale_degree: Some(0),
        scale_octave: Some(0),
        cents_from_root: Some(0.0),
        at: std::time::Instant::now(),
    };
    app.midi_held
        .lock()
        .insert((event.key_index, event.channel, event.midi_note), event);

    assert!(handle_key(
        &mut app,
        &Key::Character("p".into()),
        ModifiersState::empty(),
        false,
    ));

    assert!(app.midi_held.lock().is_empty());
    assert_eq!(app.last_status, "All notes off");
}

#[test]
fn question_mark_shows_shortcut_reference_without_dirtying_project() {
    let mut app = AppState::for_layout_tests();

    assert!(handle_key(
        &mut app,
        &Key::Character("?".into()),
        ModifiersState::empty(),
        false,
    ));

    assert_eq!(app.last_status, shortcut_help_status());
    assert!(!app.project_dirty);

    app.last_status = "Ready".to_string();
    assert!(handle_key(
        &mut app,
        &Key::Character("/".into()),
        ModifiersState::SHIFT,
        false,
    ));
    assert_eq!(app.last_status, shortcut_help_status());
}

#[derive(Clone, Copy)]
struct ShortcutReference {
    chord: &'static str,
    doc_probe: &'static str,
    action: &'static str,
}

fn documented_shortcuts() -> &'static [ShortcutReference] {
    &[
        ShortcutReference {
            chord: "?",
            doc_probe: "`?`",
            action: "Show shortcut reference",
        },
        ShortcutReference {
            chord: "Shift+/",
            doc_probe: "`Shift+/`",
            action: "Show shortcut reference",
        },
        ShortcutReference {
            chord: "Tab",
            doc_probe: "`Tab`",
            action: "Focus next control",
        },
        ShortcutReference {
            chord: "Shift+Tab",
            doc_probe: "`Shift+Tab`",
            action: "Focus previous control",
        },
        ShortcutReference {
            chord: "Enter",
            doc_probe: "`Enter`",
            action: "Activate focused control",
        },
        ShortcutReference {
            chord: "Esc",
            doc_probe: "`Esc`",
            action: "Cancel or clear note selection",
        },
        ShortcutReference {
            chord: "Ctrl/Cmd+N",
            doc_probe: "Cmd+N",
            action: "New project",
        },
        ShortcutReference {
            chord: "Ctrl/Cmd+O",
            doc_probe: "Cmd+O",
            action: "Open project",
        },
        ShortcutReference {
            chord: "Ctrl/Cmd+S",
            doc_probe: "Cmd+S",
            action: "Save project",
        },
        ShortcutReference {
            chord: "Ctrl/Cmd+Shift+S",
            doc_probe: "Cmd+Shift+S",
            action: "Save project as",
        },
        ShortcutReference {
            chord: "Ctrl/Cmd+Z",
            doc_probe: "Cmd+Z",
            action: "Undo",
        },
        ShortcutReference {
            chord: "Ctrl/Cmd+Y",
            doc_probe: "Cmd+Y",
            action: "Redo",
        },
        ShortcutReference {
            chord: "Ctrl/Cmd+Shift+Z",
            doc_probe: "Cmd+Shift+Z",
            action: "Redo",
        },
        ShortcutReference {
            chord: "Space",
            doc_probe: "`Space`",
            action: "Toggle playback",
        },
        ShortcutReference {
            chord: "Home",
            doc_probe: "`Home`",
            action: "Return to loop start",
        },
        ShortcutReference {
            chord: "R",
            doc_probe: "`R`",
            action: "Toggle recording",
        },
        ShortcutReference {
            chord: "M",
            doc_probe: "`M`",
            action: "Toggle metronome",
        },
        ShortcutReference {
            chord: "Shift+Q",
            doc_probe: "`Shift+Q`",
            action: "Toggle record quantize",
        },
        ShortcutReference {
            chord: "G",
            doc_probe: "`G`",
            action: "Toggle snap",
        },
        ShortcutReference {
            chord: "P",
            doc_probe: "`P`",
            action: "All Off",
        },
        ShortcutReference {
            chord: "N",
            doc_probe: "`N`",
            action: "Add note",
        },
        ShortcutReference {
            chord: "D",
            doc_probe: "`D`",
            action: "Duplicate note",
        },
        ShortcutReference {
            chord: "Q",
            doc_probe: "`Q`",
            action: "Quantize",
        },
        ShortcutReference {
            chord: "Delete",
            doc_probe: "`Delete`",
            action: "Delete note",
        },
        ShortcutReference {
            chord: "Backspace",
            doc_probe: "`Backspace`",
            action: "Delete note",
        },
        ShortcutReference {
            chord: "ArrowLeft",
            doc_probe: "Arrow left/right",
            action: "Move note left",
        },
        ShortcutReference {
            chord: "ArrowRight",
            doc_probe: "Arrow left/right",
            action: "Move note right",
        },
        ShortcutReference {
            chord: "ArrowUp",
            doc_probe: "Arrow up/down",
            action: "Transpose note up",
        },
        ShortcutReference {
            chord: "ArrowDown",
            doc_probe: "Arrow up/down",
            action: "Transpose note down",
        },
        ShortcutReference {
            chord: "Shift+ArrowLeft",
            doc_probe: "`Shift+ArrowLeft`",
            action: "Shorten selected note",
        },
        ShortcutReference {
            chord: "Shift+ArrowRight",
            doc_probe: "`Shift+ArrowRight`",
            action: "Lengthen selected note",
        },
        ShortcutReference {
            chord: "Shift+ArrowUp",
            doc_probe: "`Shift+ArrowUp`",
            action: "Raise note velocity",
        },
        ShortcutReference {
            chord: "Shift+ArrowDown",
            doc_probe: "`Shift+ArrowDown`",
            action: "Lower note velocity",
        },
        ShortcutReference {
            chord: "Ctrl/Cmd+C",
            doc_probe: "Cmd+C",
            action: "Copy note",
        },
        ShortcutReference {
            chord: "Ctrl/Cmd+V",
            doc_probe: "Cmd+V",
            action: "Paste note",
        },
        ShortcutReference {
            chord: "+",
            doc_probe: "`+`",
            action: "Zoom piano roll in",
        },
        ShortcutReference {
            chord: "=",
            doc_probe: "`=`",
            action: "Zoom piano roll in",
        },
        ShortcutReference {
            chord: "-",
            doc_probe: "`-`",
            action: "Zoom piano roll out",
        },
        ShortcutReference {
            chord: "Ctrl/Cmd++",
            doc_probe: "Cmd++",
            action: "Increase UI zoom",
        },
        ShortcutReference {
            chord: "Ctrl/Cmd+=",
            doc_probe: "Cmd+=",
            action: "Increase UI zoom",
        },
        ShortcutReference {
            chord: "Ctrl/Cmd+-",
            doc_probe: "Cmd+-",
            action: "Decrease UI zoom",
        },
        ShortcutReference {
            chord: "Ctrl/Cmd+0",
            doc_probe: "Cmd+0",
            action: "Reset UI zoom",
        },
    ]
}

#[test]
fn documented_keyboard_shortcuts_are_conflict_free() {
    let mut seen = std::collections::HashMap::new();

    for shortcut in documented_shortcuts() {
        assert!(
            seen.insert(shortcut.chord, shortcut.action).is_none(),
            "duplicate shortcut chord documented: {}",
            shortcut.chord
        );
    }
}

#[test]
fn keyboard_shortcut_reference_mentions_documented_chords() {
    let reference = include_str!("../../../docs/keyboard_shortcuts.md");

    for shortcut in documented_shortcuts() {
        assert!(
            reference.contains(shortcut.doc_probe),
            "shortcut reference is missing {} ({})",
            shortcut.chord,
            shortcut.action
        );
    }
}

#[test]
fn alt_modified_plain_shortcuts_do_not_dispatch() {
    let mut app = AppState::for_layout_tests();

    assert!(!handle_key(
        &mut app,
        &Key::Character("r".into()),
        ModifiersState::ALT,
        false,
    ));
    assert!(!app.music_project.lock().transport.recording);

    assert!(!handle_key(
        &mut app,
        &Key::Named(NamedKey::Space),
        ModifiersState::ALT,
        false,
    ));
    assert!(!app.music_project.lock().transport.playing);
}

#[test]
fn alt_modified_command_shortcuts_do_not_dispatch() {
    let mut app = AppState::for_layout_tests();

    assert!(!handle_key(
        &mut app,
        &Key::Character("+".into()),
        ModifiersState::CONTROL | ModifiersState::ALT,
        false,
    ));

    assert_eq!(app.ui_scale(), 1.0);
}

#[test]
fn repeated_transport_shortcuts_do_not_dispatch() {
    let mut app = AppState::for_layout_tests();

    assert!(!handle_key(
        &mut app,
        &Key::Named(NamedKey::Space),
        ModifiersState::empty(),
        true,
    ));

    assert!(!app.music_project.lock().transport.playing);
}

#[test]
fn home_key_returns_transport_to_loop_start_without_dirtying_clean_project() {
    let mut app = AppState::for_layout_tests();
    app.seek_transport_to(5.0);
    assert!(!app.project_dirty);
    assert_eq!(
        app.music_project
            .lock()
            .current_position_beats(std::time::Instant::now()),
        5.0
    );

    assert!(handle_key(
        &mut app,
        &Key::Named(NamedKey::Home),
        ModifiersState::empty(),
        false,
    ));

    assert_eq!(
        app.music_project
            .lock()
            .current_position_beats(std::time::Instant::now()),
        0.0
    );
    assert!(!app.project_dirty);
    assert_eq!(app.last_status, "Returned to start");
}

#[test]
fn repeated_arrow_shortcuts_continue_note_edits() {
    let mut app = AppState::for_layout_tests();
    app.add_clip_note_at(0.0, 69);

    assert!(handle_key(
        &mut app,
        &Key::Named(NamedKey::ArrowRight),
        ModifiersState::empty(),
        true,
    ));
    assert!(handle_key(
        &mut app,
        &Key::Named(NamedKey::ArrowUp),
        ModifiersState::empty(),
        true,
    ));

    let note = app
        .selected_clip_note()
        .expect("selected note should still exist");
    assert_eq!(note.start_beats, 0.25);
    assert_eq!(note.musical_note, 70);
}

#[test]
fn shift_up_down_adjust_selected_note_velocity() {
    let mut app = AppState::for_layout_tests();
    app.add_clip_note_at(0.0, 69);

    assert!(handle_key(
        &mut app,
        &Key::Named(NamedKey::ArrowUp),
        ModifiersState::SHIFT,
        false,
    ));
    assert_eq!(
        app.selected_clip_note().expect("note selected").velocity,
        104
    );
    assert!(app.last_status.starts_with("Changed velocity d1 o0"));

    assert!(handle_key(
        &mut app,
        &Key::Named(NamedKey::ArrowDown),
        ModifiersState::SHIFT,
        true,
    ));
    assert_eq!(
        app.selected_clip_note().expect("note selected").velocity,
        96
    );
}

#[test]
fn n_key_adds_note_at_playhead() {
    let mut app = AppState::for_layout_tests();
    app.seek_transport_to(2.0);
    app.project_dirty = false;

    assert!(handle_key(
        &mut app,
        &Key::Character("n".into()),
        ModifiersState::empty(),
        false,
    ));

    let note = app
        .selected_clip_note()
        .expect("new note should be selected");
    assert!((note.start_beats - 2.0).abs() < 0.001);
    assert_eq!(note.musical_note, app.scale_state.lock().root_midi);
    assert!(app.project_dirty);
    assert!(app.last_status.starts_with("Added note d1 o0 beat 2.00"));
}

#[test]
fn add_clip_note_uses_grid_cell_under_pointer() {
    let mut app = AppState::for_layout_tests();
    app.set_quantize_grid(QuantizeGrid::Sixteenth);
    let root = app.scale_state.lock().root_midi;

    app.add_clip_note_at(4.14, root);
    let note = app
        .selected_clip_note()
        .expect("created note should be selected");
    assert!((note.start_beats - 4.0).abs() < 0.001);

    app.add_clip_note_at(4.25, root);
    let note = app
        .selected_clip_note()
        .expect("created note should be selected");
    assert!((note.start_beats - 4.25).abs() < 0.001);
}

#[test]
fn command_c_v_copy_and_paste_selected_note_at_playhead() {
    let mut app = AppState::for_layout_tests();
    app.add_clip_note_at(0.0, 69);
    let source_id = app.selected_clip_note.expect("note selected after add");
    app.set_selected_clip_note_velocity(104);
    app.seek_transport_to(2.0);
    app.project_dirty = false;

    assert!(handle_key(
        &mut app,
        &Key::Character("c".into()),
        ModifiersState::CONTROL,
        false,
    ));
    assert!(!app.project_dirty);
    assert!(app.last_status.starts_with("Copied note d1 o0"));

    assert!(handle_key(
        &mut app,
        &Key::Character("v".into()),
        ModifiersState::CONTROL,
        false,
    ));

    let pasted = app
        .selected_clip_note()
        .expect("pasted note should be selected");
    assert_ne!(pasted.id, source_id);
    assert_eq!(pasted.start_beats, 2.0);
    assert_eq!(pasted.velocity, 104);
    assert_eq!(app.music_project.lock().clip.notes.len(), 2);
    assert!(app.project_dirty);
    assert!(app.last_status.starts_with("Pasted note d1 o0 beat 2.00"));
}

#[test]
fn text_overlap_detector_reports_collisions() {
    let boxes = vec![
        TextBox {
            source: "alpha".to_string(),
            text: "Alpha".to_string(),
            allocated: UiRect::new(0.0, 0.0, 80.0, 20.0),
            visible: UiRect::new(0.0, 0.0, 80.0, 20.0),
        },
        TextBox {
            source: "beta".to_string(),
            text: "Beta".to_string(),
            allocated: UiRect::new(60.0, 0.0, 80.0, 20.0),
            visible: UiRect::new(60.0, 0.0, 80.0, 20.0),
        },
    ];
    let issues = text_overlap_issues(&boxes);

    assert_eq!(issues.len(), 1);
    assert!(issues[0].contains("alpha"));
    assert!(issues[0].contains("beta"));
}

#[test]
fn audio_label_marks_disconnected_streams_without_duplicate_default() {
    let mut app = populated_layout_test_app();
    app.audio_stream = None;

    assert_eq!(
        selected_audio_output_name(&app),
        "Audio 1/1 disconnected default"
    );
}

#[test]
fn audio_label_reports_selected_device_position() {
    let mut app = AppState::for_layout_tests();
    app.audio_outputs = vec![
        crate::audio::AudioOutputDevice {
            name: "Built-in Output".to_string(),
            is_default: true,
        },
        crate::audio::AudioOutputDevice {
            name: "USB Interface".to_string(),
            is_default: false,
        },
    ];
    app.selected_audio_output = 1;

    assert_eq!(
        selected_audio_output_name(&app),
        "Audio 2/2 disconnected USB Interface"
    );
}

#[test]
fn audio_label_prefers_actual_connected_output_over_selected_output() {
    assert_eq!(
        audio_output_status_label(true, Some("Built-in Output"), "USB Interface", None),
        "Audio connected USB Interface"
    );
    assert_eq!(
        audio_output_status_label(
            true,
            Some("Built-in Output"),
            "USB Interface",
            Some(&AudioStreamInfo {
                sample_rate_hz: 48_000,
                channels: 2,
                sample_format: "F32".to_string(),
                buffer_frames: Some(256),
            }),
        ),
        "Audio connected USB Interface 48 kHz 2ch F32 256f 5.3ms"
    );
}

#[test]
fn midi_label_distinguishes_missing_and_disconnected_inputs() {
    let mut app = AppState::for_layout_tests();

    assert_eq!(selected_midi_input_name(&app), "MIDI no input");

    app.midi_inputs = vec!["Midi Through:Midi Through Port-0 14:0".to_string()];

    assert_eq!(
        selected_midi_input_name(&app),
        "MIDI 1/1 disconnected Midi Through:Midi Thro..."
    );
}

#[test]
fn midi_label_reports_selected_device_position() {
    let mut app = AppState::for_layout_tests();
    app.midi_inputs = vec![
        "Keyboard".to_string(),
        "Lumatone Isomorphic Keyboard".to_string(),
    ];
    app.selected_input = 1;

    assert_eq!(
        selected_midi_input_name(&app),
        "MIDI 2/2 disconnected Lumatone Isomorphic Ke..."
    );
}

#[test]
fn midi_label_prefers_actual_connected_input_over_selected_input() {
    assert_eq!(
        midi_input_status_label(
            true,
            Some("Fallback Keyboard"),
            "Lumatone Isomorphic Keyboard"
        ),
        "MIDI connected Lumatone Isomorphic Keybo..."
    );
}

#[test]
fn device_status_label_reports_connection_state() {
    assert_eq!(
        device_status_label("MIDI", true, "no input", Some("Keyboard"), 20),
        "MIDI connected Keyboard"
    );
    assert_eq!(
        device_status_label("MIDI", false, "no input", Some("Keyboard"), 20),
        "MIDI disconnected Keyboard"
    );
    assert_eq!(
        device_status_label("Audio", false, "no output", None, 20),
        "Audio no output"
    );
    assert_eq!(
        device_label_with_position("Audio disconnected USB".to_string(), "Audio", 2, 4),
        "Audio 3/4 disconnected USB"
    );
}

#[test]
fn status_bar_does_not_repeat_device_summary_in_footer() {
    let app = AppState::for_layout_tests();
    let device_summary = format!(
        "{}   {}",
        selected_midi_input_name(&app),
        selected_audio_output_name(&app)
    );

    let text = collect_surface_text_boxes(&app, 1920.0, 1080.0);

    assert!(!text.iter().any(|item| item.text == device_summary));
}

#[test]
fn status_bar_label_fits_long_status_messages() {
    let mut app = AppState::for_layout_tests();
    app.last_status = format!(
        "Loaded project: /very/long/path/{}",
        "nested-directory/".repeat(16)
    );

    let label = status_bar_label(&app, 360.0);

    assert!(label.ends_with("..."));
    assert!(estimated_text_width(&label, 12.0) <= 344.0);
}

#[test]
fn device_connect_label_reports_connect_or_reconnect() {
    assert_eq!(device_connect_label(false, true, "Connect MIDI"), "Connect");
    assert_eq!(
        device_connect_label(false, false, "Connect MIDI"),
        "Connect MIDI"
    );
    assert_eq!(
        device_connect_label(false, false, "Connect Audio"),
        "Connect Audio"
    );
    assert_eq!(
        device_connect_label(true, true, "Connect MIDI"),
        "Reconnect"
    );
    assert_eq!(
        device_connect_label(true, false, "Connect MIDI"),
        "Reconnect"
    );
}

#[test]
fn connect_action_only_reconnects_selected_connected_device() {
    assert!(selected_name_matches_connected(
        Some("Keyboard"),
        "Keyboard"
    ));
    assert!(!selected_name_matches_connected(
        Some("Fallback Keyboard"),
        "Keyboard"
    ));
    assert!(!selected_name_matches_connected(None, "Keyboard"));
    assert!(!selected_name_matches_connected(Some("Keyboard"), ""));
}

#[test]
fn selected_device_status_reports_pending_device_switch() {
    assert_eq!(
        selected_device_status("MIDI input", "Keyboard", ""),
        "Selected MIDI input: Keyboard"
    );
    assert_eq!(
        selected_device_status("MIDI input", "Keyboard", "Keyboard"),
        "Selected MIDI input: Keyboard (connected)"
    );
    assert_eq!(
        selected_device_status("audio output", "Built-in", "USB Interface"),
        "Selected audio output: Built-in; click Connect to switch"
    );
}

#[test]
fn project_file_state_label_reports_file_and_dirty_state() {
    let mut app = AppState::for_layout_tests();

    assert_eq!(project_file_state_label(&app), "No file");

    app.project_dirty = true;
    assert_eq!(project_file_state_label(&app), "Unsaved");

    app.project_path = Some(PathBuf::from("session.orbifold"));
    assert_eq!(project_file_state_label(&app), "Unsaved changes");

    app.project_dirty = false;
    assert_eq!(project_file_state_label(&app), "Saved");
}

#[test]
fn project_location_label_reports_unsaved_and_saved_location() {
    let mut app = AppState::for_layout_tests();

    assert_eq!(project_location_label(&app), "Save to choose file");

    let recent = std::env::temp_dir().join(format!(
        "orbifold_recent_label_{}.orbifold",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&recent);
    app.save_project_to_path(recent.clone());
    app.project_path = None;
    assert_eq!(
        project_location_label(&app),
        format!(
            "Recent: {}",
            compact_label(
                recent.file_stem().and_then(|value| value.to_str()).unwrap(),
                28
            )
        )
    );

    app.project_dirty = true;
    assert_eq!(project_location_label(&app), "Unsaved changes");

    app.project_path = Some(PathBuf::from("/tmp/orbifold/session.orbifold"));

    assert_eq!(project_location_label(&app), "/tmp/orbifold");
    let _ = std::fs::remove_file(recent);
}

#[test]
fn recent_project_row_label_reports_modified_age_and_missing_state() {
    assert_eq!(compact_age_label(Duration::from_secs(12)), "now");
    assert_eq!(compact_age_label(Duration::from_secs(5 * 60)), "5m");
    assert_eq!(compact_age_label(Duration::from_secs(2 * 60 * 60)), "2h");
    assert_eq!(
        compact_age_label(Duration::from_secs(3 * 24 * 60 * 60)),
        "3d"
    );

    let recent = std::env::temp_dir().join(format!(
        "orbifold_recent_modified_label_{}.orbifold",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&recent);
    std::fs::write(&recent, "orbifold_project=1\n").expect("write recent project label file");

    let label = recent_project_row_label(0, &recent);

    assert!(label.starts_with("1 orbifold_recent_mo"));
    assert!(label.ends_with(" now"));

    let _ = std::fs::remove_file(&recent);
    let missing_label = recent_project_row_label(0, &recent);

    assert!(missing_label.starts_with("1 Missing orbifold_recent"));
}

#[test]
fn current_scale_label_identifies_the_active_scale() {
    let mut app = AppState::for_layout_tests();
    app.show_scale_browser = true;

    assert_eq!(current_scale_label(&app), "Current: 12-TET  12 notes");
    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);
    assert!(
        text.iter()
            .any(|item| item.text == "Current: 12-TET  12 notes")
    );
    assert_text_overlap_free("current-scale-label", &text);
}

#[test]
fn open_recent_button_is_available_only_for_clean_recent_project_state() {
    let mut app = AppState::for_layout_tests();
    let recent = std::env::temp_dir().join(format!(
        "orbifold_open_recent_surface_{}.orbifold",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&recent);

    assert!(!surface_node_exists(&app, "file.open_recent"));

    app.save_project_to_path(recent.clone());
    app.start_new_project();

    assert!(surface_node_enabled(&app, "file.open_recent"));
    assert!(surface_node_enabled(&app, "file.forget_recent"));

    app.add_clip_note_at(0.0, 69);

    assert!(!surface_node_enabled(&app, "file.open_recent"));
    assert!(surface_node_enabled(&app, "file.forget_recent"));

    let _ = std::fs::remove_file(recent);
}

#[test]
fn pointer_clicking_forget_recent_removes_latest_recent_without_deleting_file() {
    let mut app = AppState::for_layout_tests();
    let recent = std::env::temp_dir().join(format!(
        "orbifold_pointer_forget_recent_{}.orbifold",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&recent);
    app.save_project_to_path(recent.clone());
    app.start_new_project();

    let action = click_surface_node(&mut app, "file.forget_recent", 1200.0, 760.0);

    assert_eq!(action.as_deref(), Some("file.forget_recent"));
    assert!(app.recent_project_paths().is_empty());
    assert!(recent.exists());
    assert_eq!(
        app.last_status,
        format!("Forgot recent project: {}", recent.display())
    );

    let _ = std::fs::remove_file(recent);
}

#[test]
fn project_panel_reports_empty_recent_history() {
    let app = AppState::for_layout_tests();
    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(!surface_node_exists(&app, "file.open_recent"));
    assert!(!text.iter().any(|item| item.text == "No recent projects"));
    assert_text_overlap_free("empty-recent-projects", &text);
}

#[test]
fn compact_recent_project_actions_are_visible_and_dirty_disabled() {
    let mut app = AppState::for_layout_tests();
    let first = std::env::temp_dir().join(format!(
        "orbifold_recent_row_first_{}.orbifold",
        std::process::id()
    ));
    let second = std::env::temp_dir().join(format!(
        "orbifold_recent_row_second_{}.orbifold",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&first);
    let _ = std::fs::remove_file(&second);
    app.save_project_to_path(first.clone());
    app.save_project_to_path(second.clone());
    app.start_new_project();

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(surface_node_enabled(&app, "file.open_recent"));
    assert!(surface_node_enabled(&app, "file.forget_recent"));
    assert!(text.iter().any(|item| item.text == "Recent"));
    assert!(text.iter().any(|item| item.text == "Forget"));
    assert_text_overlap_free("recent-project-rows", &text);

    app.add_clip_note_at(0.0, 69);

    assert!(!surface_node_enabled(&app, "file.open_recent"));
    assert!(surface_node_enabled(&app, "file.forget_recent"));

    let _ = std::fs::remove_file(first);
    let _ = std::fs::remove_file(second);
}

#[test]
fn missing_recent_project_is_only_forgettable() {
    let mut app = AppState::for_layout_tests();
    let missing = std::env::temp_dir().join(format!(
        "orbifold_missing_recent_row_{}.orbifold",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&missing);
    app.save_project_to_path(missing.clone());
    app.start_new_project();
    let _ = std::fs::remove_file(&missing);

    let text = collect_surface_text_boxes(&app, 1200.0, 760.0);

    assert!(!surface_node_enabled(&app, "file.open_recent"));
    assert!(surface_node_enabled(&app, "file.forget_recent"));
    assert!(text.iter().any(|item| item.text == "Forget"));

    let action = click_surface_node(&mut app, "file.forget_recent", 1200.0, 760.0);

    assert_eq!(action.as_deref(), Some("file.forget_recent"));
    assert!(app.recent_project_paths().is_empty());
    assert_eq!(
        app.last_status,
        format!("Forgot recent project: {}", missing.display())
    );

    let _ = std::fs::remove_file(missing);
}

#[test]
fn pointer_clicking_open_recent_opens_most_recent_project() {
    let mut app = AppState::for_layout_tests();
    let first = std::env::temp_dir().join(format!(
        "orbifold_pointer_recent_row_first_{}.orbifold",
        std::process::id()
    ));
    let second = std::env::temp_dir().join(format!(
        "orbifold_pointer_recent_row_second_{}.orbifold",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&first);
    let _ = std::fs::remove_file(&second);
    let root = app.scale_state.lock().root_midi;
    app.add_clip_note_at(0.0, root);
    app.save_project_to_path(first.clone());
    app.add_clip_note_at(1.0, root + 2);
    app.save_project_to_path(second.clone());
    app.start_new_project();

    let action = click_surface_node(&mut app, "file.open_recent", 1200.0, 760.0);

    assert_eq!(action.as_deref(), Some("file.open_recent"));
    assert_eq!(app.project_path.as_ref(), Some(&second));
    assert_eq!(app.music_project.lock().clip.notes.len(), 2);
    assert!(app.last_status.starts_with("Loaded project:"));

    let _ = std::fs::remove_file(first);
    let _ = std::fs::remove_file(second);
}

#[test]
fn pointer_clicking_forget_recent_removes_most_recent_only() {
    let mut app = AppState::for_layout_tests();
    let first = std::env::temp_dir().join(format!(
        "orbifold_pointer_recent_forget_first_{}.orbifold",
        std::process::id()
    ));
    let second = std::env::temp_dir().join(format!(
        "orbifold_pointer_recent_forget_second_{}.orbifold",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&first);
    let _ = std::fs::remove_file(&second);
    app.save_project_to_path(first.clone());
    app.save_project_to_path(second.clone());
    app.start_new_project();

    let action = click_surface_node(&mut app, "file.forget_recent", 1200.0, 760.0);

    assert_eq!(action.as_deref(), Some("file.forget_recent"));
    assert_eq!(app.recent_project_paths(), std::slice::from_ref(&first));
    assert!(first.exists());
    assert!(second.exists());
    assert_eq!(
        app.last_status,
        format!("Forgot recent project: {}", second.display())
    );

    let _ = std::fs::remove_file(first);
    let _ = std::fs::remove_file(second);
}

#[test]
fn note_drag_from_action_parses_note_drag_actions_only() {
    assert!(matches!(
        note_drag_from_action("note.select.42"),
        Some((42, NoteDragMode::Move))
    ));
    assert!(matches!(
        note_drag_from_action("note.resize_start.42"),
        Some((42, NoteDragMode::ResizeStart))
    ));
    assert!(matches!(
        note_drag_from_action("note.resize_end.42"),
        Some((42, NoteDragMode::ResizeEnd))
    ));
    assert!(matches!(
        note_drag_from_action("note.velocity.42"),
        Some((42, NoteDragMode::Velocity))
    ));
    assert!(note_drag_from_action("clip.add_note").is_none());
    assert!(note_drag_from_action("note.select.not-a-number").is_none());
}

#[test]
fn note_resize_edges_leave_tiny_notes_selectable() {
    assert_eq!(note_resize_edge_width(6.0), None);
    assert_eq!(note_resize_edge_width(17.9), None);
    assert_eq!(note_resize_edge_width(18.0), Some(4.5));
    assert_eq!(note_resize_edge_width(64.0), Some(8.0));
}

fn populated_layout_test_app() -> AppState {
    let mut app = AppState::for_layout_tests();
    app.scale_library = [
        ("12-TET", "12.scl"),
        ("19-EDO (Equal)", "19.scl"),
        ("31-EDO (Equal)", "31.scl"),
        ("Just Intonation Basic", "ji.scl"),
        ("Pythagorean", "pythagorean.scl"),
        ("Meantone", "meantone.scl"),
        ("Werckmeister III", "werckmeister.scl"),
        ("Bohlen-Pierce", "bohlen_pierce.scl"),
    ]
    .into_iter()
    .map(|(name, path)| ScaleLibraryItem {
        name: name.to_string(),
        path: PathBuf::from(path),
    })
    .collect();
    app.audio_assets = [
        ("Drums", AudioAssetKind::Sample, true),
        ("Textures", AudioAssetKind::Sample, true),
        ("Samples", AudioAssetKind::Sample, true),
        ("Synths", AudioAssetKind::Instrument, true),
        ("Analog", AudioAssetKind::Instrument, true),
        ("Wavetables", AudioAssetKind::Instrument, true),
        ("Pads", AudioAssetKind::Instrument, true),
        ("Plucks", AudioAssetKind::Instrument, true),
        ("Bass", AudioAssetKind::Instrument, true),
        ("Field Recordings", AudioAssetKind::Sample, true),
        ("Micronaut Presets", AudioAssetKind::Preset, true),
        ("Audio Effects", AudioAssetKind::Preset, true),
    ]
    .into_iter()
    .map(|(name, kind, is_dir)| AudioAssetItem {
        name: name.to_string(),
        path: PathBuf::from(name),
        kind,
        is_dir,
    })
    .collect();
    app.midi_inputs = vec!["Midi Through:Midi Through Port-0 14:0".to_string()];
    app.audio_outputs = vec![AudioOutputDevice {
        name: "default".to_string(),
        is_default: true,
    }];
    app.last_status = "Connected MIDI input: Midi Through:Midi Through Port-0 14:0".to_string();
    app
}

fn collect_surface_text_boxes(app: &AppState, width: f32, height: f32) -> Vec<TextBox> {
    let mut document = build_surface_document(app, width, height);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(UiSize::new(width, height), &mut text_measurer)
        .expect("surface layout should compute");
    let paint = document.paint_list();
    let mut text = Vec::new();
    collect_text_boxes_from_paint(&document, &paint, &mut text);
    text
}

fn surface_node_enabled(app: &AppState, name: &str) -> bool {
    let document = build_surface_document(app, 1200.0, 760.0);
    document
        .nodes()
        .iter()
        .find(|node| node.name == name)
        .and_then(|node| node.accessibility.as_ref())
        .unwrap_or_else(|| panic!("missing accessibility for node {name}"))
        .enabled
}

fn surface_node_exists(app: &AppState, name: &str) -> bool {
    let document = build_surface_document(app, 1200.0, 760.0);
    document.nodes().iter().any(|node| node.name == name)
}

fn click_surface_node(app: &mut AppState, name: &str, width: f32, height: f32) -> Option<String> {
    let layout = surface_rects(app, width, height);
    let mut document = build_surface_document(app, width, height);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(UiSize::new(width, height), &mut text_measurer)
        .expect("surface layout should compute");
    let point = surface_node_center(&document, name);
    let down = document.handle_input(UiInputEvent::PointerDown(point));
    assert_eq!(down.clicked, None);
    let up = document.handle_input(UiInputEvent::PointerUp(point));
    let action = up
        .clicked
        .map(|clicked| document.node(clicked).name.clone());
    if let Some(action) = action.as_deref() {
        dispatch_action(app, action, Some(point), Some(layout));
    }
    action
}

fn surface_node_center(document: &UiDocument, name: &str) -> UiPoint {
    let node = document
        .nodes()
        .iter()
        .find(|node| node.name == name)
        .unwrap_or_else(|| panic!("missing node {name}"));
    UiPoint::new(
        node.layout.rect.x + node.layout.rect.width * 0.5,
        node.layout.rect.y + node.layout.rect.height * 0.5,
    )
}

fn rect_center(rect: UiRect) -> UiPoint {
    UiPoint::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5)
}

fn piano_pointer_input(
    layout: SurfaceRects,
    kind: PointerEventKind,
    point: UiPoint,
    buttons: PointerButtons,
    timestamp_millis: u64,
) -> NativeCanvasInput {
    NativeCanvasInput {
        node: UiNodeId(0),
        key: PIANO_INPUT_CANVAS_KEY.to_string(),
        rect: layout.piano_roll,
        local_position: Some(UiPoint::new(
            point.x - layout.piano_roll.x,
            point.y - layout.piano_roll.y,
        )),
        input: RawInputEvent::Pointer(
            RawPointerEvent::new(kind, point, timestamp_millis).buttons(buttons),
        ),
    }
}

fn piano_wheel_input(
    layout: SurfaceRects,
    point: UiPoint,
    delta: UiPoint,
    modifiers: operad::KeyModifiers,
) -> NativeCanvasInput {
    NativeCanvasInput {
        node: UiNodeId(0),
        key: PIANO_INPUT_CANVAS_KEY.to_string(),
        rect: layout.piano_roll,
        local_position: Some(UiPoint::new(
            point.x - layout.piano_roll.x,
            point.y - layout.piano_roll.y,
        )),
        input: RawInputEvent::Wheel(
            operad::RawWheelEvent::pixels(point, delta, 0).modifiers(modifiers),
        ),
    }
}

fn solid_test_pixels(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let mut pixels = vec![0; width as usize * height as usize * 4];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&color);
    }
    pixels
}

fn fill_test_rect(
    pixels: &mut [u8],
    image_width: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
) {
    for row in y..y + height {
        for col in x..x + width {
            let index = ((row as usize * image_width as usize) + col as usize) * 4;
            pixels[index..index + 4].copy_from_slice(&color);
        }
    }
}

fn collect_text_boxes_from_paint(document: &UiDocument, paint: &PaintList, out: &mut Vec<TextBox>) {
    for item in &paint.items {
        match &item.kind {
            PaintKind::Text(content) if !content.text.trim().is_empty() => {
                let allocated = item.transform.transform_rect(item.rect);
                let clip = item.transform.transform_rect(item.clip_rect);
                let estimated = estimated_text_rect(
                    allocated,
                    &content.text,
                    &content.style,
                    TextHorizontalAlign::Start,
                    TextVerticalAlign::Top,
                );
                if let Some(visible) = intersect_rect(estimated, clip) {
                    out.push(TextBox {
                        source: document.node(item.node).name.clone(),
                        text: content.text.clone(),
                        allocated,
                        visible,
                    });
                }
            }
            PaintKind::SceneText(text) if !text.text.trim().is_empty() => {
                let allocated = item.transform.transform_rect(text.rect);
                let clip = item.transform.transform_rect(item.clip_rect);
                let estimated = estimated_text_rect(
                    allocated,
                    &text.text,
                    &text.style,
                    text.horizontal_align,
                    text.vertical_align,
                );
                if let Some(visible) = intersect_rect(estimated, clip) {
                    out.push(TextBox {
                        source: format!("{}:{}", document.node(item.node).name, text.text),
                        text: text.text.clone(),
                        allocated,
                        visible,
                    });
                }
            }
            PaintKind::CompositedLayer(layer) => {
                collect_text_boxes_from_paint(document, &layer.paint, out);
            }
            _ => {}
        }
    }
}

fn paint_item_count(paint: &PaintList) -> usize {
    paint
        .items
        .iter()
        .map(|item| {
            1 + match &item.kind {
                PaintKind::CompositedLayer(layer) => paint_item_count(&layer.paint),
                _ => 0,
            }
        })
        .sum()
}

fn estimated_text_rect(
    allocated: UiRect,
    text: &str,
    style: &TextStyle,
    horizontal_align: TextHorizontalAlign,
    vertical_align: TextVerticalAlign,
) -> UiRect {
    let (width, height) = estimated_text_size(text, style);
    let x = match horizontal_align {
        TextHorizontalAlign::Start => allocated.x,
        TextHorizontalAlign::Center => allocated.x + (allocated.width - width) * 0.5,
        TextHorizontalAlign::End => allocated.right() - width,
    };
    let y = match vertical_align {
        TextVerticalAlign::Top | TextVerticalAlign::Baseline => allocated.y,
        TextVerticalAlign::Center => allocated.y + (allocated.height - height) * 0.5,
        TextVerticalAlign::Bottom => allocated.bottom() - height,
    };
    UiRect::new(x, y, width.max(0.0), height.max(0.0))
}

fn estimated_text_size(text: &str, style: &TextStyle) -> (f32, f32) {
    let lines = text.lines().collect::<Vec<_>>();
    let line_count = lines.len().max(1) as f32;
    let width = lines
        .iter()
        .map(|line| estimated_line_width(line, style.font_size))
        .fold(0.0, f32::max);
    (width, style.line_height.max(style.font_size) * line_count)
}

fn estimated_line_width(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|ch| {
            let ratio = if ch.is_whitespace() {
                0.33
            } else if matches!(ch, 'i' | 'l' | 'I' | '|' | '.' | ',' | ':' | ';' | '\'') {
                0.30
            } else if matches!(ch, 'm' | 'w' | 'M' | 'W' | '@') {
                0.82
            } else if ch.is_ascii_uppercase() || ch.is_ascii_digit() {
                0.62
            } else {
                0.56
            };
            ratio * font_size
        })
        .sum()
}

fn assert_text_overlap_free(viewport: &str, boxes: &[TextBox]) {
    let issues = text_overlap_issues(boxes);
    assert!(
        issues.is_empty(),
        "{viewport} has overlapping text:\n{}",
        issues.join("\n")
    );
}

fn assert_text_allocations_are_finite(viewport: &str, boxes: &[TextBox]) {
    let issues = boxes
        .iter()
        .filter(|text| {
            !text.allocated.x.is_finite()
                || !text.allocated.y.is_finite()
                || !text.allocated.width.is_finite()
                || !text.allocated.height.is_finite()
                || !text.visible.x.is_finite()
                || !text.visible.y.is_finite()
                || !text.visible.width.is_finite()
                || !text.visible.height.is_finite()
        })
        .map(|text| format!("{} `{}` {:?}", text.source, text.text, text.visible))
        .collect::<Vec<_>>();
    assert!(
        issues.is_empty(),
        "{viewport} has non-finite text allocations:\n{}",
        issues.join("\n")
    );
}

fn text_overlap_issues(boxes: &[TextBox]) -> Vec<String> {
    let mut issues = Vec::new();
    for (left_idx, left) in boxes.iter().enumerate() {
        for right in boxes.iter().skip(left_idx + 1) {
            let Some(overlap) = intersect_rect(left.visible, right.visible) else {
                continue;
            };
            if overlap.width * overlap.height <= TEXT_OVERLAP_TOLERANCE {
                continue;
            }
            issues.push(format!(
                "{} `{}` {:?} overlaps {} `{}` {:?} by {:?}",
                left.source,
                left.text,
                left.visible,
                right.source,
                right.text,
                right.visible,
                overlap
            ));
        }
    }
    issues
}

fn intersect_rect(a: UiRect, b: UiRect) -> Option<UiRect> {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = a.right().min(b.right());
    let y2 = a.bottom().min(b.bottom());
    (x2 > x1 && y2 > y1).then(|| UiRect::new(x1, y1, x2 - x1, y2 - y1))
}
