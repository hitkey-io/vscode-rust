//! JSON with comments and trailing commas, as VS Code reads its theme,
//! settings and `extensions.json` files (`vs/base/common/json.ts`).

use serde_json::Value;

/// Parses JSONC: `//` and `/* */` comments and trailing commas are allowed.
pub fn parse(text: &str) -> Option<Value> {
    serde_json::from_str(&strip(text)).ok()
}

/// Removes comments and trailing commas, leaving strings untouched.
pub fn strip(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut prev = ' ';
                for n in chars.by_ref() {
                    if prev == '*' && n == '/' {
                        break;
                    }
                    prev = n;
                }
            }
            (']' | '}', _) => {
                // drop a trailing comma before the closing bracket
                let trimmed = out.trim_end().len();
                if out[..trimmed].ends_with(',') {
                    out.truncate(trimmed - 1);
                }
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_trailing_commas() {
        let v = parse("{\n // c\n \"a\": [\"x\" /* y */, \"c//d\",],\n \"b\": {\"k\": 1,},\n}").unwrap();
        assert_eq!(v["a"][1], "c//d");
        assert_eq!(v["b"]["k"], 1);
        assert_eq!(parse("{\"s\": \"a,]\"}").unwrap()["s"], "a,]");
    }
}
