//! Editing behaviour VS Code's editor has on top of a plain text box: the
//! auto-closing pairs, auto-indent on Enter, Backspace over a pair, Tab and
//! Shift+Tab, line comments, and the line commands (move, copy, delete,
//! insert, indent). Everything is a pure function of the text and the
//! selection (in `char` indexes, the units of egui's `CCursor`), so it is
//! tested without a UI; `view.rs` feeds it the keys the text box would
//! otherwise handle itself.
//!
//! Sources: `editor/common/languages/autoIndent`, `languageConfiguration`
//! (autoClosingPairs / autoCloseBefore), `editor/contrib/comment`,
//! `linesOperations`, `cursor/cursorTypeOperations`.

/// The result of an edit: the whole new text and the new selection.
#[derive(Clone, Debug, PartialEq)]
pub struct Edit {
    pub text: String,
    pub anchor: usize,
    pub head: usize,
}

fn chars(text: &str) -> Vec<char> {
    text.chars().collect()
}

fn edit(chars: Vec<char>, anchor: usize, head: usize) -> Edit {
    Edit { text: chars.into_iter().collect(), anchor, head }
}

/// `autoCloseBefore`: a pair is only closed before one of these (or the line end).
const CLOSE_BEFORE: &str = ";:.,=}])> \n\t";

fn closer(open: char) -> Option<char> {
    match open {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    }
}

