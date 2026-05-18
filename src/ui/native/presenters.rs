use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::app::{
    AppState, AudioAssetItem, AudioAssetKind, ScaleLibraryItem, audio_asset_workflow_status,
};
use crate::project::ClipNote;
use crate::sample_preview::{WavPreviewInfo, read_wav_preview_info};
use crate::ui::text::compact_label;
use crate::ui::text::fit_label;

pub(super) fn asset_tab_label(kind: AudioAssetKind) -> &'static str {
    kind.label()
}

pub(super) fn visible_list_start(selected: usize, total: usize, visible_rows: usize) -> usize {
    if total <= visible_rows || visible_rows == 0 {
        return 0;
    }
    let selected = selected.min(total.saturating_sub(1));
    selected
        .saturating_sub(visible_rows - 1)
        .min(total - visible_rows)
}

#[cfg(test)]
pub(super) fn project_location_label(app: &AppState) -> String {
    let Some(path) = app.project_path.as_ref() else {
        if app.project_dirty {
            return "Unsaved changes".to_string();
        }
        if let Some(recent) = recent_project_display_name(app) {
            return format!("Recent: {recent}");
        }
        return "Save to choose file".to_string();
    };
    path.parent()
        .and_then(|parent| parent.to_str())
        .filter(|parent| !parent.is_empty())
        .unwrap_or(".")
        .to_string()
}

#[cfg(test)]
pub(super) fn recent_project_display_name(app: &AppState) -> Option<String> {
    app.recent_project_paths()
        .first()
        .and_then(|path| path.file_stem().or_else(|| path.file_name()))
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(|value| compact_label(value, 28))
}

pub(super) fn recent_project_row_label(index: usize, path: &Path) -> String {
    let name = path
        .file_stem()
        .or_else(|| path.file_name())
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("project");
    if !path.exists() {
        return format!("{} Missing {}", index + 1, compact_label(name, 16));
    }
    match file_modified_age_label(path) {
        Some(age) => format!("{} {} {}", index + 1, compact_label(name, 18), age),
        None => format!("{} {}", index + 1, compact_label(name, 24)),
    }
}

pub(super) fn file_modified_age_label(path: &Path) -> Option<String> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) => {
            log::error!("Failed to read metadata for {}: {err}", path.display());
            return None;
        }
    };
    let modified = match metadata.modified() {
        Ok(modified) => modified,
        Err(err) => {
            log::error!(
                "Failed to read modification time for {}: {err}",
                path.display()
            );
            return None;
        }
    };
    let elapsed = match SystemTime::now().duration_since(modified) {
        Ok(elapsed) => elapsed,
        Err(err) => {
            log::error!(
                "Modification time for {} is in the future: {err}; using age 0",
                path.display()
            );
            Duration::from_secs(0)
        }
    };
    Some(compact_age_label(elapsed))
}

pub(super) fn compact_age_label(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 60 {
        "now".to_string()
    } else if seconds < 60 * 60 {
        format!("{}m", seconds / 60)
    } else if seconds < 24 * 60 * 60 {
        format!("{}h", seconds / (60 * 60))
    } else {
        format!("{}d", seconds / (24 * 60 * 60))
    }
}

pub(super) fn scale_library_row_label(item: &ScaleLibraryItem) -> String {
    let source = scale_library_source_label(&item.path);
    if !item.path.exists() {
        return format!("Missing {} - {source}", item.name);
    }
    format!("{} - {source}", item.name)
}

fn scale_library_source_label(path: &Path) -> String {
    if path.starts_with("scales") {
        return "bundled".to_string();
    }
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return "user".to_string();
    };
    let label = if path.is_absolute() {
        parent
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("user")
    } else {
        parent
            .to_str()
            .filter(|name| !name.is_empty())
            .unwrap_or("user")
    };
    compact_label(label, 18)
}

pub(super) fn audio_asset_row_label(item: &AudioAssetItem) -> String {
    let name = if item.is_dir {
        format!("> {}", item.name)
    } else {
        item.name.clone()
    };
    if !item.path.exists() {
        return format!("Missing {name}");
    }
    if !item.is_dir
        && let Some(size) = audio_asset_file_size_label(&item.path)
    {
        return format!("{name} {size}");
    }
    name
}

pub(super) fn audio_asset_file_size_label(path: &Path) -> Option<String> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) => {
            log::error!(
                "Failed to read asset metadata for {}: {err}",
                path.display()
            );
            return None;
        }
    };
    if !metadata.is_file() {
        return None;
    }
    Some(format_file_size(metadata.len()))
}

fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format_decimal_size(bytes as f64 / KB, "KB")
    } else if bytes < 1024 * 1024 * 1024 {
        format_decimal_size(bytes as f64 / MB, "MB")
    } else {
        format_decimal_size(bytes as f64 / GB, "GB")
    }
}

fn format_decimal_size(value: f64, unit: &str) -> String {
    if value >= 10.0 {
        format!("{value:.0} {unit}")
    } else {
        format!("{value:.1} {unit}")
    }
}

#[cfg(test)]
pub(super) fn project_file_state_label(app: &AppState) -> &'static str {
    if app.project_path.is_none() && app.project_dirty {
        "Unsaved"
    } else if app.project_path.is_none() {
        "No file"
    } else if app.project_dirty {
        "Unsaved changes"
    } else {
        "Saved"
    }
}

pub(super) fn transport_position_label(position_beats: f32) -> String {
    let position = position_beats.max(0.0);
    let bar = (position / 4.0).floor() as i32 + 1;
    let beat = (position.rem_euclid(4.0).floor() as i32) + 1;
    format!("Bar {bar}.{beat}")
}

pub(super) fn current_scale_label(app: &AppState) -> String {
    let scale = app.scale_state.lock().clone();
    format!(
        "Current: {}  {} notes",
        scale.scale.description,
        scale.scale.steps.len()
    )
}

pub(super) fn current_scale_metadata_label(app: &AppState) -> Option<String> {
    let scale = app.scale_state.lock().clone();
    equal_division_metadata(&scale.scale.steps).map(|step_cents| {
        let label = if scale.scale.description.to_ascii_lowercase().contains("tet") {
            format!("{}-TET", scale.scale.steps.len())
        } else {
            format!("{}-EDO", scale.scale.steps.len())
        };
        format!("Equal division: {label}, {step_cents:.2}c step")
    })
}

pub(super) fn current_scale_intervals_label(app: &AppState) -> String {
    let scale = app.scale_state.lock().clone();
    let mut intervals = scale
        .scale
        .steps
        .iter()
        .copied()
        .filter(|ratio| ratio.is_finite() && *ratio > 0.0)
        .take(6)
        .map(|ratio| format_cents(1200.0 * ratio.log2()))
        .collect::<Vec<_>>();
    if scale.scale.steps.len() > intervals.len() {
        intervals.push("...".to_string());
    }
    if intervals.is_empty() {
        "Intervals unavailable".to_string()
    } else {
        format!("Intervals: {}", intervals.join(", "))
    }
}

fn equal_division_metadata(steps: &[f32]) -> Option<f32> {
    if steps.len() < 2 {
        return None;
    }
    let step_cents = 1200.0 / steps.len() as f32;
    let max_error = steps
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, ratio)| ratio.is_finite() && *ratio > 0.0)
        .map(|(idx, ratio)| {
            let expected = idx as f32 * step_cents;
            (1200.0 * ratio.log2() - expected).abs()
        })
        .fold(0.0_f32, f32::max);
    (max_error < 0.05).then_some(step_cents)
}

fn format_cents(cents: f32) -> String {
    if !cents.is_finite() {
        return "?c".to_string();
    }
    if (cents - cents.round()).abs() < 0.05 {
        format!("{:.0}c", cents.round())
    } else {
        format!("{cents:.1}c")
    }
}

