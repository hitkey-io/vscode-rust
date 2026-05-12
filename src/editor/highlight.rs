//! Syntax highlighting using syntect with the VS Code 1.119+ "2026 Dark" tmTheme
//! generated from `vscode-original/extensions/theme-defaults/themes/2026-dark.json`
//! (+ its `include` chain).
//!
//! After syntect produces base token colors, we run a post-processing pass that
//! recolors matched bracket pairs in rotation — mirroring VS Code's
//! `editor.bracketPairColorization` (defaults from `editorColorRegistry.ts`).

use egui::text::LayoutJob;
use egui::{Color32, FontId, TextFormat};
use once_cell::sync::Lazy;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);

static DARK_PLUS: Lazy<Theme> = Lazy::new(|| {
    let mut cursor = std::io::Cursor::new(include_bytes!("../../assets/dark_2026.tmTheme"));
    ThemeSet::load_from_reader(&mut cursor)
        .expect("failed to load embedded 2026 Dark tmTheme")
});

// editorBracketHighlight.foreground{1,2,3} dark defaults from VS Code's
// editorColorRegistry.ts. levels 4–6 cycle the same three colors.
const BRACKET_COLORS: [Color32; 3] = [
    Color32::from_rgb(0xFF, 0xD7, 0x00), // gold
    Color32::from_rgb(0xDA, 0x70, 0xD6), // orchid
    Color32::from_rgb(0x17, 0x9F, 0xFF), // bright blue
];

const UNMATCHED_BRACKET: Color32 = Color32::from_rgb(0xFF, 0x12, 0x12);

pub fn build_layout_job(
    _ctx: &egui::Context,
    _style: &egui::Style,
    text: &str,
    language: &str,
    wrap_width: f32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;

    let syntax = pick_syntax(language);
    let mut highlighter = HighlightLines::new(syntax, &DARK_PLUS);
    let font = FontId::monospace(13.5);

    // Track nesting state across the whole document so brackets on different
    // lines still get matched.
    let mut bracket_stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut in_comment = false;

    for line in LinesWithEndings::from(text) {
        let regions = match highlighter.highlight_line(line, &SYNTAX_SET) {
            Ok(r) => r,
            Err(_) => {
                job.append(line, 0.0, TextFormat::simple(font.clone(), default_fg()));
                continue;
            }
        };

        for (style, snippet) in regions {
            // Detect comment/string scopes via foreground color heuristic: syntect
            // strings/comments use distinct theme colors that fall into known ranges.
            // A cleaner approach is scope-tag inspection, but syntect's HighlightLines
            // only hands us colors. Approximation: skip bracket coloring when the
            // current style's foreground matches the theme's string/comment palette.
            let snippet_in_string = is_string_color(style.foreground);
            let snippet_in_comment = is_comment_color(style.foreground);
            in_string = snippet_in_string;
            in_comment = snippet_in_comment;

            if snippet_in_string || snippet_in_comment {
                push_plain(
                    &mut job,
                    snippet,
                    &font,
                    Color32::from_rgb(style.foreground.r, style.foreground.g, style.foreground.b),
                );
                continue;
            }

            // Scan snippet char-by-char so each bracket gets its own color.
            let mut buf = String::new();
            let base_color = Color32::from_rgb(
                style.foreground.r,
                style.foreground.g,
                style.foreground.b,
            );
            for ch in snippet.chars() {
                if is_open_bracket(ch) {
                    flush(&mut job, &mut buf, &font, base_color);
                    let level = bracket_stack.len();
                    bracket_stack.push(ch);
                    let color = BRACKET_COLORS[level % BRACKET_COLORS.len()];
                    push_plain(&mut job, &ch.to_string(), &font, syntect_rgb(color));
                } else if is_close_bracket(ch) {
                    flush(&mut job, &mut buf, &font, base_color);
                    let matched = matches!(
                        (bracket_stack.last().copied(), ch),
                        (Some('('), ')') | (Some('['), ']') | (Some('{'), '}')
                    );
                    let color = if matched {
                        let level = bracket_stack.len() - 1;
                        bracket_stack.pop();
                        BRACKET_COLORS[level % BRACKET_COLORS.len()]
                    } else {
                        UNMATCHED_BRACKET
                    };
                    push_plain(&mut job, &ch.to_string(), &font, syntect_rgb(color));
                } else {
                    buf.push(ch);
                }
            }
            flush(&mut job, &mut buf, &font, base_color);
        }
    }
    let _ = (in_string, in_comment);
    job
}

fn push_plain(job: &mut LayoutJob, text: &str, font: &FontId, color: Color32) {
    if text.is_empty() {
        return;
    }
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: font.clone(),
            color,
            ..Default::default()
        },
    );
}

fn flush(
    job: &mut LayoutJob,
    buf: &mut String,
    font: &FontId,
    color: Color32,
) {
    if buf.is_empty() {
        return;
    }
    push_plain(job, buf, font, color);
    buf.clear();
}

fn syntect_rgb(c: Color32) -> Color32 {
    c
}

fn is_open_bracket(c: char) -> bool {
    matches!(c, '(' | '[' | '{')
}

fn is_close_bracket(c: char) -> bool {
    matches!(c, ')' | ']' | '}')
}

fn is_string_color(c: syntect::highlighting::Color) -> bool {
    // 2026 Dark string foreground = #a5d6ff and #ffa657 (in different scopes).
    let hex = (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32;
    matches!(hex, 0xa5d6ff | 0xffa657 | 0xce9178 | 0xd16969)
}

fn is_comment_color(c: syntect::highlighting::Color) -> bool {
    // 2026 Dark / dark_modern comment foreground = #8b949e or #6A9955.
    let hex = (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32;
    matches!(hex, 0x8b949e | 0x6A9955 | 0x6a9955)
}

fn pick_syntax(language: &str) -> &'static syntect::parsing::SyntaxReference {
    let token = match language {
        "rs" => "rs",
        "ts" | "tsx" => "ts",
        "jsx" | "js" => "js",
        "py" => "py",
        "go" => "go",
        "c" => "c",
        "cpp" | "cc" | "cxx" => "cpp",
        "h" | "hpp" => "h",
        "json" => "json",
        "md" | "markdown" => "md",
        "html" => "html",
        "css" => "css",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "sh" | "bash" | "zsh" => "sh",
        other => other,
    };
    SYNTAX_SET
        .find_syntax_by_token(token)
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text())
}

fn default_fg() -> Color32 {
    Color32::from_rgb(0xBB, 0xBE, 0xBF)
}
