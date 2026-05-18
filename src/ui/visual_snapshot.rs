use operad::{
    AlignedStroke, ColorRgba, PaintBrush, PaintKind, PaintList, PaintPath, PathFillRule, PathVerb,
    StrokeStyle, TextHorizontalAlign, TextStyle, TextVerticalAlign, UiDocument, UiPoint, UiRect,
    UiSize,
};

#[derive(Clone, Debug)]
pub(crate) struct VisualSnapshot {
    pub(crate) svg: String,
    pub(crate) item_count: usize,
    pub(crate) unsupported_count: usize,
}

pub(crate) fn visual_snapshot_svg(document: &UiDocument, viewport: UiSize) -> VisualSnapshot {
    let paint = document.paint_list();
    let mut writer = SvgWriter::new(viewport);
    writer.write_paint_list(document, &paint);
    writer.finish()
}

struct SvgWriter {
    viewport: UiSize,
    defs: String,
    body: String,
    item_count: usize,
    unsupported_count: usize,
    clip_index: usize,
}

impl SvgWriter {
    fn new(viewport: UiSize) -> Self {
        Self {
            viewport,
            defs: String::new(),
            body: String::new(),
            item_count: 0,
            unsupported_count: 0,
            clip_index: 0,
        }
    }

    fn finish(self) -> VisualSnapshot {
        let mut svg = String::new();
        svg.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        svg.push('\n');
        svg.push_str(&format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}" data-orbifold-snapshot="paint-list" data-items="{}" data-unsupported="{}">"#,
            fmt(self.viewport.width),
            fmt(self.viewport.height),
            fmt(self.viewport.width),
            fmt(self.viewport.height),
            self.item_count,
            self.unsupported_count
        ));
        svg.push('\n');
        svg.push_str(r##"<rect x="0" y="0" width="100%" height="100%" fill="#080c12"/>"##);
        svg.push('\n');
        if !self.defs.is_empty() {
            svg.push_str("<defs>\n");
            svg.push_str(&self.defs);
            svg.push_str("</defs>\n");
        }
        svg.push_str(&self.body);
        svg.push_str("</svg>\n");
        VisualSnapshot {
            svg,
            item_count: self.item_count,
            unsupported_count: self.unsupported_count,
        }
    }

    fn write_paint_list(&mut self, document: &UiDocument, paint: &PaintList) {
        for item in &paint.items {
            self.item_count += 1;
            match &item.kind {
                PaintKind::Rect {
                    fill,
                    stroke,
                    corner_radius,
                } => {
                    let rect = item.transform.transform_rect(item.rect);
                    let clip = item.transform.transform_rect(item.clip_rect);
                    let shape =
                        svg_rect(rect, *corner_radius * item.transform.scale, *fill, *stroke);
                    self.write_shape(clip, item.opacity, &shape);
                }
                PaintKind::RichRect(rect) => {
                    let rect = transformed_paint_rect(rect.rect, item.transform);
                    let clip = item.transform.transform_rect(item.clip_rect);
                    let fill = rect_fill_color(&item.kind);
                    let stroke = rich_rect_stroke(&item.kind);
                    let radius = rich_rect_radius(&item.kind) * item.transform.scale;
                    let shape = svg_rect(rect, radius, fill, stroke);
                    self.write_shape(clip, item.opacity, &shape);
                }
                PaintKind::Text(content) if !content.text.trim().is_empty() => {
                    let rect = item.transform.transform_rect(item.rect);
                    let clip = item.transform.transform_rect(item.clip_rect);
                    let shape = svg_text(
                        rect,
                        &content.text,
                        &scaled_text_style(&content.style, item.transform.scale),
                        TextHorizontalAlign::Start,
                        TextVerticalAlign::Top,
                    );
                    self.write_shape(clip, item.opacity, &shape);
                }
                PaintKind::SceneText(text) if !text.text.trim().is_empty() => {
                    let rect = item.transform.transform_rect(text.rect);
                    let clip = item.transform.transform_rect(item.clip_rect);
                    let shape = svg_text(
                        rect,
                        &text.text,
                        &scaled_text_style(&text.style, item.transform.scale),
                        text.horizontal_align,
                        text.vertical_align,
                    );
                    self.write_shape(clip, item.opacity, &shape);
                }
                PaintKind::Line { from, to, stroke } => {
                    let from = item.transform.transform_point(*from);
                    let to = item.transform.transform_point(*to);
                    let clip = item.transform.transform_rect(item.clip_rect);
                    let shape = svg_line(from, to, *stroke, item.transform.scale);
                    self.write_shape(clip, item.opacity, &shape);
                }
                PaintKind::Circle {
                    center,
                    radius,
                    fill,
                    stroke,
                } => {
                    let center = item.transform.transform_point(*center);
                    let clip = item.transform.transform_rect(item.clip_rect);
                    let shape = svg_circle(
                        center,
                        radius * item.transform.scale,
                        *fill,
                        *stroke,
                        item.transform.scale,
                    );
                    self.write_shape(clip, item.opacity, &shape);
                }
                PaintKind::Polygon {
                    points,
                    fill,
                    stroke,
                } => {
                    let points = points
                        .iter()
                        .map(|point| item.transform.transform_point(*point))
                        .collect::<Vec<_>>();
                    let clip = item.transform.transform_rect(item.clip_rect);
                    let shape = svg_polygon(&points, *fill, *stroke, item.transform.scale);
                    self.write_shape(clip, item.opacity, &shape);
                }
                PaintKind::Path(path) => {
                    let clip = item.transform.transform_rect(item.clip_rect);
                    let shape = svg_path(path, item.transform.scale, |point| {
                        item.transform.transform_point(point)
                    });
                    self.write_shape(clip, item.opacity, &shape);
                }
                PaintKind::Image { key, tint } => {
                    let rect = item.transform.transform_rect(item.rect);
                    let clip = item.transform.transform_rect(item.clip_rect);
                    let shape = svg_image_placeholder(rect, key, *tint);
                    self.write_shape(clip, item.opacity, &shape);
                }
                PaintKind::ImagePlacement(image) => {
                    let rect = item.transform.transform_rect(image.rect);
                    let clip = item.transform.transform_rect(item.clip_rect);
                    let shape = svg_image_placeholder(rect, &image.key, image.tint);
                    self.write_shape(clip, item.opacity, &shape);
                }
                PaintKind::CompositedLayer(layer) => {
                    self.write_paint_list(document, &layer.paint);
                }
                PaintKind::Canvas(_) | PaintKind::Text(_) | PaintKind::SceneText(_) => {
                    self.unsupported_count += 1;
                }
            }
            let _ = document;
        }
    }

    fn write_shape(&mut self, clip: UiRect, opacity: f32, shape: &str) {
        if shape.is_empty() {
            return;
        }
        let clip_id = self.next_clip(clip);
        self.body.push_str(&format!(
            r#"<g clip-path="url(#{})" opacity="{}">{}</g>"#,
            clip_id,
            fmt(opacity.clamp(0.0, 1.0)),
            shape
        ));
        self.body.push('\n');
    }

    fn next_clip(&mut self, rect: UiRect) -> String {
        let id = format!("clip{}", self.clip_index);
        self.clip_index += 1;
        self.defs.push_str(&format!(
            r#"<clipPath id="{}"><rect x="{}" y="{}" width="{}" height="{}"/></clipPath>"#,
            id,
            fmt(rect.x),
            fmt(rect.y),
            fmt(rect.width.max(0.0)),
            fmt(rect.height.max(0.0))
        ));
        self.defs.push('\n');
        id
    }
}

