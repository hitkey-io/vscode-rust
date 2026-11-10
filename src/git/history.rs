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
    /// Author time (Unix seconds).
    pub time: Option<i64>,
    pub summary: String,
    /// The whole message.
    pub message: String,
    pub refs: Vec<RefBadge>,
}

/// One graph node lane: the commit id it carries downward + its colour
/// (an index into `scmGraph.foreground1-5`, or a `REF_COLOR_*` sentinel).
#[derive(Clone, Debug, PartialEq)]
pub struct Lane {
    pub id: String,
    pub color: usize,
}

/// `ISCMHistoryItemViewModel.kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Head,
    Node,
}

/// A reference badge drawn at the right of a history row.
#[derive(Clone, Debug, PartialEq)]
pub struct Badge {
    pub name: String,
    /// Codicon name (`target`, `git-branch`, `cloud`, `tag`).
    pub icon: &'static str,
    pub color: usize,
    /// References grouped into this badge.
    pub count: usize,
    pub show_description: bool,
}

/// A history row: a commit + the swimlanes entering (above) and leaving
/// (below) it.
#[derive(Clone, Debug)]
pub struct GraphRow {
    pub commit: Commit,
    pub kind: NodeKind,
    /// Lane the commit's circle sits on.
    pub circle_index: usize,
    pub circle_color: usize,
    pub input: Vec<Lane>,
    pub output: Vec<Lane>,
    pub badges: Vec<Badge>,
}

const LOG_FORMAT: &str = "%H%x00%h%x00%P%x00%an%x00%at%x00%s%x00%D%x00%B";
const LOG_FIELDS: usize = 8;

fn parse_refs(raw: &str) -> Vec<RefBadge> {
    let mut out = Vec::new();
    for part in raw.split(',') {
        let p = part.trim();
        if p.is_empty() || p == "refs/remotes/origin/HEAD" {
            continue;
        }
        if let Some(rest) = p.strip_prefix("HEAD -> ") {
            out.push(RefBadge { name: rest.trim_start_matches("refs/heads/").to_string(), kind: RefKind::Head });
        } else if p == "HEAD" {
            out.push(RefBadge { name: "HEAD".into(), kind: RefKind::Head });
        } else if let Some(t) = p.strip_prefix("tag: refs/tags/") {
            out.push(RefBadge { name: t.to_string(), kind: RefKind::Tag });
        } else if let Some(r) = p.strip_prefix("refs/remotes/") {
            out.push(RefBadge { name: r.to_string(), kind: RefKind::Remote });
        } else if let Some(l) = p.strip_prefix("refs/heads/") {
            out.push(RefBadge { name: l.to_string(), kind: RefKind::Local });
        }
    }
    out
}

impl RefBadge {
    /// The reference's full id (`refs/heads/main`…), as the git extension
    /// names it.
    pub fn id(&self) -> String {
        match self.kind {
            RefKind::Head | RefKind::Local => format!("refs/heads/{}", self.name),
            RefKind::Remote => format!("refs/remotes/{}", self.name),
            RefKind::Tag => format!("refs/tags/{}", self.name),
        }
    }

    fn icon(&self) -> &'static str {
        match self.kind {
            RefKind::Head => "target",
            RefKind::Local => "git-branch",
            RefKind::Remote => "cloud",
            RefKind::Tag => "tag",
        }
    }
}

/// Up to `limit` commits reachable from `revs` (`HEAD`, `--all`, a ref…),
/// in topological order, as the git extension asks for its graph.
pub fn log(root: &Path, limit: usize, revs: &[String]) -> Vec<Commit> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(root).args(["log", "--topo-order", "--decorate=full", &format!("--format={LOG_FORMAT}"), "-z", &format!("-n{limit}")]);
    if revs.is_empty() {
        cmd.arg("HEAD");
    } else {
        cmd.args(revs);
    }
    cmd.arg("--");
    let Ok(out) = cmd.output() else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    parse_log(&String::from_utf8_lossy(&out.stdout))
}

