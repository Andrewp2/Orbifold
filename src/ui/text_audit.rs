use operad::{
    PaintKind, PaintList, TextHorizontalAlign, TextStyle, TextVerticalAlign, UiDocument, UiRect,
};

pub(crate) const TEXT_OVERLAP_TOLERANCE: f32 = 1.0;

#[derive(Clone, Debug)]
pub(crate) struct TextBox {
    pub(crate) source: String,
    pub(crate) text: String,
    pub(crate) allocated: UiRect,
    pub(crate) visible: UiRect,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TextAuditSummary {
    pub(crate) text_count: usize,
    pub(crate) issue_count: usize,
    pub(crate) non_finite_count: usize,
    pub(crate) sample_issue: Option<String>,
}

pub(crate) fn text_audit_summary(document: &UiDocument) -> TextAuditSummary {
    let paint = document.paint_list();
    let boxes = collect_text_boxes(document, &paint);
    text_audit_summary_for_boxes(&boxes)
}

pub(crate) fn collect_text_boxes(document: &UiDocument, paint: &PaintList) -> Vec<TextBox> {
    let mut out = Vec::new();
    collect_text_boxes_from_paint(document, paint, &mut out);
    out
}

pub(crate) fn text_audit_summary_for_boxes(boxes: &[TextBox]) -> TextAuditSummary {
    let non_finite = non_finite_text_issues(boxes);
    let overlaps = text_overlap_issues(boxes);
    let sample_issue = non_finite.first().or_else(|| overlaps.first()).cloned();
    TextAuditSummary {
        text_count: boxes.len(),
        issue_count: overlaps.len(),
        non_finite_count: non_finite.len(),
        sample_issue,
    }
}

pub(crate) fn non_finite_text_issues(boxes: &[TextBox]) -> Vec<String> {
    boxes
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
        .collect()
}

pub(crate) fn text_overlap_issues(boxes: &[TextBox]) -> Vec<String> {
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

fn intersect_rect(a: UiRect, b: UiRect) -> Option<UiRect> {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = a.right().min(b.right());
    let y2 = a.bottom().min(b.bottom());
    (x2 > x1 && y2 > y1).then(|| UiRect::new(x1, y1, x2 - x1, y2 - y1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use operad::{ApproxTextMeasurer, TextStyle, UiDocument, UiNode, UiSize, layout};

    #[test]
    fn text_audit_summary_collects_document_text() {
        let mut document = UiDocument::new(operad::root_style(200.0, 80.0));
        document.add_child(
            document.root,
            UiNode::text(
                "status.label",
                "Ready",
                TextStyle::default(),
                layout::fixed(120.0, 24.0),
            ),
        );
        document
            .compute_layout(UiSize::new(200.0, 80.0), &mut ApproxTextMeasurer)
            .expect("layout should compute");

        let summary = text_audit_summary(&document);

        assert_eq!(summary.text_count, 1);
        assert_eq!(summary.issue_count, 0);
        assert_eq!(summary.non_finite_count, 0);
        assert_eq!(summary.sample_issue, None);
    }

    #[test]
    fn text_overlap_detector_reports_collisions() {
        let boxes = vec![
            TextBox {
                source: "left".to_string(),
                text: "Alpha".to_string(),
                allocated: UiRect::new(0.0, 0.0, 50.0, 20.0),
                visible: UiRect::new(0.0, 0.0, 50.0, 20.0),
            },
            TextBox {
                source: "right".to_string(),
                text: "Beta".to_string(),
                allocated: UiRect::new(25.0, 0.0, 50.0, 20.0),
                visible: UiRect::new(25.0, 0.0, 50.0, 20.0),
            },
        ];

        let summary = text_audit_summary_for_boxes(&boxes);

        assert_eq!(summary.text_count, 2);
        assert_eq!(summary.issue_count, 1);
        assert_eq!(summary.non_finite_count, 0);
        assert!(
            summary
                .sample_issue
                .as_deref()
                .is_some_and(|issue| issue.contains("overlaps"))
        );
    }
}
