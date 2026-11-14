use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Document {
    pub path: PathBuf,
    pub text: String,
    pub saved_text: String,
    pub dirty: bool,
    pub language: &'static str,
    pub cursor_line: usize,
    pub cursor_col: usize,
    /// (line, byte_offset_within_line) — requested cursor placement, consumed by the view next frame.
    pub pending_nav: Option<(usize, usize)>,
    /// Tab is pinned to the front of the strip and protected from
    /// close-others / middle-click close.
    pub pinned: bool,
    /// Folded fold-header line numbers (1-based). When non-empty the editor
    /// renders a read-only collapsed view; clearing it returns to edit mode.
    pub folded: std::collections::BTreeSet<usize>,
    /// When `Some`, this document is a read-only **diff** tab: the string is
    /// the base (left) version of the file, diffed inline against `text`.
    pub diff_base: Option<String>,
    /// Optional tab-title override for diff tabs
    /// (e.g. `env.ts (a1b2c) ↔ env.ts (d4e5f)`).
    pub diff_title: Option<String>,
    /// When `Some`, this tab shows an extension's webview panel (that
    /// handle) instead of text; `diff_title` holds its title.
    pub webview: Option<u64>,
    /// A preview editor: opened with one click and replaced by the next
    /// one until it's kept (double-click on the tab). Italic tab title.
    pub preview: bool,
    /// Shown, never written: a version from git (HEAD, the index, a
    /// commit) or a diff. Edits are undone and Save does nothing.
    pub read_only: bool,
    /// A new file not saved anywhere yet ("Untitled-1"): Save asks where.
    pub untitled: bool,
    /// The Welcome page (`GettingStartedInput`), shown in a tab like any
    /// editor.
    pub welcome: bool,
    /// The editor group (0 = left, 1 = right of a split) the tab lives in.
    pub group: u8,
}

