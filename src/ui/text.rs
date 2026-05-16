pub(super) fn compact_label(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut out = String::new();
    for _ in 0..max_chars {
        let Some(ch) = chars.next() else {
            return out;
        };
        out.push(ch);
    }
    if chars.next().is_some() {
        out.push_str("...");
    }
    out
}

pub(super) fn fit_label(value: &str, width: f32, font_size: f32) -> String {
    let available = width.max(0.0);
    if available <= 0.0 {
        return String::new();
    }
    if estimated_text_width(value, font_size) <= available {
        return value.to_string();
    }

    let ellipsis = "...";
    let ellipsis_width = estimated_text_width(ellipsis, font_size);
    if available <= ellipsis_width {
        let mut out = String::new();
        for ch in value.chars() {
            let candidate_width = estimated_text_width_with_extra(&out, ch, font_size);
            if candidate_width > available {
                break;
            }
            out.push(ch);
        }
        return out;
    }

    let mut out = String::new();
    for ch in value.chars() {
        let candidate_width = estimated_text_width_with_extra(&out, ch, font_size) + ellipsis_width;
        if candidate_width > available {
            break;
        }
        out.push(ch);
    }
    if out.is_empty() {
        String::new()
    } else {
        out.push_str(ellipsis);
        out
    }
}

pub(super) fn estimated_text_width(value: &str, font_size: f32) -> f32 {
    value
        .chars()
        .map(|ch| estimated_char_width(ch, font_size))
        .sum()
}

fn estimated_text_width_with_extra(value: &str, extra: char, font_size: f32) -> f32 {
    estimated_text_width(value, font_size) + estimated_char_width(extra, font_size)
}

fn estimated_char_width(ch: char, font_size: f32) -> f32 {
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
}
