//! `git` CLI wrappers: working-tree status, current branch, and per-line
//! diff decorations for the editor gutter.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What kind of change a file (or line) carries. Mirrors the VS Code git
/// extension `Status` enum subset used for display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Ignored,
    TypeChanged,
    Conflicted,
}

impl ChangeKind {
    /// Single-letter badge VS Code shows after the file name (M/A/D/U/…).
    pub fn badge(self) -> &'static str {
        match self {
            ChangeKind::Modified => "M",
            ChangeKind::Added => "A",
            ChangeKind::Deleted => "D",
            ChangeKind::Renamed => "R",
            ChangeKind::Copied => "C",
            ChangeKind::Untracked => "U",
            ChangeKind::Ignored => "I",
            ChangeKind::TypeChanged => "T",
            ChangeKind::Conflicted => "!",
        }
    }

    /// `theme::Palette` decoration colour (2026-dark `gitDecoration.*`).
    pub fn decoration_color(self) -> egui::Color32 {
        use crate::theme::Palette;
        match self {
            ChangeKind::Modified | ChangeKind::TypeChanged => Palette::GIT_MODIFIED_FG,
            ChangeKind::Added | ChangeKind::Untracked => Palette::GIT_ADDED_FG,
            ChangeKind::Renamed | ChangeKind::Copied => Palette::GIT_RENAMED_FG,
            ChangeKind::Deleted => Palette::GIT_DELETED_FG,
            ChangeKind::Ignored => Palette::GIT_IGNORED_FG,
            ChangeKind::Conflicted => Palette::GIT_CONFLICT_FG,
        }
    }

    /// Deleted files are rendered with a strikethrough (decorationProvider.ts).
    pub fn strikethrough(self) -> bool {
        matches!(self, ChangeKind::Deleted)
    }
}

/// One changed path in the working tree.
#[derive(Clone, Debug)]
pub struct FileChange {
    pub path: PathBuf,
    /// Path relative to the repo root, as reported by git (for display).
    pub rel: String,
    pub kind: ChangeKind,
    /// `true` when the change is in the index (staged), `false` for the
    /// working-tree (unstaged) copy.
    pub staged: bool,
}

/// Aggregate working-tree state for the Source Control view.
#[derive(Clone, Debug, Default)]
pub struct RepoStatus {
    pub branch: Option<String>,
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
}

