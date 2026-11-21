//! The references peek (⇧F12, `editor.action.referenceSearch.trigger`),
//! ported from `contrib/gotoSymbol/browser/peek/referencesWidget.ts` /
//! `referencesTree.ts` / `referencesWidget.css`, `contrib/peekView/browser/
//! peekView.ts` + `peekViewWidget.css` and `contrib/zoneWidget`.
//!
//! VS Code inserts it as a view zone under the line (18 lines tall, the
//! lines below move down); here it is drawn over the lines below instead.
//! Inside: an arrow (a third of a line) pointing at the reference, a 22px
//! head (`peekViewTitle.*`: file name, its folder and "References (n)",
//! Close), then the preview editor (70%) and the results tree (30%,
//! 23px rows).
//!
//! Tokens: `peekView.border`, `peekViewTitle.background`,
//! `peekViewTitleLabel.foreground`, `peekViewTitleDescription.foreground`,
//! `peekViewResult.{background, lineForeground, fileForeground,
//! selectionBackground, selectionForeground, matchHighlightBackground}`,
//! `peekViewEditor.{background, matchHighlightBackground,
//! matchHighlightBorder}`, `peekViewEditorGutter.background`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use egui::{Align2, Color32, Context, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::theme::Palette;

const LINE_H: f32 = 18.0;
const HEIGHT_IN_LINES: f32 = 18.0;
const HEAD_H: f32 = 22.0; // ceil(lineHeight × 1.2)
const ROW_H: f32 = 23.0;
const RATIO: f32 = 0.7;

/// One reference: its line and char columns (the end clipped to the line).
#[derive(Clone, Debug, PartialEq)]
pub struct PeekRef {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

/// A file with references, its text and colours.
pub struct PeekFile {
    pub path: PathBuf,
    pub lines: Vec<String>,
    runs: Vec<Vec<(String, Color32)>>,
    pub refs: Vec<PeekRef>,
}

/// The focused row of the results tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sel {
    File(usize),
    Ref(usize, usize),
}

/// What the peek asks of the editor.
#[derive(Clone, Debug, PartialEq)]
pub enum PeekOutput {
    None,
    Close,
    /// Go to the reference (0-based line, char column) and close.
    Open { path: PathBuf, line: usize, col: usize },
}

pub struct ReferencesPeek {
    pub files: Vec<PeekFile>,
    pub selected: Sel,
    pub collapsed: HashSet<usize>,
    /// The file the peek was opened in, and the caret's rect then.
    pub origin: PathBuf,
    pub anchor: Rect,
    pub tree_focused: bool,
    tree_scroll: f32,
    preview_top: f32,
    reveal: bool,
    root: Option<PathBuf>,
}

/// `getWordUntilPosition` + `FilePreview.preview`: a few characters before
/// the reference (from the start of the word there, leading blanks trimmed),
/// the reference, and the rest of the line (trailing blanks trimmed); with
/// the reference's char range in the result.
pub fn preview(line: &str, start: usize, end: usize) -> (String, (usize, usize)) {
    let chars: Vec<char> = line.chars().collect();
    let (start, end) = (start.min(chars.len()), end.min(chars.len()).max(start.min(chars.len())));
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = start.saturating_sub(8);
    while from > 0 && is_word(chars[from - 1]) && chars.get(from).is_some_and(|c| is_word(*c)) {
        from -= 1;
    }
    let before: String = chars[from..start].iter().collect::<String>().trim_start().to_string();
    let inside: String = chars[start..end].iter().collect();
    let after: String = chars[end..].iter().collect::<String>().trim_end().to_string();
    let b = before.chars().count();
    (format!("{before}{inside}{after}"), (b, b + inside.chars().count()))
}

