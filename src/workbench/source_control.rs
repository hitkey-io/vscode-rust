//! Source Control view — VS Code's SCM view container: a "CHANGES" pane
//! (commit input, Commit button, resource groups) and a "GRAPH" pane (the
//! commit history), stacked like VS Code's pane view with a draggable sash.
//!
//! VS Code analogues:
//! - `src/vs/workbench/contrib/scm/browser/scmViewPane.ts` + `media/scm.css`
//!   (22px rows, `.scm-input` / `.scm-editor`, resource groups with count
//!   badges, inline actions on hover, `SCMTreeSorter` path order)
//! - `src/vs/workbench/contrib/scm/browser/scmHistoryViewPane.ts` +
//!   `scmHistory.ts` (swimlanes 11×22px, circles, reference badges)
//! - `src/vs/base/browser/ui/splitview/paneview.css` (22px pane headers,
//!   11px bold uppercase titles, actions shown while the pane is hovered)
//! - `extensions/git/package.json` (`scm/title`, `scm/resourceGroup/context`,
//!   `scm/resourceState/context`, `scm/history/title` menus and icons)
//! - `extensions/git/package.json` `viewsWelcome` (no repository texts)

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use egui::{Align2, Color32, FontId, Key, Pos2, Rect, Sense, Stroke, Ui};

use crate::git::{ChangeKind, CommitMode, FileChange, GraphRow, Model, NodeKind, Repository, ResourceGroupKind};
use crate::icons::{codicon_by_name, codicon_font};
use crate::theme::Palette;

/// Height of a pane header and of every list row (`--vscode-*` row height).
const ROW_H: f32 = 22.0;
/// Right padding of tree rows (`.scm-view .monaco-tl-contents > div`).
const PAD_R: f32 = 12.0;

/// Caller-owned UI state for the SCM view.
pub struct ScmUiState {
    /// The window isn't trusted: no provider runs (Restricted Mode).
    pub restricted: bool,
    pub commit_messages: HashMap<PathBuf, String>,
    /// Repositories expanded in a multi-repository workspace.
    pub expanded: HashSet<PathBuf>,
    pub changes_open: bool,
    pub graph_open: bool,
    /// Panes shown at all ("Views" in the title's "…" menu).
    pub changes_visible: bool,
    pub graph_visible: bool,
    /// Collapsed resource groups: (repository, group).
    pub collapsed_groups: HashSet<(PathBuf, ResourceGroupKind)>,
    /// Commit ids whose changed-file list is expanded in the GRAPH.
    pub expanded_commits: HashSet<String>,
    /// Share of the height the CHANGES pane gets when both are open.
    pub split: f32,
    /// Commit files as a folder tree ("View as Tree") instead of a list.
    pub graph_tree: bool,
    /// Scroll the graph to the HEAD row on the next frame.
    pub reveal_head: bool,
    /// Which refs the GRAPH follows (shown on its picker).
    pub graph_filter: GraphFilter,
    /// A folder is open (the welcome text differs without one).
    pub folder_open: bool,
    /// Git repositories in parent folders of the open folder.
    pub parent_repos: Vec<PathBuf>,
    /// Focus the commit input on the next frame.
    pub focus_input: bool,
    /// The selected change: (repository, path, staged).
    pub selected: Option<(PathBuf, String, bool)>,
    /// The list has the keyboard focus (selection drawn as active).
    pub list_focused: bool,
    /// The active editor's file (`scm.autoReveal` selects its row).
    pub active_path: Option<PathBuf>,
    revealed_path: Option<PathBuf>,
    initialized: bool,
}

impl Default for ScmUiState {
    fn default() -> Self {
        Self {
            restricted: false,
            commit_messages: HashMap::new(),
            expanded: HashSet::new(),
            changes_open: true,
            graph_open: true,
            changes_visible: true,
            graph_visible: true,
            collapsed_groups: HashSet::new(),
            expanded_commits: HashSet::new(),
            split: 0.5,
            graph_tree: false,
            reveal_head: false,
            graph_filter: GraphFilter::Auto,
            folder_open: true,
            parent_repos: Vec::new(),
            focus_input: false,
            selected: None,
            list_focused: false,
            active_path: None,
            revealed_path: None,
            initialized: false,
        }
    }
}

/// The GRAPH's history item reference picker ("Auto" by default).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GraphFilter {
    /// The current branch, its upstream and base (VS Code's "Auto").
    #[default]
    Auto,
    /// Every reference.
    All,
    /// One reference (`refs/heads/x`, `refs/remotes/o/x`, `refs/tags/t`).
    Ref(String),
}

impl GraphFilter {
    pub fn label(&self) -> String {
        match self {
            GraphFilter::Auto => "Auto".into(),
            GraphFilter::All => "All".into(),
            GraphFilter::Ref(r) => r
                .trim_start_matches("refs/heads/")
                .trim_start_matches("refs/remotes/")
                .trim_start_matches("refs/tags/")
                .to_string(),
        }
    }
}

/// What a click in the view asks for (run by the app).
#[derive(Clone, Debug, PartialEq)]
pub enum ScmAction {
    /// Restricted Mode's "Manage Workspace Trust" button.
    ManageWorkspaceTrust,
    /// Open the diff of a changed file (`git.openChange`).
    /// One click opens a preview editor, a double click (`pinned`) keeps it.
    OpenChange { root: PathBuf, rel: String, staged: bool, kind: ChangeKind, pinned: bool },
    /// Open the file itself (`git.openFile`).
    OpenFile { root: PathBuf, rel: String },
    /// Open the file as it is in HEAD (`git.openHEADFile`).
    OpenHeadFile { root: PathBuf, rel: String },
    /// Open every change of a group (`git.viewChanges` / `viewStagedChanges`).
    OpenGroupChanges { root: PathBuf, kind: ResourceGroupKind },
    Stage { root: PathBuf, rel: String },
    Unstage { root: PathBuf, rel: String },
    /// `git.clean` (asks first).
    Discard { root: PathBuf, rel: String, untracked: bool },
    StageAll(PathBuf),
    UnstageAll(PathBuf),
    /// `git.cleanAll` (asks first).
    DiscardAll(PathBuf),
    /// `git.ignore`.
    Ignore { root: PathBuf, rel: String },
    RevealInFinder(PathBuf),
    RevealInExplorer(PathBuf),
    /// `git.commit` and its variants.
    Commit { root: PathBuf, mode: CommitMode, scope: CommitScope, amend: bool, signoff: bool },
    UndoCommit(PathBuf),
    Refresh(PathBuf),
    Pull { root: PathBuf, rebase: bool },
    Push { root: PathBuf },
    Sync { root: PathBuf, rebase: bool },
    Fetch { root: PathBuf, prune: bool, all: bool },
    Publish(PathBuf),
    /// Pickers and prompts (the app opens Quick Access).
    Checkout(PathBuf),
    CreateBranch { root: PathBuf, from: Option<String> },
    CreateBranchFromPick(PathBuf),
    RenameBranch(PathBuf),
    DeleteBranch(PathBuf),
    Merge(PathBuf),
    Rebase(PathBuf),
    CreateTag { root: PathBuf, rev: Option<String> },
    DeleteTag(PathBuf),
    PushTags(PathBuf),
    Stash { root: PathBuf, untracked: bool, staged: bool },
    StashApplyLatest(PathBuf),
    StashPopLatest(PathBuf),
    StashApply(PathBuf),
    StashPop(PathBuf),
    StashDrop(PathBuf),
    StashDropAll(PathBuf),
    AddRemote(PathBuf),
    RemoveRemote(PathBuf),
    /// History items.
    CheckoutDetached { root: PathBuf, id: String },
    CherryPick { root: PathBuf, id: String },
    CopyText(String),
    OpenCommitDiff { root: PathBuf, rel: String, parent: String, commit: String, pinned: bool },
    GraphFilterPick(PathBuf),
    GraphViewTree(bool),
    /// Show or hide a pane (`true` = CHANGES, `false` = GRAPH).
    TogglePane(bool),
    GraphRefresh(PathBuf),
    /// Welcome view buttons.
    InitRepository,
    OpenParentRepositories,
    OpenFolder,
    CloneRepository,
}