fn is_quote(c: char) -> bool {
    matches!(c, '"' | '\'' | '`')
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether `language` closes quotes (code languages; not plain text or
/// markdown, where an apostrophe is just a letter).
pub fn closes_quotes(language: &str) -> bool {
    !matches!(language, "" | "md")
}

/// A character was typed. `Some` when the editor does something other than
/// insert it: closes a pair, steps over a closer, or surrounds a selection.
pub fn type_char(text: &str, anchor: usize, head: usize, c: char, quotes: bool) -> Option<Edit> {
    let (s, e) = (anchor.min(head), anchor.max(head));
    let mut ch = chars(text);
    let next = ch.get(e).copied();
    let prev = if s > 0 { ch.get(s - 1).copied() } else { None };
    let is_open = closer(c).is_some();
    let is_close = matches!(c, ')' | ']' | '}');
    let is_q = quotes && is_quote(c);
    if !(is_open || is_close || is_q) {
        return None;
    }
    if s != e {
        // Surround the selection; it stays selected (inside the pair).
        if is_open || is_q {
            let close = closer(c).unwrap_or(c);
            ch.insert(e, close);
            ch.insert(s, c);
            let (a, h) = if anchor <= head { (s + 1, e + 1) } else { (e + 1, s + 1) };
            return Some(edit(ch, a, h));
        }
        return None;
    }
    // Typing the closer in front of the same closer steps over it.
    if (is_close || is_q) && next == Some(c) {
        return Some(edit(ch, s + 1, s + 1));
    }
    if is_close {
        return None;
    }
    // A quote after a word character is an apostrophe (don't, it's).
    if is_q && prev.is_some_and(is_word) {
        return None;
    }
    if next.is_none_or(|n| CLOSE_BEFORE.contains(n)) {
        let close = closer(c).unwrap_or(c);
        ch.insert(s, close);
        ch.insert(s, c);
        return Some(edit(ch, s + 1, s + 1));
    }
    None
}

/// Index of the first character of the line holding `pos`.
fn line_start(ch: &[char], pos: usize) -> usize {
    ch[..pos.min(ch.len())].iter().rposition(|&c| c == '\n').map_or(0, |i| i + 1)
}

fn leading_ws(ch: &[char], from: usize, to: usize) -> String {
    ch[from..to].iter().take_while(|c| **c == ' ' || **c == '\t').collect()
}

/// Enter: a new line with the indentation of the current one, one more after
/// an opening bracket (or a Python colon), and a closer between a pair goes on
/// its own line.
pub fn enter(text: &str, anchor: usize, head: usize, unit: &str, language: &str) -> Edit {
    let (s, e) = (anchor.min(head), anchor.max(head));
    let mut ch = chars(text);
    let start = line_start(&ch, s);
    let indent = leading_ws(&ch, start, s);
    let before = if s > 0 { ch.get(s - 1).copied() } else { None };
    let after = ch.get(e).copied();
    let opens = before.is_some_and(|b| closer(b).is_some()) || (language == "py" && before == Some(':'));
    let pair = before.and_then(closer).is_some_and(|c| Some(c) == after);
    let (insert, cursor): (String, usize) = if opens && pair {
        let middle = format!("\n{indent}{unit}");
        let len = middle.chars().count();
        (format!("{middle}\n{indent}"), s + len)
    } else if opens {
        let t = format!("\n{indent}{unit}");
        let len = t.chars().count();
        (t, s + len)
    } else {
        let t = format!("\n{indent}");
        let len = t.chars().count();
        (t, s + len)
    };
    ch.splice(s..e, insert.chars());
    edit(ch, cursor, cursor)
}

/// Backspace: deletes both characters of an empty auto-closed pair; in the
/// indentation it steps back to the previous tab stop (`useTabStops`).
pub fn backspace(text: &str, anchor: usize, head: usize, tab_size: usize) -> Option<Edit> {
    if anchor != head || anchor == 0 {
        return None;
    }
    let mut ch = chars(text);
    let p = anchor;
    let (prev, next) = (ch[p - 1], ch.get(p).copied());
    if next.is_some() && (closer(prev) == next || (is_quote(prev) && next == Some(prev))) {
        ch.drain(p - 1..p + 1);
        return Some(edit(ch, p - 1, p - 1));
    }
    // Only spaces before the cursor on its line: back to the previous tab stop.
    let start = line_start(&ch, p);
    let col = p - start;
    if col > 0 && tab_size > 0 && ch[start..p].iter().all(|&c| c == ' ') && prev == ' ' {
        let stop = (col - 1) / tab_size * tab_size;
        ch.drain(start + stop..p);
        return Some(edit(ch, start + stop, start + stop));
    }
    None
}

// ── lines ────────────────────────────────────────────────────────────────

struct Lines {
    lines: Vec<Vec<char>>,
}

impl Lines {
    fn new(text: &str) -> Self {
        Lines { lines: text.split('\n').map(|l| l.chars().collect()).collect() }
    }

    fn text(&self) -> String {
        self.lines.iter().map(|l| l.iter().collect::<String>()).collect::<Vec<_>>().join("\n")
    }

    /// (line, column) of a char index.
    fn lc(&self, pos: usize) -> (usize, usize) {
        let mut p = pos;
        for (i, l) in self.lines.iter().enumerate() {
            if p <= l.len() {
                return (i, p);
            }
            p -= l.len() + 1;
        }
        let last = self.lines.len() - 1;
        (last, self.lines[last].len())
    }

    fn pos(&self, line: usize, col: usize) -> usize {
        let line = line.min(self.lines.len() - 1);
        self.lines[..line].iter().map(|l| l.len() + 1).sum::<usize>() + col.min(self.lines[line].len())
    }
}

/// The lines a command covers: those of the selection, not counting the last
/// one when the selection ends at its very start.
fn covered(l: &Lines, anchor: usize, head: usize) -> (usize, usize) {
    let (s, e) = (anchor.min(head), anchor.max(head));
    let first = l.lc(s).0;
    let (el, ec) = l.lc(e);
    let last = if e > s && ec == 0 && el > first { el - 1 } else { el };
    (first, last)
}

/// Alt+Up / Alt+Down: the covered lines swap places with their neighbour.
pub fn move_lines(text: &str, anchor: usize, head: usize, up: bool) -> Option<Edit> {
    let mut l = Lines::new(text);
    let (first, last) = covered(&l, anchor, head);
    let (a, h) = (l.lc(anchor), l.lc(head));
    if up {
        if first == 0 {
            return None;
        }
        let moved = l.lines.remove(first - 1);
        l.lines.insert(last, moved);
    } else {
        if last + 1 >= l.lines.len() {
            return None;
        }
        let moved = l.lines.remove(last + 1);
        l.lines.insert(first, moved);
    }
    let d = |lc: (usize, usize)| if up { (lc.0 - 1, lc.1) } else { (lc.0 + 1, lc.1) };
    let (na, nh) = (d(a), d(h));
    Some(Edit { text: l.text(), anchor: l.pos(na.0, na.1), head: l.pos(nh.0, nh.1) })
}

/// Alt+Shift+Up / Down: the covered lines are copied above or below; the
/// selection moves to the copy.
pub fn copy_lines(text: &str, anchor: usize, head: usize, up: bool) -> Edit {
    let mut l = Lines::new(text);
    let (first, last) = covered(&l, anchor, head);
    let (a, h) = (l.lc(anchor), l.lc(head));
    let block: Vec<Vec<char>> = l.lines[first..=last].to_vec();
    let n = block.len();
    let at = if up { first } else { last + 1 };
    for (i, line) in block.into_iter().enumerate() {
        l.lines.insert(at + i, line);
    }
    let shift = if up { 0 } else { n };
    Edit { text: l.text(), anchor: l.pos(a.0 + shift, a.1), head: l.pos(h.0 + shift, h.1) }
}

/// Selection ▸ Duplicate Selection: the selected text is inserted again right
/// after it and the copy is selected; without a selection the line is
/// duplicated below and the caret goes to the copy.
pub fn duplicate_selection(text: &str, anchor: usize, head: usize) -> Edit {
    let (s, e) = (anchor.min(head), anchor.max(head));
    if s == e {
        return copy_lines(text, anchor, head, false);
    }
    let ch = chars(text);
    let selected: String = ch[s..e].iter().collect();
    let mut out: String = ch[..e].iter().collect();
    out.push_str(&selected);
    out.extend(ch[e..].iter());
    let len = e - s;
    Edit { text: out, anchor: anchor + len, head: head + len }
}

/// ⇧⌘K: the covered lines go; the cursor lands on the line that took their place.
pub fn delete_lines(text: &str, anchor: usize, head: usize) -> Edit {
    let mut l = Lines::new(text);
    let (first, last) = covered(&l, anchor, head);
    let col = l.lc(head).1;
    l.lines.drain(first..=last);
    if l.lines.is_empty() {
        l.lines.push(Vec::new());
    }
    let line = first.min(l.lines.len() - 1);
    let p = l.pos(line, col);
    Edit { text: l.text(), anchor: p, head: p }
}

/// ⌘Enter / ⇧⌘Enter: an empty line below / above, with the indentation of the
/// current one (one more after an opening bracket when below).
pub fn insert_line(text: &str, anchor: usize, head: usize, below: bool, unit: &str) -> Edit {
    let mut l = Lines::new(text);
    let (first, last) = covered(&l, anchor, head);
    let from = if below { last } else { first };
    let mut indent: Vec<char> = l.lines[from].iter().take_while(|c| **c == ' ' || **c == '\t').copied().collect();
    if below && l.lines[from].last().is_some_and(|c| closer(*c).is_some()) {
        indent.extend(unit.chars());
    }
    let at = if below { last + 1 } else { first };
    let col = indent.len();
    l.lines.insert(at, indent);
    let p = l.pos(at, col);
    Edit { text: l.text(), anchor: p, head: p }
}

/// ⌘] and Tab over several lines: one indent unit at the start of each non-empty covered line.
pub fn indent_lines(text: &str, anchor: usize, head: usize, unit: &str) -> Edit {
    let mut l = Lines::new(text);
    let (first, last) = covered(&l, anchor, head);
    let (a, h) = (l.lc(anchor), l.lc(head));
    let n = unit.chars().count();
    let grow = |lc: (usize, usize), l: &Lines| if (first..=last).contains(&lc.0) && !l.lines[lc.0].is_empty() { lc.1 + n } else { lc.1 };
    let (ac, hc) = (grow(a, &l), grow(h, &l));
    for line in &mut l.lines[first..=last] {
        if !line.is_empty() {
            line.splice(0..0, unit.chars());
        }
    }
    Edit { text: l.text(), anchor: l.pos(a.0, ac), head: l.pos(h.0, hc) }
}

/// ⌘[ and Shift+Tab: one indent unit (a tab, or up to `unit` spaces) off the
/// start of each covered line.
pub fn outdent_lines(text: &str, anchor: usize, head: usize, tab_size: usize) -> Option<Edit> {
    let mut l = Lines::new(text);
    let (first, last) = covered(&l, anchor, head);
    let (a, h) = (l.lc(anchor), l.lc(head));
    let mut removed = vec![0usize; l.lines.len()];
    for i in first..=last {
        let line = &mut l.lines[i];
        let n = if line.first() == Some(&'\t') {
            1
        } else {
            line.iter().take(tab_size.max(1)).take_while(|c| **c == ' ').count()
        };
        line.drain(..n);
        removed[i] = n;
    }
    if removed.iter().all(|r| *r == 0) {
        return None;
    }
    let shrink = |lc: (usize, usize)| lc.1.saturating_sub(removed[lc.0]);
    Some(Edit { text: l.text(), anchor: l.pos(a.0, shrink(a)), head: l.pos(h.0, shrink(h)) })
}

/// Tab: inserts spaces up to the next tab stop (or a tab), or indents when
/// the selection covers several lines.
pub fn tab(text: &str, anchor: usize, head: usize, unit: &str, tab_size: usize) -> Edit {
    let l = Lines::new(text);
    let (first, last) = covered(&l, anchor, head);
    if first != last {
        return indent_lines(text, anchor, head, unit);
    }
    let (s, e) = (anchor.min(head), anchor.max(head));
    let mut ch = chars(text);
    let insert: String = if unit == "\t" {
        "\t".to_string()
    } else {
        let col = s - line_start(&ch, s);
        " ".repeat(tab_size - col % tab_size.max(1))
    };
    let n = insert.chars().count();
    ch.splice(s..e, insert.chars());
    edit(ch, s + n, s + n)
}

// ── comments ─────────────────────────────────────────────────────────────

/// `comments.lineComment` of a language.
fn line_comment(language: &str) -> Option<&'static str> {
    match language {
        "rs" | "ts" | "tsx" | "js" | "jsx" | "c" | "h" | "cpp" | "go" | "json" => Some("//"),
        "py" | "sh" | "yaml" | "toml" => Some("#"),
        _ => None,
    }
}

/// `comments.blockComment` of a language.
fn block_comment(language: &str) -> Option<(&'static str, &'static str)> {
    match language {
        "rs" | "ts" | "tsx" | "js" | "jsx" | "c" | "h" | "cpp" | "go" | "json" | "css" => Some(("/*", "*/")),
        "html" | "md" => Some(("<!--", "-->")),
        "py" => Some(("'''", "'''")),
        _ => None,
    }
}

/// ⌘/: comments the covered lines, or uncomments them when each already is
/// one. The token goes at the shallowest indentation; empty lines stay as
/// they are. A language with only block comments wraps each line in one.
pub fn toggle_line_comment(text: &str, anchor: usize, head: usize, language: &str) -> Option<Edit> {
    let mut l = Lines::new(text);
    let (first, last) = covered(&l, anchor, head);
    let (a, h) = (l.lc(anchor), l.lc(head));
    let (open, close): (&str, Option<&str>) = match (line_comment(language), block_comment(language)) {
        (Some(t), _) => (t, None),
        (None, Some((o, c))) => (o, Some(c)),
        _ => return None,
    };
    let body = |line: &[char]| -> usize { line.iter().take_while(|c| **c == ' ' || **c == '\t').count() };
    let filled: Vec<usize> = (first..=last).filter(|&i| l.lines[i].iter().any(|c| !c.is_whitespace())).collect();
    if filled.is_empty() {
        return None;
    }
    let starts = |line: &[char], t: &str| line[body(line)..].iter().collect::<String>().starts_with(t);
    let ends = |line: &[char], t: &str| line.iter().collect::<String>().trim_end().ends_with(t);
    let all = filled.iter().all(|&i| starts(&l.lines[i], open) && close.is_none_or(|c| ends(&l.lines[i], c)));
    let mut deltas = vec![(0usize, 0isize); l.lines.len()]; // (column the change is at, chars added)
    if all {
        for &i in &filled {
            let at = body(&l.lines[i]);
            let mut n = open.chars().count();
            if l.lines[i].get(at + n) == Some(&' ') {
                n += 1;
            }
            l.lines[i].drain(at..at + n);
            deltas[i] = (at, -(n as isize));
            if let Some(c) = close {
                let s: String = l.lines[i].iter().collect();
                let trimmed = s.trim_end();
                let keep = trimmed.strip_suffix(c).unwrap_or(trimmed);
                let keep = keep.strip_suffix(' ').unwrap_or(keep);
                l.lines[i] = keep.chars().collect();
            }
        }
    } else {
        let at = filled.iter().map(|&i| body(&l.lines[i])).min().unwrap_or(0);
        for &i in &filled {
            let token = format!("{open} ");
            l.lines[i].splice(at..at, token.chars());
            deltas[i] = (at, token.chars().count() as isize);
            if let Some(c) = close {
                l.lines[i].extend(format!(" {c}").chars());
            }
        }
    }
    let shift = |lc: (usize, usize)| -> usize {
        let (col, d) = deltas[lc.0];
        if d != 0 && lc.1 >= col { (lc.1 as isize + d).max(col as isize) as usize } else { lc.1 }
    };
    Some(Edit { text: l.text(), anchor: l.pos(a.0, shift(a)), head: l.pos(h.0, shift(h)) })
}

/// ⌥⇧A: wraps the selection (or, without one, the line's text) in the
/// language's block comment, or unwraps it.
pub fn toggle_block_comment(text: &str, anchor: usize, head: usize, language: &str) -> Option<Edit> {
    let (open, close) = block_comment(language)?;
    let (mut s, mut e) = (anchor.min(head), anchor.max(head));
    let mut ch = chars(text);
    if s == e {
        let start = line_start(&ch, s);
        let end = ch[start..].iter().position(|&c| c == '\n').map_or(ch.len(), |i| start + i);
        s = start + ch[start..end].iter().take_while(|c| c.is_whitespace()).count();
        e = end;
        if s >= e {
            return None;
        }
    }
    let inner: String = ch[s..e].iter().collect();
    let (o, c) = (open.chars().count(), close.chars().count());
    let trimmed = inner.trim();
    if trimmed.len() >= open.len() + close.len() && trimmed.starts_with(open) && trimmed.ends_with(close) {
        // Unwrap: drop the tokens and one space inside each.
        let lead = inner.chars().take_while(|c| c.is_whitespace()).count();
        let mut removed_open = o;
        if ch.get(s + lead + o) == Some(&' ') {
            removed_open += 1;
        }
        let trail = inner.chars().rev().take_while(|c| c.is_whitespace()).count();
        let close_at = e - trail - c;
        let mut removed_close = c;
        if close_at > 0 && ch.get(close_at - 1) == Some(&' ') {
            removed_close += 1;
            ch.drain(close_at - 1..close_at + c);
        } else {
            ch.drain(close_at..close_at + c);
        }
        ch.drain(s + lead..s + lead + removed_open);
        let new_e = e - removed_open - removed_close;
        return Some(edit(ch, s, new_e));
    }
    ch.splice(e..e, format!(" {close}").chars());
    ch.splice(s..s, format!("{open} ").chars());
    let new_e = e + o + 1 + c + 1;
    Some(edit(ch, s, new_e))
}

#[cfg(test)]
mod tests {
    #[test]
    fn duplicating_a_selection_or_a_line() {
        let e = super::duplicate_selection("ab cd", 3, 5);
        assert_eq!((e.text.as_str(), e.anchor, e.head), ("ab cdcd", 5, 7));
        let e = super::duplicate_selection("one\ntwo", 1, 1);
        assert_eq!((e.text.as_str(), e.head), ("one\none\ntwo", 5));
    }

    use super::*;

    fn at(text: &str) -> (String, usize) {
        // "ab|c" → ("abc", 2)
        let p = text.find('|').unwrap();
        (text.replace('|', ""), text[..p].chars().count())
    }

    fn show(e: &Edit) -> String {
        let mut ch: Vec<char> = e.text.chars().collect();
        if e.anchor == e.head {
            ch.insert(e.head, '|');
        } else {
            let (s, t) = (e.anchor.min(e.head), e.anchor.max(e.head));
            ch.insert(t, '>');
            ch.insert(s, '<');
        }
        ch.into_iter().collect()
    }

    fn typed(before: &str, c: char) -> Option<String> {
        let (t, p) = at(before);
        type_char(&t, p, p, c, true).map(|e| show(&e))
    }

    #[test]
    fn pairs_close_and_are_stepped_over() {
        assert_eq!(typed("f|", '(').as_deref(), Some("f(|)"));
        assert_eq!(typed("a = |", '[').as_deref(), Some("a = [|]"));
        assert_eq!(typed("x|)", ')').as_deref(), Some("x)|"));
        // Not before a word.
        assert_eq!(typed("|word", '('), None);
        // Quotes: not after a letter (an apostrophe), stepped over, doubled.
        assert_eq!(typed("don|", '\''), None);
        assert_eq!(typed("s = |", '"').as_deref(), Some("s = \"|\""));
        assert_eq!(typed("\"abc|\"", '"').as_deref(), Some("\"abc\"|"));
        // A closer with nothing to step over is just typed.
        assert_eq!(typed("a|", ')'), None);
    }

    #[test]
    fn a_selection_is_surrounded_and_stays_selected() {
        let e = type_char("hello world", 6, 11, '(', true).unwrap();
        assert_eq!(show(&e), "hello (<world>)");
        let q = type_char("x", 0, 1, '"', true).unwrap();
        assert_eq!(show(&q), "\"<x>\"");
    }

    #[test]
    fn enter_keeps_and_adds_indentation() {
        let (t, p) = at("    foo|");
        assert_eq!(show(&enter(&t, p, p, "    ", "ts")), "    foo\n    |");
        let (t, p) = at("if (x) {|");
        assert_eq!(show(&enter(&t, p, p, "    ", "ts")), "if (x) {\n    |");
        // Between a pair the closer goes on its own line.
        let (t, p) = at("fn a() {|}");
        assert_eq!(show(&enter(&t, p, p, "    ", "rs")), "fn a() {\n    |\n}");
        let (t, p) = at("  call(|)");
        assert_eq!(show(&enter(&t, p, p, "  ", "js")), "  call(\n    |\n  )");
        let (t, p) = at("if x:|");
        assert_eq!(show(&enter(&t, p, p, "    ", "py")), "if x:\n    |");
        assert_eq!(show(&enter(&t, p, p, "    ", "ts")), "if x:\n|");
    }

    #[test]
    fn backspace_removes_an_empty_pair_and_steps_over_indentation() {
        let (t, p) = at("f(|)");
        assert_eq!(show(&backspace(&t, p, p, 4).unwrap()), "f|");
        let (t, p) = at("\"|\"");
        assert_eq!(show(&backspace(&t, p, p, 4).unwrap()), "|");
        let (t, p) = at("a(|b)");
        assert_eq!(backspace(&t, p, p, 4), None);
        // Six spaces of indentation: back to column 4, then 0.
        let (t, p) = at("      |x");
        let e = backspace(&t, p, p, 4).unwrap();
        assert_eq!(show(&e), "    |x");
        let e = backspace(&e.text, e.head, e.head, 4).unwrap();
        assert_eq!(show(&e), "|x");
    }

    #[test]
    fn tab_goes_to_the_next_stop_and_indents_a_block() {
        let (t, p) = at("ab|");
        assert_eq!(show(&tab(&t, p, p, "    ", 4)), "ab  |");
        assert_eq!(show(&tab("a", 1, 1, "\t", 4)), "a\t|");
        let e = tab("one\ntwo\nthree", 0, 7, "    ", 4);
        assert_eq!(e.text, "    one\n    two\nthree");
        assert_eq!((e.anchor, e.head), (4, 15));
        let o = outdent_lines(&e.text, e.anchor, e.head, 4).unwrap();
        assert_eq!(o.text, "one\ntwo\nthree");
        assert_eq!((o.anchor, o.head), (0, 7));
        assert_eq!(outdent_lines("flat", 0, 0, 4), None);
    }

    #[test]
    fn lines_move_copy_delete_and_insert() {
        // Move down: b goes under c; the cursor follows its line.
        let (t, p) = at("a\nb|\nc");
        let e = move_lines(&t, p, p, false).unwrap();
        assert_eq!(show(&e), "a\nc\nb|");
        assert_eq!(move_lines("a\nb", 0, 0, true), None);
        assert_eq!(move_lines("a\nb", 2, 2, false), None);
        let e = move_lines("a\nb\nc", 2, 5, true).unwrap(); // lines b and c
        assert_eq!(e.text, "b\nc\na");
    }

    #[test]
    fn copying_and_deleting_lines() {
        let (t, p) = at("a\nb|\nc");
        assert_eq!(show(&copy_lines(&t, p, p, false)), "a\nb\nb|\nc");
        assert_eq!(show(&copy_lines(&t, p, p, true)), "a\nb|\nb\nc");
        assert_eq!(show(&delete_lines(&t, p, p)), "a\nc|");
        assert_eq!(delete_lines("only", 2, 2).text, "");
        let e = delete_lines("a\nb\nc", 2, 5); // b and c
        // A selection ending at a line start leaves that line out.
        assert_eq!(delete_lines("a\nb\nc", 2, 4).text, "a\nc");
        assert_eq!(e.text, "a");
    }

    #[test]
    fn inserting_lines_keeps_indentation() {
        let (t, p) = at("  if (x) {|");
        assert_eq!(show(&insert_line(&t, p, p, true, "  ")), "  if (x) {\n    |");
        let (t, p) = at("    x|");
        assert_eq!(show(&insert_line(&t, p, p, false, "    ")), "    |\n    x");
    }

    #[test]
    fn comments_toggle_per_language() {
        let (t, p) = at("a|\n    b\n");
        let on = toggle_line_comment(&t, 0, t.chars().count() - 1, "rs").unwrap();
        assert_eq!(on.text, "// a\n//     b\n");
        let off = toggle_line_comment(&on.text, 0, on.text.chars().count() - 1, "rs").unwrap();
        assert_eq!(off.text, t);
        let _ = p;
        // One indentation level for the token: the shallowest line decides.
        let both = toggle_line_comment("    a\n  b", 0, 9, "py").unwrap();
        assert_eq!(both.text, "  #   a\n  # b");
        // Empty lines are left alone.
        let gap = toggle_line_comment("a\n\nb", 0, 4, "ts").unwrap();
        assert_eq!(gap.text, "// a\n\n// b");
        // Block-only languages wrap each line.
        let css = toggle_line_comment("a { }", 0, 0, "css").unwrap();
        assert_eq!(css.text, "/* a { } */");
        assert_eq!(toggle_line_comment(&css.text, 0, 0, "css").unwrap().text, "a { }");
        // Plain text has none.
        assert_eq!(toggle_line_comment("x", 0, 0, ""), None);
    }

    #[test]
    fn block_comments_wrap_and_unwrap() {
        let on = toggle_block_comment("let a = 1;", 4, 9, "ts").unwrap();
        assert_eq!(on.text, "let /* a = 1 */;");
        let off = toggle_block_comment(&on.text, on.anchor, on.head, "ts").unwrap();
        assert_eq!(off.text, "let a = 1;");
        // No selection: the line's text.
        let line = toggle_block_comment("  hello", 3, 3, "css").unwrap();
        assert_eq!(line.text, "  /* hello */");
        assert_eq!(toggle_block_comment("x", 0, 1, "sh"), None);
    }
}