pub(super) fn asset_browser_summary(app: &AppState) -> String {
    let total = app.total_audio_asset_count(app.selected_audio_asset_kind);
    let visible = app.filtered_audio_asset_count(app.selected_audio_asset_kind);
    if app.audio_asset_search_query().is_empty() {
        format!("{}  {total}", app.selected_audio_asset_kind.label())
    } else {
        format!(
            "{}  {visible}/{total}",
            app.selected_audio_asset_kind.label()
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SelectedAssetDetail {
    pub(super) title: String,
    pub(super) status: String,
}

pub(super) fn selected_audio_asset_detail(app: &AppState) -> SelectedAssetDetail {
    let Some(asset) = app.selected_audio_asset_item() else {
        return SelectedAssetDetail {
            title: "No selection".to_string(),
            status: "Choose an asset row".to_string(),
        };
    };
    let mut status = selected_audio_asset_status(app, asset);
    if !asset.is_dir
        && asset.path.exists()
        && let Some(size) = audio_asset_file_size_label(&asset.path)
    {
        status = format!("{status}  {size}");
    }
    SelectedAssetDetail {
        title: compact_label(&asset.name, 34),
        status,
    }
}

fn selected_audio_asset_status(app: &AppState, asset: &AudioAssetItem) -> String {
    if is_wav_sample_file(asset)
        && asset.path.exists()
        && let Ok(info) = read_wav_preview_info(&asset.path)
    {
        if app
            .sample_instrument_assignment
            .as_ref()
            .is_some_and(|assignment| assignment.path == asset.path)
        {
            return format!("Loaded instrument  {}", wav_preview_info_label(info));
        }
        return format!("Preview only  {}", wav_preview_info_label(info));
    }
    audio_asset_workflow_status(asset).to_string()
}

fn is_wav_sample_file(asset: &AudioAssetItem) -> bool {
    asset.kind == AudioAssetKind::Sample
        && !asset.is_dir
        && asset
            .path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"))
}

fn wav_preview_info_label(info: WavPreviewInfo) -> String {
    format!(
        "{}  {}  {}",
        duration_label(info.duration_seconds),
        sample_rate_label(info.sample_rate_hz),
        channel_count_label(info.channels)
    )
}

fn duration_label(seconds: f32) -> String {
    let seconds = seconds.max(0.0);
    if seconds < 0.1 {
        "<0.1s".to_string()
    } else if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else {
        let total_seconds = seconds.round() as u64;
        format!("{}m{:02}s", total_seconds / 60, total_seconds % 60)
    }
}

fn sample_rate_label(sample_rate_hz: u32) -> String {
    if sample_rate_hz % 1000 == 0 {
        format!("{} kHz", sample_rate_hz / 1000)
    } else {
        format!("{:.1} kHz", sample_rate_hz as f32 / 1000.0)
    }
}

fn channel_count_label(channels: u16) -> String {
    match channels {
        1 => "mono".to_string(),
        2 => "stereo".to_string(),
        _ => format!("{channels} ch"),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ClipPanelSummary {
    pub(super) note_total: usize,
    pub(super) note_count: String,
    pub(super) loop_and_grid: String,
    pub(super) selected_note: Option<String>,
}

pub(super) fn clip_panel_summary(app: &AppState) -> ClipPanelSummary {
    let project = app.music_project.lock();
    let notes = project.clip.notes.clone();
    let loop_beats = project.transport.loop_beats;
    let quantize_grid = project.transport.quantize_grid;
    drop(project);

    let note_count = notes.len();
    let note_label = if note_count == 1 { "note" } else { "notes" };
    let selected_note = app.selected_clip_note.and_then(|selected| {
        notes
            .iter()
            .find(|note| note.id == selected)
            .map(|note| clip_panel_selected_note_label(app, note))
    });

    ClipPanelSummary {
        note_total: note_count,
        note_count: format!("{note_count} {note_label}"),
        loop_and_grid: format!("{loop_beats:.0} beats  Grid {}", quantize_grid.as_str()),
        selected_note,
    }
}

pub(super) fn clip_panel_selected_note_label(app: &AppState, note: &ClipNote) -> String {
    app.scale_state
        .lock()
        .note_info(note.musical_note)
        .map(|info| {
            format!(
                "Sel d{} o{} {:.1}Hz {:+.0}c b{:.2} l{:.2} v{}",
                info.degree + 1,
                info.octave,
                info.freq,
                info.cents_from_root,
                note.start_beats,
                note.duration_beats,
                note.velocity
            )
        })
        .unwrap_or_else(|| {
            format!(
                "Sel pitch {} b{:.2} l{:.2} v{}",
                note.musical_note, note.start_beats, note.duration_beats, note.velocity
            )
        })
}

pub(super) fn status_bar_label(app: &AppState, width: f32) -> String {
    let label = format!(
        "{}   |   Voices {}  Active {}   |   {}",
        app_version_label(),
        app.synth.active_voice_count(),
        app.synth.active_notes().len(),
        status_bar_message(&app.last_status)
    );
    fit_label(&label, width - 16.0, 12.0)
}

pub(super) fn app_version_label() -> &'static str {
    concat!("v", env!("CARGO_PKG_VERSION"))
}

pub(super) fn status_bar_message(status: &str) -> String {
    let segments = status
        .split("; ")
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let Some(latest) = segments.last() else {
        return "Ready".to_string();
    };
    let earlier = segments.len().saturating_sub(1);
    if earlier == 0 {
        (*latest).to_string()
    } else {
        format!("{latest} (+{earlier} earlier)")
    }
}
