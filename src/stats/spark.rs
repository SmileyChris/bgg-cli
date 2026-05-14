use sparkline::{select_sparkline, SparkTheme, SparkThemeName};

/// Render a slice of counts as a one-line sparkline using min=0 and max=max(values).
/// Zero values render as a space so gaps in the data show through.
pub(super) fn sparkline_row(values: &[usize]) -> String {
    let theme = select_sparkline(SparkThemeName::Classic);
    let max = *values.iter().max().unwrap_or(&0);
    if max == 0 {
        return " ".repeat(values.len());
    }
    values
        .iter()
        .map(|v| {
            if *v == 0 {
                " ".to_string()
            } else {
                spark_char(&theme, *v as f64, 0.0, max as f64)
            }
        })
        .collect()
}

fn spark_char(theme: &SparkTheme, value: f64, min: f64, max: f64) -> String {
    if max <= min {
        return " ".into();
    }
    theme.spark(min, max, value).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_row_uses_eight_block_chars_and_handles_zero_max() {
        assert_eq!(sparkline_row(&[0, 0, 0]), "   ");
        let out = sparkline_row(&[1, 2, 4, 8]);
        assert_eq!(out.chars().count(), 4);
        assert!(out.ends_with('█'));
    }

    #[test]
    fn sparkline_row_renders_zero_buckets_as_spaces() {
        let out = sparkline_row(&[5, 0, 0, 5]);
        let chars: Vec<char> = out.chars().collect();
        assert_eq!(chars.len(), 4);
        assert_eq!(chars[1], ' ');
        assert_eq!(chars[2], ' ');
        assert_ne!(chars[0], ' ');
        assert_ne!(chars[3], ' ');
    }
}
