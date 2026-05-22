//! One git repository: working-tree status parsed into VS Code's resource
//! groups, plus workspace repository discovery.
//!
//! Mirrors `extensions/git/src/repository.ts` (resource groups + the
//! `git status -z` porcelain map) and `model.ts:406-434`
//! (`traverseWorkspaceFolder` discovery: depth `repositoryScanMaxDepth`=1,
//! ignored folders `['node_modules']`).

use std::path::{Path, PathBuf};
use std::process::Command;

use super::status::{ChangeKind, FileChange};

/// VS Code's SCM resource groups, in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceGroupKind {
    Merge,
    Index,
    WorkingTree,
    Untracked,
}

impl ResourceGroupKind {
    pub fn label(self) -> &'static str {
        match self {
            ResourceGroupKind::Merge => "Merge Changes",
            ResourceGroupKind::Index => "Staged Changes",
            ResourceGroupKind::WorkingTree => "Changes",
            ResourceGroupKind::Untracked => "Untracked Changes",
        }
    }
}

/// A discovered repository with its current working-tree state.
#[derive(Clone, Debug, Default)]
pub struct Repository {
    pub root: PathBuf,
    pub name: String,
    /// Current branch, or `None` when detached (then `head` holds the short hash).
    pub branch: Option<String>,
    /// `true` when the branch has an upstream (drives Publish vs Sync).
    pub has_upstream: bool,
    pub ahead: usize,
    pub behind: usize,
    pub merge: Vec<FileChange>,
    pub index: Vec<FileChange>,
    /// Changes (working tree). With the default `untrackedChanges: mixed`,
    /// untracked files live here too (flagged via `kind == Untracked`).
    pub working: Vec<FileChange>,
}

impl Repository {
    pub fn total(&self) -> usize {
        self.merge.len() + self.index.len() + self.working.len()
    }

    /// Non-empty groups in VS Code display order, with their file lists.
    pub fn groups(&self) -> Vec<(ResourceGroupKind, &[FileChange])> {
        let mut out = Vec::new();
        if !self.merge.is_empty() {
            out.push((ResourceGroupKind::Merge, self.merge.as_slice()));
        }
        if !self.index.is_empty() {
            out.push((ResourceGroupKind::Index, self.index.as_slice()));
        }
        if !self.working.is_empty() {
            out.push((ResourceGroupKind::WorkingTree, self.working.as_slice()));
        }
        out
    }
}