fn transformed_paint_rect(rect: UiRect, transform: operad::PaintTransform) -> UiRect {
    transform.transform_rect(rect)
}

fn rect_fill_color(kind: &PaintKind) -> ColorRgba {
    match kind {
        PaintKind::RichRect(rect) => rect.fill.fallback_color(),
        _ => ColorRgba::TRANSPARENT,
    }
}

fn rich_rect_stroke(kind: &PaintKind) -> Option<StrokeStyle> {
    match kind {
        PaintKind::RichRect(rect) => rect.stroke.map(aligned_stroke_style),
        _ => None,
    }
}

fn rich_rect_radius(kind: &PaintKind) -> f32 {
    match kind {
        PaintKind::RichRect(rect) => rect.corner_radii.max_radius(),
        _ => 0.0,
    }
}

fn aligned_stroke_style(stroke: AlignedStroke) -> StrokeStyle {
    stroke.style
}

fn scaled_text_style(style: &TextStyle, scale: f32) -> TextStyle {
    let mut style = style.clone();
    style.font_size *= scale;
    style.line_height *= scale;
    style
}

fn svg_rect(rect: UiRect, radius: f32, fill: ColorRgba, stroke: Option<StrokeStyle>) -> String {
    let mut attrs = format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}" fill-opacity="{}""#,
        fmt(rect.x),
        fmt(rect.y),
        fmt(rect.width.max(0.0)),
        fmt(rect.height.max(0.0)),
        fmt(radius.max(0.0)),
        color_rgb(fill),
        color_opacity(fill)
    );
    push_stroke_attrs(&mut attrs, stroke, 1.0);
    attrs.push_str("/>");
    attrs
}