/// Which changes a Commit takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitScope {
    /// Staged changes, or all of them when nothing is staged (smart commit).
    Default,
    Staged,
    All,
}

/// A context or "…" menu the view asks for, opened natively by the app.
#[derive(Clone, Debug, PartialEq)]
pub enum ScmMenu {
    /// "…" of the CHANGES pane (`scm/title`).
    ChangesTitle(PathBuf),
    /// The Commit button's dropdown.
    CommitDropdown(PathBuf),
    /// Right-click on a resource group.
    Group { root: PathBuf, kind: ResourceGroupKind },
    /// Right-click on a changed file.
    File { root: PathBuf, rel: String, kind: ResourceGroupKind, untracked: bool },
    /// Right-click on a commit in the GRAPH.
    Commit { root: PathBuf, id: String, summary: String, parents: Vec<String> },
    /// "…" of the GRAPH pane.
    GraphTitle(PathBuf),
    /// "…" of the view title: which panes are open.
    Views,
}

/// Domain events emitted by the SCM view (dispatched in `app.rs`).
#[derive(Default)]
pub struct ScmOutput {
    pub actions: Vec<ScmAction>,
    /// `(menu, where)` — a native menu to open (window coordinates).
    pub menu: Option<(ScmMenu, Pos2)>,
}

impl ScmOutput {
    fn act(&mut self, a: ScmAction) {
        self.actions.push(a);
    }
}

pub fn show(
    ui: &mut Ui,
    model: &Model,
    history: &[GraphRow],
    graph_root: Option<&Path>,
    st: &mut ScmUiState,
) -> ScmOutput {
    let mut out = ScmOutput::default();
    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

    let title = crate::workbench::sidebar::title_bar(ui, "SOURCE CONTROL");
    if let Some(pos) = header_action(ui, "scm_title_more", egui::pos2(title.right() - 23.0, title.center().y), "ellipsis", "Views and More Actions...") {
        out.menu = Some((ScmMenu::Views, pos));
    }

    if model.repos.is_empty() || st.restricted {
        welcome(ui, st, &mut out);
        return out;
    }

    if !st.initialized {
        for r in &model.repos {
            st.expanded.insert(r.root.clone());
        }
        st.initialized = true;
    }
    // `scm.autoReveal`: the active editor's file is selected in the list.
    if st.active_path != st.revealed_path {
        st.revealed_path = st.active_path.clone();
        if let Some(path) = &st.active_path {
            let found = model.repos.iter().find_map(|r| {
                let rel = path.strip_prefix(&r.root).ok()?.to_string_lossy().into_owned();
                let staged = r.working.iter().all(|f| f.rel != rel) && r.index.iter().any(|f| f.rel == rel);
                (r.working.iter().chain(r.index.iter()).any(|f| f.rel == rel)).then(|| (r.root.clone(), rel, staged))
            });
            if found.is_some() {
                st.selected = found;
            }
        }
    }
    // A click anywhere else takes the focus from the list.
    if ui.input(|i| i.pointer.any_pressed()) && !ui.rect_contains_pointer(ui.max_rect()) {
        st.list_focused = false;
    }

    let area = ui.available_rect_before_wrap();
    ui.allocate_rect(area, Sense::hover());
    let total = area.height();
    let headers = st.changes_visible as u8 + st.graph_visible as u8;
    let body_h = (total - headers as f32 * ROW_H).max(0.0);
    let changes_open = st.changes_open && st.changes_visible;
    let graph_open = st.graph_open && st.graph_visible;
    let (changes_h, graph_h) = match (changes_open, graph_open) {
        (true, true) => {
            let c = (body_h * st.split).clamp(ROW_H.min(body_h), (body_h - ROW_H).max(0.0));
            (c, body_h - c)
        }
        (true, false) => (body_h, 0.0),
        (false, true) => (0.0, body_h),
        (false, false) => (0.0, 0.0),
    };
    let changes_header = Rect::from_min_size(area.min, egui::vec2(area.width(), if st.changes_visible { ROW_H } else { 0.0 }));
    let changes_body = Rect::from_min_size(changes_header.left_bottom(), egui::vec2(area.width(), changes_h));
    // A collapsed GRAPH pane keeps its header at the bottom, as in VS Code.
    let graph_top = if graph_open || !st.changes_visible { changes_body.bottom() } else { area.bottom() - ROW_H };
    let graph_header = Rect::from_min_size(egui::pos2(area.left(), graph_top), egui::vec2(area.width(), ROW_H));
    let graph_body = Rect::from_min_size(graph_header.left_bottom(), egui::vec2(area.width(), graph_h));

    let active = graph_root
        .and_then(|r| model.repos.iter().find(|x| x.root == r))
        .or(model.repos.first());
    let pointer = ui.ctx().pointer_hover_pos();
    let changes_hovered = pointer.is_some_and(|p| changes_header.union(changes_body).contains(p));
    let graph_hovered = pointer.is_some_and(|p| graph_header.union(graph_body).contains(p));

    // CHANGES pane.
    if st.changes_visible && pane_header(ui, changes_header, "CHANGES", st.changes_open, false) {
        st.changes_open = !st.changes_open;
    }
    if changes_hovered && st.changes_visible {
        if let Some(repo) = active {
            let actions = [("ellipsis", "More Actions..."), ("refresh", "Refresh"), ("check", "Commit")];
            for (i, (name, tip)) in actions.iter().enumerate() {
                let c = egui::pos2(changes_header.right() - 23.0 - 22.0 * i as f32, changes_header.center().y);
                if let Some(pos) = header_action(ui, ("scm_changes_act", i), c, name, tip) {
                    match *name {
                        "check" => out.act(ScmAction::Commit {
                            root: repo.root.clone(),
                            mode: CommitMode::Plain,
                            scope: CommitScope::Default,
                            amend: false,
                            signoff: false,
                        }),
                        "refresh" => out.act(ScmAction::Refresh(repo.root.clone())),
                        _ => out.menu = Some((ScmMenu::ChangesTitle(repo.root.clone()), pos)),
                    }
                }
            }
        }
    }
    if changes_open && changes_h > 0.0 {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(changes_body).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(changes_body);
        child.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        egui::ScrollArea::vertical()
            .id_salt("scm_changes_scroll")
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                ui.style_mut().spacing.scroll.floating = true;
                let single = model.repos.len() == 1;
                for repo in &model.repos {
                    if !single && !repo_row(ui, repo, st, &mut out) {
                        continue;
                    }
                    let indent = if single { 0.0 } else { 8.0 };
                    commit_input(ui, repo, st, &mut out, indent);
                    commit_button(ui, repo, &mut out, indent);
                    ui.add_space(4.5);
                    for (kind, files) in repo.groups() {
                        group(ui, repo, kind, files, st, &mut out, indent);
                    }
                }
                ui.add_space(4.0);
            });
    }

    // GRAPH pane, with the sash above it.
    if changes_open && graph_open {
        let sash = Rect::from_center_size(graph_header.center_top(), egui::vec2(area.width(), 4.0));
        let resp = ui.interact(sash, ui.id().with("scm_sash"), Sense::drag());
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if resp.dragged() && body_h > 0.0 {
            st.split = ((changes_h + resp.drag_delta().y) / body_h).clamp(0.1, 0.9);
        }
    }
    if !st.graph_visible {
        return out;
    }
    if pane_header(ui, graph_header, "GRAPH", st.graph_open, st.changes_visible) {
        st.graph_open = !st.graph_open;
    }
    if let (true, Some(root)) = (graph_hovered, graph_root) {
        graph_header_actions(ui, graph_header, root, active, st, &mut out);
    }
    if graph_open && graph_h > 0.0 {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(graph_body).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(graph_body);
        child.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        let mut scroll = egui::ScrollArea::vertical().id_salt("scm_graph_scroll").auto_shrink([false, false]);
        if st.reveal_head {
            st.reveal_head = false;
            if let Some(i) = history.iter().position(|r| r.kind == NodeKind::Head) {
                scroll = scroll.vertical_scroll_offset((i as f32 * ROW_H - ROW_H).max(0.0));
            }
        }
        scroll.show(&mut child, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.style_mut().spacing.scroll.floating = true;
            graph_rows(ui, history, graph_root, st, &mut out);
        });
    }
    out
}

