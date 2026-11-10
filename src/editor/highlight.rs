//! Syntax highlighting using syntect with the active colour theme's
//! `tokenColors` (theme JSON plus its `include` chain).
//!
//! After syntect produces base token colors, we run a post-processing pass that
//! recolors matched bracket pairs in rotation — mirroring VS Code's
//! `editor.bracketPairColorization` (defaults from `editorColorRegistry.ts`).

use egui::text::LayoutJob;
use egui::{Color32, FontId, TextFormat};

// Mirror the editor metrics (VS Code macOS defaults: Menlo 12 / line-height 18).
const EDITOR_FONT_SIZE: f32 = 12.0;
const EDITOR_LINE_HEIGHT: f32 = EDITOR_FONT_SIZE * 1.5;
use once_cell::sync::Lazy;
use syntect::easy::HighlightLines;
use crate::theme::Palette;
use std::sync::{Arc, Mutex};
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);

/// The active theme's TextMate rules as a syntect theme, plus the colours
/// the bracket pass needs. Rebuilt when the colour theme changes.
struct SyntaxTheme {
    theme: Theme,
    /// Foregrounds of string and comment scopes: brackets inside them keep
    /// the token colour.
    plain_colors: Vec<syntect::highlighting::Color>,
    /// `editorBracketHighlight.foreground1-6` that aren't transparent.
    brackets: Vec<Color32>,
    unexpected_bracket: Color32,
}

static SYNTAX_THEME: Lazy<Mutex<Option<(u32, Arc<SyntaxTheme>)>>> = Lazy::new(|| Mutex::new(None));

fn syntax_theme() -> Arc<SyntaxTheme> {
    let generation = crate::theme::generation();
    let mut cache = SYNTAX_THEME.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((g, t)) = cache.as_ref() {
        if *g == generation && generation != 0 {
            return t.clone();
        }
    }
    let t = Arc::new(build_syntax_theme());
    *cache = Some((crate::theme::generation(), t.clone()));
    t
}

fn syntect_color(c: Color32) -> syntect::highlighting::Color {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    syntect::highlighting::Color { r, g, b, a }
}

fn build_syntax_theme() -> SyntaxTheme {
    use std::str::FromStr;
    use syntect::highlighting::{FontStyle, Highlighter, ScopeSelectors, StyleModifier, ThemeItem, ThemeSettings};

    let colors = crate::theme::current();
    let to_syntect = |c: &crate::theme::color::Color| syntect::highlighting::Color { r: c.r, g: c.g, b: c.b, a: (c.a * 255.0).round() as u8 };
    let mut theme = Theme {
        settings: ThemeSettings {
            foreground: Some(syntect_color(Palette::EDITOR_FG())),
            background: Some(syntect_color(Palette::EDITOR_BG())),
            ..Default::default()
        },
        ..Default::default()
    };
    for rule in &colors.token_rules {
        if rule.scopes.is_empty() {
            continue;
        }
        let Ok(scope) = ScopeSelectors::from_str(&rule.scopes.join(", ")) else { continue };
        let font_style = rule.font_style.as_deref().map(|s| {
            let mut f = FontStyle::empty();
            for part in s.split_whitespace() {
                match part {
                    "bold" => f |= FontStyle::BOLD,
                    "italic" => f |= FontStyle::ITALIC,
                    "underline" => f |= FontStyle::UNDERLINE,
                    _ => {}
                }
            }
            f
        });
        theme.scopes.push(ThemeItem {
            scope,
            style: StyleModifier {
                foreground: rule.foreground.as_ref().map(to_syntect),
                background: rule.background.as_ref().map(to_syntect),
                font_style,
            },
        });
    }
    // syntect's (Sublime) grammars name some tokens differently from VS
    // Code's TextMate grammars: give those scopes the colour the theme sets
    // for VS Code's name.
    const BRIDGES: &[(&str, &str)] = &[
        ("meta.structure.dictionary.key.json string.quoted.double.json", "source.json support.type.property-name.json"),
        ("meta.mapping.key string.quoted.double, meta.mapping.key.yaml", "source.yaml entity.name.tag.yaml"),
    ];
    let mut bridged = Vec::new();
    {
        let highlighter = Highlighter::new(&theme);
        for (syntect_scope, vscode_scope) in BRIDGES {
            let stack: Vec<syntect::parsing::Scope> =
                vscode_scope.split(' ').filter_map(|s| syntect::parsing::Scope::new(s).ok()).collect();
            let style = highlighter.style_for_stack(&stack);
            if let Ok(scope) = ScopeSelectors::from_str(syntect_scope) {
                bridged.push(ThemeItem {
                    scope,
                    style: StyleModifier { foreground: Some(style.foreground), background: None, font_style: None },
                });
            }
        }
    }
    theme.scopes.extend(bridged);
    let highlighter = Highlighter::new(&theme);
    let mut plain_colors = Vec::new();
    for scope in [
        "string.quoted.double",
        "string.quoted.single",
        "string.template",
        "string.regexp",
        "comment.line",
        "comment.block",
        "comment.block.documentation",
    ] {
        let stack: Vec<syntect::parsing::Scope> =
            scope.split('.').scan(String::new(), |acc, part| {
                if !acc.is_empty() {
                    acc.push('.');
                }
                acc.push_str(part);
                Some(acc.clone())
            })
            .last()
            .and_then(|s| syntect::parsing::Scope::new(&s).ok())
            .into_iter()
            .collect();
        let fg = highlighter.style_for_stack(&stack).foreground;
        if !plain_colors.contains(&fg) {
            plain_colors.push(fg);
        }
    }
    let brackets = [
        Palette::BRACKET_1(),
        Palette::BRACKET_2(),
        Palette::BRACKET_3(),
        Palette::BRACKET_4(),
        Palette::BRACKET_5(),
        Palette::BRACKET_6(),
    ]
    .into_iter()
    .filter(|c| c.a() > 0)
    .collect();
    SyntaxTheme { theme, plain_colors, brackets, unexpected_bracket: Palette::BRACKET_UNEXPECTED() }
}

