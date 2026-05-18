use std::fs;
use std::path::Path;

use crate::scale::ScalaScale;

pub(crate) fn parse_scala(path: &Path) -> Result<ScalaScale, String> {
    let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_scala_contents(&data)
}

pub(crate) fn parse_scala_contents(data: &str) -> Result<ScalaScale, String> {
    let mut lines = data
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('!'));

    let description = lines
        .next()
        .ok_or_else(|| "Missing scale description".to_string())?
        .to_string();
    let count_line = lines
        .next()
        .ok_or_else(|| "Missing scale count".to_string())?;
    let count = count_line
        .parse::<usize>()
        .map_err(|_| format!("Invalid scale count: {count_line}"))?;

    let mut parsed_steps = Vec::with_capacity(count);
    for line in lines {
        let ratio = parse_scala_ratio(line).ok_or_else(|| format!("Invalid scale step: {line}"))?;
        parsed_steps.push(ratio);
        if parsed_steps.len() >= count {
            break;
        }
    }

    if parsed_steps.len() < count {
        return Err(format!(
            "Expected {count} scale steps, found {}",
            parsed_steps.len()
        ));
    }

    let mut steps = Vec::with_capacity(count.max(1));
    steps.push(1.0);
    let parsed_without_period = if parsed_steps
        .last()
        .is_some_and(|ratio| (*ratio - 2.0).abs() <= f32::EPSILON * 8.0)
    {
        &parsed_steps[..parsed_steps.len() - 1]
    } else {
        parsed_steps.as_slice()
    };
    steps.extend_from_slice(parsed_without_period);

    if steps
        .iter()
        .any(|ratio| !ratio.is_finite() || *ratio <= 0.0)
    {
        return Err("Scale steps must be positive finite ratios".to_string());
    }

    Ok(ScalaScale { description, steps })
}

fn parse_scala_ratio(s: &str) -> Option<f32> {
    let trimmed = s.split_whitespace().next().unwrap_or("");
    if trimmed.contains('/') {
        let mut parts = trimmed.split('/');
        let num: f32 = parts.next()?.parse().ok()?;
        let den: f32 = parts.next()?.parse().ok()?;
        if parts.next().is_some() || den == 0.0 {
            return None;
        }
        let ratio = num / den;
        ratio.is_finite().then_some(ratio)
    } else {
        let cents: f32 = trimmed.parse().ok()?;
        let ratio = 2.0_f32.powf(cents / 1200.0);
        ratio.is_finite().then_some(ratio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx_eq(left: f32, right: f32) {
        assert!(
            (left - right).abs() < 0.000_01,
            "left: {left}, right: {right}"
        );
    }

    #[test]
    fn parse_standard_scala_scale_prepends_unison_and_removes_octave_period() {
        let scale = parse_scala_contents(
            r#"
! comment
12 tone equal temperament
12
100.0
200.0
300.0
400.0
500.0
600.0
700.0
800.0
900.0
1000.0
1100.0
2/1
"#,
        )
        .expect("scale should parse");

        assert_eq!(scale.description, "12 tone equal temperament");
        assert_eq!(scale.steps.len(), 12);
        assert_approx_eq(scale.steps[0], 1.0);
        assert_approx_eq(scale.steps[1], 2.0_f32.powf(100.0 / 1200.0));
        assert_approx_eq(scale.steps[11], 2.0_f32.powf(1100.0 / 1200.0));
    }

    #[test]
    fn parse_ratio_steps() {
        let scale = parse_scala_contents(
            r#"
Just pentatonic
5
16/15
9/8
6/5
3/2
2/1
"#,
        )
        .expect("scale should parse");

        assert_eq!(
            scale.steps,
            vec![1.0, 16.0 / 15.0, 9.0 / 8.0, 6.0 / 5.0, 3.0 / 2.0]
        );
    }

    #[test]
    fn parse_large_equal_temperament_scale() {
        let count = 4096;
        let mut data = format!("Large generated EDO\n{count}\n");
        for step in 1..=count {
            let cents = step as f32 * 1200.0 / count as f32;
            data.push_str(&format!("{cents:.8}\n"));
        }

        let scale = parse_scala_contents(&data).expect("large generated scale should parse");

        assert_eq!(scale.description, "Large generated EDO");
        assert_eq!(scale.steps.len(), count);
        assert_approx_eq(scale.steps[0], 1.0);
        assert_approx_eq(scale.steps[1], 2.0_f32.powf(1.0 / count as f32));
        assert!(scale.steps[count - 1] < 2.0);
    }

    #[test]
    fn parse_errors_when_step_count_is_not_met() {
        let err = parse_scala_contents(
            r#"
Too short
3
100.0
200.0
"#,
        )
        .expect_err("scale should fail");

        assert_eq!(err, "Expected 3 scale steps, found 2");
    }

    #[test]
    fn bundled_scales_parse() {
        let Ok(entries) = std::fs::read_dir("scales") else {
            return;
        };
        let mut parsed = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("scl") {
                continue;
            }
            parse_scala(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
            parsed += 1;
        }
        assert!(parsed > 0, "expected bundled .scl files");
    }
}