// ── Panes ──────────────────────────────────────────────────────────────────

/// A pane header (`paneview.css`): chevron, 11px bold uppercase title, a
/// top border between panes. Returns `true` when clicked (toggle).
fn pane_header(ui: &mut Ui, rect: Rect, title: &str, open: bool, border: bool) -> bool {
    let resp = ui.interact(rect, ui.id().with(("pane_header", title)), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));
    let p = ui.painter();
    p.rect_filled(rect, 0.0, Palette::VSCE_SIDEBAR_SECTION_HEADER_BG());
    if border {
        p.hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, Palette::VSCE_SIDEBAR_SECTION_HEADER_BORDER()));
    }
    let cy = rect.center().y;
    let chev = if open { "chevron-down" } else { "chevron-right" };
    paint_codicon(p, egui::pos2(rect.left() + 10.0, cy), chev, 16.0, Palette::VSCE_SIDEBAR_SECTION_HEADER_FG());
    p.text(
        egui::pos2(rect.left() + 20.0, cy),
        Align2::LEFT_CENTER,
        title,
        crate::ui_font::bold(11.0),
        Palette::VSCE_SIDEBAR_SECTION_HEADER_FG(),
    );
    resp.clicked()
}

/// A 22px icon action in a pane header. Returns where its menu would open
/// when clicked.
fn header_action(ui: &mut Ui, id: impl std::hash::Hash, center: Pos2, icon: &str, tip: &str) -> Option<Pos2> {
    let r = Rect::from_center_size(center, egui::vec2(22.0, 22.0));
    let resp = ui.interact(r, ui.id().with(id), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    if resp.hovered() {
        ui.painter().rect_filled(r.shrink(1.0), egui::CornerRadius::same(5), Palette::VSCE_TOOLBAR_HOVER_BG());
    }
    paint_codicon(ui.painter(), center, icon, 16.0, Palette::VSCE_ICON_2026_FG());
    let clicked = resp.clicked();
    resp.on_hover_text(tip);
    clicked.then(|| r.left_bottom())
}

/// GRAPH header actions, right to left: "…", refresh, publish/push, pull,
/// fetch, go to current, then the reference picker ("⑂ Auto").
fn graph_header_actions(ui: &mut Ui, rect: Rect, root: &Path, repo: Option<&Repository>, st: &mut ScmUiState, out: &mut ScmOutput) {
    let has_upstream = repo.is_some_and(|r| r.has_upstream);
    let root = root.to_path_buf();
    let mut actions: Vec<(&str, &str, ScmAction)> = vec![
        ("ellipsis", "More Actions...", ScmAction::GraphRefresh(root.clone())),
        ("refresh", "Refresh", ScmAction::GraphRefresh(root.clone())),
    ];
    if has_upstream {
        actions.push(("repo-push", "Push", ScmAction::Push { root: root.clone() }));
    } else {
        actions.push(("cloud-upload", "Publish Branch...", ScmAction::Publish(root.clone())));
    }
    actions.push(("repo-pull", "Pull", ScmAction::Pull { root: root.clone(), rebase: false }));
    actions.push(("repo-fetch", "Fetch From All Remotes", ScmAction::Fetch { root: root.clone(), prune: false, all: true }));
    let mut x = rect.right() - 23.0;
    for (i, (icon, tip, action)) in actions.into_iter().enumerate() {
        if let Some(pos) = header_action(ui, ("scm_graph_act", i), egui::pos2(x, rect.center().y), icon, tip) {
            if i == 0 {
                out.menu = Some((ScmMenu::GraphTitle(root.clone()), pos));
            } else {
                out.act(action);
            }
        }
        x -= 22.0;
    }
    if header_action(ui, "scm_graph_target", egui::pos2(x, rect.center().y), "target", "Go to Current History Item").is_some() {
        st.reveal_head = true;
    }
    x -= 11.0 + 4.0;
    // The reference picker: git-branch icon + its label.
    let label = st.graph_filter.label();
    let font = FontId::proportional(13.0);
    let w = ui.painter().layout_no_wrap(label.clone(), font.clone(), Palette::FG()).size().x + 16.0 + 6.0;
    let r = Rect::from_min_max(egui::pos2(x - w, rect.top() + 2.0), egui::pos2(x, rect.bottom() - 2.0));
    let resp = ui.interact(r, ui.id().with("scm_graph_ref_picker"), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, egui::CornerRadius::same(5), Palette::VSCE_TOOLBAR_HOVER_BG());
    }
    paint_codicon(ui.painter(), egui::pos2(r.left() + 3.0 + 8.0, rect.center().y), "git-branch", 16.0, Palette::VSCE_ICON_2026_FG());
    ui.painter().text(egui::pos2(r.left() + 3.0 + 16.0 + 1.0, rect.center().y), Align2::LEFT_CENTER, &label, font, Palette::VSCE_ICON_2026_FG());
    if resp.on_hover_text("History Item Reference Picker").clicked() {
        out.act(ScmAction::GraphFilterPick(root));
    }
}

// ── Welcome ────────────────────────────────────────────────────────────────