impl RepoStatus {
    pub fn total(&self) -> usize {
        self.staged.len() + self.unstaged.len()
    }
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// Resolve the git repository root that contains `dir`, or `None` if `dir`
/// isn't inside a working tree.
pub fn repo_root(dir: &Path) -> Option<PathBuf> {
    let out = git(dir, &["rev-parse", "--show-toplevel"])?;
    let trimmed = out.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

/// Current branch name (or `None` when detached / not a repo).
pub fn branch(root: &Path) -> Option<String> {
    let out = git(root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let name = out.trim().to_string();
    if name.is_empty() || name == "HEAD" {
        None
    } else {
        Some(name)
    }
}

fn kind_from_code(code: char) -> Option<ChangeKind> {
    match code {
        'M' => Some(ChangeKind::Modified),
        'A' => Some(ChangeKind::Added),
        'D' => Some(ChangeKind::Deleted),
        'R' | 'C' => Some(ChangeKind::Renamed),
        'U' => Some(ChangeKind::Conflicted),
        _ => None,
    }
}

/// Parse `git status --porcelain=v1` into staged / unstaged change lists.
pub fn status(root: &Path) -> RepoStatus {
    let mut repo = RepoStatus {
        branch: branch(root),
        ..Default::default()
    };

    let Some(out) = git(root, &["status", "--porcelain=v1", "--untracked-files=all"]) else {
        return repo;
    };

    for line in out.lines() {
        if line.len() < 3 {
            continue;
        }
        let bytes: Vec<char> = line.chars().collect();
        let x = bytes[0]; // index (staged) status
        let y = bytes[1]; // worktree (unstaged) status
        // Path starts after the status code + single space. Renames use
        // "orig -> new"; we keep the new name.
        let rest: String = bytes[3..].iter().collect();
        let rel = rest
            .rsplit(" -> ")
            .next()
            .unwrap_or(&rest)
            .trim()
            .to_string();
        let abs = root.join(&rel);

        if x == '?' && y == '?' {
            repo.unstaged.push(FileChange {
                path: abs,
                rel,
                kind: ChangeKind::Untracked,
                staged: false,
            });
            continue;
        }

        if let Some(kind) = kind_from_code(x) {
            repo.staged.push(FileChange {
                path: abs.clone(),
                rel: rel.clone(),
                kind,
                staged: true,
            });
        }
        if let Some(kind) = kind_from_code(y) {
            repo.unstaged.push(FileChange {
                path: abs,
                rel,
                kind,
                staged: false,
            });
        }
    }

    repo
}

/// How a single editor line differs from the indexed (HEAD) version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffKind {
    Added,
    Modified,
    /// A deletion occurred just *above* this line (rendered as a small
    /// caret in VS Code's gutter).
    DeletedAbove,
}

/// Per-line diff decorations for `file` against the index, keyed by 1-based
/// line number in the current (working-tree) file. Uses `git diff -U0`.
pub fn line_changes(root: &Path, file: &Path) -> BTreeMap<usize, DiffKind> {
    let mut map = BTreeMap::new();
    let rel = file.strip_prefix(root).unwrap_or(file);
    let Some(rel_str) = rel.to_str() else {
        return map;
    };

    let Some(out) = git(
        root,
        &["diff", "--no-color", "--unified=0", "--", rel_str],
    ) else {
        return map;
    };

    // Parse hunk headers: @@ -oldStart,oldLen +newStart,newLen @@
    for line in out.lines() {
        if !line.starts_with("@@") {
            continue;
        }
        let Some(plus) = line.split('+').nth(1) else {
            continue;
        };
        let spec = plus.split('@').next().unwrap_or("").trim();
        let mut parts = spec.split(',');
        let start: usize = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let new_len: usize = parts.next().and_then(|s| s.parse().ok()).unwrap_or(1);

        // Determine deleted length from the "-" spec for added/modified call.
        let minus = line.split('-').nth(1).unwrap_or("");
        let mspec = minus.split('+').next().unwrap_or("").trim();
        let old_len: usize = mspec
            .split(',')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        if new_len == 0 {
            // Pure deletion — mark the line where content was removed.
            let anchor = start.max(1);
            map.insert(anchor, DiffKind::DeletedAbove);
        } else {
            let kind = if old_len == 0 {
                DiffKind::Added
            } else {
                DiffKind::Modified
            };
            for ln in start..start + new_len {
                map.insert(ln, kind);
            }
        }
    }

    map
}

/// Per-line gutter decorations computed **in-process** from a base (HEAD)
/// text and the current working text, via `similar`. Used for the live
/// as-you-type gutter so we don't spawn `git diff` on every keystroke.
pub fn line_changes_diff(base: &str, working: &str) -> BTreeMap<usize, DiffKind> {
    use similar::{DiffOp, TextDiff};
    let mut map = BTreeMap::new();
    let diff = TextDiff::from_lines(base, working);
    for op in diff.ops() {
        match *op {
            DiffOp::Equal { .. } => {}
            DiffOp::Insert {
                new_index, new_len, ..
            } => {
                for i in 0..new_len {
                    map.insert(new_index + i + 1, DiffKind::Added);
                }
            }
            DiffOp::Replace {
                new_index, new_len, ..
            } => {
                for i in 0..new_len {
                    map.insert(new_index + i + 1, DiffKind::Modified);
                }
            }
            DiffOp::Delete { new_index, .. } => {
                map.insert((new_index + 1).max(1), DiffKind::DeletedAbove);
            }
        }
    }
    map
}

/// Stage (`git add`) a path.
pub fn stage(root: &Path, rel: &str) -> bool {
    git(root, &["add", "--", rel]).is_some()
}

/// Unstage (`git restore --staged`) a path.
pub fn unstage(root: &Path, rel: &str) -> bool {
    git(root, &["restore", "--staged", "--", rel]).is_some()
}

/// Stage every change (`git add -A`).
pub fn stage_all(root: &Path) -> bool {
    git(root, &["add", "-A"]).is_some()
}

/// Unstage everything (`git restore --staged .`).
pub fn unstage_all(root: &Path) -> bool {
    git(root, &["restore", "--staged", "."]).is_some()
}

/// Discard working-tree changes for a path. Tracked files are reverted with
/// `git restore`; untracked files (`Untracked`) are deleted from disk.
pub fn discard(root: &Path, rel: &str, untracked: bool) -> bool {
    if untracked {
        std::fs::remove_file(root.join(rel)).is_ok()
    } else {
        git(root, &["restore", "--", rel]).is_some()
    }
}

/// The HEAD version of a file as text (`git show HEAD:<rel>`). `None` for new
/// files not yet in HEAD, or on any error.
pub fn head_blob(root: &Path, rel: &str) -> Option<String> {
    git(root, &["show", &format!("HEAD:{rel}")])
}

/// Commit staged changes (or, with `all`, every tracked change via `-a`).
/// Returns `Ok(())` or the captured stderr/stdout on failure.
pub fn commit(root: &Path, message: &str, all: bool) -> Result<(), String> {
    let mut args: Vec<&str> = vec!["commit", "-m", message];
    if all {
        args.insert(1, "-a");
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        Err(format!("{stderr}{stdout}").trim().to_string())
    }
}

/// What to do after a commit — the SCM "Commit" button dropdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitMode {
    Plain,
    AndPush,
    AndSync,
}

/// Commit (optionally `-a` when nothing staged) then optionally push / sync,
/// matching the Commit / Commit & Push / Commit & Sync dropdown.
pub fn commit_with(
    root: &Path,
    message: &str,
    all: bool,
    mode: CommitMode,
) -> Result<(), String> {
    commit(root, message, all)?;
    match mode {
        CommitMode::Plain => {}
        CommitMode::AndPush => {
            push(root);
        }
        CommitMode::AndSync => {
            pull(root);
            push(root);
        }
    }
    Ok(())
}

/// All local branch names (current branch first).
pub fn branches(root: &Path) -> Vec<String> {
    let Some(out) = git(
        root,
        &["branch", "--format=%(HEAD)%(refname:short)"],
    ) else {
        return Vec::new();
    };
    let mut current = None;
    let mut rest = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('*') {
            current = Some(name.trim().to_string());
        } else {
            rest.push(line.to_string());
        }
    }
    let mut all = Vec::new();
    if let Some(c) = current {
        all.push(c);
    }
    all.extend(rest);
    all
}

