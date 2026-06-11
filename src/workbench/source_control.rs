//! Source Control view — VS Code's multi-repository SCM panel.
//!
//! Layout (top → bottom): the "SOURCE CONTROL" title with a "…" overflow;
//! one **collapsible section per repository** (header: chevron + repo icon +
//! name + branch + count + commit/sync/refresh/"…" actions); each expanded
//! repo shows its commit input + Commit button, then resource groups
//! (Merge / Staged Changes / Changes) with file rows; finally a "GRAPH"
//! section rendering the active repo's commit history as a lane graph.
//!
//! Single-repo workspaces skip the repo header (the lone repo is always
//! expanded), matching VS Code.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use egui::{Align2, FontId, Key, Pos2, Sense, Stroke, Ui};

use crate::git::{
    ChangeKind, CommitMode, FileChange, GraphRow, Model, RefKind, Repository, ResourceGroupKind,
};
use crate::icons::{self, codicon_font};
use crate::theme::Palette;
use crate::vscode_widgets::primitives::{badge, button, icon_button, label, BadgeProps, ButtonProps,
    IconButtonProps, LabelProps};

/// Caller-owned UI state for the SCM view.
#[derive(Default)]
pub struct ScmUiState {
    pub commit_messages: HashMap<PathBuf, String>,
    pub expanded: HashSet<PathBuf>,
    pub graph_open: bool,
    /// The "CHANGES" parent group (multi-repo) collapse state.
    pub changes_open: bool,
    /// Commit ids whose changed-file list is expanded in the GRAPH.
    pub expanded_commits: HashSet<String>,
    /// `false` until the first render decides the initial expand state.
    initialized: bool,
}

/// Domain events emitted by the SCM view (dispatched in `app.rs`).
#[derive(Default)]
pub struct ScmOutput {
    /// `(repo_root, rel, staged)` — open a diff of this file.
    pub open_diff: Option<(PathBuf, String, bool)>,
    pub stage: Option<(PathBuf, String)>,
    pub unstage: Option<(PathBuf, String)>,
    /// `(repo_root, rel, untracked)`.
    pub discard: Option<(PathBuf, String, bool)>,
    pub stage_all: Option<PathBuf>,
    pub unstage_all: Option<PathBuf>,
    pub commit: Option<(PathBuf, CommitMode)>,
    /// `(repo_root, anchor)` — open the Commit-mode dropdown.
    pub commit_menu: Option<(PathBuf, Pos2)>,
    /// `(repo_root, anchor)` — open the per-repo "…" menu.
    pub repo_menu: Option<(PathBuf, Pos2)>,
    pub refresh: Option<PathBuf>,
    pub sync: Option<PathBuf>,
    /// `(repo_root, rel, parent_rev, commit_rev)` — open a commit's file diff.
    pub open_commit_diff: Option<(PathBuf, String, String, String)>,
}

pub fn show(
    ui: &mut Ui,
    model: &Model,
    history: &[GraphRow],
    graph_root: Option<&std::path::Path>,
    st: &mut ScmUiState,
) -> ScmOutput {
    let mut out = ScmOutput::default();

    title_row(ui);

    if model.repos.is_empty() {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            crate::vscode_widgets::forms::form_helper(
                ui,
                &crate::vscode_widgets::forms::FormHelperProps::new(
                    "The open folder is not a Git repository.",
                ),
            );
        });
        return out;
    }

    // First render: expand all repos + the CHANGES group.
    if !st.initialized {
        for r in &model.repos {
            st.expanded.insert(r.root.clone());
        }
        st.changes_open = true;
        st.graph_open = true; // VS Code shows the GRAPH section expanded by default
        st.initialized = true;
    }

    let single = model.repos.len() == 1;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.style_mut().spacing.scroll.floating = true;

            if single {
                if let Some(repo) = model.repos.first() {
                    repo_section(ui, repo, true, st, &mut out);
                }
            } else {
                // Multi-repo: a "CHANGES" parent group containing repo rows.
                changes_header(ui, model, st);
                if st.changes_open {
                    for repo in &model.repos {
                        repo_section(ui, repo, false, st, &mut out);
                    }
                }
            }

            // GRAPH section (active repo's history).
            ui.add_space(6.0);
            graph_section(ui, history, graph_root, st, &mut out);
        });

    out
}