fn parse_log(text: &str) -> Vec<Commit> {
    let tokens: Vec<&str> = text.split('\0').collect();
    let mut commits = Vec::new();
    let mut i = 0;
    while i + LOG_FIELDS <= tokens.len() {
        let id = tokens[i].trim();
        if id.is_empty() {
            i += 1;
            continue;
        }
        commits.push(Commit {
            id: id.to_string(),
            short_id: tokens[i + 1].to_string(),
            parents: tokens[i + 2].split_whitespace().map(str::to_string).collect(),
            author: tokens[i + 3].to_string(),
            time: tokens[i + 4].trim().parse().ok(),
            summary: tokens[i + 5].to_string(),
            refs: parse_refs(tokens[i + 6]),
            message: tokens[i + 7].trim_end().to_string(),
        });
        i += LOG_FIELDS;
    }
    commits
}

/// Reference colour sentinels (`scmGraph.historyItemRefColor` = charts.blue,
/// `historyItemRemoteRefColor` = charts.purple, `historyItemBaseRefColor`).
pub const REF_COLOR_LOCAL: usize = 100;
pub const REF_COLOR_REMOTE: usize = 101;
pub const REF_COLOR_BASE: usize = 102;

/// `scmGraph.foreground1-5`.
const LANE_COUNT: usize = 5;

/// What the graph is drawn for: the current branch, its upstream, and the
/// other references shown (whose badges take their lane's colour).
#[derive(Clone, Debug, Default)]
pub struct GraphRefs {
    /// `refs/heads/main`.
    pub current: Option<String>,
    /// `refs/remotes/origin/main`.
    pub remote: Option<String>,
    /// Further references in the filter (coloured like their lane).
    pub others: Vec<String>,
    /// The commit HEAD points at.
    pub head: Option<String>,
}