fn svg_text(
    rect: UiRect,
    text: &str,
    style: &TextStyle,
    horizontal_align: TextHorizontalAlign,
    vertical_align: TextVerticalAlign,
) -> String {
    let x = match horizontal_align {
        TextHorizontalAlign::Start => rect.x,
        TextHorizontalAlign::Center => rect.x + rect.width * 0.5,
        TextHorizontalAlign::End => rect.right(),
    };
    let y = match vertical_align {
        TextVerticalAlign::Top => rect.y,
        TextVerticalAlign::Center => rect.y + rect.height * 0.5,
        TextVerticalAlign::Baseline => rect.y,
        TextVerticalAlign::Bottom => rect.bottom(),
    };
    let anchor = match horizontal_align {
        TextHorizontalAlign::Start => "start",
        TextHorizontalAlign::Center => "middle",
        TextHorizontalAlign::End => "end",
    };
    let baseline = match vertical_align {
        TextVerticalAlign::Top => "hanging",
        TextVerticalAlign::Center => "middle",
        TextVerticalAlign::Baseline => "alphabetic",
        TextVerticalAlign::Bottom => "text-after-edge",
    };
    format!(
        r#"<text x="{}" y="{}" fill="{}" fill-opacity="{}" font-family="Inter, system-ui, sans-serif" font-size="{}" text-anchor="{}" dominant-baseline="{}">{}</text>"#,
        fmt(x),
        fmt(y),
        color_rgb(style.color),
        color_opacity(style.color),
        fmt(style.font_size),
        anchor,
        baseline,
        escape_xml(text)
    )
}

fn svg_line(from: UiPoint, to: UiPoint, stroke: StrokeStyle, scale: f32) -> String {
    format!(
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-opacity="{}" stroke-width="{}" stroke-linecap="round"/>"#,
        fmt(from.x),
        fmt(from.y),
        fmt(to.x),
        fmt(to.y),
        color_rgb(stroke.color),
        color_opacity(stroke.color),
        fmt((stroke.width * scale).max(0.0))
    )
}

fn svg_circle(
    center: UiPoint,
    radius: f32,
    fill: ColorRgba,
    stroke: Option<StrokeStyle>,
    scale: f32,
) -> String {
    let mut attrs = format!(
        r#"<circle cx="{}" cy="{}" r="{}" fill="{}" fill-opacity="{}""#,
        fmt(center.x),
        fmt(center.y),
        fmt(radius.max(0.0)),
        color_rgb(fill),
        color_opacity(fill)
    );
    push_stroke_attrs(&mut attrs, stroke, scale);
    attrs.push_str("/>");
    attrs
}

fn svg_polygon(
    points: &[UiPoint],
    fill: ColorRgba,
    stroke: Option<StrokeStyle>,
    scale: f32,
) -> String {
    if points.is_empty() {
        return String::new();
    }
    let points = points
        .iter()
        .map(|point| format!("{},{}", fmt(point.x), fmt(point.y)))
        .collect::<Vec<_>>()
        .join(" ");
    let mut attrs = format!(
        r#"<polygon points="{}" fill="{}" fill-opacity="{}""#,
        points,
        color_rgb(fill),
        color_opacity(fill)
    );
    push_stroke_attrs(&mut attrs, stroke, scale);
    attrs.push_str("/>");
    attrs
}

fn svg_path(path: &PaintPath, scale: f32, transform: impl Fn(UiPoint) -> UiPoint) -> String {
    if path.verbs.is_empty() {
        return String::new();
    }
    let mut d = String::new();
    for verb in &path.verbs {
        match *verb {
            PathVerb::MoveTo(point) => {
                let point = transform(point);
                d.push_str(&format!("M {} {} ", fmt(point.x), fmt(point.y)));
            }
            PathVerb::LineTo(point) => {
                let point = transform(point);
                d.push_str(&format!("L {} {} ", fmt(point.x), fmt(point.y)));
            }
            PathVerb::QuadraticTo { control, to } => {
                let control = transform(control);
                let to = transform(to);
                d.push_str(&format!(
                    "Q {} {} {} {} ",
                    fmt(control.x),
                    fmt(control.y),
                    fmt(to.x),
                    fmt(to.y)
                ));
            }
            PathVerb::CubicTo {
                control_a,
                control_b,
                to,
            } => {
                let control_a = transform(control_a);
                let control_b = transform(control_b);
                let to = transform(to);
                d.push_str(&format!(
                    "C {} {} {} {} {} {} ",
                    fmt(control_a.x),
                    fmt(control_a.y),
                    fmt(control_b.x),
                    fmt(control_b.y),
                    fmt(to.x),
                    fmt(to.y)
                ));
            }
            PathVerb::Close => d.push_str("Z "),
        }
    }
    let fill = path.fill.as_ref().map(brush_color);
    let stroke = path.stroke.map(aligned_stroke_style);
    let fill_rule = match path.fill_rule {
        PathFillRule::NonZero => "nonzero",
        PathFillRule::EvenOdd => "evenodd",
    };
    let mut attrs = format!(
        r#"<path d="{}" fill="{}" fill-opacity="{}" fill-rule="{}""#,
        d.trim(),
        color_rgb(fill.unwrap_or(ColorRgba::TRANSPARENT)),
        fill.map(color_opacity).unwrap_or("0".to_string()),
        fill_rule
    );
    push_stroke_attrs(&mut attrs, stroke, scale);
    attrs.push_str("/>");
    attrs
}