/// The "CHANGES" parent group header (multi-repo) with collapse-all + "…".
fn changes_header(ui: &mut Ui, _model: &Model, st: &mut ScmUiState) {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), Sense::click());
    let p = ui.painter();
    let cy = rect.center().y;
    let chev = if st.changes_open { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT };
    p.text(egui::pos2(rect.left() + 12.0, cy), Align2::CENTER_CENTER, chev.to_string(),
        codicon_font(12.0), Palette::FG_DESCRIPTION);
    p.text(egui::pos2(rect.left() + 26.0, cy), Align2::LEFT_CENTER, "CHANGES",
        FontId::proportional(11.0), Palette::FG);
    if resp.clicked() {
        st.changes_open = !st.changes_open;
    }
}

fn title_row(ui: &mut Ui) {
    use crate::vscode_widgets::layout::{toolbar_container, ToolbarContainerProps};
    ui.allocate_ui(egui::vec2(ui.available_width(), 30.0), |ui| {
        ui.painter()
            .rect_filled(ui.available_rect_before_wrap(), 0.0, Palette::SIDEBAR_BG);
        toolbar_container(
            ui,
            &ToolbarContainerProps::new().title("SOURCE CONTROL"),
            |ui| {
                let _ = icon_button(ui, &IconButtonProps::new(icons::ELLIPSIS).icon_size(14.0))
                    .on_hover_text("Views and More Actions…");
            },
        );
    });
}

