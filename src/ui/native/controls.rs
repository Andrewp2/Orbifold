use operad::{
    AccessibilityMeta, AccessibilityRole, ColorRgba, FontFamily, InputBehavior, TextStyle,
    UiDocument, UiNode, UiRect, UiVisual, WidgetActionBinding, WidgetActionMode, layout, widgets,
};

use crate::ui::accessibility::button_accessibility_label;
use crate::ui::text::fit_label;
use crate::ui::theme::{accent, color, muted, stroke, strong};

pub(super) fn add_button_at(
    document: &mut UiDocument,
    name: impl Into<String>,
    label: impl Into<String>,
    rect: UiRect,
    active: bool,
    enabled: bool,
) {
    add_button_at_with_visible_label(document, name, label, None::<String>, rect, active, enabled);
}

pub(super) fn add_button_at_with_visible_label(
    document: &mut UiDocument,
    name: impl Into<String>,
    label: impl Into<String>,
    visible_label: Option<impl Into<String>>,
    rect: UiRect,
    active: bool,
    enabled: bool,
) {
    let name = name.into();
    let label = label.into();
    let visible_label = visible_label
        .map(Into::into)
        .unwrap_or_else(|| label.clone());
    let fitted_label = fit_label(&visible_label, rect.width - 8.0, 12.0);
    let options = button_options(&name, &label, rect, active, enabled);
    widgets::button(document, document.root, name, fitted_label, options);
}

pub(super) fn add_toggle_button_at(
    document: &mut UiDocument,
    name: impl Into<String>,
    label: impl Into<String>,
    rect: UiRect,
    selected: bool,
    enabled: bool,
) {
    let name = name.into();
    let label = label.into();
    let fitted_label = fit_label(&label, rect.width - 8.0, 12.0);
    let mut options = button_options(&name, &label, rect, false, enabled);
    options.pressed_visual = Some(active_button_visual());
    options.pressed_hovered_visual = Some(UiVisual::panel(
        color(42, 121, 148),
        Some(stroke(accent(), 1.0)),
        4.0,
    ));
    widgets::toggle_button(
        document,
        document.root,
        name,
        fitted_label,
        selected,
        options,
    );
}

pub(super) fn add_selectable_at(
    document: &mut UiDocument,
    name: impl Into<String>,
    label: impl Into<String>,
    rect: UiRect,
    selected: bool,
    enabled: bool,
) {
    let name = name.into();
    let label = label.into();
    let fitted_label = fit_label(&label, rect.width - 8.0, 12.0);
    let mut options = widgets::SelectableLabelOptions::default().with_layout(layout::absolute(
        rect.x,
        rect.y,
        rect.width,
        rect.height,
    ));
    options.visual = UiVisual::panel(color(27, 36, 48), Some(stroke(color(48, 64, 84), 1.0)), 4.0);
    options.hovered_visual = UiVisual::panel(
        color(33, 48, 62),
        Some(stroke(color(61, 82, 106), 1.0)),
        4.0,
    );
    options.selected_visual =
        UiVisual::panel(color(35, 104, 129), Some(stroke(accent(), 1.0)), 4.0);
    options.selected_hovered_visual =
        UiVisual::panel(color(42, 121, 148), Some(stroke(accent(), 1.0)), 4.0);
    options.focused_visual =
        UiVisual::panel(ColorRgba::TRANSPARENT, Some(stroke(accent(), 1.0)), 4.0);
    options.disabled_visual =
        UiVisual::panel(color(24, 29, 36), Some(stroke(color(44, 52, 64), 1.0)), 4.0);
    options.text_style = control_text_style(enabled);
    options.selected = selected;
    options.enabled = enabled;
    if enabled {
        options.action = Some(WidgetActionBinding::action(name.clone()));
    }
    options.accessibility_label = Some(button_accessibility_label(&name, &label));
    widgets::selectable_label(document, document.root, name, fitted_label, options);
}

