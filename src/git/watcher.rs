//! Filesystem watcher for the workspace, used to auto-refresh Git status and
//! the Explorer when files change on disk (VS Code uses an OS file watcher;
//! we wrap `notify`'s `RecommendedWatcher`).
//!
//! Events are coalesced into a single `dirty` flag the UI polls each frame —
//! we don't try to do fine-grained invalidation, just "something changed,
//! re-run `git status`". Events inside `.git/` are ignored except for refs/
//! HEAD changes (branch switch, commit) so a normal `git status` poll storm
//! doesn't fire continuously.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher as _};

pub struct Watcher {
    _inner: RecommendedWatcher,
    dirty: Arc<AtomicBool>,
}

impl Watcher {
    /// Start watching `root` recursively. Returns `None` if the watcher
    /// couldn't be created (the app then falls back to manual refresh).
    pub fn new(root: &Path) -> Option<Self> {
        let dirty = Arc::new(AtomicBool::new(false));
        let flag = dirty.clone();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                if event.paths.iter().any(|p| is_relevant(p)) {
                    flag.store(true, Ordering::Relaxed);
                }
            }
        })
        .ok()?;
        watcher.watch(root, RecursiveMode::Recursive).ok()?;
        Some(Self {
            _inner: watcher,
            dirty,
        })
    }

    /// Atomically take the dirty flag: returns `true` (and resets) if any
    /// relevant change happened since the last call.
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::Relaxed)
    }
}

/// Ignore churn inside `.git/` (objects, index lock, FETCH_HEAD, …) except the
/// few refs that signal a branch switch or commit, and ignore obvious build
/// noise so a `cargo build` doesn't spam refreshes.
fn is_relevant(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.contains("/target/") || s.contains("/node_modules/") {
        return false;
    }
    if let Some(idx) = s.find("/.git/") {
        let after = &s[idx + 6..];
        return after == "HEAD" || after.starts_with("refs/") || after == "index";
    }
    if s.ends_with("/.git") {
        return false;
    }
    true
}
