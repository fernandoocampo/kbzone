//! Pure plain-text table rendering for list-style command output.
//!
//! Columns are auto-fit to their longest cell so values never push later
//! columns out of alignment. Only the last column is capped (and truncated
//! with an ellipsis); every other column is always shown in full.

const LAST_COL_MAX_WIDTH: usize = 40;
const COL_GAP: &str = "  ";
const ELLIPSIS: char = '…';

fn char_len(s: &str) -> usize {
    s.chars().count()
}

fn truncate_cell(cell: &str, max: usize) -> String {
    if char_len(cell) <= max {
        return cell.to_string();
    }
    cell.chars()
        .take(max.saturating_sub(1))
        .chain(std::iter::once(ELLIPSIS))
        .collect()
}

fn fit_last_column(cell: &str) -> String {
    truncate_cell(cell, LAST_COL_MAX_WIDTH)
}

fn format_line(cells: &[String], widths: &[usize]) -> String {
    let last = cells.len().saturating_sub(1);
    let line = cells
        .iter()
        .zip(widths)
        .enumerate()
        .map(|(i, (cell, width))| {
            if i == last {
                cell.clone()
            } else {
                format!("{cell:<width$}")
            }
        })
        .collect::<Vec<_>>()
        .join(COL_GAP);
    format!("{line}\n")
}

/// Renders `headers`, a separator line and `rows` as an aligned table.
///
/// Every row must have the same number of cells as `headers`.
pub(crate) fn render_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let last = headers.len().saturating_sub(1);
    let header_cells: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    let body: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, cell)| {
                    if i == last {
                        fit_last_column(cell)
                    } else {
                        cell.clone()
                    }
                })
                .collect()
        })
        .collect();

    let widths: Vec<usize> = (0..headers.len())
        .map(|i| {
            body.iter()
                .map(|row| char_len(&row[i]))
                .chain(std::iter::once(char_len(headers[i])))
                .max()
                .unwrap_or(0)
        })
        .collect();

    let separator_len = widths.iter().sum::<usize>() + COL_GAP.len() * last;
    let mut out = format_line(&header_cells, &widths);
    out.push_str(&"-".repeat(separator_len));
    out.push('\n');
    body.iter()
        .for_each(|row| out.push_str(&format_line(row, &widths)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[&str]) -> Vec<String> {
        cells.iter().map(|c| c.to_string()).collect()
    }

    #[test]
    fn pads_columns_to_widest_cell_including_header() {
        let out = render_table(
            &["KEY", "CATEGORY", "TAGS"],
            &[row(&["a", "quote", "x"]), row(&["bb", "bookmark", "y"])],
        );
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "KEY  CATEGORY  TAGS");
        assert_eq!(lines[2], "a    quote     x");
        assert_eq!(lines[3], "bb   bookmark  y");
    }

    #[test]
    fn long_key_widens_column_and_keeps_later_columns_aligned() {
        let long_key = "a-very-long-key-that-used-to-break-the-layout-of-the-table";
        let out = render_table(
            &["KEY", "CATEGORY", "TAGS"],
            &[row(&[long_key, "quote", "t"]), row(&["k", "concept", "t"])],
        );
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[2].starts_with(long_key));
        let category_col = |l: &str| l.find("quote").or_else(|| l.find("concept"));
        assert_eq!(category_col(lines[2]), category_col(lines[3]));
        assert_eq!(category_col(lines[0]).is_none(), true);
        assert_eq!(lines[0].find("CATEGORY"), category_col(lines[2]));
    }

    #[test]
    fn counts_chars_not_bytes() {
        let out = render_table(
            &["KEY", "CAT"],
            &[row(&["señal", "X"]), row(&["abcde", "Y"])],
        );
        let lines: Vec<&str> = out.lines().collect();
        let col = |l: &str| l.chars().position(|c| c == 'X' || c == 'Y').unwrap();
        assert_eq!(col(lines[2]), 7);
        assert_eq!(col(lines[3]), 7);
    }

    #[test]
    fn no_rows_renders_header_and_separator_only() {
        let out = render_table(&["KEY", "TAGS"], &[]);
        assert_eq!(out, "KEY  TAGS\n---------\n");
    }

    #[test]
    fn separator_matches_table_width() {
        let out = render_table(&["KEY", "TAGS"], &[row(&["longer-key", "t"])]);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[1].len(), "longer-key".len() + 2 + "TAGS".len());
    }

    #[test]
    fn last_column_longer_than_cap_is_truncated_with_ellipsis() {
        let tags = "t".repeat(LAST_COL_MAX_WIDTH + 10);
        let out = render_table(&["KEY", "TAGS"], &[row(&["k", &tags])]);
        let cell = out.lines().nth(2).unwrap().trim_start_matches('k').trim();
        assert_eq!(cell.chars().count(), LAST_COL_MAX_WIDTH);
        assert!(cell.ends_with(ELLIPSIS));
    }

    #[test]
    fn last_column_within_cap_is_untouched() {
        let out = render_table(&["KEY", "TAGS"], &[row(&["k", "rust, cli"])]);
        assert!(out.lines().nth(2).unwrap().ends_with("rust, cli"));
    }

    #[test]
    fn truncation_never_splits_multibyte_chars() {
        let tags = "é".repeat(LAST_COL_MAX_WIDTH + 5);
        let cell = truncate_cell(&tags, LAST_COL_MAX_WIDTH);
        assert_eq!(cell.chars().count(), LAST_COL_MAX_WIDTH);
    }

    #[test]
    fn non_last_columns_are_never_truncated() {
        let ns = "n".repeat(LAST_COL_MAX_WIDTH + 20);
        let out = render_table(&["NS", "TAGS"], &[row(&[&ns, "t"])]);
        assert!(out.lines().nth(2).unwrap().starts_with(&ns));
    }

    #[test]
    fn last_column_has_no_trailing_spaces() {
        let out = render_table(
            &["KEY", "TAGS"],
            &[row(&["k", "short"]), row(&["k2", "a longer tag list"])],
        );
        assert!(out.lines().all(|l| l == l.trim_end()));
    }
}
