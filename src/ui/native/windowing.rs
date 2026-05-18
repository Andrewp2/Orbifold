use operad::UiSize;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event_loop::ActiveEventLoop;

use crate::app::AppState;

use super::MIN_LAYOUT_HEIGHT;

pub(super) fn requested_or_monitor_window_size(
    requested: Option<(f64, f64)>,
    event_loop: &ActiveEventLoop,
) -> UiSize {
    let size = requested
        .map(|(width, height)| LogicalSize::new(width, height))
        .unwrap_or_else(|| initial_window_size(event_loop));
    UiSize::new(size.width as f32, size.height as f32)
}

fn initial_window_size(event_loop: &ActiveEventLoop) -> LogicalSize<f64> {
    event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())
        .map(|monitor| initial_window_size_for_monitor(monitor.size(), monitor.scale_factor()))
        .unwrap_or_else(|| LogicalSize::new(1400.0, MIN_LAYOUT_HEIGHT as f64))
}

pub(super) fn initial_window_size_for_monitor(
    monitor_size: PhysicalSize<u32>,
    scale_factor: f64,
) -> LogicalSize<f64> {
    let scale = scale_factor.max(1.0);
    let logical_width = monitor_size.width as f64 / scale;
    let logical_height = monitor_size.height as f64 / scale;
    LogicalSize::new(
        (logical_width * 0.9).clamp(1400.0, 3200.0),
        (logical_height * 0.88).clamp(MIN_LAYOUT_HEIGHT as f64, 1900.0),
    )
}

pub(super) fn window_title_for_app(app: &AppState) -> String {
    app.window_title()
}
