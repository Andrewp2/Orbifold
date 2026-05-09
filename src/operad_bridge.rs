use eframe::egui;
use operad::{
    ApproxTextMeasurer, ColorRgba, FontFamily, PaintKind, PaintTransform, ScenePrimitive,
    StrokeStyle, TextStyle, UiDocument, UiNode, UiPoint, UiRect, UiSize, layout,
};

pub(crate) fn paint_scene(
    ui: &egui::Ui,
    rect: egui::Rect,
    name: impl Into<String>,
    primitives: Vec<ScenePrimitive>,
) {
    let mut document = UiDocument::new(operad::root_style(rect.width(), rect.height()));
    document.add_child(
        document.root,
        UiNode::scene(name, primitives, layout::fixed(rect.width(), rect.height())),
    );
    let mut text_measurer = ApproxTextMeasurer;
    if document
        .compute_layout(UiSize::new(rect.width(), rect.height()), &mut text_measurer)
        .is_ok()
    {
        paint_document_at(ui, rect.min, &document);
    }
}

pub(crate) fn compute_and_paint_document(
    ui: &egui::Ui,
    rect: egui::Rect,
    document: &mut UiDocument,
) -> bool {
    let mut text_measurer = ApproxTextMeasurer;
    if document
        .compute_layout(UiSize::new(rect.width(), rect.height()), &mut text_measurer)
        .is_err()
    {
        return false;
    }
    paint_document_at(ui, rect.min, document);
    true
}

pub(crate) fn clicked_node_name(
    response: &egui::Response,
    rect: egui::Rect,
    document: &UiDocument,
) -> Option<String> {
    if !response.clicked() {
        return None;
    }
    let pos = response.interact_pointer_pos()?;
    if !rect.contains(pos) {
        return None;
    }
    let local = UiPoint::new(pos.x - rect.min.x, pos.y - rect.min.y);
    document
        .hit_test(local)
        .map(|id| document.node(id).name.clone())
}

pub(crate) fn rect_primitive(
    origin: egui::Pos2,
    rect: egui::Rect,
    fill: egui::Color32,
    stroke: Option<egui::Stroke>,
) -> ScenePrimitive {
    ScenePrimitive::Polygon {
        points: vec![
            local_point(origin, rect.left_top()),
            local_point(origin, rect.right_top()),
            local_point(origin, rect.right_bottom()),
            local_point(origin, rect.left_bottom()),
        ],
        fill: color_from_egui(fill),
        stroke: stroke.map(stroke_from_egui),
    }
}

pub(crate) fn line_primitive(
    origin: egui::Pos2,
    from: egui::Pos2,
    to: egui::Pos2,
    stroke: egui::Stroke,
) -> ScenePrimitive {
    ScenePrimitive::Line {
        from: local_point(origin, from),
        to: local_point(origin, to),
        stroke: stroke_from_egui(stroke),
    }
}

pub(crate) fn circle_primitive(
    origin: egui::Pos2,
    center: egui::Pos2,
    radius: f32,
    fill: egui::Color32,
    stroke: Option<egui::Stroke>,
) -> ScenePrimitive {
    ScenePrimitive::Circle {
        center: local_point(origin, center),
        radius,
        fill: color_from_egui(fill),
        stroke: stroke.map(stroke_from_egui),
    }
}

pub(crate) fn local_point(origin: egui::Pos2, point: egui::Pos2) -> UiPoint {
    UiPoint::new(point.x - origin.x, point.y - origin.y)
}

pub(crate) fn stroke_from_egui(stroke: egui::Stroke) -> StrokeStyle {
    StrokeStyle::new(color_from_egui(stroke.color), stroke.width)
}

