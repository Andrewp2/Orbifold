use std::collections::HashMap;

#[test]
fn linux_desktop_file_points_at_orbifold_binary() {
    let desktop = include_str!("../packaging/linux/orbifold.desktop");
    let entries = desktop_entries(desktop);

    assert_eq!(entries.get("Type").map(String::as_str), Some("Application"));
    assert_eq!(entries.get("Name").map(String::as_str), Some("Orbifold"));
    assert_eq!(entries.get("Exec").map(String::as_str), Some("orbifold"));
    assert_eq!(entries.get("Icon").map(String::as_str), Some("orbifold"));
    assert_eq!(entries.get("Terminal").map(String::as_str), Some("false"));
    assert!(
        entries
            .get("Categories")
            .is_some_and(|categories| categories.contains("Audio"))
    );
}

#[test]
fn linux_desktop_icon_asset_matches_desktop_icon_name() {
    let desktop = include_str!("../packaging/linux/orbifold.desktop");
    let entries = desktop_entries(desktop);
    let icon_name = entries
        .get("Icon")
        .expect("desktop file should name an icon");
    let icon_path = format!("packaging/linux/icons/hicolor/scalable/apps/{icon_name}.svg");
    let icon = std::fs::read_to_string(&icon_path).expect("desktop icon asset should exist");

    assert!(icon.contains("<svg"));
    assert!(icon.contains("viewBox=\"0 0 128 128\""));
}

#[test]
fn linux_png_icon_matches_source_icon() {
    let source = include_bytes!("../orbifold_icon.png");
    let packaged = include_bytes!("../packaging/linux/icons/hicolor/64x64/apps/orbifold.png");

    assert_eq!(packaged, source);
    assert_eq!(&packaged[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn favicon_ico_contains_browser_icon_sizes() {
    let favicon = include_bytes!("../favicon.ico");

    assert_eq!(&favicon[0..2], &[0, 0], "ICO reserved field");
    assert_eq!(&favicon[2..4], &[1, 0], "ICO image type should be icon");
    let count = u16::from_le_bytes([favicon[4], favicon[5]]) as usize;
    assert_eq!(count, 4);
    assert!(favicon.len() >= 6 + count * 16);

    let mut sizes = (0..count)
        .map(|index| {
            let offset = 6 + index * 16;
            (favicon[offset], favicon[offset + 1])
        })
        .collect::<Vec<_>>();
    sizes.sort_unstable();

    assert_eq!(sizes, vec![(16, 16), (32, 32), (48, 48), (64, 64)]);
}

fn desktop_entries(desktop: &str) -> HashMap<String, String> {
    desktop
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('[') || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}
