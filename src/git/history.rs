//! Commit history + graph lane layout for the SCM "GRAPH" section.
//!
//! `log()` mirrors `extensions/git/src/git.ts:1444-1467` (`git log
//! --format=<fmt> --decorate=full -z`); `build_graph()` ports the per-commit
//! swimlane algorithm from
//! `src/vs/workbench/contrib/scm/browser/scmHistory.ts:289-360`.

use std::path::Path;
use std::process::Command;

use super::status::ChangeKind;

/// A file changed by a single commit.
#[derive(Clone, Debug)]
pub struct CommitFile {
    pub rel: String,
    pub kind: ChangeKind,
}

/// Files changed by `commit` (vs its first parent), via
/// `git diff-tree --no-commit-id -r -z --name-status`.
pub fn commit_changes(root: &Path, commit: &Commit) -> Vec<CommitFile> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["diff-tree", "--no-commit-id", "-r", "-z", "--name-status", &commit.id])
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let tokens: Vec<&str> = text.split('\0').filter(|t| !t.is_empty()).collect();
    let mut files = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let code = tokens[i];
        i += 1;
        let kind = match code.chars().next() {
            Some('A') => ChangeKind::Added,
            Some('D') => ChangeKind::Deleted,
            Some('M') => ChangeKind::Modified,
            Some('R') => ChangeKind::Renamed,
            Some('C') => ChangeKind::Copied,
            Some('T') => ChangeKind::TypeChanged,
            _ => ChangeKind::Modified,
        };
        // Renames/copies have an extra path token (orig\0new); show the new.
        let is_rename = matches!(kind, ChangeKind::Renamed | ChangeKind::Copied);
        let rel = if is_rename {
            let _orig = tokens.get(i).copied().unwrap_or("");
            let new = tokens.get(i + 1).copied().unwrap_or("");
            i += 2;
            new.to_string()
        } else {
            let p = tokens.get(i).copied().unwrap_or("");
            i += 1;
            p.to_string()
        };
        if !rel.is_empty() {
            files.push(CommitFile { rel, kind });
        }
    }
    files
}

/// Blob text of `rel` at revision `rev` (`git show <rev>:<rel>`); `None` for
/// missing files (added/deleted on one side) or errors.
pub fn blob_at(root: &Path, rev: &str, rel: &str) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["show", &format!("{rev}:{rel}")])
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// What a ref label decorates a commit with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefKind {
    Head,
    Local,
    Remote,
    Tag,
}

#[derive(Clone, Debug)]
pub struct RefBadge {
    pub name: String,
    pub kind: RefKind,
}

#[derive(Clone, Debug)]
pub struct Commit {
    pub id: String,
    pub short_id: String,
    pub parents: Vec<String>,
    pub author: String,
    pub summary: String,
    pub refs: Vec<RefBadge>,
}

/// One graph node lane: the commit id it carries downward + its colour index
/// into the lane palette.
#[derive(Clone, Debug)]
pub struct Lane {
    pub id: String,
    pub color: usize,
}

/// A history row: a commit + the swimlanes entering (above) and leaving
/// (below) it, and the lane index the commit's circle sits on.
#[derive(Clone, Debug)]
pub struct GraphRow {
    pub commit: Commit,
    pub circle_lane: usize,
    pub circle_color: usize,
    pub input: Vec<Lane>,
    pub output: Vec<Lane>,
}

const LOG_FORMAT: &str = "%H%x00%h%x00%P%x00%an%x00%s%x00%D";

fn parse_refs(raw: &str) -> Vec<RefBadge> {
    let mut out = Vec::new();
    for part in raw.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        // "HEAD -> refs/heads/master"
        if let Some(rest) = p.strip_prefix("HEAD -> ") {
            let name = rest.trim_start_matches("refs/heads/").to_string();
            out.push(RefBadge {
                name,
                kind: RefKind::Head,
            });
        } else if p == "HEAD" {
            out.push(RefBadge {
                name: "HEAD".into(),
                kind: RefKind::Head,
            });
        } else if let Some(t) = p.strip_prefix("tag: refs/tags/") {
            out.push(RefBadge {
                name: t.to_string(),
                kind: RefKind::Tag,
            });
        } else if let Some(r) = p.strip_prefix("refs/remotes/") {
            out.push(RefBadge {
                name: r.to_string(),
                kind: RefKind::Remote,
            });
        } else if let Some(l) = p.strip_prefix("refs/heads/") {
            out.push(RefBadge {
                name: l.to_string(),
                kind: RefKind::Local,
            });
        }
    }
    out
}

/// Load up to `limit` commits of the repository's current history.
pub fn log(root: &Path, limit: usize) -> Vec<Commit> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "log",
            "--decorate=full",
            &format!("--format={LOG_FORMAT}"),
            "-z",
            &format!("-n{limit}"),
        ])
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let tokens: Vec<&str> = text.split('\0').collect();
    let mut commits = Vec::new();
    // 6 NUL-separated tokens per commit.
    let mut i = 0;
    while i + 6 <= tokens.len() {
        let id = tokens[i].trim();
        if id.is_empty() {
            i += 1;
            continue;
        }
        let short_id = tokens[i + 1].to_string();
        let parents = tokens[i + 2]
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        let author = tokens[i + 3].to_string();
        let summary = tokens[i + 4].to_string();
        let refs = parse_refs(tokens[i + 5]);
        commits.push(Commit {
            id: id.to_string(),
            short_id,
            parents,
            author,
            summary,
            refs,
        });
        i += 6;
    }
    commits
}

const LANE_COUNT: usize = 5;

/// Port of `toISCMHistoryItemViewModelArray` (`scmHistory.ts:289-360`):
/// compute input/output swimlanes per commit.
pub fn build_graph(commits: &[Commit]) -> Vec<GraphRow> {
    let mut rows: Vec<GraphRow> = Vec::with_capacity(commits.len());
    let mut color_index: usize = 0; // rotates for new lanes
    let mut next_color = |ci: &mut usize| -> usize {
        *ci = (*ci + 1) % LANE_COUNT;
        *ci
    };

    let mut prev_output: Vec<Lane> = Vec::new();

    for commit in commits {
        let input: Vec<Lane> = prev_output.clone();

        // circle lane = first input lane carrying this commit, else append.
        let circle_lane = input
            .iter()
            .position(|l| l.id == commit.id)
            .unwrap_or(input.len());
        let circle_color = input
            .get(circle_lane)
            .map(|l| l.color)
            .unwrap_or_else(|| {
                let c = color_index;
                color_index = next_color(&mut color_index);
                c
            });

        // Build output swimlanes.
        let mut output: Vec<Lane> = Vec::new();
        let mut first_parent_added = false;
        if !commit.parents.is_empty() {
            for node in &input {
                if node.id == commit.id {
                    if !first_parent_added {
                        output.push(Lane {
                            id: commit.parents[0].clone(),
                            color: circle_color,
                        });
                        first_parent_added = true;
                    }
                    // collapse other lanes that also carried this commit
                    continue;
                }
                output.push(node.clone());
            }
        }
        // Extra parents (merge) → new lanes with rotating colours.
        let start = if first_parent_added { 1 } else { 0 };
        for p in commit.parents.iter().skip(start) {
            let color = next_color(&mut color_index);
            output.push(Lane {
                id: p.clone(),
                color,
            });
        }

        rows.push(GraphRow {
            commit: commit.clone(),
            circle_lane,
            circle_color,
            input,
            output: output.clone(),
        });
        prev_output = output;
    }

    rows
}