fn repo_section(
    ui: &mut Ui,
    repo: &Repository,
    single: bool,
    st: &mut ScmUiState,
    out: &mut ScmOutput,
) {
    let expanded = single || st.expanded.contains(&repo.root);

    if !single {
        // Hand-rolled collapsible repo header (chevron + icon + name + branch
        // + count + action cluster).
        let h = 24.0;
        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), h), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, 0.0, Palette::LIST_HOVER_BG);
        }
        let p = ui.painter();
        let cy = rect.center().y;
        // Indent under the "CHANGES" parent group (+ a left guide line).
        let lx = rect.left() + 12.0;
        p.line_segment(
            [egui::pos2(lx, rect.top()), egui::pos2(lx, rect.bottom())],
            Stroke::new(1.0, Palette::VSCE_TREE_INDENT_GUIDE),
        );
        let chev = if expanded { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT };
        p.text(egui::pos2(rect.left() + 24.0, cy), Align2::CENTER_CENTER, chev.to_string(),
            codicon_font(12.0), Palette::FG_DESCRIPTION);
        p.text(egui::pos2(rect.left() + 38.0, cy), Align2::LEFT_CENTER, icons::REPO.to_string(),
            codicon_font(14.0), Palette::FG_DESCRIPTION);
        p.text(egui::pos2(rect.left() + 58.0, cy), Align2::LEFT_CENTER, &repo.name,
            FontId::proportional(13.0), Palette::FG);
        // Branch indicator: git-branch glyph (codicon font!) + name (proportional).
        let name_w = p
            .layout_no_wrap(repo.name.clone(), FontId::proportional(13.0), Palette::FG)
            .size()
            .x;
        if let Some(b) = &repo.branch {
            let bx = rect.left() + 58.0 + name_w + 12.0;
            p.text(egui::pos2(bx, cy), Align2::LEFT_CENTER, icons::GIT_BRANCH.to_string(),
                codicon_font(13.0), Palette::FG_DESCRIPTION);
            p.text(egui::pos2(bx + 16.0, cy), Align2::LEFT_CENTER, b,
                FontId::proportional(11.5), Palette::FG_DESCRIPTION);
        }

        if resp.clicked() {
            if st.expanded.contains(&repo.root) {
                st.expanded.remove(&repo.root);
            } else {
                st.expanded.insert(repo.root.clone());
            }
        }

        // Right action cluster (right-to-left): "…", refresh, sync, commit ✓.
        let mut x = rect.right() - 18.0;
        for (glyph, tip) in [
            (icons::ELLIPSIS, "More Actions…"),
            (icons::REFRESH, "Refresh"),
            (icons::SYNC, "Synchronize Changes"),
            (icons::CHECK, "Commit"),
        ] {
            let c = egui::pos2(x, cy);
            let r = egui::Rect::from_center_size(c, egui::vec2(20.0, 20.0));
            let ar = ui.interact(r, resp.id.with(("repoact", glyph as u32)), Sense::click());
            if ar.hovered() {
                ui.painter().rect_filled(r, 3.0, Palette::SIDEBAR_BG);
            }
            ui.painter().text(c, Align2::CENTER_CENTER, glyph.to_string(),
                codicon_font(13.0), Palette::FG_DESCRIPTION);
            let ar = ar.on_hover_text(tip);
            if ar.clicked() {
                match glyph {
                    g if g == icons::CHECK => out.commit = Some((repo.root.clone(), CommitMode::Plain)),
                    g if g == icons::SYNC => out.sync = Some(repo.root.clone()),
                    g if g == icons::REFRESH => out.refresh = Some(repo.root.clone()),
                    _ => out.repo_menu = Some((repo.root.clone(), c)),
                }
            }
            x -= 22.0;
        }
        // Count badge before the actions.
        if repo.total() > 0 {
            let bx = x - 6.0;
            ui.painter().text(egui::pos2(bx, cy), Align2::RIGHT_CENTER,
                repo.total().to_string(), FontId::proportional(11.0), Palette::FG_DESCRIPTION);
        }
    }

    if !expanded {
        return;
    }

    // Indent the repo body slightly when multi-repo.
    let indent = if single { 0.0 } else { 6.0 };
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width() - indent, 0.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            if indent > 0.0 {
                ui.add_space(0.0);
            }
            commit_box(ui, repo, st, out);
            for (kind, files) in repo.groups() {
                group(ui, repo, kind, files, out);
            }
        },
    );
}