impl ReferencesPeek {
    /// Builds the model (`ReferencesModel`): locations sorted by path then
    /// position, duplicates dropped, grouped per file; the reference nearest
    /// to `origin` (same path prefix, then line × 100 + column) selected.
    /// `text_of` gives a file's text (open editor or disk).
    pub fn new(
        locations: &[(PathBuf, [usize; 4])],
        text_of: impl Fn(&Path) -> Option<String>,
        origin: (&Path, usize, usize),
        anchor: Rect,
        root: Option<PathBuf>,
    ) -> Option<Self> {
        let mut locs: Vec<(PathBuf, [usize; 4])> = locations.to_vec();
        locs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        locs.dedup();
        let mut files: Vec<PeekFile> = Vec::new();
        for (path, r) in locs {
            if files.last().is_none_or(|f| f.path != path) {
                let Some(text) = text_of(&path) else { continue };
                let language = crate::editor::buffer::detect_language(&path);
                files.push(PeekFile {
                    lines: text.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect(),
                    runs: crate::editor::highlight::line_runs(&text, language),
                    path: path.clone(),
                    refs: Vec::new(),
                });
            }
            let f = files.last_mut().unwrap();
            let Some(line) = f.lines.get(r[0]) else { continue };
            let col = |ch16: usize| utf16_to_char(line, ch16);
            let start = col(r[1]);
            let end = if r[2] == r[0] { col(r[3]) } else { line.chars().count() };
            let item = PeekRef { line: r[0], start, end: end.max(start) };
            if f.refs.last() != Some(&item) {
                f.refs.push(item);
            }
        }
        files.retain(|f| !f.refs.is_empty());
        if files.is_empty() {
            return None;
        }
        let origin_s = origin.0.display().to_string();
        let mut best: Option<((usize, usize), (usize, usize))> = None;
        for (fi, f) in files.iter().enumerate() {
            let prefix = f.path.display().to_string().chars().zip(origin_s.chars()).take_while(|(a, b)| a == b).count();
            for (ri, r) in f.refs.iter().enumerate() {
                let dist = r.line.abs_diff(origin.1) * 100 + r.start.abs_diff(origin.2);
                let key = (usize::MAX - prefix, dist);
                if best.is_none_or(|(k, _)| key < k) {
                    best = Some((key, (fi, ri)));
                }
            }
        }
        let (fi, ri) = best.map(|(_, s)| s).unwrap_or((0, 0));
        Some(Self {
            files,
            selected: Sel::Ref(fi, ri),
            collapsed: HashSet::new(),
            origin: origin.0.to_path_buf(),
            anchor,
            tree_focused: true,
            tree_scroll: 0.0,
            preview_top: 0.0,
            reveal: true,
            root,
        })
    }

    pub fn count(&self) -> usize {
        self.files.iter().map(|f| f.refs.len()).sum()
    }

    /// The tree's rows in order (a folded file hides its references).
    pub fn rows(&self) -> Vec<Sel> {
        let mut out = Vec::new();
        for (fi, f) in self.files.iter().enumerate() {
            out.push(Sel::File(fi));
            if !self.collapsed.contains(&fi) {
                out.extend((0..f.refs.len()).map(|ri| Sel::Ref(fi, ri)));
            }
        }
        out
    }

    /// The reference the preview shows: the selected one, or a selected
    /// file's first.
    fn shown(&self) -> (usize, usize) {
        match self.selected {
            Sel::File(f) => (f, 0),
            Sel::Ref(f, r) => (f, r),
        }
    }

    fn open_output(&self) -> PeekOutput {
        let (f, r) = self.shown();
        let (file, rf) = (&self.files[f], &self.files[f].refs[r]);
        PeekOutput::Open { path: file.path.clone(), line: rf.line, col: rf.start }
    }

    fn select(&mut self, sel: Sel) {
        if self.selected != sel {
            self.selected = sel;
            self.reveal = true;
        }
    }

