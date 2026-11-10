//! Setting descriptions as text (settingsTree.ts `renderSettingMarkdown`):
//! paragraphs 8px apart, `[text](url)` links, `` `code` `` in the
//! preformat colours, `**bold**`, list items, and `#setting.key#`
//! references shown as "Category: Label" links that reveal the setting.

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId};

use crate::theme::Palette;

pub const LINE_HEIGHT: f32 = 18.0;
pub const PARAGRAPH_GAP: f32 = 8.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    Url(String),
    Setting(String),
}

/// One paragraph (or list item) laid out as a job, with its links as char
/// ranges of the job's text.
#[derive(Clone)]
pub struct Block {
    pub job: LayoutJob,
    pub links: Vec<(std::ops::Range<usize>, Link)>,
    /// Inline code spans (char ranges), painted as boxes behind the text.
    pub codes: Vec<std::ops::Range<usize>>,
    /// Left indent of a list item.
    pub indent: f32,
}

#[derive(Clone, Copy)]
pub struct Style {
    pub size: f32,
    pub color: Color32,
}

impl Default for Style {
    fn default() -> Self {
        Style { size: 13.0, color: Palette::FG().gamma_multiply(0.9) }
    }
}

fn fmt(font: FontId, color: Color32) -> TextFormat {
    TextFormat { font_id: font, color, line_height: Some(LINE_HEIGHT), valign: egui::Align::Center, ..Default::default() }
}

/// Splits a description into blocks. Plain descriptions keep their line
/// breaks; markdown ones join soft breaks.
pub fn blocks(text: &str, markdown: bool, style: Style) -> Vec<Block> {
    if !markdown {
        return vec![plain_block(text, style)];
    }
    let mut out = Vec::new();
    let mut para: Vec<&str> = Vec::new();
    let flush = |para: &mut Vec<&str>, out: &mut Vec<Block>| {
        if !para.is_empty() {
            out.push(inline_block(&para.join(" "), style, 0.0));
            para.clear();
        }
    };
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            flush(&mut para, &mut out);
        } else if let Some(item) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            flush(&mut para, &mut out);
            let mut b = inline_block(&format!("\u{2022} {item}"), style, 16.0);
            b.indent = 16.0;
            out.push(b);
        } else {
            para.push(t);
        }
    }
    flush(&mut para, &mut out);
    if out.is_empty() {
        out.push(plain_block("", style));
    }
    out
}

fn plain_block(text: &str, style: Style) -> Block {
    let mut job = LayoutJob::default();
    job.append(text, 0.0, fmt(FontId::proportional(style.size), style.color));
    Block { job, links: Vec::new(), codes: Vec::new(), indent: 0.0 }
}

/// A `#setting.key#` reference's text ("Files: Auto Save").
pub fn setting_ref_label(key: &str) -> String {
    match super::model::find(key) {
        Some(i) => {
            let (cat, label) = super::model::display_format(&super::model::settings()[i], "");
            if cat.is_empty() { label } else { format!("{cat}: {label}") }
        }
        None => key.to_string(),
    }
}