fn svg_image_placeholder(rect: UiRect, key: &str, tint: Option<ColorRgba>) -> String {
    let fill = tint.unwrap_or(ColorRgba::new(80, 96, 112, 128));
    format!(
        r##"<g><rect x="{}" y="{}" width="{}" height="{}" fill="{}" fill-opacity="{}"/><text x="{}" y="{}" fill="#d7e1ea" fill-opacity="0.7" font-family="Inter, system-ui, sans-serif" font-size="10" dominant-baseline="middle">{}</text></g>"##,
        fmt(rect.x),
        fmt(rect.y),
        fmt(rect.width.max(0.0)),
        fmt(rect.height.max(0.0)),
        color_rgb(fill),
        color_opacity(fill),
        fmt(rect.x + 4.0),
        fmt(rect.y + rect.height * 0.5),
        escape_xml(key)
    )
}

fn brush_color(brush: &PaintBrush) -> ColorRgba {
    brush.fallback_color()
}

fn push_stroke_attrs(attrs: &mut String, stroke: Option<StrokeStyle>, scale: f32) {
    if let Some(stroke) = stroke {
        attrs.push_str(&format!(
            r#" stroke="{}" stroke-opacity="{}" stroke-width="{}""#,
            color_rgb(stroke.color),
            color_opacity(stroke.color),
            fmt((stroke.width * scale).max(0.0))
        ));
    } else {
        attrs.push_str(r#" stroke="none""#);
    }
}

fn color_rgb(color: ColorRgba) -> String {
    format!("rgb({},{},{})", color.r, color.g, color.b)
}

fn color_opacity(color: ColorRgba) -> String {
    fmt(color.a as f32 / 255.0)
}

fn fmt(value: f32) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    let rounded = (value * 100.0).round() / 100.0;
    if (rounded - rounded.round()).abs() < 0.001 {
        format!("{}", rounded.round() as i32)
    } else {
        format!("{rounded:.2}")
    }
}

fn escape_xml(text: &str) -> String {
    text.chars()
        .flat_map(|ch| match ch {
            '&' => "&amp;".chars().collect::<Vec<_>>(),
            '<' => "&lt;".chars().collect::<Vec<_>>(),
            '>' => "&gt;".chars().collect::<Vec<_>>(),
            '"' => "&quot;".chars().collect::<Vec<_>>(),
            '\'' => "&apos;".chars().collect::<Vec<_>>(),
            _ => vec![ch],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use operad::{ApproxTextMeasurer, PaintRect, TextStyle, UiDocument, UiNode, layout};

    #[test]
    fn visual_snapshot_contains_svg_shapes_and_text() {
        let mut document = UiDocument::new(operad::root_style(200.0, 80.0));
        document.add_child(
            document.root,
            UiNode::paint_rect(
                "panel",
                PaintRect::solid(
                    UiRect::new(0.0, 0.0, 160.0, 40.0),
                    ColorRgba::new(1, 2, 3, 255),
                ),
                layout::fixed(160.0, 40.0),
            ),
        );
        document.add_child(
            document.root,
            UiNode::text(
                "label",
                "Ready & Able",
                TextStyle::default(),
                layout::fixed(160.0, 24.0),
            ),
        );
        document
            .compute_layout(UiSize::new(200.0, 80.0), &mut ApproxTextMeasurer)
            .expect("layout should compute");

        let snapshot = visual_snapshot_svg(&document, UiSize::new(200.0, 80.0));

        assert!(snapshot.svg.contains("<svg"));
        assert!(snapshot.svg.contains("<rect"));
        assert!(snapshot.svg.contains("Ready &amp; Able"));
        assert!(snapshot.item_count >= 2);
        assert_eq!(snapshot.unsupported_count, 0);
    }
}