    /// Keys, before the editor sees them: Escape closes; while the tree has
    /// the focus ↑ / ↓ move, ← / → fold a file, Enter opens the reference
    /// (or folds a file).
    pub fn keys(&mut self, ctx: &Context) -> PeekOutput {
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            return PeekOutput::Close;
        }
        if !self.tree_focused {
            return PeekOutput::None;
        }
        let rows = self.rows();
        let at = rows.iter().position(|r| *r == self.selected).unwrap_or(0);
        let mut out = PeekOutput::None;
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::ArrowDown) && at + 1 < rows.len() {
                self.select(rows[at + 1]);
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowUp) && at > 0 {
                self.select(rows[at - 1]);
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                match self.selected {
                    Sel::File(f) => {
                        self.collapsed.insert(f);
                    }
                    Sel::Ref(f, _) => self.select(Sel::File(f)),
                }
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowRight) {
                if let Sel::File(f) = self.selected {
                    if !self.collapsed.remove(&f) {
                        self.select(Sel::Ref(f, 0));
                    }
                }
            }
            if i.consume_key(Modifiers::NONE, Key::Enter) {
                match self.selected {
                    Sel::File(f) => {
                        if !self.collapsed.remove(&f) {
                            self.collapsed.insert(f);
                        }
                    }
                    Sel::Ref(..) => out = self.open_output(),
                }
            }
        });
        out
    }

    /// Draws the peek under the anchor line, across the editor up to its
    /// minimap (`editor` and the minimap + scrollbar strip width).
    pub fn show(&mut self, ctx: &Context, editor: Rect, strip: f32) -> PeekOutput {
        let mut out = PeekOutput::None;
        let arrow_h = (LINE_H / 3.0).round();
        let top = self.anchor.bottom();
        let zone = Rect::from_min_max(Pos2::new(editor.left(), top), Pos2::new(editor.right() - strip, top + HEIGHT_IN_LINES * LINE_H));
        // A press outside takes the focus from the tree.
        if ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !zone.contains(p))) {
            self.tree_focused = false;
        }
        let border = Palette::PEEK_BORDER();
        egui::Area::new(egui::Id::new("references-peek"))
            .order(egui::Order::Foreground)
            .fixed_pos(zone.min)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(zone.intersect(editor));
                let (_, bg) = ui.allocate_exact_size(zone.size(), Sense::hover());
                let _ = bg;
                let p = ui.painter().clone();
                // The arrow: a triangle a third of a line tall, its tip at the
                // bottom of the reference's line.
                let ax = self.anchor.left().max(zone.left() + arrow_h);
                p.add(egui::Shape::convex_polygon(
                    vec![Pos2::new(ax, top), Pos2::new(ax + arrow_h, top + arrow_h), Pos2::new(ax - arrow_h, top + arrow_h)],
                    border,
                    Stroke::NONE,
                ));
                let container = Rect::from_min_max(Pos2::new(zone.left(), top + arrow_h), zone.max);
                let head = Rect::from_min_size(container.min, Vec2::new(container.width(), HEAD_H));
                p.rect_filled(head, 0.0, Palette::PEEK_TITLE_BG());
                // Title: file name, its folder, "- References (n)".
                let (f, _) = self.shown();
                let file = &self.files[f];
                let name = file.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let dir = file.path.parent().map(crate::workbench::welcome::tildify).unwrap_or_default();
                let cy = head.center().y;
                let mut x = head.left() + 20.0;
                let title_font = FontId::proportional(13.0);
                let r = p.text(Pos2::new(x, cy), Align2::LEFT_CENTER, &name, title_font.clone(), Palette::PEEK_TITLE_LABEL_FG());
                x = r.right() + 6.5;
                let r = p.text(Pos2::new(x, cy + 0.5), Align2::LEFT_CENTER, &dir, FontId::proportional(11.7), Palette::PEEK_TITLE_DESC_FG());
                x = r.right() + 3.9;
                let r = p.text(Pos2::new(x, cy), Align2::LEFT_CENTER, "-", title_font.clone(), Palette::PEEK_TITLE_DESC_FG());
                x = r.right() + 3.9;
                let meta = format!("References ({})", self.count());
                let r = p.text(Pos2::new(x, cy), Align2::LEFT_CENTER, &meta, title_font, Palette::PEEK_TITLE_DESC_FG());
                let title = Rect::from_min_max(Pos2::new(head.left() + 20.0, head.top()), Pos2::new(r.right(), head.bottom()));
                let tr = ui.interact(title, ui.id().with("peek-title"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                if tr.clicked() {
                    out = self.open_output();
                }
                // Close.
                let close = Rect::from_center_size(Pos2::new(head.right() - 2.0 - 11.0, cy), Vec2::splat(22.0));
                let cr = ui.interact(close, ui.id().with("peek-close"), Sense::click());
                if cr.hovered() {
                    p.rect_filled(close.shrink(2.0), 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
                }
                p.text(close.center(), Align2::CENTER_CENTER, crate::icons::codicon_by_name("close").unwrap_or('x').to_string(), crate::icons::codicon_font(16.0), Palette::PEEK_TITLE_LABEL_FG());
                cr.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Close"));
                if cr.clicked() {
                    out = PeekOutput::Close;
                }
                // Borders: the container's top and bottom, the body's top.
                p.hline(container.x_range(), container.top() + 0.5, Stroke::new(1.0, border));
                p.hline(container.x_range(), container.bottom() - 0.5, Stroke::new(1.0, border));
                p.hline(container.x_range(), head.bottom() + 0.5, Stroke::new(1.0, border));
                let body = Rect::from_min_max(Pos2::new(container.left(), head.bottom() + 1.0), Pos2::new(container.right(), container.bottom() - 1.0));
                let split = body.left() + (body.width() * RATIO).round();
                let preview_rect = Rect::from_min_max(body.min, Pos2::new(split, body.bottom()));
                let tree_rect = Rect::from_min_max(Pos2::new(split, body.top()), body.max);
                if let Some(o) = self.preview_ui(ui, preview_rect) {
                    out = o;
                }
                if let Some(o) = self.tree_ui(ui, tree_rect) {
                    out = o;
                }
            });
        out
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui, rect: Rect) -> Option<PeekOutput> {
        let (f, r) = self.shown();
        let file = &self.files[f];
        let rf = file.refs[r].clone();
        let visible = (rect.height() / LINE_H).floor().max(1.0);
        let max_top = (file.lines.len() as f32 - visible).max(0.0);
        if self.reveal {
            // revealRangeInCenter.
            self.preview_top = (rf.line as f32 - (visible / 2.0).floor()).clamp(0.0, max_top);
        }
        let resp = ui.interact(rect, ui.id().with("peek-preview"), Sense::click());
        if resp.hovered() {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            if dy != 0.0 {
                self.preview_top = (self.preview_top - dy / LINE_H).clamp(0.0, max_top);
            }
        }
        if resp.clicked() {
            self.tree_focused = false;
        }
        let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        p.rect_filled(rect, 0.0, Palette::PEEK_EDITOR_BG());
        let font = crate::icons::editor_mono_font(12.0);
        let cw = ui.fonts_mut(|fo| fo.glyph_width(&font, '0'));
        let digits = file.lines.len().to_string().len().max(5) as f32;
        let numbers_end = rect.left() + 12.0 + (digits * cw).round();
        let text_x = numbers_end + 18.0;
        p.rect_filled(Rect::from_min_max(rect.min, Pos2::new(text_x - 4.0, rect.bottom())), 0.0, Palette::PEEK_EDITOR_GUTTER_BG());
        let first = self.preview_top.floor() as usize;
        let offset = (self.preview_top - first as f32) * LINE_H;
        for (k, line_no) in (first..file.lines.len()).enumerate() {
            let y = rect.top() + k as f32 * LINE_H - offset;
            if y > rect.bottom() {
                break;
            }
            let row = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), LINE_H));
            let num_fg = if line_no == rf.line { Palette::LINE_NUMBER_ACTIVE_FG() } else { Palette::LINE_NUMBER_FG() };
            p.text(Pos2::new(numbers_end, row.center().y), Align2::RIGHT_CENTER, (line_no + 1).to_string(), font.clone(), num_fg);
            let x_of = |c: usize| text_x + c as f32 * cw;
            for (ri, other) in file.refs.iter().enumerate().filter(|(_, o)| o.line == line_no) {
                let m = Rect::from_min_max(Pos2::new(x_of(other.start), row.top()), Pos2::new(x_of(other.end), row.bottom()));
                p.rect_filled(m, 0.0, Palette::PEEK_EDITOR_MATCH_BG());
                p.rect_stroke(m, 0.0, Stroke::new(2.0, Palette::PEEK_EDITOR_MATCH_BORDER()), StrokeKind::Inside);
                if ri == r {
                    p.rect_filled(m, 0.0, Palette::SELECTION_BG());
                }
            }
            let mut x = text_x;
            match file.runs.get(line_no) {
                Some(runs) => {
                    for (text, colour) in runs {
                        let t = text.replace('\t', "    ");
                        p.text(Pos2::new(x, row.center().y), Align2::LEFT_CENTER, &t, font.clone(), *colour);
                        x += t.chars().count() as f32 * cw;
                    }
                }
                None => {
                    p.text(Pos2::new(x, row.center().y), Align2::LEFT_CENTER, file.lines[line_no].replace('\t', "    "), font.clone(), Palette::EDITOR_FG());
                }
            }
        }
        self.reveal = false;
        resp.double_clicked().then(|| self.open_output())
    }

    fn tree_ui(&mut self, ui: &mut egui::Ui, rect: Rect) -> Option<PeekOutput> {
        let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        p.rect_filled(rect, 0.0, Palette::PEEK_RESULT_BG());
        let rows = self.rows();
        let content_h = rows.len() as f32 * ROW_H;
        let max_scroll = (content_h - rect.height()).max(0.0);
        if let Some(at) = rows.iter().position(|r| *r == self.selected) {
            let (y0, y1) = (at as f32 * ROW_H, (at + 1) as f32 * ROW_H);
            if y0 < self.tree_scroll {
                self.tree_scroll = y0;
            } else if y1 > self.tree_scroll + rect.height() {
                self.tree_scroll = y1 - rect.height();
            }
        }
        if ui.rect_contains_pointer(rect) {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            self.tree_scroll -= dy;
        }
        self.tree_scroll = self.tree_scroll.clamp(0.0, max_scroll);
        let mut out = None;
        let ui_font = FontId::proportional(13.0);
        for (k, row) in rows.iter().enumerate() {
            let y = rect.top() + k as f32 * ROW_H - self.tree_scroll;
            if y + ROW_H < rect.top() || y > rect.bottom() {
                continue;
            }
            let r = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), ROW_H));
            let resp = ui.interact(r.intersect(rect), ui.id().with(("peek-row", k)), Sense::click());
            let selected = *row == self.selected;
            if selected {
                let bg = if self.tree_focused { Palette::PEEK_RESULT_SEL_BG() } else { Palette::LIST_INACTIVE_SELECTION_BG() };
                p.rect_filled(r, 0.0, bg);
                if self.tree_focused {
                    p.rect_stroke(r, 0.0, Stroke::new(1.0, Palette::VSCE_LIST_FOCUS_OUTLINE_2026()), StrokeKind::Inside);
                }
            } else if resp.hovered() {
                p.rect_filled(r, 0.0, Palette::LIST_HOVER_BG());
            }
            let cy = r.center().y;
            match *row {
                Sel::File(fi) => {
                    let f = &self.files[fi];
                    let chev = if self.collapsed.contains(&fi) { "chevron-right" } else { "chevron-down" };
                    p.text(Pos2::new(r.left() + 10.0, cy), Align2::CENTER_CENTER, crate::icons::codicon_by_name(chev).unwrap_or('v').to_string(), crate::icons::codicon_font(16.0), Palette::PEEK_RESULT_FILE_FG());
                    if let Some((glyph, colour)) = crate::file_icons::icon_for(&f.path) {
                        p.text(Pos2::new(r.left() + 28.0, cy + 0.5), Align2::CENTER_CENTER, glyph.to_string(), crate::file_icons::seti_font(18.0), colour);
                    }
                    let name = f.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    let fg = if selected && self.tree_focused { Palette::PEEK_RESULT_SEL_FG() } else { Palette::PEEK_RESULT_FILE_FG() };
                    let nr = p.text(Pos2::new(r.left() + 38.0, cy), Align2::LEFT_CENTER, &name, ui_font.clone(), fg);
                    let rel = f
                        .path
                        .parent()
                        .and_then(|d| self.root.as_ref().and_then(|root| d.strip_prefix(root).ok()).map(|d| d.display().to_string()).or_else(|| Some(crate::workbench::welcome::tildify(d))))
                        .unwrap_or_default();
                    if !rel.is_empty() {
                        p.text(Pos2::new(nr.right() + 6.5, cy + 0.5), Align2::LEFT_CENTER, &rel, FontId::proportional(11.7), fg.gamma_multiply(0.7));
                    }
                    // The count badge, 12px from the right.
                    let count = f.refs.len().to_string();
                    let tw = ui.fonts_mut(|fo| fo.layout_no_wrap(count.clone(), FontId::proportional(11.0), Color32::WHITE).size().x);
                    let w = (tw + 10.0).max(18.0);
                    let badge = Rect::from_center_size(Pos2::new(r.right() - 12.0 - w / 2.0, cy), Vec2::new(w, 18.0));
                    p.rect_filled(badge, 9.0, Palette::BADGE_BG());
                    p.text(badge.center(), Align2::CENTER_CENTER, &count, FontId::proportional(11.0), Palette::BADGE_FG());
                    let label = format!("{name}, {} reference{}", f.refs.len(), if f.refs.len() == 1 { "" } else { "s" });
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label.clone()));
                }
                Sel::Ref(fi, ri) => {
                    let f = &self.files[fi];
                    let rf = &f.refs[ri];
                    let (text, (hs, he)) = preview(f.lines.get(rf.line).map(String::as_str).unwrap_or(""), rf.start, rf.end);
                    let base = if selected && self.tree_focused { Palette::PEEK_RESULT_SEL_FG() } else { Palette::PEEK_RESULT_LINE_FG() };
                    let chars: Vec<char> = text.chars().collect();
                    let part = |a: usize, b: usize| chars[a.min(chars.len())..b.min(chars.len())].iter().collect::<String>();
                    let mut x = r.left() + 30.0;
                    for (s, hl) in [(part(0, hs), false), (part(hs, he), true), (part(he, chars.len()), false)] {
                        if s.is_empty() {
                            continue;
                        }
                        let g = ui.fonts_mut(|fo| fo.layout_no_wrap(s.clone(), ui_font.clone(), Color32::WHITE));
                        if hl {
                            p.rect_filled(Rect::from_min_size(Pos2::new(x, cy - 9.0), Vec2::new(g.size().x, 18.0)), 0.0, Palette::PEEK_RESULT_MATCH_BG());
                        }
                        p.text(Pos2::new(x, cy), Align2::LEFT_CENTER, &s, ui_font.clone(), if hl { Palette::PEEK_RESULT_FILE_FG() } else { base });
                        x += g.size().x;
                    }
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, text.clone()));
                }
            }
            if resp.clicked() {
                self.tree_focused = true;
                if let Sel::File(fi) = *row {
                    if !self.collapsed.remove(&fi) {
                        self.collapsed.insert(fi);
                    }
                }
                self.select(*row);
            }
            if resp.double_clicked() && matches!(row, Sel::Ref(..)) {
                out = Some(self.open_output());
            }
        }
        out
    }
}

