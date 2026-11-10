//! `when` clauses (context key expressions) for contributed views and menus.
//!
//! VS Code analogue: `src/vs/platform/contextkey/common/contextkey.ts`.
//! Supported: keys, `true`/`false`, quoted strings, numbers, `!`, `&&`,
//! `||`, parentheses, `==`/`===`, `!=`/`!==`, `<`, `<=`, `>`, `>=`, `in`.
//! Unknown keys are undefined (falsy), as in VS Code.

use std::collections::BTreeMap;

use serde_json::Value;

/// Evaluate `expr` against the context keys (`setContext` values).
pub fn eval(expr: &str, keys: &BTreeMap<String, Value>) -> bool {
    let tokens = tokenize(expr);
    let mut p = Parser { tokens, pos: 0, keys };
    truthy(&p.or())
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Str(String),
    Num(f64),
    Op(&'static str),
}

fn tokenize(s: &str) -> Vec<Tok> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let rest: String = chars[i..chars.len().min(i + 3)].iter().collect();
        let op = ["===", "!==", "==", "!=", "&&", "||", "<=", ">=", "=~"]
            .into_iter()
            .find(|o| rest.starts_with(o));
        if let Some(o) = op {
            out.push(Tok::Op(o));
            i += o.len();
            continue;
        }
        if "!()<>".contains(c) {
            out.push(Tok::Op(match c {
                '!' => "!",
                '(' => "(",
                ')' => ")",
                '<' => "<",
                _ => ">",
            }));
            i += 1;
            continue;
        }
        if c == '\'' || c == '"' {
            let mut j = i + 1;
            while j < chars.len() && chars[j] != c {
                j += 1;
            }
            out.push(Tok::Str(chars[i + 1..j.min(chars.len())].iter().collect()));
            i = j + 1;
            continue;
        }
        let mut j = i;
        while j < chars.len() && !chars[j].is_whitespace() && !"!()<>=&|".contains(chars[j]) {
            j += 1;
        }
        if j == i {
            i += 1;
            continue;
        }
        let word: String = chars[i..j].iter().collect();
        out.push(match word.parse::<f64>() {
            Ok(n) => Tok::Num(n),
            Err(_) => Tok::Ident(word),
        });
        i = j;
    }
    out
}

struct Parser<'a> {
    tokens: Vec<Tok>,
    pos: usize,
    keys: &'a BTreeMap<String, Value>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn eat(&mut self, op: &str) -> bool {
        if self.peek() == Some(&Tok::Op(leak(op))) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn or(&mut self) -> Value {
        let mut v = self.and();
        while self.eat("||") {
            let r = self.and();
            v = Value::Bool(truthy(&v) || truthy(&r));
        }
        v
    }

    fn and(&mut self) -> Value {
        let mut v = self.unary();
        while self.eat("&&") {
            let r = self.unary();
            v = Value::Bool(truthy(&v) && truthy(&r));
        }
        v
    }

    fn unary(&mut self) -> Value {
        if self.eat("!") {
            return Value::Bool(!truthy(&self.unary()));
        }
        self.comparison()
    }

    fn comparison(&mut self) -> Value {
        let left = self.primary();
        let op = match self.peek() {
            Some(Tok::Op(o)) if ["==", "===", "!=", "!==", "<", "<=", ">", ">=", "=~"].contains(o) => *o,
            Some(Tok::Ident(w)) if w == "in" => "in",
            _ => return left,
        };
        self.pos += 1;
        // The right side of a comparison is a literal: `view == explorer`
        // compares with the string "explorer".
        let right = match self.tokens.get(self.pos).cloned() {
            Some(Tok::Ident(w)) if w != "true" && w != "false" && op != "in" => {
                self.pos += 1;
                Value::String(w)
            }
            _ => self.primary(),
        };
        let num = |v: &Value| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()));
        Value::Bool(match op {
            "==" | "===" => loose_eq(&left, &right),
            "!=" | "!==" => !loose_eq(&left, &right),
            "<" => num(&left) < num(&right) && num(&left).is_some(),
            "<=" => num(&left) <= num(&right) && num(&left).is_some(),
            ">" => num(&left) > num(&right) && num(&right).is_some(),
            ">=" => num(&left) >= num(&right) && num(&right).is_some(),
            "in" => match &right {
                Value::Array(a) => a.iter().any(|x| loose_eq(x, &left)),
                Value::Object(o) => left.as_str().is_some_and(|k| o.contains_key(k)),
                _ => false,
            },
            // Regular expressions aren't evaluated: never matches.
            _ => false,
        })
    }

    fn primary(&mut self) -> Value {
        match self.tokens.get(self.pos).cloned() {
            Some(Tok::Op("(")) => {
                self.pos += 1;
                let v = self.or();
                self.eat(")");
                v
            }
            Some(Tok::Str(s)) => {
                self.pos += 1;
                Value::String(s)
            }
            Some(Tok::Num(n)) => {
                self.pos += 1;
                serde_json::json!(n)
            }
            Some(Tok::Ident(w)) => {
                self.pos += 1;
                match w.as_str() {
                    "true" => Value::Bool(true),
                    "false" => Value::Bool(false),
                    _ => self.keys.get(&w).cloned().unwrap_or_else(|| builtin(&w)),
                }
            }
            _ => {
                self.pos += 1;
                Value::Null
            }
        }
    }
}

/// Keys VS Code always defines.
fn builtin(key: &str) -> Value {
    match key {
        "isMac" => Value::Bool(cfg!(target_os = "macos")),
        "isLinux" => Value::Bool(cfg!(target_os = "linux")),
        "isWindows" => Value::Bool(cfg!(target_os = "windows")),
        "isWeb" | "isMobile" => Value::Bool(false),
        "workspaceFolderCount" => serde_json::json!(1),
        _ => Value::Null,
    }
}

fn loose_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::String(x), y) | (y, Value::String(x)) if !y.is_string() => match y {
            Value::Bool(v) => x == &v.to_string(),
            Value::Number(n) => x == &n.to_string(),
            Value::Null => false,
            _ => false,
        },
        _ => a == b,
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

fn leak(op: &str) -> &'static str {
    ["===", "!==", "==", "!=", "&&", "||", "<=", ">=", "=~", "!", "(", ")", "<", ">"]
        .into_iter()
        .find(|o| *o == op)
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_like_vs_code() {
        let mut k = BTreeMap::new();
        k.insert("claude-code:doesNotSupportSecondarySidebar".to_string(), Value::Bool(false));
        k.insert("view".to_string(), Value::String("explorer".into()));
        k.insert("count".to_string(), serde_json::json!(3));
        assert!(!eval("claude-code:doesNotSupportSecondarySidebar", &k));
        assert!(eval("!claude-code:doesNotSupportSecondarySidebar", &k));
        assert!(eval("view == explorer && count > 2", &k));
        assert!(eval("view == 'other' || (count >= 3 && !missing)", &k));
        assert!(!eval("missing", &k));
        assert!(eval("isMac || !isMac", &k));
    }
}