/// Port of `toISCMHistoryItemViewModelArray` (scmHistory.ts): input and
/// output swimlanes per commit, lane colours from the references on them,
/// and the badges `_renderBadges` shows in its default ("filter") mode.
pub fn build_graph(commits: &[Commit], refs: &GraphRefs) -> Vec<GraphRow> {
    // colorMap: reference id → colour (None = the colour of its lane).
    let mut color_map: Vec<(String, Option<usize>)> = Vec::new();
    if let Some(c) = &refs.current {
        color_map.push((c.clone(), Some(REF_COLOR_LOCAL)));
        if let Some(r) = &refs.remote {
            color_map.push((r.clone(), Some(REF_COLOR_REMOTE)));
        }
    }
    for o in &refs.others {
        if !color_map.iter().any(|(id, _)| id == o) {
            color_map.push((o.clone(), None));
        }
    }
    let lookup = |id: &str| color_map.iter().find(|(k, _)| k == id).map(|(_, c)| *c);
    let sorted_refs = |c: &Commit| -> Vec<RefBadge> {
        let mut v = c.refs.clone();
        let rank = |r: &RefBadge| {
            let id = r.id();
            if Some(&id) == refs.current.as_ref() {
                0
            } else if Some(&id) == refs.remote.as_ref() {
                1
            } else {
                match r.kind {
                    RefKind::Head | RefKind::Local => 2,
                    RefKind::Remote => 3,
                    RefKind::Tag => 4,
                }
            }
        };
        v.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.name.cmp(&b.name)));
        v
    };
    // getLabelColorIdentifier: the first reference with a defined colour.
    let label_color = |c: &Commit| sorted_refs(c).iter().find_map(|r| lookup(&r.id()).flatten());

    let mut color_index: isize = -1;
    let mut rows: Vec<GraphRow> = Vec::with_capacity(commits.len());
    for commit in commits {
        let input: Vec<Lane> = rows.last().map(|r| r.output.clone()).unwrap_or_default();
        let mut output: Vec<Lane> = Vec::new();
        let mut first_parent_added = false;
        if !commit.parents.is_empty() {
            for node in &input {
                if node.id == commit.id {
                    if !first_parent_added {
                        output.push(Lane { id: commit.parents[0].clone(), color: label_color(commit).unwrap_or(node.color) });
                        first_parent_added = true;
                    }
                    continue;
                }
                output.push(node.clone());
            }
        }
        for i in (if first_parent_added { 1 } else { 0 })..commit.parents.len() {
            let color = if i == 0 {
                label_color(commit)
            } else {
                commits.iter().find(|h| h.id == commit.parents[i]).and_then(label_color)
            };
            let color = color.unwrap_or_else(|| {
                color_index = (color_index + 1).rem_euclid(LANE_COUNT as isize);
                color_index as usize
            });
            output.push(Lane { id: commit.parents[i].clone(), color });
        }

        let input_index = input.iter().position(|l| l.id == commit.id);
        let circle_index = input_index.unwrap_or(input.len());
        let circle_color = if circle_index < output.len() {
            output[circle_index].color
        } else if circle_index < input.len() {
            input[circle_index].color
        } else {
            REF_COLOR_LOCAL
        };

        // Badges ("filter" mode): the first reference if coloured, with its
        // name; then the other coloured ones grouped by colour and icon.
        let mut badges = Vec::new();
        let colored: Vec<(RefBadge, Option<usize>)> = sorted_refs(commit)
            .into_iter()
            .filter_map(|r| {
                let entry = lookup(&r.id())?;
                Some((r, Some(entry.unwrap_or(circle_color))))
            })
            .collect();
        let mut rest = colored;
        if let Some((first, Some(color))) = rest.first().cloned() {
            if Some(&first.id()) == sorted_refs(commit).first().map(|r| r.id()).as_ref() {
                badges.push(Badge { name: first.name.clone(), icon: first.icon(), color, count: 1, show_description: true });
                rest.remove(0);
            }
        }
        let mut groups: Vec<(usize, &'static str, usize)> = Vec::new();
        for (r, color) in &rest {
            let color = color.unwrap_or(circle_color);
            match groups.iter_mut().find(|(c, i, _)| *c == color && *i == r.icon()) {
                Some(g) => g.2 += 1,
                None => groups.push((color, r.icon(), 1)),
            }
        }
        for (color, icon, count) in groups {
            badges.push(Badge { name: String::new(), icon, color, count, show_description: false });
        }

        rows.push(GraphRow {
            commit: commit.clone(),
            kind: if refs.head.as_deref() == Some(commit.id.as_str()) { NodeKind::Head } else { NodeKind::Node },
            circle_index,
            circle_color,
            input,
            output,
            badges,
        });
    }
    rows
}

/// The graph of `root` for a reference filter: `None` = "Auto" (the current
/// branch and its upstream), `Some("--all")` = every reference, else one
/// reference.
pub fn graph(root: &Path, filter: Option<&str>, limit: usize) -> Vec<GraphRow> {
    let git = |args: &[&str]| -> Option<String> {
        let out = Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let head = git(&["rev-parse", "HEAD"]);
    let current = git(&["symbolic-ref", "-q", "HEAD"]);
    let remote = current
        .as_ref()
        .and_then(|_| git(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]))
        .map(|u| format!("refs/remotes/{u}"));
    let mut refs = GraphRefs { current: current.clone(), remote: remote.clone(), others: Vec::new(), head };
    let revs: Vec<String> = match filter {
        None => {
            let mut v = vec!["HEAD".to_string()];
            if let Some(r) = &remote {
                v.push(r.clone());
            }
            v
        }
        Some("--all") => {
            refs.others = git(&["for-each-ref", "--format=%(refname)", "refs/heads", "refs/remotes", "refs/tags"])
                .map(|s| s.lines().filter(|l| !l.ends_with("/HEAD")).map(str::to_string).collect())
                .unwrap_or_default();
            vec!["--all".to_string()]
        }
        Some(r) => {
            refs.others = vec![r.to_string()];
            vec![r.to_string()]
        }
    };
    build_graph(&log(root, limit, &revs), &refs)
}