/// `viewsWelcome` of the git extension: what the view says without a
/// repository.
fn welcome(ui: &mut Ui, st: &ScmUiState, out: &mut ScmOutput) {
    let (texts, button, after): (&[&str], (&str, ScmAction), &str) = if st.restricted {
        // The SCM view's own welcome in Restricted Mode (scm.contribution.ts).
        (&["None of the registered source control providers work in Restricted Mode."], ("Manage Workspace Trust", ScmAction::ManageWorkspaceTrust), "")
    } else if !st.parent_repos.is_empty() {
        (
            &["A Git repository was found in the parent folders of the workspace or the open file(s)."],
            ("Open Repository", ScmAction::OpenParentRepositories),
            "Use the git.openRepositoryInParentFolders setting to control whether Git repositories in parent folders of workspaces or open files are opened. To learn more read our docs.",
        )
    } else if st.folder_open {
        (
            &["The folder currently open doesn't have a Git repository. You can initialize a repository which will enable source control features powered by Git."],
            ("Initialize Repository", ScmAction::InitRepository),
            "To learn more about how to use Git and source control in VS Code read our docs.",
        )
    } else {
        (
            &["In order to use Git features, you can open a folder containing a Git repository or clone from a URL."],
            ("Open Folder", ScmAction::OpenFolder),
            "To learn more about how to use Git and source control in VS Code read our docs.",
        )
    };
    // Same layout as every view welcome (run_debug's paragraph / button):
    // galleys sized to the 20px-inset width, so nothing widens the side bar.
    use super::run_debug::paragraph;
    let id = ui.id().with("scm-welcome");
    ui.add_space(13.0);
    for (i, t) in texts.iter().enumerate() {
        paragraph(ui, t, id.with(i));
    }
    ui.add_space(13.0);
    if super::run_debug::button(ui, button.0) {
        out.act(button.1.clone());
    }
    ui.add_space(13.0);
    if !st.folder_open && !st.restricted {
        if super::run_debug::button(ui, "Clone Repository") {
            out.act(ScmAction::CloneRepository);
        }
        ui.add_space(13.0);
    }
    if !after.is_empty() {
        paragraph(ui, after, id.with("after"));
    }
}

// ── CHANGES ────────────────────────────────────────────────────────────────

/// A repository row in a multi-repository workspace. Returns whether it is
/// expanded.
fn repo_row(ui: &mut Ui, repo: &Repository, st: &mut ScmUiState, out: &mut ScmOutput) -> bool {
    let expanded = st.expanded.contains(&repo.root);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 0.0, Palette::VSCE_LIST_HOVER_2026_BG());
    }
    let p = ui.painter();
    let cy = rect.center().y;
    paint_codicon(p, egui::pos2(rect.left() + 18.0, cy), if expanded { "chevron-down" } else { "chevron-right" }, 16.0, Palette::VSCE_ICON_2026_FG());
    paint_codicon(p, egui::pos2(rect.left() + 36.0, cy), "repo", 16.0, Palette::VSCE_ICON_2026_FG());
    let name = p.layout_no_wrap(repo.name.clone(), crate::ui_font::bold(13.0), Palette::FG());
    let nw = name.size().x;
    p.galley(egui::pos2(rect.left() + 46.0, cy - name.size().y / 2.0), name, Palette::FG());
    if let Some(b) = &repo.branch {
        let x = rect.left() + 46.0 + nw + 6.0;
        paint_codicon(p, egui::pos2(x + 8.0, cy), "git-branch", 16.0, Palette::VSCE_ICON_2026_FG());
        p.text(egui::pos2(x + 18.0, cy), Align2::LEFT_CENTER, b, FontId::proportional(13.0), Palette::FG_DESCRIPTION());
    }
    if resp.clicked() {
        if expanded {
            st.expanded.remove(&repo.root);
        } else {
            st.expanded.insert(repo.root.clone());
        }
    }
    if resp.secondary_clicked() {
        out.menu = Some((ScmMenu::ChangesTitle(repo.root.clone()), ui.ctx().pointer_interact_pos().unwrap_or(rect.center())));
    }
    st.expanded.contains(&repo.root)
}

/// The commit message box (`.scm-input` / `.scm-editor`): input background,
/// 1px input border (focus border when focused), 4px radius.
fn commit_input(ui: &mut Ui, repo: &Repository, st: &mut ScmUiState, out: &mut ScmOutput, indent: f32) {
    let branch = repo.branch.clone().unwrap_or_else(|| "HEAD".into());
    let placeholder = format!("Message (⌘Enter to commit on \"{branch}\")");
    ui.add_space(4.0);
    let (row, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), Sense::hover());
    let rect = Rect::from_min_max(egui::pos2(row.left() + 19.0 + indent, row.top()), egui::pos2(row.right() - PAD_R, row.bottom()));
    ui.painter().rect_filled(rect, egui::CornerRadius::same(4), Palette::VSCE_INPUT_2026_BG());
    let msg = st.commit_messages.entry(repo.root.clone()).or_default();
    let inner = Rect::from_min_max(egui::pos2(rect.left() + 7.0, rect.top() + 4.0), egui::pos2(rect.right() - 7.0, rect.bottom() - 4.0));
    let id = egui::Id::new(("scm_commit_input", &repo.root));
    // A child that doesn't move this Ui's cursor (scope_builder would).
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let resp = child.add(
        egui::TextEdit::singleline(msg)
            .id(id)
            .frame(egui::Frame::NONE)
            .desired_width(f32::INFINITY)
            .hint_text(egui::RichText::new(&placeholder).color(Palette::VSCE_INPUT_2026_PLACEHOLDER_FG()))
            .font(FontId::proportional(13.0))
            .text_color(Palette::FG()),
    );
    if st.focus_input {
        st.focus_input = false;
        resp.request_focus();
    }
    let border = if resp.has_focus() { Palette::FOCUS_BORDER() } else { Palette::INPUT_BORDER() };
    ui.painter().rect_stroke(rect, egui::CornerRadius::same(4), Stroke::new(1.0, border), egui::StrokeKind::Inside);
    if resp.has_focus() && ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter)) {
        out.act(ScmAction::Commit { root: repo.root.clone(), mode: CommitMode::Plain, scope: CommitScope::Default, amend: false, signoff: false });
    }
}

/// The Commit button (`scm.css .button-container`): a 25px split button,
/// "✓ Commit" centred, a divider, the dropdown chevron.
fn commit_button(ui: &mut Ui, repo: &Repository, out: &mut ScmOutput, indent: f32) {
    ui.add_space(9.0);
    let (row, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), Sense::hover());
    let rect = Rect::from_min_max(egui::pos2(row.left() + 19.0 + indent, row.top()), egui::pos2(row.right() - PAD_R, row.bottom()));
    let enabled = repo.total() > 0;
    let drop_w = 25.0;
    let main = Rect::from_min_max(rect.min, egui::pos2(rect.right() - drop_w, rect.bottom()));
    let drop = Rect::from_min_max(egui::pos2(main.right(), rect.top()), rect.max);
    let main_resp = ui.interact(main, egui::Id::new(("scm_commit_btn", &repo.root)), Sense::click());
    let drop_resp = ui.interact(drop, egui::Id::new(("scm_commit_drop", &repo.root)), Sense::click());
    main_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Commit"));
    let p = ui.painter();
    let alpha = if enabled { 1.0 } else { 0.4 };
    p.rect_filled(rect, egui::CornerRadius::same(4), with_alpha(Palette::BUTTON_BG(), alpha));
    if enabled && main_resp.hovered() {
        p.rect_filled(main, egui::CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 }, Palette::BUTTON_HOVER());
    }
    if enabled && drop_resp.hovered() {
        p.rect_filled(drop, egui::CornerRadius { nw: 0, sw: 0, ne: 4, se: 4 }, Palette::BUTTON_HOVER());
    }
    let fg = with_alpha(Color32::WHITE, if enabled { 1.0 } else { 0.6 });
    p.vline(drop.left(), drop.y_range().shrink(4.0), Stroke::new(1.0, with_alpha(Color32::WHITE, 0.4 * alpha)));
    let label = p.layout_no_wrap("Commit".into(), FontId::proportional(13.0), fg);
    let w = 16.0 + 4.0 + label.size().x;
    let x0 = main.center().x - w / 2.0;
    paint_codicon(p, egui::pos2(x0 + 8.0, main.center().y), "check", 16.0, fg);
    p.galley(egui::pos2(x0 + 20.0, main.center().y - label.size().y / 2.0), label, fg);
    paint_codicon(p, drop.center(), "chevron-down", 16.0, fg);
    if enabled && main_resp.clicked() {
        out.act(ScmAction::Commit { root: repo.root.clone(), mode: CommitMode::Plain, scope: CommitScope::Default, amend: false, signoff: false });
    }
    if enabled && drop_resp.clicked() {
        out.menu = Some((ScmMenu::CommitDropdown(repo.root.clone()), drop.left_bottom()));
    }
    let _ = main_resp.on_hover_text(format!("Commit Changes on \"{}\"", repo.branch.as_deref().unwrap_or("HEAD")));
}

