//! Editor widgets for extension language features: the suggest list
//! (completion), the hover card, and the problems shown in it.
//!
//! VS Code analogues:
//! - `src/vs/editor/contrib/suggest/browser/suggestWidget.ts` and
//!   `media/suggest.css` (430px wide, one editor line per item, up to 12
//!   items, kind icon, label with the typed prefix in bold, detail on the
//!   focused item, `editorSuggestWidget.*` colours)
//! - `src/vs/editor/contrib/hover/browser/hoverWidget` / `hover.css`
//!   (`editorHoverWidget.*`, markdown content, problems first)
//! - symbol icon colours: `src/vs/editor/contrib/symbolIcons/browser/symbolIcons.ts`
//!
//! Tokens: `--vscode-editorSuggestWidget-*`, `--vscode-editorHoverWidget-*`,
//! `--vscode-editorError-foreground`, `--vscode-editorWarning-foreground`.

use egui::{Align2, Color32, Context, CornerRadius, FontId, Order, Sense, Stroke, StrokeKind};

use crate::extensions::{CompletionEntry, Diagnostic};
use crate::icons::{codicon_by_name, codicon_font, editor_mono_font};
use crate::theme::Palette;

const SUGGEST_W: f32 = 430.0;
const ROW_H: f32 = 18.0;
const MAX_ROWS: usize = 12;

/// An open suggest list.
pub struct Suggest {
    /// All items from the providers.
    pub items: Vec<CompletionEntry>,
    /// Byte offset where the replaced word starts.
    pub start: usize,
    /// Indices into `items` matching the typed word, best first.
    pub shown: Vec<usize>,
    pub selected: usize,
    /// Path of the document it belongs to.
    pub path: std::path::PathBuf,
}

impl Suggest {
    /// Filter by the word typed since `start` (prefix first, then any
    /// in-order match), keeping the selection on the same item if it stays.
    pub fn refilter(&mut self, word: &str) {
        let w = word.to_lowercase();
        let keep = self.shown.get(self.selected).copied();
        let mut scored: Vec<(u8, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, it)| {
                let f = it.filter_text.to_lowercase();
                let rank = if w.is_empty() || f.starts_with(&w) {
                    0
                } else if subsequence(&f, &w) {
                    1
                } else {
                    return None;
                };
                Some((if it.preselect { 0 } else { rank + 1 }, i))
            })
            .collect();
        scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| self.items[a.1].sort_text.cmp(&self.items[b.1].sort_text)));
        self.shown = scored.into_iter().map(|(_, i)| i).collect();
        self.selected = keep.and_then(|k| self.shown.iter().position(|&i| i == k)).unwrap_or(0);
    }

    pub fn current(&self) -> Option<&CompletionEntry> {
        self.shown.get(self.selected).map(|&i| &self.items[i])
    }
}

fn subsequence(hay: &str, needle: &str) -> bool {
    let mut it = hay.chars();
    needle.chars().all(|c| it.any(|h| h == c))
}

/// Codicon and colour for a `CompletionItemKind` (symbolIcons.ts dark
/// defaults).
fn kind_icon(kind: &str) -> (&'static str, Color32) {
    let purple = Color32::from_rgb(0xB1, 0x80, 0xD7);
    let blue = Color32::from_rgb(0x75, 0xBE, 0xFF);
    let orange = Color32::from_rgb(0xEE, 0x9D, 0x28);
    match kind {
        "Method" => ("symbol-method", purple),
        "Function" => ("symbol-function", purple),
        "Constructor" => ("symbol-constructor", purple),
        "Field" => ("symbol-field", blue),
        "Variable" => ("symbol-variable", blue),
        "Class" => ("symbol-class", orange),
        "Interface" => ("symbol-interface", blue),
        "Module" => ("symbol-module", Palette::FG),
        "Property" => ("symbol-property", Palette::FG),
        "Unit" => ("symbol-unit", Palette::FG),
        "Value" => ("symbol-value", Palette::FG),
        "Enum" => ("symbol-enum", orange),
        "EnumMember" => ("symbol-enum-member", blue),
        "Keyword" => ("symbol-keyword", Palette::FG),
        "Snippet" => ("symbol-snippet", Palette::FG),
        "Color" => ("symbol-color", Palette::FG),
        "File" => ("symbol-file", Palette::FG),
        "Reference" => ("symbol-reference", Palette::FG),
        "Folder" => ("folder", Palette::FG),
        "Constant" => ("symbol-constant", Palette::FG),
        "Struct" => ("symbol-struct", Palette::FG),
        "Event" => ("symbol-event", orange),
        "Operator" => ("symbol-operator", Palette::FG),
        "TypeParameter" => ("symbol-type-parameter", Palette::FG),
        _ => ("symbol-text", Palette::FG),
    }
}

