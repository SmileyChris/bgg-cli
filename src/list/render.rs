use crate::list::Col;
use crate::model::CollectionItem;
use anstream::println;
use anstyle::{Effects, Style};
use std::io::IsTerminal;

const MUTED: Style = Style::new().effects(Effects::DIMMED);

pub(crate) fn print_footer(shown: usize, total: usize) {
    if total == 0 {
        println!("{MUTED}No items match the filter.{MUTED:#}");
    } else if shown < total {
        println!("{MUTED}Showing {shown} of {total} items.{MUTED:#}");
    } else {
        println!("{MUTED}{total} items.{MUTED:#}");
    }
}

pub(crate) fn render_table(items: &[&CollectionItem], cols: &[Col]) {
    let linkify = std::io::stdout().is_terminal();

    let mut rows: Vec<Vec<String>> = Vec::with_capacity(items.len());
    for it in items {
        rows.push(cols.iter().map(|c| c.cell(it, linkify)).collect());
    }

    let last_idx = cols.len().saturating_sub(1);
    let mut widths: Vec<usize> = cols.iter().map(|c| c.header().chars().count()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            let w = display_width(cell);
            if w > widths[i] {
                widths[i] = w;
            }
        }
    }

    let mut header = String::new();
    for (i, c) in cols.iter().enumerate() {
        if i == last_idx && c.is_last_friendly() {
            header.push_str(c.header());
        } else {
            header.push_str(&pad_right(c.header(), widths[i]));
            header.push_str("  ");
        }
    }
    println!("{header}");

    for row in rows {
        let mut line = String::new();
        for (i, cell) in row.iter().enumerate() {
            if i == last_idx && cols[i].is_last_friendly() {
                line.push_str(cell);
            } else {
                line.push_str(&pad_right(cell, widths[i]));
                line.push_str("  ");
            }
        }
        println!("{line}");
    }
}

fn pad_right(s: &str, width: usize) -> String {
    let w = display_width(s);
    if w >= width {
        s.to_string()
    } else {
        let mut out = s.to_string();
        out.push_str(&" ".repeat(width - w));
        out
    }
}

fn display_width(s: &str) -> usize {
    let mut visible = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&']') {
            while let Some(c2) = chars.next() {
                if c2 == '\x1b' && chars.next() == Some('\\') {
                    break;
                }
            }
        } else {
            visible.push(c);
        }
    }
    visible.chars().count()
}
