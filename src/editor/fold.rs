//! Indentation-based code folding.
//!
//! VS Code computes foldable regions from two sources: language-aware
//! markers (`#region`, bracket pairs) and a generic indentation model. We
//! port the indentation model — it covers the common case (a block whose
//! following lines are more deeply indented) without needing a parser.
//!
//! A line `H` is a *fold header* when at least one of the lines that follow
//! it — skipping blank lines — is indented deeper than `H`. The region runs
//! from `H` down to the last consecutive line that is deeper than `H`
//! (trailing blank lines inside the deeper block are absorbed; a blank line
//! followed by a shallower line ends the region).

use std::collections::BTreeMap;

/// Visible width of a line's leading whitespace, with tabs counted as 4.
/// Returns `None` for blank/whitespace-only lines (they don't define a level).
fn indent_of(line: &str) -> Option<usize> {
    let mut width = 0usize;
    for ch in line.chars() {
        match ch {
            ' ' => width += 1,
            '\t' => width += 4,
            _ => return Some(width),
        }
    }
    None
}

/// Map of `header_line -> last_line` (both 1-based, inclusive) for every
/// foldable region in `text`.
pub fn foldable_ranges(text: &str) -> BTreeMap<usize, usize> {
    let lines: Vec<&str> = text.split('\n').collect();
    let n = lines.len();
    let indents: Vec<Option<usize>> = lines.iter().map(|l| indent_of(l)).collect();
    let mut ranges = BTreeMap::new();

    for i in 0..n {
        let Some(cur) = indents[i] else { continue };
        // Walk forward absorbing deeper (or blank) lines.
        let mut j = i + 1;
        let mut last_deeper = i;
        while j < n {
            match indents[j] {
                None => {
                    // Blank line: tentatively part of the region, but only
                    // kept if a deeper line follows before a shallower one.
                    j += 1;
                }
                Some(level) if level > cur => {
                    last_deeper = j;
                    j += 1;
                }
                Some(_) => break,
            }
        }
        if last_deeper > i {
            // Convert to 1-based line numbers.
            ranges.insert(i + 1, last_deeper + 1);
        }
    }

    ranges
}

/// Returns `true` if `line` (1-based) is hidden by one of the currently
/// folded `headers`, given the computed `ranges`. A line is hidden when it
/// falls strictly after a folded header and within its region.
pub fn is_hidden(
    line: usize,
    headers: &std::collections::BTreeSet<usize>,
    ranges: &BTreeMap<usize, usize>,
) -> bool {
    headers.iter().any(|&h| {
        if let Some(&end) = ranges.get(&h) {
            line > h && line <= end
        } else {
            false
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_simple_block() {
        let text = "fn main() {\n    let x = 1;\n    let y = 2;\n}\n";
        let r = foldable_ranges(text);
        // Line 1 is the header, body runs through line 3 (the deeper lines).
        assert_eq!(r.get(&1), Some(&3));
    }

    #[test]
    fn blank_line_inside_block_is_absorbed() {
        let text = "a\n  b\n\n  c\nd\n";
        let r = foldable_ranges(text);
        assert_eq!(r.get(&1), Some(&4));
    }

    #[test]
    fn no_fold_for_flat_text() {
        let text = "a\nb\nc\n";
        assert!(foldable_ranges(text).is_empty());
    }
}