/// `SCMTreeSorter` with the default "path" key: `comparePaths` — fewer
/// folders first, then folder by folder, each by `compareFileNames`.
pub fn sort_changes(files: &mut [&FileChange]) {
    files.sort_by(|a, b| compare_paths(&a.rel, &b.rel));
}

fn compare_paths(a: &str, b: &str) -> std::cmp::Ordering {
    let pa: Vec<&str> = a.split('/').collect();
    let pb: Vec<&str> = b.split('/').collect();
    for i in 0.. {
        let (end_a, end_b) = (i == pa.len() - 1, i == pb.len() - 1);
        match (end_a, end_b) {
            (true, true) => return compare_file_names(pa[i], pb[i]),
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }
        let c = compare_file_names(pa[i], pb[i]);
        if c != std::cmp::Ordering::Equal {
            return c;
        }
    }
    std::cmp::Ordering::Equal
}

/// `compareFileNames`: case-insensitive with numbers compared by value.
fn compare_file_names(a: &str, b: &str) -> std::cmp::Ordering {
    let key = |s: &str| -> Vec<(bool, String, u64)> {
        let mut out = Vec::new();
        let mut cur = String::new();
        let mut digits = false;
        for ch in s.chars() {
            if ch.is_ascii_digit() != digits && !cur.is_empty() {
                let n = if digits { cur.parse().unwrap_or(0) } else { 0 };
                out.push((digits, std::mem::take(&mut cur).to_lowercase(), n));
            }
            digits = ch.is_ascii_digit();
            cur.push(ch);
        }
        if !cur.is_empty() {
            let n = if digits { cur.parse().unwrap_or(0) } else { 0 };
            out.push((digits, cur.to_lowercase(), n));
        }
        out
    };
    let (ka, kb) = (key(a), key(b));
    for (x, y) in ka.iter().zip(kb.iter()) {
        let c = match (x.0, y.0) {
            (true, true) => x.2.cmp(&y.2),
            _ => x.1.cmp(&y.1),
        };
        if c != std::cmp::Ordering::Equal {
            return c;
        }
    }
    ka.len().cmp(&kb.len()).then_with(|| a.cmp(b))
}

/// A resource group: twistie, label, inline actions on hover, count badge,
/// then its files.
fn group(
    ui: &mut Ui,
    repo: &Repository,
    kind: ResourceGroupKind,
    files: &[FileChange],
    st: &mut ScmUiState,
    out: &mut ScmOutput,
    indent: f32,
) {
    let key = (repo.root.clone(), kind);
    let open = !st.collapsed_groups.contains(&key);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, kind.label()));
    let hovered = ui.rect_contains_pointer(rect);
    if hovered {
        ui.painter().rect_filled(rect, 0.0, Palette::VSCE_LIST_HOVER_2026_BG());
    }
    let cy = rect.center().y;
    let left = rect.left() + indent;
    paint_codicon(ui.painter(), egui::pos2(left + 18.5, cy), if open { "chevron-down" } else { "chevron-right" }, 16.0, Palette::VSCE_ICON_2026_FG());
    ui.painter().text(egui::pos2(left + 30.5, cy), Align2::LEFT_CENTER, kind.label(), FontId::proportional(13.0), Palette::FG());

    // Count badge (`CountBadge`): 18px round, badge colours.
    let count = files.len().to_string();
    let g = ui.painter().layout_no_wrap(count, FontId::proportional(11.0), Palette::VSCE_BADGE_2026_FG());
    let bw = (g.size().x + 12.0).max(18.0);
    let badge = Rect::from_center_size(egui::pos2(rect.right() - PAD_R - bw / 2.0, cy), egui::vec2(bw, 18.0));
    ui.painter().rect_filled(badge, egui::CornerRadius::same(9), Palette::VSCE_BADGE_2026_BG());
    ui.painter().galley(badge.center() - g.size() / 2.0, g, Palette::VSCE_BADGE_2026_FG());

    let mut acted = false;
    if hovered {
        let root = repo.root.clone();
        let actions: Vec<(&str, &str, ScmAction)> = match kind {
            ResourceGroupKind::Index => vec![
                ("remove", "Unstage All Changes", ScmAction::UnstageAll(root.clone())),
                ("diff-multiple", "Open Staged Changes", ScmAction::OpenGroupChanges { root, kind }),
            ],
            ResourceGroupKind::Merge => vec![("add", "Stage All Merge Changes", ScmAction::StageAll(root))],
            _ => vec![
                ("add", "Stage All Changes", ScmAction::StageAll(root.clone())),
                ("discard", "Discard All Changes", ScmAction::DiscardAll(root.clone())),
                ("diff-multiple", "Open Changes", ScmAction::OpenGroupChanges { root, kind }),
            ],
        };
        let mut x = badge.left() - 4.0 - 10.0;
        for (i, (icon, tip, action)) in actions.into_iter().enumerate() {
            if row_action(ui, ("grp_act", &repo.root, kind as u8, i), egui::pos2(x, cy), icon, tip) {
                out.act(action);
                acted = true;
            }
            x -= 20.0;
        }
    }
    if resp.clicked() && !acted {
        if open {
            st.collapsed_groups.insert(key.clone());
        } else {
            st.collapsed_groups.remove(&key);
        }
    }
    if resp.secondary_clicked() {
        out.menu = Some((ScmMenu::Group { root: repo.root.clone(), kind }, ui.ctx().pointer_interact_pos().unwrap_or(rect.center())));
    }
    if !open {
        return;
    }
    let mut sorted: Vec<&FileChange> = files.iter().collect();
    sort_changes(&mut sorted);
    for fc in sorted {
        file_row(ui, repo, kind, fc, st, out, indent);
    }
}