fn commit_box(ui: &mut Ui, repo: &Repository, st: &mut ScmUiState, out: &mut ScmOutput) {
    let branch = repo.branch.clone().unwrap_or_else(|| "HEAD".into());
    let placeholder = format!("Message (⌘Enter to commit on \"{branch}\")");
    let pad_l = 16.0;
    let pad_r = 8.0;
    ui.add_space(3.0);

    // --- message input: VS Code's .scm-editor — single line, 26px, input
    // background, 1px input border (focus border when focused), radius 4.
    let input_h = 26.0;
    let w = ui.available_width() - pad_l - pad_r;
    let mut commit_now = false;
    ui.horizontal(|ui| {
        ui.add_space(pad_l);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, input_h), Sense::hover());
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(4), Palette::VSCE_INPUT_BG);
        let msg = st.commit_messages.entry(repo.root.clone()).or_default();
        let inner = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 8.0, rect.top() + 4.0),
            egui::vec2(rect.width() - 16.0, rect.height() - 8.0),
        );
        let resp = ui
            .scope_builder(
                egui::UiBuilder::new().max_rect(inner).layout(*ui.layout()),
                |ui| {
                    ui.add(
                        egui::TextEdit::singleline(msg)
                            .background_color(egui::Color32::TRANSPARENT)
                            .desired_width(f32::INFINITY)
                            .hint_text(&placeholder)
                            .font(FontId::proportional(12.5))
                            .text_color(Palette::FG),
                    )
                },
            )
            .inner;
        let border = if resp.has_focus() {
            Palette::FOCUS_BORDER
        } else {
            Palette::INPUT_BORDER
        };
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(4),
            Stroke::new(1.0, border),
            egui::StrokeKind::Inside,
        );
        if resp.has_focus() && ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter)) {
            commit_now = true;
        }
    });
    if commit_now {
        out.commit = Some((repo.root.clone(), CommitMode::Plain));
    }
    ui.add_space(4.0);

    // --- commit split button: one 26px monaco-button-dropdown — centred
    // "✓ Commit", a 1px divider, then the caret. Muted when no changes.
    let has_changes = repo.total() > 0;
    let a = if has_changes { 1.0 } else { 0.45 };
    ui.horizontal(|ui| {
        ui.add_space(pad_l);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, input_h), Sense::click());
        let caret_w = 20.0;
        let main = egui::Rect::from_min_max(
            rect.min,
            egui::pos2(rect.right() - caret_w, rect.bottom()),
        );
        let caret = egui::Rect::from_min_max(main.right_top(), rect.max);
        let hovered = has_changes && resp.hovered();
        let bg = if hovered { Palette::BUTTON_HOVER } else { Palette::BUTTON_BG };
        let p = ui.painter();
        p.rect_filled(rect, egui::CornerRadius::same(2), with_alpha(bg, a));
        // divider between the button body and the caret
        p.vline(
            caret.left(),
            caret.y_range().shrink(5.0),
            Stroke::new(1.0, with_alpha(egui::Color32::BLACK, 0.4 * a)),
        );
        let fg = with_alpha(Palette::FG_BRIGHT, if has_changes { 1.0 } else { 0.6 });
        // centred "✓ Commit"
        let label_w = p
            .layout_no_wrap("Commit".into(), FontId::proportional(13.0), fg)
            .size()
            .x;
        let cxs = main.center().x - (label_w + 18.0) / 2.0;
        p.text(
            egui::pos2(cxs + 8.0, main.center().y),
            Align2::CENTER_CENTER,
            icons::CHECK.to_string(),
            codicon_font(13.0),
            fg,
        );
        p.text(
            egui::pos2(cxs + 18.0, main.center().y),
            Align2::LEFT_CENTER,
            "Commit",
            FontId::proportional(13.0),
            fg,
        );
        p.text(
            caret.center(),
            Align2::CENTER_CENTER,
            icons::CHEVRON_DOWN.to_string(),
            codicon_font(12.0),
            fg,
        );
        if has_changes && resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                if pos.x >= caret.left() {
                    out.commit_menu = Some((repo.root.clone(), caret.left_bottom()));
                } else {
                    out.commit = Some((repo.root.clone(), CommitMode::Plain));
                }
            }
        }
        ui.add_space(pad_r);
    });
    ui.add_space(4.0);
}

fn group(
    ui: &mut Ui,
    repo: &Repository,
    kind: ResourceGroupKind,
    files: &[FileChange],
    out: &mut ScmOutput,
) {
    // Group header: label + count + hover actions.
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        label(ui, &LabelProps::new(kind.label()).size(11.0).color(Palette::FG_DESCRIPTION));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            badge(ui, &BadgeProps::counter(&files.len().to_string()));
            ui.add_space(6.0);
            // Group inline actions.
            let actions: &[(char, &str)] = match kind {
                ResourceGroupKind::Index => &[(icons::REMOVE, "Unstage All Changes")],
                ResourceGroupKind::WorkingTree => {
                    &[(icons::DISCARD, "Discard All Changes"), (icons::ADD, "Stage All Changes")]
                }
                ResourceGroupKind::Merge => &[(icons::ADD, "Stage All Merge Changes")],
                ResourceGroupKind::Untracked => {
                    &[(icons::DISCARD, "Discard All"), (icons::ADD, "Stage All")]
                }
            };
            for (glyph, tip) in actions {
                if icon_button(ui, &IconButtonProps::new(*glyph).icon_size(13.0))
                    .on_hover_text(*tip)
                    .clicked()
                {
                    match (kind, *glyph) {
                        (ResourceGroupKind::Index, _) => out.unstage_all = Some(repo.root.clone()),
                        (_, g) if g == icons::ADD => out.stage_all = Some(repo.root.clone()),
                        _ => {} // discard-all: handled per-row for now
                    }
                }
            }
        });
    });
    ui.add_space(2.0);

    for fc in files {
        file_row(ui, repo, kind, fc, out);
    }
}

