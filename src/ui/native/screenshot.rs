use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use operad::platform::PixelSize;
use operad::{
    ApproxTextMeasurer, ColorRgba, EmptyResourceResolver, RenderFrameRequest, RenderOptions,
    RenderTarget, RendererAdapter, UiSize, WgpuRenderer,
};
use winit::dpi::PhysicalSize;

use crate::app::AppState;
use crate::ui::theme::color;

use super::{MIN_EFFECTIVE_UI_SCALE, MIN_LAYOUT_HEIGHT, MIN_LAYOUT_WIDTH, build_surface_document};

pub(super) fn write_startup_screenshot(
    app: &AppState,
    requested_size: Option<(f64, f64)>,
) -> Result<PathBuf, String> {
    let fallback_size = PhysicalSize::new(1400, MIN_LAYOUT_HEIGHT as u32);
    let size = screenshot_physical_size(requested_size, fallback_size);
    let ui_scale = screenshot_ui_scale_for_values(1.0, requested_size, size, app.ui_scale());
    let logical_size = logical_size_for_window(size, ui_scale);
    let path = write_operad_screenshot(app, size, logical_size, ui_scale)?;
    log::info!("Wrote screenshot to {}", path.display());
    Ok(path)
}

fn write_operad_screenshot(
    app: &AppState,
    size: PhysicalSize<u32>,
    logical_size: UiSize,
    ui_scale: f32,
) -> Result<PathBuf, String> {
    let viewport = logical_size;
    let mut document = build_surface_document(app, logical_size.width, logical_size.height);
    let mut text_measurer = ApproxTextMeasurer;
    document
        .compute_layout(logical_size, &mut text_measurer)
        .map_err(|error| error.to_string())?;
    let options = RenderOptions {
        clear_color: color(8, 12, 18),
        scale_factor: ui_scale,
        ..Default::default()
    };
    let request = RenderFrameRequest::new(
        RenderTarget::snapshot(PixelSize::new(size.width.max(1), size.height.max(1))),
        viewport,
        document.paint_list(),
    )
    .options(options);
    let output = WgpuRenderer::new()
        .render_frame(request, &EmptyResourceResolver)
        .map_err(|error| error.to_string())?;
    let image = output
        .snapshot
        .ok_or_else(|| "snapshot render did not return image data".to_string())?;
    validate_screenshot_pixels(
        image.size.width,
        image.size.height,
        &image.pixels,
        color(8, 12, 18),
    )?;
    let path = next_screenshot_path()?;
    write_png_rgba(&path, image.size.width, image.size.height, &image.pixels)?;
    let latest = Path::new("screenshots").join("latest.png");
    write_png_rgba(&latest, image.size.width, image.size.height, &image.pixels)?;
    Ok(path)
}

pub(super) fn validate_screenshot_pixels(
    width: u32,
    height: u32,
    pixels: &[u8],
    background: ColorRgba,
) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("screenshot image has zero-sized dimensions".to_string());
    }
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixel_count| pixel_count.checked_mul(4))
        .ok_or_else(|| "screenshot image dimensions overflow pixel buffer size".to_string())?;
    if pixels.len() != expected_len {
        return Err(format!(
            "screenshot pixel buffer has {} bytes; expected {expected_len}",
            pixels.len()
        ));
    }

    let mut active_count = 0_usize;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0_u32;
    let mut max_y = 0_u32;
    for y in 0..height {
        for x in 0..width {
            let index = ((y as usize * width as usize) + x as usize) * 4;
            if screenshot_pixel_is_active(&pixels[index..index + 4], background) {
                active_count += 1;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    let pixel_count = (width as usize) * (height as usize);
    let minimum_active = (pixel_count / 200).max(1);
    if active_count < minimum_active {
        return Err(format!(
            "screenshot appears blank: only {active_count} active pixels"
        ));
    }

    let active_width = max_x - min_x + 1;
    let active_height = max_y - min_y + 1;
    let width_coverage = active_width as f32 / width as f32;
    let height_coverage = active_height as f32 / height as f32;
    if width_coverage < 0.9 || height_coverage < 0.9 {
        return Err(format!(
            "screenshot content appears cropped: active bounds cover {:.0}% x {:.0}% of image",
            width_coverage * 100.0,
            height_coverage * 100.0
        ));
    }

    Ok(())
}

fn screenshot_pixel_is_active(pixel: &[u8], background: ColorRgba) -> bool {
    let color_distance = (pixel[0] as i16 - background.r as i16).abs()
        + (pixel[1] as i16 - background.g as i16).abs()
        + (pixel[2] as i16 - background.b as i16).abs();
    pixel[3] > 0 && color_distance > 12
}

fn next_screenshot_path() -> Result<PathBuf, String> {
    let directory = Path::new("screenshots");
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    Ok(directory.join(format!("ui-{timestamp}.png")))
}

fn write_png_rgba(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    image::save_buffer_with_format(
        path,
        pixels,
        width,
        height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|error| error.to_string())
}

pub(super) fn ui_scale_for_pixel_size(
    dpi_scale: f32,
    width: u32,
    height: u32,
    user_scale: f32,
) -> f32 {
    ui_scale_for_values(dpi_scale, PhysicalSize::new(width, height), user_scale)
}

pub(super) fn ui_scale_for_values(dpi_scale: f32, size: PhysicalSize<u32>, user_scale: f32) -> f32 {
    let large_screen_scale = if size.width >= 3600 || size.height >= 2000 {
        2.0
    } else if size.width >= 3000 || size.height >= 1700 {
        1.6
    } else if size.width >= 2400 || size.height >= 1400 {
        1.25
    } else {
        1.0
    };
    let display_scale = dpi_scale.max(large_screen_scale).max(1.0);
    let user_scale = user_scale.clamp(0.75, 2.0);
    let requested_scale = (display_scale * user_scale).clamp(0.75, 3.0);
    requested_scale.min(max_ui_scale_for_minimum_layout(size))
}

fn max_ui_scale_for_minimum_layout(size: PhysicalSize<u32>) -> f32 {
    let width_scale = size.width.max(1) as f32 / MIN_LAYOUT_WIDTH;
    let height_scale = size.height.max(1) as f32 / MIN_LAYOUT_HEIGHT;
    width_scale.min(height_scale).max(1.0)
}

pub(super) fn logical_size_for_window(size: PhysicalSize<u32>, ui_scale: f32) -> UiSize {
    let scale = effective_ui_scale(ui_scale);
    UiSize::new(
        size.width.max(1) as f32 / scale,
        size.height.max(1) as f32 / scale,
    )
}

pub(super) fn effective_ui_scale(ui_scale: f32) -> f32 {
    ui_scale.max(MIN_EFFECTIVE_UI_SCALE)
}

pub(super) fn screenshot_physical_size(
    requested_size: Option<(f64, f64)>,
    actual_window_size: PhysicalSize<u32>,
) -> PhysicalSize<u32> {
    requested_size
        .map(|(width, height)| {
            PhysicalSize::new(screenshot_dimension(width), screenshot_dimension(height))
        })
        .unwrap_or(actual_window_size)
}

fn screenshot_dimension(value: f64) -> u32 {
    value.round().clamp(1.0, u32::MAX as f64) as u32
}

pub(super) fn screenshot_ui_scale_for_values(
    window_dpi_scale: f32,
    requested_size: Option<(f64, f64)>,
    screenshot_size: PhysicalSize<u32>,
    user_scale: f32,
) -> f32 {
    let dpi_scale = if requested_size.is_some() {
        1.0
    } else {
        window_dpi_scale
    };
    ui_scale_for_values(dpi_scale, screenshot_size, user_scale)
}