impl Document {
    pub fn open(path: PathBuf) -> std::io::Result<Self> {
        // Reject obvious binary file types before trying read_to_string, which
        // would surface non-UTF-8 errors to the user. VS Code's resolver chain
        // does the same via `BinaryEditor` / `getMime` — we just blacklist.
        if is_known_binary(&path) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "binary file",
            ));
        }
        let text = std::fs::read_to_string(&path)?;
        let language = detect_language(&path);
        Ok(Self {
            saved_text: text.clone(),
            text,
            dirty: false,
            language,
            path,
            cursor_line: 1,
            cursor_col: 1,
            pending_nav: None,
            pinned: false,
            folded: std::collections::BTreeSet::new(),
            diff_base: None,
            diff_title: None,
            webview: None,
            preview: false,
            read_only: false,
            untitled: false,
            welcome: false,
            group: 0,
        })
    }

    /// Build a read-only diff document from two in-memory blobs (no disk
    /// read) — used for commit diffs in the GRAPH, where the file may not
    /// exist on disk at the requested revisions.
    pub fn diff(path: PathBuf, base: String, working: String, title: String) -> Self {
        let language = detect_language(&path);
        Self {
            saved_text: working.clone(),
            text: working,
            dirty: false,
            language,
            path,
            cursor_line: 1,
            cursor_col: 1,
            pending_nav: None,
            pinned: false,
            folded: std::collections::BTreeSet::new(),
            diff_base: Some(base),
            diff_title: Some(title),
            webview: None,
            preview: false,
            read_only: true,
            untitled: false,
            welcome: false,
            group: 0,
        }
    }

    /// A tab for an extension's webview panel.
    /// A read-only view of `text` for `path` titled `title`, e.g.
    /// "gone.txt (Deleted)" — the file as it is in HEAD or the index.
    pub fn read_only(path: PathBuf, text: String, title: String) -> Self {
        let mut d = Self::diff(path, String::new(), text, title);
        d.diff_base = None;
        d
    }

    pub fn webview(handle: u64, title: String) -> Self {
        Self {
            saved_text: String::new(),
            text: String::new(),
            dirty: false,
            language: "",
            path: PathBuf::from(format!("webview-panel:{handle}")),
            cursor_line: 1,
            cursor_col: 1,
            pending_nav: None,
            pinned: false,
            folded: std::collections::BTreeSet::new(),
            diff_base: None,
            diff_title: Some(title),
            webview: Some(handle),
            preview: false,
            read_only: true,
            untitled: false,
            welcome: false,
            group: 0,
        }
    }

    /// The Welcome page's tab. It opens italic, as VS Code shows it.
    pub fn welcome() -> Self {
        let mut d = Self::read_only(PathBuf::from("Welcome"), String::new(), String::new());
        d.diff_title = None;
        d.language = "";
        d.welcome = true;
        d.preview = true;
        d
    }

    /// `workbench.action.files.newUntitledFile`: an empty plain-text
    /// document titled "Untitled-<n>".
    pub fn untitled(n: usize) -> Self {
        let mut d = Self::read_only(PathBuf::from(format!("Untitled-{n}")), String::new(), String::new());
        d.diff_title = None;
        d.read_only = false;
        d.untitled = true;
        d.language = "";
        d
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if self.read_only {
            return Ok(());
        }
        if self.untitled {
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "untitled: choose where to save"));
        }
        std::fs::write(&self.path, &self.text)?;
        self.saved_text = self.text.clone();
        self.dirty = false;
        Ok(())
    }

    pub fn check_dirty(&mut self) {
        self.dirty = self.text != self.saved_text;
    }

    pub fn display_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.to_string_lossy().into_owned())
    }

    /// VS Code's language identifier (`onLanguage:<id>` activation events).
    pub fn language_id(&self) -> &'static str {
        match self.language {
            "rs" => "rust",
            "ts" => "typescript",
            "tsx" => "typescriptreact",
            "js" => "javascript",
            "jsx" => "javascriptreact",
            "py" => "python",
            "go" => "go",
            "c" | "h" => "c",
            "cpp" => "cpp",
            "json" => "json",
            "md" => "markdown",
            "html" => "html",
            "css" => "css",
            "toml" => "toml",
            "yaml" | "yml" => "yaml",
            "sh" => "shellscript",
            "vue" => "vue",
            _ => "plaintext",
        }
    }

    pub fn language_label(&self) -> &'static str {
        match self.language {
            "rs" => "Rust",
            "ts" => "TypeScript",
            "tsx" => "TypeScript React",
            "js" => "JavaScript",
            "jsx" => "JavaScript React",
            "py" => "Python",
            "go" => "Go",
            "c" => "C",
            "cpp" => "C++",
            "h" => "C Header",
            "json" => "JSON",
            "md" => "Markdown",
            "html" => "HTML",
            "css" => "CSS",
            "toml" => "TOML",
            "yaml" | "yml" => "YAML",
            "sh" => "Shell",
            _ => "Plain Text",
        }
    }
}

fn is_known_binary(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tiff" | "tif" | "ico" | "webp"
            | "avif" | "heic" | "heif" | "psd" | "ai"
            | "pdf"
            | "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac"
            | "mp4" | "mov" | "avi" | "mkv" | "webm" | "wmv"
            | "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar"
            | "exe" | "dll" | "so" | "dylib" | "a" | "o" | "lib"
            | "ttf" | "otf" | "woff" | "woff2" | "eot"
            | "wasm"
            | "sqlite" | "db"
    )
}

pub fn detect_language(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "rs" => "rs",
        "ts" => "ts",
        "tsx" => "tsx",
        "js" | "mjs" | "cjs" => "js",
        "jsx" => "jsx",
        "py" => "py",
        "go" => "go",
        "c" => "c",
        "cpp" | "cc" | "cxx" => "cpp",
        "h" | "hpp" => "h",
        "json" => "json",
        "md" | "markdown" => "md",
        "html" | "htm" => "html",
        "css" => "css",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "sh" | "bash" | "zsh" => "sh",
        _ => "",
    }
}
