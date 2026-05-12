use std::path::{Path, PathBuf};

pub struct SearchQuery {
    pub text: String,
    pub match_case: bool,
    pub whole_word: bool,
    pub regex: bool,
}

pub struct SearchHit {
    pub line: usize,
    pub byte_start: usize,
    pub preview: String,
}

pub struct FileResult {
    pub path: PathBuf,
    pub hits: Vec<SearchHit>,
}

pub struct SearchOutcome {
    pub results: Vec<FileResult>,
    pub files_scanned: usize,
    pub total_hits: usize,
    pub truncated: bool,
    pub error: Option<String>,
}

const MAX_FILE_SIZE: u64 = 1024 * 1024; // 1 MiB
const MAX_TOTAL_HITS: usize = 1000;
const MAX_FILES_MATCHED: usize = 200;
const MAX_LINE_PREVIEW: usize = 200;

const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".git",
    ".svn",
    ".hg",
    ".idea",
    ".vscode",
    ".next",
    ".turbo",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".tox",
    ".venv",
    "venv",
    ".cargo",
];

const SKIP_FILENAMES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "Pipfile.lock",
    "poetry.lock",
    ".DS_Store",
];

pub fn run(root: &Path, query: &SearchQuery) -> SearchOutcome {
    let mut outcome = SearchOutcome {
        results: Vec::new(),
        files_scanned: 0,
        total_hits: 0,
        truncated: false,
        error: None,
    };

    if query.text.is_empty() {
        return outcome;
    }

    let matcher = match build_matcher(query) {
        Ok(m) => m,
        Err(e) => {
            outcome.error = Some(e);
            return outcome;
        }
    };

    walk(root, &matcher, &mut outcome);
    outcome
}

enum Matcher {
    Plain {
        needle_lower: String,
        needle_orig: String,
        match_case: bool,
        whole_word: bool,
    },
    Regex(regex::Regex),
}

impl Matcher {
    fn find_all(&self, line: &str) -> Vec<(usize, usize)> {
        match self {
            Matcher::Plain {
                needle_lower,
                needle_orig,
                match_case,
                whole_word,
            } => {
                let (haystack_owned, needle) = if *match_case {
                    (line.to_string(), needle_orig.as_str())
                } else {
                    (line.to_lowercase(), needle_lower.as_str())
                };
                let haystack = haystack_owned.as_str();
                let mut hits = Vec::new();
                let mut start = 0usize;
                while start <= haystack.len() {
                    let slice = &haystack[start..];
                    let idx = match slice.find(needle) {
                        Some(i) => i,
                        None => break,
                    };
                    let abs_start = start + idx;
                    let abs_end = abs_start + needle.len();
                    let accept = !*whole_word
                        || is_word_boundary(haystack, abs_start, abs_end);
                    if accept {
                        hits.push((abs_start, abs_end));
                    }
                    if abs_end == abs_start {
                        start = abs_end + 1;
                    } else {
                        start = abs_end;
                    }
                }
                hits
            }
            Matcher::Regex(re) => re
                .find_iter(line)
                .map(|m| (m.start(), m.end()))
                .collect(),
        }
    }
}

fn build_matcher(q: &SearchQuery) -> Result<Matcher, String> {
    if q.regex {
        let pattern = if q.match_case {
            q.text.clone()
        } else {
            format!("(?i){}", q.text)
        };
        regex::Regex::new(&pattern)
            .map(Matcher::Regex)
            .map_err(|e| format!("Invalid regex: {e}"))
    } else {
        Ok(Matcher::Plain {
            needle_lower: q.text.to_lowercase(),
            needle_orig: q.text.clone(),
            match_case: q.match_case,
            whole_word: q.whole_word,
        })
    }
}

fn is_word_boundary(text: &str, start: usize, end: usize) -> bool {
    let prev = text[..start].chars().next_back();
    let next = text[end..].chars().next();
    let prev_ok = prev.map_or(true, |c| !is_word_char(c));
    let next_ok = next.map_or(true, |c| !is_word_char(c));
    prev_ok && next_ok
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn walk(root: &Path, matcher: &Matcher, outcome: &mut SearchOutcome) {
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| accept_entry(e))
    {
        if outcome.results.len() >= MAX_FILES_MATCHED
            || outcome.total_hits >= MAX_TOTAL_HITS
        {
            outcome.truncated = true;
            return;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if SKIP_FILENAMES.contains(&name) {
            continue;
        }
        let meta = match path.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.len() > MAX_FILE_SIZE {
            continue;
        }
        if is_binary(path) {
            continue;
        }
        outcome.files_scanned += 1;

        let content = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut file_hits: Vec<SearchHit> = Vec::new();
        let mut stop = false;
        for (i, line) in content.lines().enumerate() {
            for (s, _e) in matcher.find_all(line) {
                file_hits.push(SearchHit {
                    line: i + 1,
                    byte_start: s,
                    preview: trim_preview(line),
                });
                if outcome.total_hits + file_hits.len() >= MAX_TOTAL_HITS {
                    outcome.truncated = true;
                    stop = true;
                    break;
                }
            }
            if stop {
                break;
            }
        }

        if !file_hits.is_empty() {
            outcome.total_hits += file_hits.len();
            outcome.results.push(FileResult {
                path: path.to_path_buf(),
                hits: file_hits,
            });
        }

        if stop {
            return;
        }
    }
}

fn accept_entry(e: &walkdir::DirEntry) -> bool {
    let name_os = e.file_name();
    let name = name_os.to_string_lossy();
    if IGNORED_DIRS.iter().any(|d| *d == name.as_ref()) {
        return false;
    }
    if e.depth() > 0 && name.starts_with('.') && name.as_ref() != ".env" {
        return false;
    }
    true
}

fn is_binary(path: &Path) -> bool {
    use std::io::Read;
    if let Ok(mut f) = std::fs::File::open(path) {
        let mut buf = [0u8; 8192];
        if let Ok(n) = f.read(&mut buf) {
            return buf[..n].contains(&0);
        }
    }
    false
}

fn trim_preview(line: &str) -> String {
    let trimmed = line.trim_start();
    let collected: String = trimmed.chars().take(MAX_LINE_PREVIEW).collect();
    if trimmed.chars().count() > MAX_LINE_PREVIEW {
        format!("{collected}…")
    } else {
        collected
    }
}
