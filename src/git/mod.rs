//! Git integration, shelling out to the `git` CLI.
//!
//! VS Code's SCM is provided by the built-in `git` extension, which talks to
//! the `git` binary over its CLI. We do the same — no libgit2 dependency,
//! works wherever `git` is on `PATH`. This module mirrors the extension's
//! multi-repository model: a workspace can contain several repositories
//! (the root and/or nested subfolders), each with its own status, branch and
//! commit history.

pub mod history;
pub mod repository;
pub mod status;
pub mod watcher;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use history::{
    blob_at, build_graph, commit_changes, log, Commit, CommitFile, GraphRow, Lane, RefBadge,
    RefKind,
};
pub use repository::{discover_repos, status_v2, Repository, ResourceGroupKind};
pub use status::{
    ahead_behind, branch, branches, checkout, commit, commit_with, discard, head_blob,
    line_changes, line_changes_diff, pull, push, repo_root, stage, stage_all, unstage,
    unstage_all, ChangeKind, CommitMode, DiffKind, FileChange,
};
pub use watcher::Watcher;

/// How far below the workspace root to scan for repositories
/// (`git.repositoryScanMaxDepth`, default 1).
const SCAN_MAX_DEPTH: usize = 1;

/// The set of repositories in the current workspace.
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub repos: Vec<Repository>,
}

impl Model {
    /// Discover every repository in `workspace` and load each one's status.
    pub fn discover(workspace: &Path) -> Self {
        let mut model = Model {
            repos: discover_repos(workspace, SCAN_MAX_DEPTH)
                .into_iter()
                .map(|root| status_v2(&root))
                .collect(),
        };
        model.disambiguate_names();
        model
    }

    /// Re-run status for every known repository (roots are kept; this does not
    /// re-scan the filesystem for new repos — call `discover` for that).
    pub fn refresh(&mut self) {
        for repo in &mut self.repos {
            *repo = status_v2(&repo.root);
        }
        self.disambiguate_names();
    }

    pub fn is_empty(&self) -> bool {
        self.repos.is_empty()
    }

    /// The repository that owns `path` — the one whose root is the longest
    /// prefix of `path` (handles nested repos correctly).
    pub fn repo_for(&self, path: &Path) -> Option<&Repository> {
        self.repos
            .iter()
            .filter(|r| path.starts_with(&r.root))
            .max_by_key(|r| r.root.as_os_str().len())
    }

    /// Union of all repositories' decorations: absolute path → change kind.
    pub fn decorations(&self) -> BTreeMap<PathBuf, ChangeKind> {
        let mut map = BTreeMap::new();
        for repo in &self.repos {
            // Index first, then working tree so the working-tree state wins.
            for fc in repo.index.iter().chain(repo.working.iter()) {
                map.insert(fc.path.clone(), fc.kind);
            }
        }
        map
    }

    pub fn total_changes(&self) -> usize {
        self.repos.iter().map(|r| r.total()).sum()
    }

    /// If two repos share a folder name, suffix with their parent dir so the
    /// SCM list stays unambiguous (VS Code does the same).
    fn disambiguate_names(&mut self) {
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        for r in &self.repos {
            *seen.entry(r.name.clone()).or_default() += 1;
        }
        for r in &mut self.repos {
            if seen.get(&r.name).copied().unwrap_or(0) > 1 {
                if let Some(parent) = r.root.parent().and_then(|p| p.file_name()) {
                    r.name = format!("{} • {}", r.name, parent.to_string_lossy());
                }
            }
        }
    }
}
