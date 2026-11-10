//! Glob patterns for "files to include / exclude" (`vs/base/common/glob.ts`
//! syntax: `*`, `**`, `?`, `{a,b}`, `[...]`), and the Search view's
//! expansion of what's typed (queryBuilder.ts `parseSearchPaths`): patterns
//! separated by commas, each matching anywhere under the folder
//! (`**/p`) and everything inside a match (`p/**`); a leading `./` or `/`
//! anchors to the folder.

use regex::Regex;

/// A compiled set of patterns; matches a path relative to the searched
/// folder, with `/` separators.
#[derive(Clone, Debug, Default)]
pub struct GlobSet {
    patterns: Vec<Regex>,
}

impl GlobSet {
    /// Parses the text of a "files to include/exclude" box.
    pub fn parse(text: &str) -> GlobSet {
        let mut patterns = Vec::new();
        for raw in split_patterns(text) {
            let p = raw.trim().replace('\\', "/");
            if p.is_empty() {
                continue;
            }
            let anchored = p.starts_with("./") || p.starts_with('/');
            let p = p.trim_start_matches("./").trim_start_matches('/').trim_end_matches('/');
            let base = if anchored || p.starts_with("**/") { p.to_string() } else { format!("**/{p}") };
            for g in [base.clone(), format!("{base}/**")] {
                if let Some(re) = to_regex(&g) {
                    patterns.push(re);
                }
            }
        }
        GlobSet { patterns }
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn matches(&self, relative: &str) -> bool {
        self.patterns.iter().any(|re| re.is_match(relative))
    }
}

/// Splits on commas outside `{...}` groups.
fn split_patterns(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '{' => {
                depth += 1;
                cur.push(c);
            }
            '}' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// A glob as an anchored regex.
pub fn to_regex(glob: &str) -> Option<Regex> {
    let mut re = String::from("^");
    let chars: Vec<char> = glob.chars().collect();
    let mut i = 0;
    let mut in_group = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '*' if chars.get(i + 1) == Some(&'*') => {
                // `**/` = any number of folders; a trailing `**` = anything.
                if chars.get(i + 2) == Some(&'/') {
                    re.push_str("(?:[^/]*/)*");
                    i += 3;
                    continue;
                }
                re.push_str(".*");
                i += 2;
                continue;
            }
            '*' => re.push_str("[^/]*"),
            '?' => re.push_str("[^/]"),
            '{' => {
                in_group += 1;
                re.push_str("(?:");
            }
            '}' if in_group > 0 => {
                in_group -= 1;
                re.push(')');
            }
            ',' if in_group > 0 => re.push('|'),
            '[' => {
                let end = chars[i + 1..].iter().position(|&c| c == ']').map(|p| i + 1 + p);
                match end {
                    Some(end) => {
                        let body: String = chars[i + 1..end].iter().collect();
                        let body = body.strip_prefix('!').map(|b| format!("^{b}")).unwrap_or(body);
                        re.push('[');
                        re.push_str(&body.replace('\\', "\\\\"));
                        re.push(']');
                        i = end + 1;
                        continue;
                    }
                    None => re.push_str("\\["),
                }
            }
            c => re.push_str(&regex::escape(&c.to_string())),
        }
        i += 1;
    }
    re.push('$');
    Regex::new(&re).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_like_the_search_view() {
        let set = GlobSet::parse("*.ts, src/**/include");
        assert!(set.matches("nuxt.config.ts"));
        assert!(set.matches("server/api/x.ts"));
        assert!(set.matches("src/a/b/include"));
        assert!(set.matches("src/a/include/file.rs"));
        assert!(!set.matches("app.vue"));
        let folder = GlobSet::parse("server");
        assert!(folder.matches("server/api/hello.ts"));
        assert!(!folder.matches("pages/index.vue"));
        let anchored = GlobSet::parse("./pages");
        assert!(anchored.matches("pages/index.vue"));
        assert!(!anchored.matches("x/pages/index.vue"));
        let braces = GlobSet::parse("*.{ts,vue}");
        assert!(braces.matches("app.vue") && braces.matches("a/b.ts") && !braces.matches("a.json"));
        assert!(GlobSet::parse("  ").is_empty());
    }
}