fn file_row(
    ui: &mut Ui,
    repo: &Repository,
    kind: ResourceGroupKind,
    fc: &FileChange,
    out: &mut ScmOutput,
) {
    let staged = matches!(kind, ResourceGroupKind::Index);
    let untracked = matches!(fc.kind, ChangeKind::Untracked);
    let row_h = 22.0;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), Sense::click());
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("scm:{}", fc.rel))
    });
    let hovered = resp.hovered();
    if hovered {
        ui.painter().rect_filled(rect, 0.0, Palette::LIST_HOVER_BG);
    }
    let p = ui.painter();
    let cy = rect.center().y;
    // Faded for untracked (VS Code `.resource.faded { opacity: 0.7 }`).
    let alpha = if untracked { 0.7 } else { 1.0 };
    let fg = with_alpha(Palette::FG, alpha);
    let dim = with_alpha(Palette::FG_DESCRIPTION, alpha);

    let name = fc.rel.rsplit('/').next().unwrap_or(&fc.rel);
    let parent: String = {
        let mut it = fc.rel.rsplitn(2, '/');
        let _ = it.next();
        it.next().unwrap_or("").to_string()
    };
    // File-type icon from the Seti theme (same as the Explorer tree), tinted
    // by the row's faded alpha. Falls back to the codicon file glyph.
    let icon_path = repo.root.join(&fc.rel);
    if let Some((glyph, color)) = crate::file_icons::icon_for(&icon_path) {
        p.text(egui::pos2(rect.left() + 13.0, cy), Align2::LEFT_CENTER, glyph.to_string(),
            crate::file_icons::seti_font(15.0), with_alpha(color, alpha));
    } else {
        p.text(egui::pos2(rect.left() + 12.0, cy), Align2::LEFT_CENTER, icons::FILE.to_string(),
            codicon_font(14.0), dim);
    }
    let name_pos = egui::pos2(rect.left() + 32.0, cy);
    let name_galley = p.layout_no_wrap(name.to_string(), FontId::proportional(13.0), fg);
    let name_w = name_galley.size().x;
    p.galley(egui::pos2(name_pos.x, cy - name_galley.size().y / 2.0), name_galley, fg);
    if fc.kind.strikethrough() {
        let y = cy;
        p.line_segment(
            [egui::pos2(name_pos.x, y), egui::pos2(name_pos.x + name_w, y)],
            Stroke::new(1.0, fg),
        );
    }
    if !parent.is_empty() {
        p.text(egui::pos2(name_pos.x + name_w + 6.0, cy), Align2::LEFT_CENTER, &parent,
            FontId::proportional(11.5), dim);
    }

    // Status letter on the right, tinted by decoration colour.
    p.text(egui::pos2(rect.right() - 14.0, cy), Align2::RIGHT_CENTER, fc.kind.badge(),
        FontId::proportional(12.0), with_alpha(fc.kind.decoration_color(), alpha));

    // Hover inline actions (right-to-left): primary stage/unstage, discard.
    let mut handled_action = false;
    if hovered {
        let mut x = rect.right() - 34.0;
        // primary
        let (pg, ptip) = if staged {
            (icons::REMOVE, "Unstage Changes")
        } else {
            (icons::ADD, "Stage Changes")
        };
        if hover_action(ui, resp.id.with("prim"), x, cy, pg, ptip) {
            handled_action = true;
            if staged {
                out.unstage = Some((repo.root.clone(), fc.rel.clone()));
            } else {
                out.stage = Some((repo.root.clone(), fc.rel.clone()));
            }
        }
        x -= 22.0;
        if !staged {
            if hover_action(ui, resp.id.with("disc"), x, cy, icons::DISCARD, "Discard Changes") {
                handled_action = true;
                out.discard = Some((repo.root.clone(), fc.rel.clone(), untracked));
            }
        }
    }

    if resp.clicked() && !handled_action {
        // Default click → open the diff (openChange).
        out.open_diff = Some((repo.root.clone(), fc.rel.clone(), staged));
    }
}