/// One changed file: Seti icon, name (struck through when deleted), its
/// folder, inline actions on hover, the status letter.
fn file_row(
    ui: &mut Ui,
    repo: &Repository,
    kind: ResourceGroupKind,
    fc: &FileChange,
    st: &mut ScmUiState,
    out: &mut ScmOutput,
    indent: f32,
) {
    let staged = matches!(kind, ResourceGroupKind::Index);
    let untracked = matches!(fc.kind, ChangeKind::Untracked);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("scm:{}", fc.rel)));
    let hovered = ui.rect_contains_pointer(rect);
    let selected = st.selected.as_ref().is_some_and(|(r, rel, s)| *r == repo.root && *rel == fc.rel && *s == staged);
    if selected {
        // list.activeSelectionBackground + focusOutline while the list has
        // the focus, list.inactiveSelectionBackground otherwise.
        if st.list_focused {
            ui.painter().rect_filled(rect, 0.0, Palette::VSCE_LIST_ACTIVE_SELECTION_2026_BG());
            ui.painter().rect_stroke(rect.shrink(0.5), 0.0, Stroke::new(1.0, Palette::VSCE_LIST_FOCUS_OUTLINE_2026()), egui::StrokeKind::Inside);
        } else {
            ui.painter().rect_filled(rect, 0.0, Palette::VSCE_LIST_INACTIVE_SELECTION_2026_BG());
        }
    } else if hovered {
        ui.painter().rect_filled(rect, 0.0, Palette::VSCE_LIST_HOVER_2026_BG());
    }
    let p = ui.painter();
    let cy = rect.center().y;
    let left = rect.left() + indent;
    let name = fc.rel.rsplit('/').next().unwrap_or(&fc.rel);
    let parent = fc.rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let icon_path = repo.root.join(&fc.rel);
    match crate::file_icons::icon_for(&icon_path) {
        Some((glyph, color)) => {
            p.text(egui::pos2(left + 25.5, cy), Align2::CENTER_CENTER, glyph.to_string(), crate::file_icons::seti_font(20.0), color);
        }
        None => paint_codicon(p, egui::pos2(left + 25.5, cy), "file", 16.0, Palette::VSCE_ICON_2026_FG()),
    }
    let letter_x = rect.right() - PAD_R - 8.0;
    let decoration = fc.kind.decoration_color();
    let name_g = p.layout_no_wrap(name.to_string(), FontId::proportional(13.0), Palette::FG());
    let name_w = name_g.size().x;
    let nx = left + 38.5;
    // Text stops before the inline actions / status letter.
    let text_clip = Rect::from_min_max(egui::pos2(rect.left(), rect.top()), egui::pos2(letter_x - 14.0, rect.bottom()));
    let tp = ui.painter_at(text_clip);
    tp.galley(egui::pos2(nx, cy - name_g.size().y / 2.0), name_g, Palette::FG());
    if fc.kind.strikethrough() {
        tp.line_segment([egui::pos2(nx, cy + 0.5), egui::pos2(nx + name_w, cy + 0.5)], Stroke::new(1.0, Palette::FG()));
    }
    if !parent.is_empty() {
        tp.text(egui::pos2(nx + name_w + 6.0, cy), Align2::LEFT_CENTER, parent, FontId::proportional(12.0), Palette::FG_DESCRIPTION());
    }
    let _ = decoration;
    p.text(egui::pos2(letter_x, cy), Align2::CENTER_CENTER, fc.kind.badge(), FontId::proportional(12.0), with_alpha(Palette::FG(), 0.75));

    let mut acted = false;
    if hovered {
        let root = repo.root.clone();
        let rel = fc.rel.clone();
        // Right to left: stage/unstage, discard, open file.
        let mut actions: Vec<(&str, &str, ScmAction)> = Vec::new();
        if staged {
            actions.push(("remove", "Unstage Changes", ScmAction::Unstage { root: root.clone(), rel: rel.clone() }));
        } else {
            actions.push(("add", "Stage Changes", ScmAction::Stage { root: root.clone(), rel: rel.clone() }));
            if kind != ResourceGroupKind::Merge {
                actions.push(("discard", "Discard Changes", ScmAction::Discard { root: root.clone(), rel: rel.clone(), untracked }));
            }
        }
        actions.push(("go-to-file", "Open File", ScmAction::OpenFile { root, rel }));
        let mut x = letter_x - 8.0 - 5.0 - 10.0;
        for (i, (icon, tip, action)) in actions.into_iter().enumerate() {
            if row_action(ui, ("file_act", &fc.rel, kind as u8, i), egui::pos2(x, cy), icon, tip) {
                out.act(action);
                acted = true;
            }
            x -= 20.0;
        }
    }
    if (resp.clicked() || resp.double_clicked()) && !acted {
        st.selected = Some((repo.root.clone(), fc.rel.clone(), staged));
        st.list_focused = true;
        out.act(ScmAction::OpenChange {
            root: repo.root.clone(),
            rel: fc.rel.clone(),
            staged,
            kind: fc.kind,
            pinned: resp.double_clicked(),
        });
    }
    if resp.secondary_clicked() {
        st.selected = Some((repo.root.clone(), fc.rel.clone(), staged));
        st.list_focused = true;
    }
    if resp.secondary_clicked() {
        out.menu = Some((
            ScmMenu::File { root: repo.root.clone(), rel: fc.rel.clone(), kind, untracked },
            ui.ctx().pointer_interact_pos().unwrap_or(rect.center()),
        ));
    }
    let tip = format!("{} • {}", repo.root.join(&fc.rel).display(), fc.kind_label());
    resp.on_hover_text(tip);
}

/// A 20px inline row action (`.actions .action-label { padding: 2px }`).
fn row_action(ui: &mut Ui, id: impl std::hash::Hash, center: Pos2, icon: &str, tip: &str) -> bool {
    let r = Rect::from_center_size(center, egui::vec2(20.0, 20.0));
    let resp = ui.interact(r, ui.id().with(id), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    if resp.hovered() {
        ui.painter().rect_filled(r, egui::CornerRadius::same(5), Palette::VSCE_TOOLBAR_HOVER_BG());
    }
    paint_codicon(ui.painter(), center, icon, 16.0, Palette::VSCE_ICON_2026_FG());
    let clicked = resp.clicked();
    resp.on_hover_text(tip);
    clicked
}

// ── GRAPH ──────────────────────────────────────────────────────────────────

/// `SWIMLANE_WIDTH` / `SWIMLANE_HEIGHT` / radii from scmHistory.ts.
const LANE_W: f32 = 11.0;
const CURVE_R: f32 = 5.0;
const CIRCLE_R: f32 = 4.0;

fn graph_rows(ui: &mut Ui, history: &[GraphRow], root: Option<&Path>, st: &mut ScmUiState, out: &mut ScmOutput) {
    for row in history {
        let open = st.expanded_commits.contains(&row.commit.id);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("commit:{}", row.commit.summary)));
        let hovered = ui.rect_contains_pointer(rect);
        let row_bg = if hovered {
            ui.painter().rect_filled(rect, 0.0, Palette::VSCE_LIST_HOVER_2026_BG());
            blend(Palette::SIDEBAR_BG(), Palette::VSCE_LIST_HOVER_2026_BG())
        } else {
            Palette::SIDEBAR_BG()
        };
        let graph_w = paint_graph(ui.painter(), rect, row, row_bg, open);
        paint_history_label(ui, rect, graph_w, row);
        if resp.clicked() {
            if open {
                st.expanded_commits.remove(&row.commit.id);
            } else {
                st.expanded_commits.insert(row.commit.id.clone());
            }
        }
        if let (true, Some(root)) = (resp.secondary_clicked(), root) {
            out.menu = Some((
                ScmMenu::Commit {
                    root: root.to_path_buf(),
                    id: row.commit.id.clone(),
                    summary: row.commit.summary.clone(),
                    parents: row.commit.parents.clone(),
                },
                ui.ctx().pointer_interact_pos().unwrap_or(rect.center()),
            ));
        }
        resp.on_hover_text(hover_text(row));
        if open {
            if let Some(root) = root {
                commit_files(ui, root, row, st.graph_tree, out);
            }
        }
    }
}

/// The hover card text of a commit.
fn hover_text(row: &GraphRow) -> String {
    let c = &row.commit;
    let when = c.time.map(relative_time).unwrap_or_default();
    let mut s = format!("{}", c.author);
    if !when.is_empty() {
        s.push_str(&format!(", {when}"));
    }
    s.push_str(&format!("\n\n{}\n\n{}", c.message.trim_end(), c.short_id));
    s
}