/// Check out an existing branch.
pub fn checkout(root: &Path, name: &str) -> bool {
    git(root, &["checkout", name]).is_some()
}

/// `(ahead, behind)` relative to the upstream tracking branch. `(0, 0)` when
/// there is no upstream or on error.
pub fn ahead_behind(root: &Path) -> (usize, usize) {
    let Some(out) = git(
        root,
        &["rev-list", "--count", "--left-right", "@{u}...HEAD"],
    ) else {
        return (0, 0);
    };
    let mut parts = out.split_whitespace();
    let behind = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let ahead = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (ahead, behind)
}

/// `git pull` (best-effort).
pub fn pull(root: &Path) -> bool {
    git(root, &["pull", "--ff-only"]).is_some()
}

/// `git push` (best-effort).
pub fn push(root: &Path) -> bool {
    git(root, &["push"]).is_some()
}

/// Decoration map for the Explorer: absolute path → change kind. Folders are
/// not included; the file tree rolls those up itself.
pub fn decorations(root: &Path) -> std::collections::BTreeMap<PathBuf, ChangeKind> {
    let st = status(root);
    let mut map = std::collections::BTreeMap::new();
    // Unstaged takes display priority over staged for the same file (VS Code
    // shows the working-tree state), so insert staged first then unstaged.
    for fc in st.staged.iter().chain(st.unstaged.iter()) {
        map.insert(fc.path.clone(), fc.kind);
    }
    map
}