fn hover_action(ui: &Ui, id: egui::Id, x: f32, cy: f32, glyph: char, tip: &str) -> bool {
    let center = egui::pos2(x, cy);
    let r = egui::Rect::from_center_size(center, egui::vec2(18.0, 18.0));
    let resp = ui.interact(r, id, Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 3.0, Palette::SIDEBAR_BG);
    }
    let resp = resp.on_hover_text(tip);
    ui.painter().text(center, Align2::CENTER_CENTER, glyph.to_string(),
        codicon_font(13.0), Palette::FG_DESCRIPTION);
    resp.clicked()
}

// ── GRAPH ──────────────────────────────────────────────────────────────────

const LANE_W: f32 = 11.0;
const GRAPH_ROW_H: f32 = 22.0;
const CIRCLE_R: f32 = 4.0;

fn graph_section(
    ui: &mut Ui,
    history: &[GraphRow],
    graph_root: Option<&std::path::Path>,
    st: &mut ScmUiState,
    out: &mut ScmOutput,
) {
    // Collapsible "GRAPH" header.
    let h = 22.0;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), Sense::click());
    let p = ui.painter();
    let cy = rect.center().y;
    let chev = if st.graph_open { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT };
    p.text(egui::pos2(rect.left() + 12.0, cy), Align2::CENTER_CENTER, chev.to_string(),
        codicon_font(12.0), Palette::FG_DESCRIPTION);
    p.text(egui::pos2(rect.left() + 26.0, cy), Align2::LEFT_CENTER, "GRAPH",
        FontId::proportional(11.0), Palette::FG_DESCRIPTION);
    if resp.clicked() {
        st.graph_open = !st.graph_open;
    }
    if !st.graph_open {
        return;
    }

    for row in history {
        let clicked = graph_row(ui, row);
        let expanded = st.expanded_commits.contains(&row.commit.id);
        if clicked {
            if expanded {
                st.expanded_commits.remove(&row.commit.id);
            } else {
                st.expanded_commits.insert(row.commit.id.clone());
            }
        }
        // Expanded → list the commit's changed files (vs first parent).
        if (expanded || clicked && !expanded) && st.expanded_commits.contains(&row.commit.id) {
            if let Some(root) = graph_root {
                let files = crate::git::commit_changes(root, &row.commit);
                let parent = row.commit.parents.first().cloned().unwrap_or_default();
                for f in &files {
                    if commit_file_row(ui, f) {
                        out.open_commit_diff = Some((
                            root.to_path_buf(),
                            f.rel.clone(),
                            parent.clone(),
                            row.commit.id.clone(),
                        ));
                    }
                }
            }
        }
    }
}