/// Find-match decoration for `build_layout_job`: byte ranges of every match in
/// the document plus the index of the current one. The current match gets the
/// brighter `editor.findMatchBackground`; the rest get the dimmer
/// `findMatchHighlightBackground` — exactly VS Code's split.
#[derive(Clone, Default)]
pub struct FindHighlight {
    /// Sorted, non-overlapping byte ranges `(start, end)` into the text.
    pub ranges: Vec<(usize, usize)>,
    /// Index into `ranges` of the current match.
    pub current: usize,
    /// Extension decorations (`TextEditor.setDecorations`), any order.
    pub decorations: Vec<DecorationSpan>,
}

/// A decorated byte range: background and/or text colour. Find matches
/// paint over decoration backgrounds, like VS Code's layering.
#[derive(Clone, Debug, PartialEq)]
pub struct DecorationSpan {
    pub start: usize,
    pub end: usize,
    pub background: Option<Color32>,
    pub color: Option<Color32>,
    pub underline: Option<Color32>,
}

pub fn build_layout_job(
    _ctx: &egui::Context,
    _style: &egui::Style,
    text: &str,
    language: &str,
    wrap_width: f32,
    find: Option<&FindHighlight>,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;

    let syntax = pick_syntax(language);
    let st = syntax_theme();
    let mut highlighter = HighlightLines::new(syntax, &st.theme);
    let font = crate::icons::editor_mono_font(EDITOR_FONT_SIZE);
    let mut w = JobWriter { job: &mut job, pos: 0, find };

    // Track nesting state across the whole document so brackets on different
    // lines still get matched.
    let mut bracket_stack: Vec<char> = Vec::new();

    for line in LinesWithEndings::from(text) {
        let regions = match highlighter.highlight_line(line, &SYNTAX_SET) {
            Ok(r) => r,
            Err(_) => {
                w.push(line, &font, default_fg());
                continue;
            }
        };

        for (style, snippet) in regions {
            // syntect's HighlightLines only hands out styles, so strings and
            // comments are recognised by the theme's colours for those scopes;
            // brackets inside them keep the token colour.
            if st.plain_colors.contains(&style.foreground) || st.brackets.is_empty() {
                w.push(
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
                    flush(&mut w, &mut buf, &font, base_color);
                    let level = bracket_stack.len();
                    bracket_stack.push(ch);
                    let color = st.brackets[level % st.brackets.len()];
                    w.push(&ch.to_string(), &font, syntect_rgb(color));
                } else if is_close_bracket(ch) {
                    flush(&mut w, &mut buf, &font, base_color);
                    let matched = matches!(
                        (bracket_stack.last().copied(), ch),
                        (Some('('), ')') | (Some('['), ']') | (Some('{'), '}')
                    );
                    let color = if matched {
                        let level = bracket_stack.len() - 1;
                        bracket_stack.pop();
                        st.brackets[level % st.brackets.len()]
                    } else {
                        st.unexpected_bracket
                    };
                    w.push(&ch.to_string(), &font, syntect_rgb(color));
                } else {
                    buf.push(ch);
                }
            }
            flush(&mut w, &mut buf, &font, base_color);
        }
    }
    job
}

