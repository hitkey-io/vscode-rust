use std::path::{Path, PathBuf};

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
        })
    }

    pub fn save(&mut self) -> std::io::Result<()> {
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

fn detect_language(path: &Path) -> &'static str {
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