/// What the user did with the suggest list.
pub enum SuggestEvent {
    Accept,
}

/// Draw the suggest list under the caret.
pub fn suggest_widget(ctx: &Context, s: &mut Suggest, caret: egui::Rect, word_len: usize) -> Option<SuggestEvent> {
    if s.shown.is_empty() {
        return None;
    }
    let mut event = None;
    let rows = s.shown.len().min(MAX_ROWS);
    let top = s.selected.saturating_sub(MAX_ROWS - 1);
    let height = rows as f32 * ROW_H + 2.0;
    let screen = ctx.content_rect();
    let mut pos = egui::pos2(caret.left() - 24.0, caret.bottom() + 1.0);
    if pos.y + height > screen.bottom() - 24.0 {
        pos.y = caret.top() - height - 1.0;
    }
    pos.x = pos.x.min(screen.right() - SUGGEST_W - 4.0).max(screen.left());
    let resp = egui::Area::new(egui::Id::new("suggest_widget"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(SUGGEST_W, height), Sense::hover());
            let p = ui.painter();
            p.add(egui::epaint::Shadow { offset: [0, 0], blur: 8, spread: 0, color: Color32::from_black_alpha(90) }
                .as_shape(rect, CornerRadius::same(3)));
            p.rect(rect, CornerRadius::same(3), Palette::VSCE_EDITOR_WIDGET_BG, Stroke::new(1.0, Palette::VSCE_EDITOR_WIDGET_BORDER), StrokeKind::Inside);
            for (row, &idx) in s.shown.iter().enumerate().skip(top).take(MAX_ROWS) {
                let item = &s.items[idx];
                let r = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + 1.0, rect.top() + 1.0 + (row - top) as f32 * ROW_H),
                    egui::vec2(SUGGEST_W - 2.0, ROW_H),
                );
                let rr = ui.interact(r, egui::Id::new(("suggest_row", idx)), Sense::click());
                rr.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &item.label));
                let selected = row == s.selected;
                if selected {
                    ui.painter().rect_filled(r, 0.0, Palette::VSCE_SUGGEST_SELECTED_BG);
                } else if rr.hovered() {
                    ui.painter().rect_filled(r, 0.0, Palette::LIST_HOVER_BG);
                }
                let (icon, color) = kind_icon(&item.kind);
                if let Some(g) = codicon_by_name(icon) {
                    ui.painter().text(egui::pos2(r.left() + 12.0, r.center().y), Align2::CENTER_CENTER, g.to_string(), codicon_font(14.0), color);
                }
                // Label: the typed prefix in bold.
                let font = editor_mono_font(12.0);
                let bold_n = word_len.min(item.label.chars().count());
                let mut job = egui::text::LayoutJob::default();
                let split = item.label.char_indices().nth(bold_n).map(|(b, _)| b).unwrap_or(item.label.len());
                let (head, tail) = item.label.split_at(split);
                if !head.is_empty() && item.label.to_lowercase().starts_with(&head.to_lowercase()) {
                    job.append(head, 0.0, egui::TextFormat { font_id: font.clone(), color: Palette::VSCE_LIST_HIGHLIGHT_FG, ..Default::default() });
                    job.append(tail, 0.0, egui::TextFormat { font_id: font.clone(), color: Palette::FG, ..Default::default() });
                } else {
                    job.append(&item.label, 0.0, egui::TextFormat { font_id: font.clone(), color: Palette::FG, ..Default::default() });
                }
                let g = ui.fonts_mut(|f| f.layout_job(job));
                ui.painter().galley(egui::pos2(r.left() + 24.0, r.center().y - g.size().y / 2.0), g, Palette::FG);
                if selected {
                    if let Some(d) = &item.detail {
                        ui.painter_at(r).text(egui::pos2(r.right() - 8.0, r.center().y), Align2::RIGHT_CENTER, d, FontId::proportional(11.5), Palette::FG_DESCRIPTION);
                    }
                }
                if rr.clicked() {
                    s.selected = row;
                    event = Some(SuggestEvent::Accept);
                }
            }
            rect
        });
    // Documentation of the focused item, beside the list.
    if let Some(doc) = s.current().and_then(|i| i.documentation.clone()).filter(|d| !d.trim().is_empty()) {
        let side = egui::pos2(resp.inner.right() + 2.0, resp.inner.top());
        card(ctx, "suggest_details", side, false, &[], &[doc], 330.0);
    }
    event
}