/// "5 minutes ago" (`fromNow` with full words).
pub fn relative_time(unix: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let s = (now - unix).max(0);
    let units = [(60, 1, "second"), (3600, 60, "minute"), (86400, 3600, "hour"), (604800, 86400, "day"), (2592000, 604800, "week"), (31536000, 2592000, "month"), (i64::MAX, 31536000, "year")];
    if s < 30 {
        return "now".into();
    }
    for (limit, size, name) in units {
        if s < limit {
            let v = ((s as f64) / size as f64).round() as i64;
            return format!("{v} {name}{} ago", if v == 1 { "" } else { "s" });
        }
    }
    String::new()
}

/// Files a commit changed, under it: the graph's lanes continue on the left
/// (`renderSCMHistoryGraphPlaceholder`), then the file.
fn commit_files(ui: &mut Ui, root: &Path, row: &GraphRow, tree: bool, out: &mut ScmOutput) {
    let files = crate::git::commit_changes(root, &row.commit);
    let parent = row.commit.parents.first().cloned().unwrap_or_default();
    let lanes = row.output.len().max(1);
    let graph_w = LANE_W * (lanes as f32 + 1.0);
    let mut entries: Vec<(usize, Option<&crate::git::CommitFile>, String)> = Vec::new();
    if tree {
        let mut last_dir = String::new();
        let mut sorted: Vec<&crate::git::CommitFile> = files.iter().collect();
        sorted.sort_by(|a, b| compare_paths(&a.rel, &b.rel));
        for f in sorted {
            let dir = f.rel.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
            if !dir.is_empty() && dir != last_dir {
                entries.push((0, None, dir.clone()));
            }
            last_dir = dir.clone();
            entries.push((if dir.is_empty() { 0 } else { 1 }, Some(f), String::new()));
        }
    } else {
        entries = files.iter().map(|f| (0, Some(f), String::new())).collect();
    }
    for (depth, file, dir) in entries {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::click());
        if ui.rect_contains_pointer(rect) {
            ui.painter().rect_filled(rect, 0.0, Palette::VSCE_LIST_HOVER_2026_BG());
        }
        let p = ui.painter();
        for (i, lane) in row.output.iter().enumerate() {
            let x = rect.left() + LANE_W * (i as f32 + 1.0);
            let w = if i == row.circle_index { 3.0 } else { 1.0 };
            p.vline(x, rect.y_range(), Stroke::new(w, lane_color(lane.color)));
        }
        let cy = rect.center().y;
        let left = rect.left() + graph_w + 8.0 * depth as f32;
        let Some(f) = file else {
            paint_codicon(p, egui::pos2(left + 8.0, cy), "folder", 16.0, Palette::VSCE_ICON_2026_FG());
            p.text(egui::pos2(left + 20.0, cy), Align2::LEFT_CENTER, &dir, FontId::proportional(13.0), Palette::FG());
            continue;
        };
        let name = f.rel.rsplit('/').next().unwrap_or(&f.rel);
        let folder = f.rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        match crate::file_icons::icon_for(Path::new(&f.rel)) {
            Some((glyph, color)) => {
                p.text(egui::pos2(left + 8.0, cy), Align2::CENTER_CENTER, glyph.to_string(), crate::file_icons::seti_font(20.0), color);
            }
            None => paint_codicon(p, egui::pos2(left + 8.0, cy), "file", 16.0, Palette::VSCE_ICON_2026_FG()),
        }
        let g = p.layout_no_wrap(name.to_string(), FontId::proportional(13.0), Palette::FG());
        let nw = g.size().x;
        let nx = left + 21.0;
        p.galley(egui::pos2(nx, cy - g.size().y / 2.0), g, Palette::FG());
        if f.kind.strikethrough() {
            p.line_segment([egui::pos2(nx, cy + 0.5), egui::pos2(nx + nw, cy + 0.5)], Stroke::new(1.0, Palette::FG()));
        }
        if !tree && !folder.is_empty() {
            p.text(egui::pos2(nx + nw + 6.0, cy), Align2::LEFT_CENTER, folder, FontId::proportional(12.0), Palette::FG_DESCRIPTION());
        }
        p.text(egui::pos2(rect.right() - PAD_R - 8.0, cy), Align2::CENTER_CENTER, f.kind.badge(), FontId::proportional(12.0), with_alpha(Palette::FG(), 0.75));
        if resp.clicked() || resp.double_clicked() {
            out.act(ScmAction::OpenCommitDiff {
                root: root.to_path_buf(),
                rel: f.rel.clone(),
                parent: parent.clone(),
                commit: row.commit.id.clone(),
                pinned: resp.double_clicked(),
            });
        }
    }
}

fn lane_color(c: usize) -> Color32 {
    match c {
        crate::git::history::REF_COLOR_LOCAL => Palette::SCM_REF_LOCAL(),
        crate::git::history::REF_COLOR_REMOTE => Palette::SCM_REF_REMOTE(),
        crate::git::history::REF_COLOR_BASE => Palette::SCM_REF_BASE(),
        _ => Palette::SCM_GRAPH_LANES()[c % Palette::SCM_GRAPH_LANES().len()],
    }
}

/// Points on a circle arc from `a0` to `a1` (radians, y down).
fn arc(center: Pos2, r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let steps = 10;
    (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / steps as f32;
            egui::pos2(center.x + r * a.cos(), center.y + r * a.sin())
        })
        .collect()
}