/// A UTF-16 column on `line` as a char column.
fn utf16_to_char(line: &str, ch16: usize) -> usize {
    let mut units = 0;
    for (i, c) in line.chars().enumerate() {
        if units >= ch16 {
            return i;
        }
        units += c.len_utf16();
    }
    line.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_keeps_a_word_before_and_the_rest_of_the_line() {
        // A few characters before, from the start of the word there.
        let (text, hl) = preview("    let total = compute(total, 2);   ", 24, 29);
        assert_eq!(text, "compute(total, 2);");
        assert_eq!(hl, (8, 13));
        // At the line start nothing comes before.
        assert_eq!(preview("total = 1", 0, 5), ("total = 1".to_string(), (0, 5)));
    }

    #[test]
    fn the_model_sorts_groups_and_selects_the_nearest_reference() {
        let a = PathBuf::from("/w/a.md");
        let b = PathBuf::from("/w/b.md");
        let texts = |p: &Path| Some(if p.ends_with("a.md") { "x foo\nfoo\n".to_string() } else { "foo foo\n".to_string() });
        let locs = vec![(b.clone(), [0, 4, 0, 7]), (a.clone(), [1, 0, 1, 3]), (b.clone(), [0, 0, 0, 3]), (a.clone(), [0, 2, 0, 5]), (a.clone(), [0, 2, 0, 5])];
        let peek = ReferencesPeek::new(&locs, texts, (&b, 0, 5), Rect::NOTHING, None).unwrap();
        assert_eq!(peek.files.iter().map(|f| f.path.clone()).collect::<Vec<_>>(), vec![a.clone(), b.clone()]);
        assert_eq!(peek.files[0].refs, vec![PeekRef { line: 0, start: 2, end: 5 }, PeekRef { line: 1, start: 0, end: 3 }]);
        assert_eq!(peek.count(), 4);
        // In b.md at column 5: the second "foo" there (4..7) is nearest.
        assert_eq!(peek.selected, Sel::Ref(1, 1));
        assert_eq!(peek.rows().len(), 6);
    }
}
