//! What the editor marks around the selection by default: the other
//! occurrences of the selected text (`editor.selectionHighlight`) and the
//! bracket matching the one at the caret (`editor.matchBrackets: always`).
//! Pure functions of the text and the caret, in the units the editor needs
//! (byte ranges for the layout decorations, char indexes for the brackets).

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The other places the selected text occurs (byte ranges, the selection
/// itself left out). A selection that is a whole word matches whole words
/// only; anything else matches as plain text. Nothing for an empty or
/// multi-line selection, or one longer than 200 characters.
pub fn selection_occurrences(text: &str, anchor: usize, head: usize) -> Vec<(usize, usize)> {
    let (s, e) = (anchor.min(head), anchor.max(head));
    if s == e || e - s > 200 {
        return Vec::new();
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if e > chars.len() {
        return Vec::new();
    }
    let byte = |i: usize| chars.get(i).map_or(text.len(), |(b, _)| *b);
    let (bs, be) = (byte(s), byte(e));
    let selected = &text[bs..be];
    if selected.contains('\n') || selected.trim().is_empty() {
        return Vec::new();
    }
    let whole_word = selected.chars().all(is_word)
        && !chars.get(s.wrapping_sub(1)).is_some_and(|(_, c)| is_word(*c))
        && !chars.get(e).is_some_and(|(_, c)| is_word(*c));
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find(selected) {
        let (a, b) = (from + found, from + found + selected.len());
        from = a + selected.chars().next().map_or(1, char::len_utf8);
        if a == bs {
            continue;
        }
        if whole_word {
            let before = text[..a].chars().next_back().is_some_and(is_word);
            let after = text[b..].chars().next().is_some_and(is_word);
            if before || after {
                continue;
            }
        }
        out.push((a, b));
        if out.len() >= 2000 {
            break;
        }
    }
    out
}

fn partner(c: char) -> Option<(char, bool)> {
    // (the other bracket, whether `c` opens)
    match c {
        '(' => Some((')', true)),
        '[' => Some((']', true)),
        '{' => Some(('}', true)),
        ')' => Some(('(', false)),
        ']' => Some(('[', false)),
        '}' => Some(('{', false)),
        _ => None,
    }
}

/// The two bracket characters (char indexes) of the pair at the caret: the
/// bracket after it, else the one before it, with its match. Brackets in
/// strings and comments aren't told apart.
pub fn bracket_pair(text: &str, caret: usize) -> Option<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let at = |i: usize| chars.get(i).copied();
    let candidates = [Some(caret), caret.checked_sub(1)];
    for i in candidates.into_iter().flatten() {
        let Some(c) = at(i) else { continue };
        let Some((other, opens)) = partner(c) else { continue };
        let mut depth = 0usize;
        if opens {
            for (j, &d) in chars.iter().enumerate().skip(i + 1) {
                if d == c {
                    depth += 1;
                } else if d == other {
                    if depth == 0 {
                        return Some((i, j));
                    }
                    depth -= 1;
                }
            }
        } else {
            for j in (0..i).rev() {
                let d = chars[j];
                if d == c {
                    depth += 1;
                } else if d == other {
                    if depth == 0 {
                        return Some((j, i));
                    }
                    depth -= 1;
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occurrences_of_the_selection() {
        let text = "let foo = foo + food; foo";
        // "foo" at 4..7 is a whole word: the other two whole words, not "food".
        assert_eq!(selection_occurrences(text, 4, 7), vec![(10, 13), (22, 25)]);
        // A fragment ("oo") matches anywhere, "food" included.
        assert_eq!(selection_occurrences(text, 5, 7).len(), 4 - 1);
        // Nothing for no selection, whitespace or a selection over a line.
        assert!(selection_occurrences(text, 4, 4).is_empty());
        assert!(selection_occurrences("a b\nc", 1, 2).is_empty());
        assert!(selection_occurrences("a\nb a\nb", 0, 3).is_empty());
    }

    #[test]
    fn brackets_pair_up_across_nesting() {
        let text = "f(a, [b], {c})";
        assert_eq!(bracket_pair(text, 1), Some((1, 13))); // before "(" → the pair
        assert_eq!(bracket_pair(text, 14), Some((1, 13))); // after ")"
        assert_eq!(bracket_pair(text, 5), Some((5, 7))); // before "["
        assert_eq!(bracket_pair(text, 3), None); // in between, away from brackets
        assert_eq!(bracket_pair("(()", 0), None); // unbalanced
    }
}