/// A hover-style card with its top-left (or, with `above`, bottom-left) at
/// `pos`: problems first, then markdown blocks.
pub fn card(
    ctx: &Context,
    id: &str,
    pos: egui::Pos2,
    above: bool,
    problems: &[&Diagnostic],
    blocks: &[String],
    max_w: f32,
) -> egui::Rect {
    egui::Area::new(egui::Id::new(id))
        .order(Order::Tooltip)
        .fixed_pos(pos)
        .pivot(if above { Align2::LEFT_BOTTOM } else { Align2::LEFT_TOP })
        .constrain(true)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(Palette::VSCE_EDITOR_WIDGET_BG)
                .stroke(Stroke::new(1.0, Palette::VSCE_EDITOR_WIDGET_BORDER))
                .corner_radius(3)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.set_max_width(max_w);
                    ui.spacing_mut().item_spacing.y = 4.0;
                    for d in problems {
                        let color = match d.severity {
                            0 => Palette::VSCE_EDITOR_ERROR_FG,
                            1 => Palette::VSCE_EDITOR_WARNING_FG,
                            _ => Palette::VSCE_EDITOR_INFO_FG,
                        };
                        let mut text = d.message.clone();
                        if let Some(src) = &d.source {
                            text.push_str(&format!("  {src}"));
                            if let Some(code) = &d.code {
                                text.push_str(&format!("({code})"));
                            }
                        }
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new("●").color(color).size(10.0));
                            ui.label(egui::RichText::new(text).color(Palette::FG).size(13.0));
                        });
                    }
                    if !problems.is_empty() && !blocks.is_empty() {
                        ui.separator();
                    }
                    for b in blocks {
                        markdown(ui, b);
                    }
                });
        })
        .response
        .rect
}

/// Minimal markdown: fenced code in the editor font, `**bold**`, `` `code` ``
/// and links shown as their text, headings as bold lines.
fn markdown(ui: &mut egui::Ui, md: &str) {
    let mut in_code = false;
    let mut code = String::new();
    for line in md.lines() {
        if line.trim_start().starts_with("```") {
            if in_code {
                ui.label(egui::RichText::new(code.trim_end()).font(editor_mono_font(12.0)).color(Palette::FG));
                code.clear();
            }
            in_code = !in_code;
            continue;
        }
        if in_code {
            code.push_str(line);
            code.push('\n');
            continue;
        }
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t == "---" || t == "***" {
            ui.separator();
            continue;
        }
        let heading = t.starts_with('#');
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = ui.available_width();
        let body = t.trim_start_matches('#').trim();
        inline(&mut job, body, heading);
        ui.label(job);
    }
    if in_code && !code.is_empty() {
        ui.label(egui::RichText::new(code.trim_end()).font(editor_mono_font(12.0)).color(Palette::FG));
    }
}

fn inline(job: &mut egui::text::LayoutJob, text: &str, bold: bool) {
    let plain = |bold: bool| egui::TextFormat {
        font_id: if bold { crate::ui_font::bold(13.0) } else { FontId::proportional(13.0) },
        color: Palette::FG,
        ..Default::default()
    };
    let mut rest = text;
    let mut strong = bold;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("**") {
            strong = !strong || bold;
            rest = r;
            continue;
        }
        if let Some(r) = rest.strip_prefix('`') {
            let end = r.find('`').unwrap_or(r.len());
            job.append(&r[..end], 0.0, egui::TextFormat { font_id: editor_mono_font(12.0), color: Palette::FG, ..Default::default() });
            rest = r.get(end + 1..).unwrap_or("");
            continue;
        }
        if let Some(r) = rest.strip_prefix('[') {
            if let Some(close) = r.find("](") {
                let after = &r[close + 2..];
                let end = after.find(')').unwrap_or(after.len());
                job.append(&r[..close], 0.0, egui::TextFormat { color: Palette::VSCE_EXTENSION_VERIFIED_FG, ..plain(strong) });
                rest = after.get(end + 1..).unwrap_or("");
                continue;
            }
        }
        let next = rest
            .char_indices()
            .skip(1)
            .find(|(_, c)| matches!(c, '*' | '`' | '['))
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        let chunk = &rest[..next];
        job.append(&chunk.replace('\\', ""), 0.0, plain(strong));
        rest = &rest[next..];
    }
}
