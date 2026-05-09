use operad::{
    ClipBehavior, ColorRgba, FontFamily, FontWeight, StrokeStyle, TextStyle, UiDocument, UiNode,
    UiNodeId, UiNodeStyle, UiVisual, layout, widgets,
};

const SHELL_BG: ColorRgba = ColorRgba::new(18, 22, 28, 255);
const PANEL_STROKE: ColorRgba = ColorRgba::new(55, 65, 82, 255);
const BUTTON_BG: ColorRgba = ColorRgba::new(38, 45, 57, 255);
const BUTTON_ACTIVE_BG: ColorRgba = ColorRgba::new(36, 104, 130, 255);
const RECORD_BG: ColorRgba = ColorRgba::new(116, 35, 44, 255);
const MUTED_TEXT: ColorRgba = ColorRgba::new(172, 181, 194, 255);
const STRONG_TEXT: ColorRgba = ColorRgba::new(235, 240, 247, 255);
const ACCENT: ColorRgba = ColorRgba::new(79, 196, 202, 255);

pub(crate) fn document(width: f32, height: f32) -> UiDocument {
    UiDocument::new(operad::root_style(width.max(1.0), height.max(1.0)))
}

pub(crate) fn panel_row(document: &mut UiDocument, name: &str) -> UiNodeId {
    let layout = layout::with_padding_all(
        layout::with_size(layout::row(), layout::percent(1.0), layout::percent(1.0)),
        6.0,
    );
    document.add_child(
        document.root,
        UiNode::container(
            name,
            UiNodeStyle {
                layout,
                clip: ClipBehavior::Clip,
                ..Default::default()
            },
        )
        .with_visual(UiVisual::panel(
            SHELL_BG,
            Some(StrokeStyle::new(PANEL_STROKE, 1.0)),
            0.0,
        )),
    )
}

pub(crate) fn spacer(document: &mut UiDocument, parent: UiNodeId, name: &str, width: f32) {
    document.add_child(
        parent,
        UiNode::container(
            name,
            UiNodeStyle {
                layout: layout::fixed(width.max(0.0), 1.0),
                ..Default::default()
            },
        ),
    );
}

pub(crate) fn divider(document: &mut UiDocument, parent: UiNodeId, name: &str) {
    document.add_child(
        parent,
        UiNode::container(
            name,
            UiNodeStyle {
                layout: layout::fixed(1.0, 24.0),
                ..Default::default()
            },
        )
        .with_visual(UiVisual::panel(PANEL_STROKE, None, 0.0)),
    );
}

pub(crate) fn button(
    document: &mut UiDocument,
    parent: UiNodeId,
    name: &str,
    label: impl Into<String>,
    width: f32,
    active: bool,
    enabled: bool,
) -> UiNodeId {
    let mut options = widgets::ButtonOptions::new(layout::fixed(width, 26.0));
    options.visual = UiVisual::panel(
        if active { BUTTON_ACTIVE_BG } else { BUTTON_BG },
        Some(StrokeStyle::new(
            if active { ACCENT } else { PANEL_STROKE },
            if active { 1.5 } else { 1.0 },
        )),
        4.0,
    );
    options.pressed_visual = Some(UiVisual::panel(
        ColorRgba::new(18, 24, 31, 255),
        Some(StrokeStyle::new(ACCENT, 1.0)),
        4.0,
    ));
    options.disabled_visual = Some(UiVisual::panel(
        ColorRgba::new(28, 32, 39, 160),
        Some(StrokeStyle::new(ColorRgba::new(50, 58, 70, 180), 1.0)),
        4.0,
    ));
    options.text_style = text_style(13.0, STRONG_TEXT, false);
    options.enabled = enabled;
    widgets::button(document, parent, name, label, options)
}

pub(crate) fn record_button(
    document: &mut UiDocument,
    parent: UiNodeId,
    name: &str,
    label: impl Into<String>,
    active: bool,
) -> UiNodeId {
    let mut options = widgets::ButtonOptions::new(layout::fixed(82.0, 26.0));
    options.visual = UiVisual::panel(
        if active { RECORD_BG } else { BUTTON_BG },
        Some(StrokeStyle::new(
            if active {
                ColorRgba::new(255, 96, 104, 255)
            } else {
                PANEL_STROKE
            },
            if active { 1.5 } else { 1.0 },
        )),
        4.0,
    );
    options.text_style = text_style(13.0, STRONG_TEXT, false);
    widgets::button(document, parent, name, label, options)
}

pub(crate) fn checkbox(
    document: &mut UiDocument,
    parent: UiNodeId,
    name: &str,
    label: impl Into<String>,
    checked: bool,
    width: f32,
) -> UiNodeId {
    let mut options = widgets::CheckboxOptions::default();
    options.layout = layout::fixed(width, 26.0);
    options.box_visual = UiVisual::panel(
        ColorRgba::new(28, 34, 43, 255),
        Some(StrokeStyle::new(PANEL_STROKE, 1.0)),
        3.0,
    );
    options.checked_box_visual = Some(UiVisual::panel(
        ColorRgba::new(21, 72, 82, 255),
        Some(StrokeStyle::new(ACCENT, 1.0)),
        3.0,
    ));
    options.check_color = ACCENT;
    options.text_style = text_style(13.0, STRONG_TEXT, false);
    widgets::checkbox(document, parent, name, label, checked, options)
}

pub(crate) fn label(
    document: &mut UiDocument,
    parent: UiNodeId,
    name: &str,
    text: impl Into<String>,
    width: f32,
    strong: bool,
) -> UiNodeId {
    widgets::label(
        document,
        parent,
        name,
        text,
        text_style(13.0, if strong { STRONG_TEXT } else { MUTED_TEXT }, false),
        layout::fixed(width, 24.0),
    )
}

pub(crate) fn mono_label(
    document: &mut UiDocument,
    parent: UiNodeId,
    name: &str,
    text: impl Into<String>,
    width: f32,
) -> UiNodeId {
    widgets::label(
        document,
        parent,
        name,
        text,
        text_style(12.0, STRONG_TEXT, true),
        layout::fixed(width, 24.0),
    )
}

fn text_style(font_size: f32, color: ColorRgba, monospace: bool) -> TextStyle {
    TextStyle {
        font_size,
        line_height: font_size + 4.0,
        family: if monospace {
            FontFamily::Monospace
        } else {
            FontFamily::SansSerif
        },
        weight: FontWeight::NORMAL,
        color,
        ..Default::default()
    }
}
