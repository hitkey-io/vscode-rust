//! Text positions between the editor (UTF-8 byte offsets, 1-based lines
//! and character columns) and the extension API (0-based lines, columns in
//! UTF-16 code units, as `vscode.Position`).

use super::{DecoratedRange, WireEdit};

/// Byte offset of `(line, character)` (0-based, UTF-16 character), clamped
/// to the line and the text.
pub fn utf16_to_byte(text: &str, line: usize, character: usize) -> usize {
    let mut start = 0usize;
    for (i, l) in text.split('\n').enumerate() {
        if i == line {
            let mut units = 0usize;
            for (b, ch) in l.char_indices() {
                if units >= character {
                    return start + b;
                }
                units += ch.len_utf16();
            }
            return start + l.trim_end_matches('\r').len();
        }
        start += l.len() + 1;
    }
    text.len()
}

/// UTF-16 column of the `char_col`-th character (0-based) of `line`.
pub fn char_col_to_utf16(text: &str, line: usize, char_col: usize) -> usize {
    text.split('\n')
        .nth(line)
        .map(|l| l.chars().take(char_col).map(char::len_utf16).sum())
        .unwrap_or(0)
}

/// Byte offset of the `char_index`-th character.
pub fn char_to_byte(text: &str, char_index: usize) -> usize {
    text.char_indices().nth(char_index).map(|(b, _)| b).unwrap_or(text.len())
}

/// `(line, UTF-16 character)` of a byte offset (0-based).
pub fn byte_to_position(text: &str, byte: usize) -> (usize, usize) {
    let byte = byte.min(text.len());
    let before = &text[..byte];
    let line = before.matches('\n').count();
    let line_start = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
    (line, text[line_start..byte].chars().map(char::len_utf16).sum())
}

/// Apply edits (ranges in the document as it was before any of them).
pub fn apply_edits(text: &mut String, edits: &[WireEdit]) {
    let mut spans: Vec<(usize, usize, &str)> = edits
        .iter()
        .map(|e| {
            let a = utf16_to_byte(text, e.range[0], e.range[1]);
            let b = utf16_to_byte(text, e.range[2], e.range[3]);
            (a.min(b), a.max(b), e.text.as_str())
        })
        .collect();
    // Last first, so earlier offsets stay valid.
    spans.sort_by(|x, y| y.0.cmp(&x.0).then(y.1.cmp(&x.1)));
    for (a, b, t) in spans {
        text.replace_range(a..b, t);
    }
}

/// Byte spans of decorated ranges; whole-line decorations cover their lines.
pub fn decoration_spans(text: &str, ranges: &[DecoratedRange], whole_line: bool) -> Vec<(usize, usize)> {
    ranges
        .iter()
        .map(|r| {
            if whole_line {
                (utf16_to_byte(text, r.range[0], 0), utf16_to_byte(text, r.range[2], usize::MAX))
            } else {
                (utf16_to_byte(text, r.range[0], r.range[1]), utf16_to_byte(text, r.range[2], r.range[3]))
            }
        })
        .filter(|(a, b)| b > a)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_positions() {
        let t = "a😀b\nцх";
        assert_eq!(utf16_to_byte(t, 0, 3), 5); // after the surrogate pair
        assert_eq!(utf16_to_byte(t, 1, 1), 7 + 2); // "a😀b\n" is 7 bytes
        assert_eq!(utf16_to_byte(t, 1, 99), t.len());
        assert_eq!(char_col_to_utf16(t, 0, 2), 3);
    }

    #[test]
    fn edits_apply_from_the_end() {
        let mut t = "hello world\nsecond".to_string();
        apply_edits(
            &mut t,
            &[
                WireEdit { range: [0, 0, 0, 5], text: "HELLO".into() },
                WireEdit { range: [1, 0, 1, 0], text: ">> ".into() },
            ],
        );
        assert_eq!(t, "HELLO world\n>> second");
    }
}