fn git_out(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// Is `XY` a merge-conflict status (`repository.ts:3049-3060`)?
fn is_conflict(x: u8, y: u8) -> bool {
    matches!(
        (x, y),
        (b'D', b'D')
            | (b'A', b'U')
            | (b'U', b'D')
            | (b'U', b'A')
            | (b'D', b'U')
            | (b'A', b'A')
            | (b'U', b'U')
    )
}

fn index_kind(x: u8) -> Option<ChangeKind> {
    match x {
        b'M' => Some(ChangeKind::Modified),
        b'A' => Some(ChangeKind::Added),
        b'D' => Some(ChangeKind::Deleted),
        b'R' => Some(ChangeKind::Renamed),
        b'C' => Some(ChangeKind::Copied),
        b'T' => Some(ChangeKind::TypeChanged),
        _ => None,
    }
}

fn worktree_kind(y: u8) -> Option<ChangeKind> {
    match y {
        b'M' => Some(ChangeKind::Modified),
        b'D' => Some(ChangeKind::Deleted),
        b'A' => Some(ChangeKind::Added),
        b'R' => Some(ChangeKind::Renamed),
        b'C' => Some(ChangeKind::Copied),
        b'T' => Some(ChangeKind::TypeChanged),
        _ => None,
    }
}

/// Parse the `## ` branch header: `## main`, `## x...origin/x`,
/// `## x...origin/x [ahead N, behind M]`.
fn parse_branch_header(s: &str, repo: &mut Repository) {
    let body = s.trim_start_matches("## ").trim();
    // Detached: "HEAD (no branch)".
    if body.starts_with("HEAD") {
        repo.branch = None;
        return;
    }
    let (names, track) = match body.split_once(' ') {
        Some((n, t)) => (n, Some(t)),
        None => (body, None),
    };
    let (local, upstream) = match names.split_once("...") {
        Some((l, u)) => (l, Some(u)),
        None => (names, None),
    };
    repo.branch = Some(local.to_string());
    repo.has_upstream = upstream.is_some();
    if let Some(t) = track {
        // t = "[ahead N, behind M]" / "[ahead N]" / "[behind M]"
        let inner = t.trim_start_matches('[').trim_end_matches(']');
        for part in inner.split(',') {
            let part = part.trim();
            if let Some(n) = part.strip_prefix("ahead ") {
                repo.ahead = n.trim().parse().unwrap_or(0);
            } else if let Some(n) = part.strip_prefix("behind ") {
                repo.behind = n.trim().parse().unwrap_or(0);
            }
        }
    }
}

/// Working-tree status for `root` parsed into resource groups, via
/// `git status -z --branch -uall`.
pub fn status_v2(root: &Path) -> Repository {
    let mut repo = Repository {
        root: root.to_path_buf(),
        name: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        ..Default::default()
    };

    let Some(bytes) = git_out(root, &["status", "-z", "--branch", "-uall"]) else {
        return repo;
    };

    // NUL-separated tokens. The first token is the `## ` branch header; the
    // rest are `XY <path>` entries (with an extra NUL token for renames).
    let tokens: Vec<&[u8]> = bytes.split(|&b| b == 0).collect();
    let mut i = 0;
    while i < tokens.len() {
        let tok = tokens[i];
        i += 1;
        if tok.is_empty() {
            continue;
        }
        if tok.starts_with(b"## ") || tok == b"##" {
            parse_branch_header(&String::from_utf8_lossy(tok), &mut repo);
            continue;
        }
        if tok.len() < 3 {
            continue;
        }
        let x = tok[0];
        let y = tok[1];
        // tok[2] is a space; path follows.
        let path = String::from_utf8_lossy(&tok[3..]).into_owned();
        // Renames/copies carry a second NUL token (the original path); consume it.
        if x == b'R' || x == b'C' || y == b'R' || y == b'C' {
            if i < tokens.len() {
                i += 1; // skip original path
            }
        }
        let abs = root.join(&path);

        // Untracked / ignored.
        if x == b'?' && y == b'?' {
            repo.working.push(FileChange {
                path: abs,
                rel: path,
                kind: ChangeKind::Untracked,
                staged: false,
            });
            continue;
        }
        if x == b'!' && y == b'!' {
            continue; // ignored files not listed by default
        }

        // Merge conflict → merge group.
        if is_conflict(x, y) {
            repo.merge.push(FileChange {
                path: abs,
                rel: path,
                kind: ChangeKind::Conflicted,
                staged: false,
            });
            continue;
        }

        // Index (staged) change.
        if let Some(kind) = index_kind(x) {
            repo.index.push(FileChange {
                path: abs.clone(),
                rel: path.clone(),
                kind,
                staged: true,
            });
        }
        // Working-tree (unstaged) change — a file may appear in both groups.
        if let Some(kind) = worktree_kind(y) {
            repo.working.push(FileChange {
                path: abs,
                rel: path,
                kind,
                staged: false,
            });
        }
    }

    repo
}

/// Folders never descended into during discovery (VS Code default
/// `repositoryScanIgnoredFolders = ['node_modules']`, plus `.git`).
const SCAN_IGNORED: &[&str] = &["node_modules", ".git"];

fn has_git(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Discover every repository in `workspace`: the root if it is a repo, plus
/// folders up to `max_depth` (1 = immediate children). Mirrors
/// `traverseWorkspaceFolder` (`model.ts:406-434`).
pub fn discover_repos(workspace: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut repos = Vec::new();
    // BFS: (path, depth). A folder is traversed (its children read) only while
    // depth < max_depth.
    let mut queue = vec![(workspace.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = queue.pop() {
        if has_git(&dir) {
            repos.push(dir.clone());
        }
        if depth >= max_depth {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let name = e.file_name();
            let name = name.to_string_lossy();
            if SCAN_IGNORED.iter().any(|ig| *ig == name) {
                continue;
            }
            queue.push((p, depth + 1));
        }
    }
    repos.sort();
    repos.dedup();
    repos
}