fn inline_block(text: &str, style: Style, _indent: f32) -> Block {
    let mut job = LayoutJob::default();
    let mut links = Vec::new();
    let mut codes = Vec::new();
    let normal = fmt(FontId::proportional(style.size), style.color);
    let bold = fmt(crate::ui_font::semibold(style.size), style.color);
    let link_fmt = fmt(FontId::proportional(style.size), Palette::TEXT_LINK_FG());
    // Reserves the code's width; the text itself is painted over its box
    // (egui can't align a smaller font on the baseline).
    let code = fmt(crate::ui_font::mono(11.0), Color32::TRANSPARENT);
    let mut chars = text.char_indices().peekable();
    let mut buf = String::new();
    let push = |job: &mut LayoutJob, buf: &mut String, f: &TextFormat| {
        if !buf.is_empty() {
            job.append(buf, 0.0, f.clone());
            buf.clear();
        }
    };
    let char_len = |job: &LayoutJob| job.text.chars().count();
    let rest = |i: usize| &text[i..];
    while let Some((i, c)) = chars.next() {
        match c {
            '`' => {
                if let Some(end) = rest(i + 1).find('`') {
                    push(&mut job, &mut buf, &normal);
                    let inner = &text[i + 1..i + 1 + end];
                    match inner.strip_prefix('#').and_then(|k| k.strip_suffix('#')).filter(|k| k.contains('.')) {
                        // `#files.autoSave#` in code is still a setting link.
                        Some(key) => {
                            let start = char_len(&job);
                            job.append(&setting_ref_label(key), 0.0, link_fmt.clone());
                            links.push((start..char_len(&job), Link::Setting(key.to_string())));
                        }
                        // 1px 3px padding and a 1px border around the text.
                        None => {
                            let start = char_len(&job);
                            job.append(inner, 4.0, code.clone());
                            codes.push(start..char_len(&job));
                            job.append("", 4.0, normal.clone());
                        }
                    }
                    while chars.peek().is_some_and(|(j, _)| *j <= i + 1 + end) {
                        chars.next();
                    }
                    continue;
                }
                buf.push(c);
            }
            '*' if rest(i).starts_with("**") => {
                if let Some(end) = rest(i + 2).find("**") {
                    push(&mut job, &mut buf, &normal);
                    job.append(&text[i + 2..i + 2 + end], 0.0, bold.clone());
                    while chars.peek().is_some_and(|(j, _)| *j < i + 4 + end) {
                        chars.next();
                    }
                    continue;
                }
                buf.push(c);
            }
            '[' => {
                // [label](target)
                let parsed = rest(i + 1).find("](").and_then(|close| {
                    let after = i + 1 + close + 2;
                    rest(after).find(')').map(|end| (close, after, end))
                });
                match parsed {
                    Some((close, after, end)) => {
                        push(&mut job, &mut buf, &normal);
                        let label = text[i + 1..i + 1 + close].replace('`', "");
                        let target = &text[after..after + end];
                        let link = match target.strip_prefix('#').and_then(|t| t.strip_suffix('#')) {
                            Some(key) => Link::Setting(key.to_string()),
                            None => Link::Url(target.to_string()),
                        };
                        let start = char_len(&job);
                        job.append(&label, 0.0, link_fmt.clone());
                        links.push((start..char_len(&job), link));
                        while chars.peek().is_some_and(|(j, _)| *j < after + end + 1) {
                            chars.next();
                        }
                        continue;
                    }
                    None => buf.push(c),
                }
            }
            '#' => {
                // `#editor.tabSize#`: a reference to another setting.
                let end = rest(i + 1).find('#');
                let key = end.map(|e| &text[i + 1..i + 1 + e]).filter(|k| {
                    !k.is_empty() && k.contains('.') && k.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))
                });
                match (key, end) {
                    (Some(key), Some(e)) => {
                        push(&mut job, &mut buf, &normal);
                        let start = char_len(&job);
                        job.append(&setting_ref_label(key), 0.0, link_fmt.clone());
                        links.push((start..char_len(&job), Link::Setting(key.to_string())));
                        while chars.peek().is_some_and(|(j, _)| *j <= i + 1 + e) {
                            chars.next();
                        }
                        continue;
                    }
                    _ => buf.push(c),
                }
            }
            '\\' if chars.peek().is_some_and(|(_, n)| n.is_ascii_punctuation()) => {
                if let Some((_, n)) = chars.next() {
                    buf.push(n);
                }
            }
            _ => buf.push(c),
        }
    }
    push(&mut job, &mut buf, &normal);
    Block { job, links, codes, indent: 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_code_and_setting_refs() {
        let b = blocks(
            "Format a file on save. When #files.autoSave# is set to `afterDelay`, see [docs](https://x.y).",
            true,
            Style { size: 13.0, color: Color32::WHITE },
        );
        assert_eq!(b.len(), 1);
        let text = &b[0].job.text;
        assert!(text.contains("Files: Auto Save"), "{text}");
        assert!(text.contains("afterDelay") && !text.contains('`'));
        assert_eq!(b[0].links.len(), 2);
        assert_eq!(b[0].links[0].1, Link::Setting("files.autoSave".into()));
        assert_eq!(b[0].links[1].1, Link::Url("https://x.y".into()));
        let chars: Vec<char> = text.chars().collect();
        let r = b[0].links[1].0.clone();
        assert_eq!(chars[r].iter().collect::<String>(), "docs");
    }

    #[test]
    fn paragraphs_and_lists() {
        let b = blocks("First line\ncontinues.\n\n- one\n- two", true, Style { size: 13.0, color: Color32::WHITE });
        assert_eq!(b.len(), 3);
        assert_eq!(b[0].job.text, "First line continues.");
        assert_eq!(b[1].indent, 16.0);
    }
}
