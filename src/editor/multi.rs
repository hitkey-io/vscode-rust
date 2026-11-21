//! Several cursors (multicursor.ts, cursorTypeOperations.ts, simplified):
//! pure functions over the text and selections `(anchor, head)` in char
//! indexes, the last one primary (the one VS Code reveals).
//!
//! - Add Selection To Next Find Match (⌘D): an empty selection first grows
//!   to the word at the caret (later matches are whole words, case
//!   sensitive); otherwise the next occurrence of the primary selection's
//!   text after it, wrapping, not already selected (case-insensitive, like
//!   the find widget's defaults).
//! - Select All Occurrences of Find Match (⇧⌘L).
//! - Add Cursor Above / Below (⌥⌘↑ / ⌥⌘↓): the same column on the line
//!   above / below each selection's head.
//! - Typing, Backspace, Delete, ← / → (with ⇧), Home / End at every cursor.

pub type Sel = (usize, usize);

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn word_at(chars: &[char], at: usize) -> Option<(usize, usize)> {
    let at = at.min(chars.len());
    let mut a = at;
    let mut b = at;
    while a > 0 && is_word(chars[a - 1]) {
        a -= 1;
    }
    while b < chars.len() && is_word(chars[b]) {
        b += 1;
    }
    (a < b).then_some((a, b))
}