pub(crate) fn paint_document_at(ui: &egui::Ui, origin: egui::Pos2, document: &UiDocument) {
    let painter = ui.painter();
    for item in document.paint_list().items {
        let clip_rect = offset_rect(origin, transform_rect(item.clip_rect, item.transform))
            .intersect(ui.clip_rect());
        if clip_rect.is_negative() || clip_rect.width() <= 0.0 || clip_rect.height() <= 0.0 {
            continue;
        }
        let painter = painter.with_clip_rect(clip_rect);
        match &item.kind {
            PaintKind::Rect {
                fill,
                stroke,
                corner_radius,
            } => {
                let rect = offset_rect(origin, transform_rect(item.rect, item.transform));
                if fill.a > 0 {
                    painter.rect_filled(rect, *corner_radius, color(*fill, item.opacity));
                }
                if let Some(stroke) = *stroke {
                    painter.rect_stroke(
                        rect,
                        *corner_radius,
                        egui::Stroke::new(stroke.width, color(stroke.color, item.opacity)),
                        egui::StrokeKind::Inside,
                    );
                }
            }
            PaintKind::Text(text) => {
                let rect = offset_rect(origin, transform_rect(item.rect, item.transform));
                painter.text(
                    rect.left_top(),
                    egui::Align2::LEFT_TOP,
                    &text.text,
                    font_id(&text.style, item.transform.scale),
                    color(text.style.color, item.opacity),
                );
            }
            PaintKind::Line { from, to, stroke } => {
                painter.line_segment(
                    [
                        offset_point(origin, transform_point(*from, item.transform)),
                        offset_point(origin, transform_point(*to, item.transform)),
                    ],
                    egui::Stroke::new(stroke.width, color(stroke.color, item.opacity)),
                );
            }
            PaintKind::Circle {
                center,
                radius,
                fill,
                stroke,
            } => {
                let center = offset_point(origin, transform_point(*center, item.transform));
                let radius = radius * item.transform.scale;
                if fill.a > 0 {
                    painter.circle_filled(center, radius, color(*fill, item.opacity));
                }
                if let Some(stroke) = *stroke {
                    painter.circle_stroke(
                        center,
                        radius,
                        egui::Stroke::new(stroke.width, color(stroke.color, item.opacity)),
                    );
                }
            }
            PaintKind::Polygon {
                points,
                fill,
                stroke,
            } => {
                let points = points
                    .iter()
                    .map(|point| offset_point(origin, transform_point(*point, item.transform)))
                    .collect::<Vec<_>>();
                painter.add(egui::Shape::convex_polygon(
                    points,
                    color(*fill, item.opacity),
                    stroke
                        .map(|stroke| {
                            egui::Stroke::new(stroke.width, color(stroke.color, item.opacity))
                        })
                        .unwrap_or(egui::Stroke::NONE),
                ));
            }
            PaintKind::Canvas(_) | PaintKind::Image { .. } => {}
        }
    }
}

pub(crate) fn color(value: ColorRgba, opacity: f32) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        value.r,
        value.g,
        value.b,
        ((value.a as f32) * opacity.clamp(0.0, 1.0)).round() as u8,
    )
}

pub(crate) fn color_from_egui(value: egui::Color32) -> ColorRgba {
    ColorRgba::new(value.r(), value.g(), value.b(), value.a())
}

fn offset_rect(origin: egui::Pos2, rect: UiRect) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(origin.x + rect.x, origin.y + rect.y),
        egui::vec2(rect.width, rect.height),
    )
}

fn offset_point(origin: egui::Pos2, point: UiPoint) -> egui::Pos2 {
    egui::pos2(origin.x + point.x, origin.y + point.y)
}

fn transform_point(point: UiPoint, transform: PaintTransform) -> UiPoint {
    UiPoint::new(
        point.x * transform.scale + transform.translation.x,
        point.y * transform.scale + transform.translation.y,
    )
}

fn transform_rect(rect: UiRect, transform: PaintTransform) -> UiRect {
    let top_left = transform_point(UiPoint::new(rect.x, rect.y), transform);
    UiRect::new(
        top_left.x,
        top_left.y,
        rect.width * transform.scale,
        rect.height * transform.scale,
    )
}

fn font_id(style: &TextStyle, scale: f32) -> egui::FontId {
    let size = style.font_size * scale.max(0.0);
    match style.family {
        FontFamily::Monospace => egui::FontId::monospace(size),
        FontFamily::SansSerif | FontFamily::Serif | FontFamily::Named(_) => {
            egui::FontId::proportional(size)
        }
    }
}