/// One changed-file row under an expanded commit. Returns `true` on click.
fn commit_file_row(ui: &mut Ui, f: &crate::git::CommitFile) -> bool {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 0.0, Palette::LIST_HOVER_BG);
    }
    let cy = rect.center().y;
    let name = f.rel.rsplit('/').next().unwrap_or(&f.rel);
    let parent: String = {
        let mut it = f.rel.rsplitn(2, '/');
        let _ = it.next();
        it.next().unwrap_or("").to_string()
    };
    let p = ui.painter();
    if let Some((glyph, color)) = crate::file_icons::icon_for(std::path::Path::new(&f.rel)) {
        p.text(egui::pos2(rect.left() + 35.0, cy), Align2::LEFT_CENTER, glyph.to_string(),
            crate::file_icons::seti_font(15.0), color);
    } else {
        p.text(egui::pos2(rect.left() + 34.0, cy), Align2::LEFT_CENTER, icons::FILE.to_string(),
            codicon_font(14.0), Palette::FG_DESCRIPTION);
    }
    let np = egui::pos2(rect.left() + 54.0, cy);
    let g = p.layout_no_wrap(name.to_string(), FontId::proportional(13.0), Palette::FG);
    let nw = g.size().x;
    p.galley(egui::pos2(np.x, cy - g.size().y / 2.0), g, Palette::FG);
    if f.kind.strikethrough() {
        p.line_segment([egui::pos2(np.x, cy), egui::pos2(np.x + nw, cy)],
            Stroke::new(1.0, Palette::FG));
    }
    if !parent.is_empty() {
        p.text(egui::pos2(np.x + nw + 6.0, cy), Align2::LEFT_CENTER, &parent,
            FontId::proportional(11.5), Palette::FG_DESCRIPTION);
    }
    p.text(egui::pos2(rect.right() - 14.0, cy), Align2::RIGHT_CENTER, f.kind.badge(),
        FontId::proportional(12.0), f.kind.decoration_color());
    resp.clicked()
}

fn lane_color(idx: usize) -> egui::Color32 {
    // Sentinels ≥ 100 are reference colours (blue/purple/orange); the backbone
    // lane carrying a branch ref uses these instead of the rotating palette.
    match idx {
        crate::git::history::REF_COLOR_LOCAL => Palette::SCM_REF_LOCAL,
        crate::git::history::REF_COLOR_REMOTE => Palette::SCM_REF_REMOTE,
        crate::git::history::REF_COLOR_BASE => Palette::SCM_REF_BASE,
        _ => Palette::SCM_GRAPH_LANES[idx % Palette::SCM_GRAPH_LANES.len()],
    }
}

