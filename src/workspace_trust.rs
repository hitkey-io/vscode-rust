//! Workspace Trust (services/workspaces/common/workspaceTrust.ts,
//! contrib/workspace/browser/workspace.contribution.ts).
//!
//! VS Code 1.145 opens every folder nobody has trusted in **Restricted
//! Mode**: `security.workspace.trust.startupPrompt` defaults to `never`, so
//! there is no dialog, only the Restricted Mode banner (until dismissed) and
//! status bar item. A folder is trusted when it or one of its parents is in
//! the trusted list (Trusted Folders & Workspaces); the empty window is
//! trusted (`security.workspace.trust.emptyWindow`), and everything is when
//! `security.workspace.trust.enabled` is off.
//!
//! The list and the dismissed banners are kept in
//! `<user data>/User/globalStorage/workspace-trust.json`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
struct Stored {
    #[serde(default)]
    trusted: Vec<PathBuf>,
    /// Folders whose Restricted Mode banner was closed (`untilDismissed`).
    #[serde(default, rename = "bannerDismissed")]
    banner_dismissed: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct TrustStore {
    /// `None`: kept in memory only (tests).
    path: Option<PathBuf>,
    stored: Stored,
}

impl TrustStore {
    pub fn in_memory() -> Self {
        Self::default()
    }

    pub fn load(user_data_dir: &Path) -> Self {
        let path = user_data_dir.join("User").join("globalStorage").join("workspace-trust.json");
        let stored = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Self { path: Some(path), stored }
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(&self.stored) {
            let _ = std::fs::write(path, text);
        }
    }

    /// The trusted folders, as the Workspace Trust editor lists them.
    pub fn trusted_folders(&self) -> &[PathBuf] {
        &self.stored.trusted
    }

    /// Whether `folder` (or its parent chain) is trusted.
    pub fn is_folder_trusted(&self, folder: &Path) -> bool {
        self.stored.trusted.iter().any(|t| folder.starts_with(t))
    }

    /// The trusted entry that makes `folder` trusted, if any.
    pub fn trusted_by(&self, folder: &Path) -> Option<&Path> {
        self.stored.trusted.iter().find(|t| folder.starts_with(t)).map(PathBuf::as_path)
    }

    /// Trust `folder` (`setUrisTrust(true)`): adds it unless a parent is
    /// already there.
    pub fn trust(&mut self, folder: &Path) {
        if !self.is_folder_trusted(folder) {
            self.stored.trusted.push(folder.to_path_buf());
            self.save();
        }
    }

    /// Don't Trust: removes `folder` and every entry that trusted it.
    pub fn untrust(&mut self, folder: &Path) {
        let before = self.stored.trusted.len();
        self.stored.trusted.retain(|t| !folder.starts_with(t));
        if self.stored.trusted.len() != before {
            self.save();
        }
    }

    /// Removes one entry of the list (Delete Path).
    pub fn remove(&mut self, entry: &Path) {
        self.stored.trusted.retain(|t| t != entry);
        self.save();
    }

    pub fn banner_dismissed(&self, folder: &Path) -> bool {
        self.stored.banner_dismissed.iter().any(|f| f == folder)
    }

    pub fn dismiss_banner(&mut self, folder: &Path) {
        if !self.banner_dismissed(folder) {
            self.stored.banner_dismissed.push(folder.to_path_buf());
            self.save();
        }
    }
}

/// Whether the window is trusted: `workspace` is the open folder (none =
/// the empty window), `enabled` / `empty_window` the two settings.
pub fn is_trusted(store: &TrustStore, workspace: Option<&Path>, enabled: bool, empty_window: bool) -> bool {
    if !enabled {
        return true;
    }
    match workspace {
        None => empty_window,
        Some(folder) => store.is_folder_trusted(folder),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_are_trusted_through_their_parents() {
        let mut store = TrustStore::in_memory();
        let ws = Path::new("/projects/app");
        assert!(!is_trusted(&store, Some(ws), true, true));
        assert!(is_trusted(&store, None, true, true));
        assert!(is_trusted(&store, Some(ws), false, true));
        store.trust(Path::new("/projects"));
        assert!(is_trusted(&store, Some(ws), true, true));
        assert_eq!(store.trusted_by(ws), Some(Path::new("/projects")));
        // Trusting a child of a trusted folder adds nothing.
        store.trust(ws);
        assert_eq!(store.trusted_folders().len(), 1);
        // Don't Trust removes what trusted it.
        store.untrust(ws);
        assert!(!is_trusted(&store, Some(ws), true, true));
    }

    #[test]
    fn banner_dismissal_is_per_folder() {
        let mut store = TrustStore::in_memory();
        store.dismiss_banner(Path::new("/a"));
        assert!(store.banner_dismissed(Path::new("/a")));
        assert!(!store.banner_dismissed(Path::new("/b")));
    }
}