pub(super) fn add_label_at(
    document: &mut UiDocument,
    name: impl Into<String>,
    text: impl Into<String>,
    rect: UiRect,
    strong_text: bool,
) {
    let text = text.into();
    let fitted_text = fit_label(&text, rect.width, 12.0);
    widgets::label(
        document,
        document.root,
        name,
        fitted_text,
        TextStyle {
            font_size: 12.0,
            line_height: 16.0,
            family: FontFamily::SansSerif,
            color: if strong_text { strong() } else { muted() },
            ..Default::default()
        },
        layout::absolute(rect.x, rect.y, rect.width, rect.height),
    );
}

pub(super) fn add_hit_at(document: &mut UiDocument, name: impl Into<String>, rect: UiRect) {
    add_hit_at_to(
        document,
        document.root,
        name,
        rect,
        WidgetActionMode::Activate,
    );
}

pub(super) fn add_pointer_edit_hit_at(
    document: &mut UiDocument,
    name: impl Into<String>,
    rect: UiRect,
) {
    add_pointer_edit_hit_at_to(document, document.root, name, rect);
}

pub(super) fn add_hit_at_to(
    document: &mut UiDocument,
    parent: operad::UiNodeId,
    name: impl Into<String>,
    rect: UiRect,
    action_mode: WidgetActionMode,
) {
    add_hit_at_with_mode(document, parent, name, rect, action_mode);
}

pub(super) fn add_pointer_edit_hit_at_to(
    document: &mut UiDocument,
    parent: operad::UiNodeId,
    name: impl Into<String>,
    rect: UiRect,
) {
    add_hit_at_with_mode(document, parent, name, rect, WidgetActionMode::PointerEdit);
}

fn button_options(
    name: &str,
    label: &str,
    rect: UiRect,
    active: bool,
    enabled: bool,
) -> widgets::ButtonOptions {
    let mut options =
        widgets::ButtonOptions::new(layout::absolute(rect.x, rect.y, rect.width, rect.height));
    options.visual = if active {
        active_button_visual()
    } else {
        UiVisual::panel(color(27, 36, 48), Some(stroke(color(48, 64, 84), 1.0)), 4.0)
    };
    options.pressed_visual = Some(UiVisual::panel(
        color(13, 20, 29),
        Some(stroke(accent(), 1.0)),
        4.0,
    ));
    options.disabled_visual = Some(UiVisual::panel(
        color(24, 29, 36),
        Some(stroke(color(44, 52, 64), 1.0)),
        4.0,
    ));
    options.text_style = control_text_style(enabled);
    options.enabled = enabled;
    if enabled {
        options.action = Some(WidgetActionBinding::action(name.to_string()));
    }
    options.accessibility_label = Some(button_accessibility_label(name, label));
    options
}

fn control_text_style(enabled: bool) -> TextStyle {
    TextStyle {
        font_size: 12.0,
        line_height: 16.0,
        family: FontFamily::SansSerif,
        color: if enabled { strong() } else { muted() },
        ..Default::default()
    }
}

fn active_button_visual() -> UiVisual {
    UiVisual::panel(color(35, 104, 129), Some(stroke(accent(), 1.0)), 4.0)
}

fn add_hit_at_with_mode(
    document: &mut UiDocument,
    parent: operad::UiNodeId,
    name: impl Into<String>,
    rect: UiRect,
    action_mode: WidgetActionMode,
) {
    let name = name.into();
    let hit_target = UiNode::container(
        name.clone(),
        layout::absolute(rect.x, rect.y, rect.width, rect.height),
    )
    .with_visual(UiVisual::TRANSPARENT)
    .with_input(InputBehavior {
        pointer: true,
        focusable: false,
        keyboard: false,
    })
    .with_action(WidgetActionBinding::action(name.clone()))
    .with_action_mode(action_mode)
    .with_accessibility(AccessibilityMeta::new(AccessibilityRole::Group).hidden());

    document.add_child(parent, hit_target);
}
