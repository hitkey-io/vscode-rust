use std::path::{Path, PathBuf};

pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub expanded: bool,
    pub children: Option<Vec<FileNode>>,
}

impl FileNode {
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        let is_dir = path.is_dir();
        Self {
            path,
            name,
            is_dir,
            expanded: false,
            children: None,
        }
    }

    pub fn root(path: PathBuf) -> Self {
        let mut root = Self::new(path);
        root.expanded = true;
        root.ensure_loaded();
        root
    }

    pub fn ensure_loaded(&mut self) {
        if !self.is_dir || self.children.is_some() {
            return;
        }
        self.children = Some(load_children(&self.path));
    }

    pub fn toggle(&mut self) {
        if !self.is_dir {
            return;
        }
        self.expanded = !self.expanded;
        if self.expanded {
            self.ensure_loaded();
        }
    }

    /// Expand every ancestor folder of `target` so the file becomes visible in
    /// the tree (VS Code's "reveal active file"). Loads children lazily along
    /// the path. Returns true if `target` is within this subtree.
    pub fn reveal(&mut self, target: &std::path::Path) -> bool {
        if target == self.path {
            return true;
        }
        if !self.is_dir || !target.starts_with(&self.path) {
            return false;
        }
        self.expanded = true;
        self.ensure_loaded();
        if let Some(children) = self.children.as_mut() {
            for child in children {
                if target.starts_with(&child.path) && child.reveal(target) {
                    return true;
                }
            }
        }
        true
    }
}

/// Files/folders never shown in the Explorer. Mirrors VS Code's default
/// `files.exclude` (see `vscode-original/src/vs/workbench/contrib/files/.../`).
/// Any other dotfile (e.g. `.gitignore`, `.eslintrc`, `.env`)
/// IS visible by default — VS Code only hides version-control metadata
/// and OS noise.
const ALWAYS_HIDDEN: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "CVS",
    ".DS_Store",
    ".Trash",
    "Thumbs.db",
];

fn load_children(path: &Path) -> Vec<FileNode> {
    let mut entries: Vec<FileNode> = match std::fs::read_dir(path) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| !ALWAYS_HIDDEN.contains(&n))
                    .unwrap_or(false)
            })
            .map(|e| FileNode::new(e.path()))
            .collect(),
        Err(_) => Vec::new(),
    };
    // Natural-ish sort: folders first, then files; within each group case-insensitive lexical.
    // VS Code uses a full numeric-aware comparator; we approximate with lowercase compare,
    // which gets the vast majority of file orderings right.
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    entries
}