/// Render one commit row; returns `true` when clicked (toggle file list).
fn graph_row(ui: &mut Ui, row: &GraphRow) -> bool {
    let lanes = row.input.len().max(row.output.len()).max(1) + 1;
    let graph_w = lanes as f32 * LANE_W;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), GRAPH_ROW_H), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 0.0, Palette::LIST_HOVER_BG);
    }
    let p = ui.painter();
    let top = rect.top();
    let mid = rect.center().y;
    let bottom = rect.bottom();
    let x = |i: usize| rect.left() + LANE_W * (i as f32 + 1.0);
    let cx = x(row.circle_lane);

    // Swimlane strokes are 1px in VS Code (scmHistory.ts createPath default).
    // Input lanes: top → mid (route the commit's own lane into the circle).
    for (i, lane) in row.input.iter().enumerate() {
        let col = lane_color(lane.color);
        if lane.id == row.commit.id {
            p.line_segment([egui::pos2(x(i), top), egui::pos2(cx, mid)], Stroke::new(1.0, col));
        } else {
            p.line_segment([egui::pos2(x(i), top), egui::pos2(x(i), mid)], Stroke::new(1.0, col));
        }
    }
    // Output lanes: mid → bottom (new/merge lanes fan out from the circle).
    for (j, lane) in row.output.iter().enumerate() {
        let col = lane_color(lane.color);
        let carried = row.input.iter().enumerate().any(|(i, l)| l.id == lane.id && i == j && l.id != row.commit.id);
        let from = if carried { egui::pos2(x(j), mid) } else { egui::pos2(cx, mid) };
        p.line_segment([from, egui::pos2(x(j), bottom)], Stroke::new(1.0, col));
    }
    // The commit circle. Geometry from scmHistory.ts: CIRCLE_RADIUS = 4, each
    // circle carries a 2px stroke in the sidebar background so it reads clear
    // of the lanes. A plain node is a filled disc (r ≈ 5); HEAD is a ring
    // (filled disc with the centre punched back to the sidebar colour).
    let ccol = lane_color(row.circle_color);
    let halo = Palette::SIDEBAR_BG;
    let is_head = row.commit.refs.iter().any(|r| r.kind == RefKind::Head);
    let center = egui::pos2(cx, mid);
    if is_head {
        p.circle_filled(center, CIRCLE_R + 3.0, halo); // 2px sidebar ring
        p.circle_filled(center, CIRCLE_R + 2.0, ccol); // outer colour
        p.circle_filled(center, CIRCLE_R - 2.0, halo); // punched centre → ring
    } else {
        p.circle_filled(center, CIRCLE_R + 2.0, halo); // sidebar separation
        p.circle_filled(center, CIRCLE_R + 1.0, ccol); // r ≈ 5 colour disc
    }

    // Right of the graph: ref badges + summary + author.
    let mut tx = rect.left() + graph_w + 8.0;
    for rf in &row.commit.refs {
        let col = match rf.kind {
            RefKind::Head | RefKind::Local => Palette::SCM_REF_LOCAL,
            RefKind::Remote => Palette::SCM_REF_REMOTE,
            RefKind::Tag => Palette::SCM_REF_BASE,
        };
        // The ref glyph must be drawn with the codicon font; a tag uses the
        // tag glyph, branches/remotes the git-branch fork.
        // Ref label geometry from scm.css: line-height 18, border-radius 10,
        // font-size 12, git-branch codicon 12px. (.label-container gap 4px.)
        let glyph = if rf.kind == RefKind::Tag { icons::TAG } else { icons::GIT_BRANCH };
        let icon_font = codicon_font(12.0);
        let name_font = FontId::proportional(12.0);
        let icon_w = p.layout_no_wrap(glyph.to_string(), icon_font.clone(), col).size().x;
        let name_g = p.layout_no_wrap(rf.name.clone(), name_font.clone(), col);
        let pill_h = 18.0;
        let w = 6.0 + icon_w + 3.0 + name_g.size().x + 6.0;
        let pill = egui::Rect::from_min_size(egui::pos2(tx, mid - pill_h / 2.0), egui::vec2(w, pill_h));
        p.rect_filled(pill, egui::CornerRadius::same(9), with_alpha(col, 0.18));
        p.text(egui::pos2(tx + 6.0, mid), Align2::LEFT_CENTER, glyph.to_string(), icon_font, col);
        p.text(egui::pos2(tx + 6.0 + icon_w + 3.0, mid), Align2::LEFT_CENTER, &rf.name, name_font, col);
        tx += w + 4.0;
    }
    // Author, right-aligned; reserve its width so the subject can be clipped
    // before it collides with it.
    let author_font = FontId::proportional(11.0);
    let author_w = p
        .layout_no_wrap(row.commit.author.clone(), author_font.clone(), Palette::FG_DESCRIPTION)
        .size()
        .x;
    p.text(egui::pos2(rect.right() - 10.0, mid), Align2::RIGHT_CENTER, &row.commit.author,
        author_font, Palette::FG_DESCRIPTION);
    // Subject, single line, ellipsised to the space left of the author column.
    let avail = (rect.right() - 10.0 - author_w - 12.0 - tx).max(20.0);
    let mut job = egui::text::LayoutJob::single_section(
        row.commit.summary.clone(),
        egui::text::TextFormat::simple(FontId::proportional(12.5), Palette::FG),
    );
    job.wrap = egui::text::TextWrapping {
        max_width: avail,
        max_rows: 1,
        overflow_character: Some('…'),
        ..Default::default()
    };
    let galley = p.layout_job(job);
    p.galley(egui::pos2(tx, mid - galley.size().y * 0.5), galley, Palette::FG);

    resp.clicked()
}

fn with_alpha(c: egui::Color32, a: f32) -> egui::Color32 {
    let [r, g, b, al] = c.to_array();
    let al = (al as f32 * a) as u8;
    egui::Color32::from_rgba_premultiplied(
        ((r as u16 * al as u16) / 255) as u8,
        ((g as u16 * al as u16) / 255) as u8,
        ((b as u16 * al as u16) / 255) as u8,
        al,
    )
}