/// Per-line syntax-coloured runs `(text, color)`, for the minimap. Lighter
/// than `build_layout_job` (no bracket colourisation), but uses the same
/// syntect theme so the miniature matches the editor's palette.
pub fn line_runs(text: &str, language: &str) -> Vec<Vec<(String, Color32)>> {
    let syntax = pick_syntax(language);
    let st = syntax_theme();
    let mut hl = HighlightLines::new(syntax, &st.theme);
    let mut out = Vec::new();
    for line in LinesWithEndings::from(text) {
        let mut runs = Vec::new();
        match hl.highlight_line(line, &SYNTAX_SET) {
            Ok(regions) => {
                for (style, snippet) in regions {
                    let s = snippet.trim_end_matches('\n');
                    if s.is_empty() {
                        continue;
                    }
                    runs.push((
                        s.to_string(),
                        Color32::from_rgb(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        ),
                    ));
                }
            }
            Err(_) => runs.push((line.trim_end().to_string(), default_fg())),
        }
        out.push(runs);
    }
    out
}

/// Appends runs to the LayoutJob while tracking the byte offset into the
/// source text, splitting each run at find-match boundaries so matched spans
/// get a background wash (`editor.findMatch*Background`).
struct JobWriter<'a> {
    job: &'a mut LayoutJob,
    pos: usize,
    find: Option<&'a FindHighlight>,
}

impl JobWriter<'_> {
    fn push(&mut self, text: &str, font: &FontId, color: Color32) {
        if text.is_empty() {
            return;
        }
        let start = self.pos;
        let end = start + text.len();
        self.pos = end;

        let Some(f) = self.find else {
            self.append(text, font, color, None, None);
            return;
        };
        // Cut points from every find match and decoration touching this run.
        let mut cuts: Vec<usize> = vec![start, end];
        for &(s, e) in &f.ranges {
            if e > start && s < end {
                cuts.extend([s.max(start), e.min(end)]);
            }
        }
        for d in &f.decorations {
            if d.end > start && d.start < end {
                cuts.extend([d.start.max(start), d.end.min(end)]);
            }
        }
        if cuts.len() == 2 {
            self.append(text, font, color, None, None);
            return;
        }
        cuts.sort_unstable();
        cuts.dedup();
        for w in cuts.windows(2) {
            let (a, b) = (w[0], w[1]);
            // Stay on character boundaries (ranges come from other code).
            if a >= b || !text.is_char_boundary(a - start) || !text.is_char_boundary(b - start) {
                continue;
            }
            let mut bg = None;
            let mut fg = color;
            let mut underline = None;
            for d in &f.decorations {
                if d.start <= a && b <= d.end {
                    bg = d.background.or(bg);
                    fg = d.color.unwrap_or(fg);
                    underline = d.underline.or(underline);
                }
            }
            for (i, &(s, e)) in f.ranges.iter().enumerate() {
                if s <= a && b <= e {
                    bg = Some(if i == f.current {
                        crate::theme::Palette::FIND_MATCH_BG()
                    } else {
                        crate::theme::Palette::FIND_MATCH_HL_BG()
                    });
                }
            }
            self.append(&text[a - start..b - start], font, fg, bg, underline);
        }
    }

    fn append(
        &mut self,
        text: &str,
        font: &FontId,
        color: Color32,
        bg: Option<Color32>,
        underline: Option<Color32>,
    ) {
        if text.is_empty() {
            return;
        }
        self.job.append(
            text,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color,
                background: bg.unwrap_or(Color32::TRANSPARENT),
                underline: underline.map(|c| egui::Stroke::new(1.0, c)).unwrap_or_default(),
                line_height: Some(EDITOR_LINE_HEIGHT),
                ..Default::default()
            },
        );
    }
}

fn flush(
    w: &mut JobWriter<'_>,
    buf: &mut String,
    font: &FontId,
    color: Color32,
) {
    if buf.is_empty() {
        return;
    }
    w.push(buf, font, color);
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

fn pick_syntax(language: &str) -> &'static syntect::parsing::SyntaxReference {
    // syntect's default syntax set has no TypeScript; fall back to JavaScript
    // (covers comments / keywords / strings / templates well enough for TS too).
    let candidates: &[&str] = match language {
        "rs" => &["rs"],
        "ts" | "tsx" => &["ts", "tsx", "js"],
        "jsx" | "js" => &["jsx", "js"],
        "py" => &["py"],
        "go" => &["go"],
        "c" => &["c"],
        "cpp" | "cc" | "cxx" => &["cpp"],
        "h" | "hpp" => &["h", "cpp"],
        "json" => &["json"],
        "md" | "markdown" => &["md"],
        "html" => &["html"],
        "css" => &["css"],
        "toml" => &["toml"],
        "yaml" | "yml" => &["yaml"],
        "sh" | "bash" | "zsh" => &["sh"],
        other => {
            return SYNTAX_SET
                .find_syntax_by_token(other)
                .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());
        }
    };
    for token in candidates {
        if let Some(s) = SYNTAX_SET.find_syntax_by_token(token) {
            return s;
        }
    }
    SYNTAX_SET.find_syntax_plain_text()
}

fn default_fg() -> Color32 {
    Palette::EDITOR_FG()
}