/// `renderSCMHistoryItemGraph`: lanes, curves and the commit's circle.
/// Returns the graph's width.
fn paint_graph(p: &egui::Painter, rect: Rect, row: &GraphRow, row_bg: Color32, open: bool) -> f32 {
    use std::f32::consts::FRAC_PI_2;
    let o = rect.min;
    let pt = |x: f32, y: f32| egui::pos2(o.x + x, o.y + y);
    let w = LANE_W;
    let h = ROW_H;
    let line = |pts: Vec<Pos2>, c: Color32, width: f32| p.add(egui::Shape::line(pts, Stroke::new(width, c)));
    let input = &row.input;
    let output = &row.output;
    let circle_index = row.circle_index;
    let circle_color = lane_color(row.circle_color);

    let mut out_idx = 0usize;
    for (index, lane) in input.iter().enumerate() {
        let color = lane_color(lane.color);
        let xi = w * (index as f32 + 1.0);
        if lane.id == row.commit.id {
            if index != circle_index {
                // "/" into the circle's lane, then "-".
                let mut pts = arc(pt(w * index as f32, 0.0), w, 0.0, FRAC_PI_2);
                pts.push(pt(w * (circle_index as f32 + 1.0), w));
                line(pts, color, 1.0);
            } else {
                out_idx += 1;
            }
        } else if out_idx < output.len() && lane.id == output[out_idx].id {
            if index == out_idx {
                line(vec![pt(xi, 0.0), pt(xi, h)], color, 1.0);
            } else {
                let xo = w * (out_idx as f32 + 1.0);
                let mut pts = vec![pt(xi, 0.0), pt(xi, 6.0)];
                pts.extend(arc(pt(xi - CURVE_R, 6.0), CURVE_R, 0.0, FRAC_PI_2));
                pts.push(pt(xo + CURVE_R, h / 2.0));
                pts.extend(arc(pt(xo + CURVE_R, h / 2.0 + CURVE_R), CURVE_R, -FRAC_PI_2, -std::f32::consts::PI));
                pts.push(pt(xo, h));
                line(pts, color, 1.0);
            }
            out_idx += 1;
        }
    }
    // Remaining parents: "-\" out of the circle.
    for parent in row.commit.parents.iter().skip(1) {
        let Some(pi) = output.iter().rposition(|l| &l.id == parent) else { continue };
        let color = lane_color(output[pi].color);
        let pts = arc(pt(w * pi as f32, h), w, -FRAC_PI_2, 0.0);
        line(pts, color, 1.0);
        line(vec![pt(w * pi as f32, h / 2.0), pt(w * (circle_index as f32 + 1.0), h / 2.0)], color, 1.0);
    }
    let cx = w * (circle_index as f32 + 1.0);
    let input_index = input.iter().position(|l| l.id == row.commit.id);
    if let Some(ii) = input_index {
        line(vec![pt(cx, 0.0), pt(cx, h / 2.0)], lane_color(input[ii].color), 1.0);
    }
    if !row.commit.parents.is_empty() {
        // An expanded item's own lane is drawn thicker (scm.css).
        line(vec![pt(cx, h / 2.0), pt(cx, h)], circle_color, if open { 3.0 } else { 1.0 });
    }
    let c = pt(cx, w);
    match row.kind {
        NodeKind::Head => {
            // r 7 (stroke 2 in the row colour), centre punched to the row.
            p.circle_filled(c, CIRCLE_R + 4.0, row_bg);
            p.circle_filled(c, CIRCLE_R + 2.0, circle_color);
            p.circle_filled(c, CIRCLE_R, row_bg);
        }
        NodeKind::Node if row.commit.parents.len() > 1 => {
            p.circle_filled(c, CIRCLE_R + 3.0, row_bg);
            p.circle_filled(c, CIRCLE_R + 1.0, circle_color);
            p.circle_filled(c, CIRCLE_R, row_bg);
            p.circle_filled(c, CIRCLE_R - 2.0, circle_color);
        }
        NodeKind::Node => {
            p.circle_filled(c, CIRCLE_R + 2.0, row_bg);
            p.circle_filled(c, CIRCLE_R, circle_color);
        }
    }
    w * (input.len().max(output.len()).max(1) as f32 + 1.0)
}

/// Subject (bold for HEAD), author as the description, then the reference
/// badges at the right end.
fn paint_history_label(ui: &Ui, rect: Rect, graph_w: f32, row: &GraphRow) {
    let p = ui.painter();
    let cy = rect.center().y;
    let right = rect.right() - PAD_R;

    // Badges, right to left.
    let mut bx = right;
    let mut badge_rects = Vec::new();
    for b in row.badges.iter().rev() {
        let bg = lane_color(b.color);
        let fg = Palette::SIDEBAR_BG();
        let desc = if b.show_description { Some(p.layout_no_wrap(b.name.clone(), FontId::proportional(12.0), fg)) } else { None };
        let icon_w = if b.icon == "git-branch" { 18.0 } else { 18.0 };
        let count = (b.count > 1).then(|| p.layout_no_wrap(b.count.to_string(), FontId::proportional(12.0), fg));
        let w = icon_w + count.as_ref().map(|g| g.size().x + 4.0).unwrap_or(0.0) + desc.as_ref().map(|g| g.size().x.min(100.0) + 4.0).unwrap_or(0.0);
        let r = Rect::from_min_max(egui::pos2(bx - w, cy - 9.0), egui::pos2(bx, cy + 9.0));
        badge_rects.push((r, bg, fg, count, desc, b.icon));
        bx -= w + 4.0;
    }
    for (r, bg, fg, count, desc, icon) in badge_rects {
        p.rect_filled(r, egui::CornerRadius::same(9), bg);
        let mut x = r.left();
        if let Some(g) = count {
            p.galley(egui::pos2(x + 4.0, cy - g.size().y / 2.0), g.clone(), fg);
            x += g.size().x + 4.0;
        }
        let size = if icon == "git-branch" { 12.0 } else { 16.0 };
        paint_codicon(p, egui::pos2(x + 9.0, cy), icon, size, fg);
        x += 18.0;
        if let Some(g) = desc {
            let clip = Rect::from_min_max(egui::pos2(x, r.top()), egui::pos2(x + 100.0, r.bottom()));
            p.with_clip_rect(clip.intersect(r)).galley(egui::pos2(x, cy - g.size().y / 2.0), g, fg);
        }
    }

    // Subject + author, ellipsised before the badges.
    let x = rect.left() + graph_w;
    let avail = (bx - 4.0 - x).max(10.0);
    let head = row.kind == NodeKind::Head;
    let subject_font = if head { crate::ui_font::bold(13.0) } else { FontId::proportional(13.0) };
    let mut job = egui::text::LayoutJob::default();
    job.append(&row.commit.summary, 0.0, egui::text::TextFormat::simple(subject_font, Palette::FG()));
    job.append(
        &row.commit.author,
        6.5,
        egui::text::TextFormat::simple(
            FontId::proportional(11.7),
            with_alpha(Palette::FG(), 0.7),
        ),
    );
    job.wrap = egui::text::TextWrapping { max_width: avail, max_rows: 1, overflow_character: Some('…'), ..Default::default() };
    let g = p.layout_job(job);
    p.galley(egui::pos2(x, cy - g.size().y / 2.0), g, Palette::FG());
}

// ── helpers ────────────────────────────────────────────────────────────────

fn paint_codicon(p: &egui::Painter, center: Pos2, name: &str, size: f32, color: Color32) {
    if let Some(glyph) = codicon_by_name(name) {
        p.text(center, Align2::CENTER_CENTER, glyph.to_string(), codicon_font(size), color);
    }
}

fn blend(base: Color32, over: Color32) -> Color32 {
    let a = over.a() as f32 / 255.0;
    let [br, bg, bb, _] = base.to_array();
    let [or, og, ob, _] = over.to_array();
    // `over` is premultiplied.
    let mix = |b: u8, o: u8| (o as f32 + b as f32 * (1.0 - a)).round().min(255.0) as u8;
    Color32::from_rgb(mix(br, or), mix(bg, og), mix(bb, ob))
}

fn with_alpha(c: Color32, a: f32) -> Color32 {
    let [r, g, b, al] = c.to_array();
    let al = (al as f32 * a) as u8;
    Color32::from_rgba_premultiplied(
        ((r as u16 * al as u16) / 255) as u8,
        ((g as u16 * al as u16) / 255) as u8,
        ((b as u16 * al as u16) / 255) as u8,
        al,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_sort_like_vs_code() {
        let mk = |rel: &str| FileChange { rel: rel.into(), ..FileChange::for_test(rel) };
        let files = [mk("src/main.rs"), mk("todo.md"), mk("gone.txt"), mk("file10.txt"), mk("file9.txt")];
        let mut v: Vec<&FileChange> = files.iter().collect();
        sort_changes(&mut v);
        let order: Vec<&str> = v.iter().map(|f| f.rel.as_str()).collect();
        assert_eq!(order, ["file9.txt", "file10.txt", "gone.txt", "todo.md", "src/main.rs"]);
    }

    #[test]
    fn relative_times_read_like_vs_code() {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        assert_eq!(relative_time(now - 5), "now");
        assert_eq!(relative_time(now - 120), "2 minutes ago");
        assert_eq!(relative_time(now - 3 * 86400), "3 days ago");
    }
}
