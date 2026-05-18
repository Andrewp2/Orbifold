use operad::{ColorRgba, StrokeStyle};

pub(super) const fn color(r: u8, g: u8, b: u8) -> ColorRgba {
    ColorRgba::new(r, g, b, 255)
}

pub(super) const fn fade(base: ColorRgba, alpha: f32) -> ColorRgba {
    ColorRgba::new(base.r, base.g, base.b, (alpha * 255.0) as u8)
}

pub(super) const fn stroke(color: ColorRgba, width: f32) -> StrokeStyle {
    StrokeStyle::new(color, width)
}

pub(super) const fn strong() -> ColorRgba {
    color(232, 238, 247)
}

pub(super) const fn muted() -> ColorRgba {
    color(159, 170, 184)
}

pub(super) const fn accent() -> ColorRgba {
    color(64, 211, 219)
}

pub(super) const fn clip_color() -> ColorRgba {
    color(132, 81, 238)
}