/// Char-index occurrences of `needle` in `chars`.
fn occurrences(chars: &[char], needle: &[char], whole_word: bool, case_sensitive: bool) -> Vec<(usize, usize)> {
    let fold = |c: char| if case_sensitive { c } else { c.to_lowercase().next().unwrap_or(c) };
    let n = needle.len();
    let mut out = Vec::new();
    if n == 0 || n > chars.len() {
        return out;
    }
    let mut i = 0;
    while i + n <= chars.len() {
        if (0..n).all(|k| fold(chars[i + k]) == fold(needle[k])) {
            let ok = !whole_word || (!(i > 0 && is_word(chars[i - 1])) && !chars.get(i + n).is_some_and(|c| is_word(*c)));
            if ok {
                out.push((i, i + n));
                i += n;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// ⌘D. `word_mode` says whether the session started from an empty
/// selection (whole words, case sensitive); it is set when this call grows
/// the caret to a word. `None` when there's nothing to add.
pub fn add_next_occurrence(text: &str, sels: &[Sel], word_mode: &mut bool) -> Option<Vec<Sel>> {
    let chars: Vec<char> = text.chars().collect();
    let &(a, h) = sels.last()?;
    if sels.iter().all(|(a, h)| a == h) {
        // Every caret grows to its word.
        let grown: Vec<Sel> = sels.iter().filter_map(|&(_, h)| word_at(&chars, h)).collect();
        if grown.is_empty() {
            return None;
        }
        *word_mode = true;
        return Some(grown);
    }
    let (s, e) = (a.min(h), a.max(h));
    let needle: Vec<char> = chars[s..e.min(chars.len())].to_vec();
    let all = occurrences(&chars, &needle, *word_mode, *word_mode);
    let taken = |r: &(usize, usize)| sels.iter().any(|&(x, y)| (x.min(y), x.max(y)) == *r);
    let next = all.iter().find(|r| r.0 >= e && !taken(r)).or_else(|| all.iter().find(|r| !taken(r)))?;
    let mut out = sels.to_vec();
    out.push(*next);
    Some(out)
}

/// ⇧⌘L: every occurrence of the primary selection (or of the word at an
/// empty caret, whole words), the primary one last.
pub fn select_all_occurrences(text: &str, primary: Sel) -> Vec<Sel> {
    let chars: Vec<char> = text.chars().collect();
    let (a, h) = primary;
    let (range, word) = if a == h {
        match word_at(&chars, h) {
            Some(r) => (r, true),
            None => return vec![primary],
        }
    } else {
        ((a.min(h), a.max(h)), false)
    };
    let needle: Vec<char> = chars[range.0..range.1.min(chars.len())].to_vec();
    let mut all: Vec<Sel> = occurrences(&chars, &needle, word, word).into_iter().filter(|r| *r != range).collect();
    all.push(range);
    all
}

fn line_starts(chars: &[char]) -> Vec<usize> {
    let mut v = vec![0];
    v.extend(chars.iter().enumerate().filter(|(_, c)| **c == '\n').map(|(i, _)| i + 1));
    v
}

fn line_of(starts: &[usize], at: usize) -> usize {
    starts.partition_point(|&s| s <= at) - 1
}

/// ⌥⌘↑ / ⌥⌘↓: a caret at the same column on the line above / below the
/// primary's head (clamped to that line), added as the new primary.
pub fn add_cursor_vertical(text: &str, sels: &[Sel], up: bool) -> Vec<Sel> {
    let chars: Vec<char> = text.chars().collect();
    let starts = line_starts(&chars);
    let mut out = sels.to_vec();
    let Some(&(_, h)) = sels.last() else { return out };
    let line = line_of(&starts, h);
    let col = h - starts[line];
    let target = if up { line.checked_sub(1) } else { (line + 1 < starts.len()).then_some(line + 1) };
    if let Some(t) = target {
        let end = starts.get(t + 1).map_or(chars.len(), |s| s - 1);
        let at = (starts[t] + col).min(end);
        if !out.iter().any(|&(x, y)| x == at && y == at) {
            out.push((at, at));
        }
    }
    out
}

/// What to do at every cursor.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Insert(String),
    Backspace,
    Delete,
    Left { shift: bool },
    Right { shift: bool },
    Home { shift: bool },
    End { shift: bool },
}

/// Applies `op` at every selection; returns the text and the selections in
/// the same order (selections that end up equal are merged, the primary
/// kept last).
pub fn apply(text: &str, sels: &[Sel], op: &Op) -> (String, Vec<Sel>) {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let starts = line_starts(&chars);
    // Cursor moves don't touch the text.
    let moved = |&(a, h): &Sel| -> Option<Sel> {
        let (s, e) = (a.min(h), a.max(h));
        Some(match op {
            Op::Left { shift: false } => {
                let p = if s != e { s } else { h.saturating_sub(1) };
                (p, p)
            }
            Op::Right { shift: false } => {
                let p = if s != e { e } else { (h + 1).min(len) };
                (p, p)
            }
            Op::Left { shift: true } => (a, h.saturating_sub(1)),
            Op::Right { shift: true } => (a, (h + 1).min(len)),
            Op::Home { shift } => {
                let p = starts[line_of(&starts, h)];
                if *shift { (a, p) } else { (p, p) }
            }
            Op::End { shift } => {
                let l = line_of(&starts, h);
                let p = starts.get(l + 1).map_or(len, |s| s - 1);
                if *shift { (a, p) } else { (p, p) }
            }
            _ => return None,
        })
    };
    if let Some(first) = sels.first() {
        if moved(first).is_some() {
            return (text.to_string(), dedupe(sels.iter().map(|s| moved(s).unwrap()).collect()));
        }
    }
    // Edits: each selection replaces a range with some text.
    let mut edits: Vec<(usize, usize, usize, String)> = sels
        .iter()
        .enumerate()
        .map(|(k, &(a, h))| {
            let (s, e) = (a.min(h), a.max(h));
            let (s, e, ins) = match op {
                Op::Insert(t) => (s, e, t.clone()),
                Op::Backspace if s == e => (s.saturating_sub(1), e, String::new()),
                Op::Delete if s == e => (s, (e + 1).min(len), String::new()),
                _ => (s, e, String::new()),
            };
            (k, s, e, ins)
        })
        .collect();
    edits.sort_by_key(|e| (e.1, e.2));
    let mut out = String::new();
    let mut at = 0;
    let mut result = vec![(0, 0); sels.len()];
    let mut new_len = 0usize;
    for (k, s, e, ins) in &edits {
        let s = (*s).max(at);
        let e = (*e).max(s);
        let piece: String = chars[at..s].iter().collect();
        new_len += piece.chars().count();
        out.push_str(&piece);
        out.push_str(ins);
        new_len += ins.chars().count();
        result[*k] = (new_len, new_len);
        at = e;
    }
    out.extend(chars[at.min(len)..].iter());
    (out, dedupe(result))
}

fn dedupe(sels: Vec<Sel>) -> Vec<Sel> {
    let primary = *sels.last().unwrap_or(&(0, 0));
    let mut out: Vec<Sel> = Vec::new();
    for s in &sels[..sels.len().saturating_sub(1)] {
        if *s != primary && !out.contains(s) {
            out.push(*s);
        }
    }
    out.push(primary);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_d_grows_to_the_word_then_adds_the_next_whole_word() {
        let text = "foo food foo Foo foo";
        let mut word = false;
        let s = add_next_occurrence(text, &[(1, 1)], &mut word).unwrap();
        assert_eq!(s, vec![(0, 3)]);
        assert!(word);
        let s = add_next_occurrence(text, &s, &mut word).unwrap();
        assert_eq!(s, vec![(0, 3), (9, 12)]); // not "food", not "Foo"
        let s = add_next_occurrence(text, &s, &mut word).unwrap();
        assert_eq!(s, vec![(0, 3), (9, 12), (17, 20)]);
        assert_eq!(add_next_occurrence(text, &s, &mut word), None); // all taken
        // From a selection: plain text, any case, wrapping round.
        let mut word = false;
        let s = add_next_occurrence(text, &[(13, 16)], &mut word).unwrap();
        assert_eq!(s, vec![(13, 16), (17, 20)]);
        let s = add_next_occurrence(text, &s, &mut word).unwrap();
        assert_eq!(s.last(), Some(&(0, 3)));
    }

    #[test]
    fn select_all_and_vertical_cursors() {
        assert_eq!(select_all_occurrences("a b a a", (2, 2)), vec![(2, 3)]);
        assert_eq!(select_all_occurrences("ab x ab ab", (5, 7)), vec![(0, 2), (8, 10), (5, 7)]);
        let text = "abcd\nx\nabcdef";
        assert_eq!(add_cursor_vertical(text, &[(3, 3)], false), vec![(3, 3), (6, 6)]);
        assert_eq!(add_cursor_vertical(text, &[(3, 3), (6, 6)], false), vec![(3, 3), (6, 6), (8, 8)]);
        assert_eq!(add_cursor_vertical(text, &[(3, 3)], true), vec![(3, 3)]);
    }

    #[test]
    fn edits_apply_at_every_cursor() {
        let (t, s) = apply("foo x foo", &[(0, 3), (6, 9)], &Op::Insert("bar".into()));
        assert_eq!(t, "bar x bar");
        assert_eq!(s, vec![(3, 3), (9, 9)]);
        let (t, s) = apply("ab\nab", &[(1, 1), (4, 4)], &Op::Backspace);
        assert_eq!(t, "b\nb");
        assert_eq!(s, vec![(0, 0), (2, 2)]);
        let (t, s) = apply("ab\nab", &[(0, 0), (3, 3)], &Op::Delete);
        assert_eq!(t, "b\nb");
        assert_eq!(s, vec![(0, 0), (2, 2)]);
        let (_, s) = apply("ab\ncd", &[(1, 1), (4, 4)], &Op::End { shift: true });
        assert_eq!(s, vec![(1, 2), (4, 5)]);
        let (_, s) = apply("ab", &[(1, 1), (2, 2)], &Op::Left { shift: false });
        assert_eq!(s, vec![(0, 0), (1, 1)]);
        // Cursors that meet become one.
        let (_, s) = apply("ab", &[(0, 0), (1, 1)], &Op::Left { shift: false });
        assert_eq!(s, vec![(0, 0)]);
    }
}
