//! Bounded native compiler for RICS variables and nested CSS used by lyric themes.
//! Browser selectors are kept intact; unsupported language directives are errors,
//! never silently evaluated or passed to a JavaScript runtime.
use std::{collections::BTreeMap, time::Duration};
use web_time::Instant;
#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub timeout_ms: u64,
    pub max_iterations: usize,
}
impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            timeout_ms: 3000,
            max_iterations: 10000,
        }
    }
}
#[derive(Debug, Clone)]
pub struct CompileResult {
    pub css: String,
    pub errors: Vec<String>,
    pub timed_out: bool,
}
struct Compiler {
    vars: BTreeMap<String, String>,
    steps: usize,
    options: CompileOptions,
    start: Instant,
}
impl Compiler {
    fn check(&mut self) -> Result<(), String> {
        self.steps += 1;
        if self.steps > self.options.max_iterations
            || self.start.elapsed() > Duration::from_millis(self.options.timeout_ms)
        {
            Err("Compilation limit reached".into())
        } else {
            Ok(())
        }
    }
    fn expand(&mut self, s: &str) -> Result<String, String> {
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            self.check()?;
            if c == '$' {
                let mut name = String::new();
                while chars
                    .peek()
                    .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '-'))
                {
                    name.push(chars.next().unwrap());
                }
                let value = self
                    .vars
                    .get(&name)
                    .ok_or_else(|| format!("Undefined variable ${name}"))?;
                out.push_str(value);
            } else {
                out.push(c);
            }
        }
        Ok(out)
    }
    fn block(&mut self, src: &str, parents: &[String], depth: usize) -> Result<String, String> {
        if depth > 64 {
            return Err("Theme nesting exceeds 64 levels".into());
        }
        let mut out = String::new();
        let mut declarations = String::new();
        let mut start = 0;
        let mut i = 0;
        let bytes = src.as_bytes();
        let mut quote = 0u8;
        let mut parens = 0usize;
        while i < bytes.len() {
            self.check()?;
            let c = bytes[i];
            if quote != 0 {
                if c == b'\\' {
                    i += 2;
                    continue;
                }
                if c == quote {
                    quote = 0;
                }
                i += 1;
                continue;
            }
            if c == b'\'' || c == b'"' {
                quote = c;
                i += 1;
                continue;
            }
            if c == b'/' && bytes.get(i + 1) == Some(&b'*') {
                let end = src[i + 2..].find("*/").ok_or("Unterminated comment")? + i + 4;
                declarations.push_str(&src[i..end]);
                if start == i {
                    start = end;
                }
                i = end;
                continue;
            }
            if c == b'(' {
                parens += 1;
            } else if c == b')' {
                parens = parens.saturating_sub(1);
            }
            if parens == 0 && c == b';' {
                let statement = src[start..i].trim();
                if let Some(var) = statement.strip_prefix('$') {
                    let (name, value) = var.split_once(':').ok_or("Invalid variable")?;
                    let expanded = self.expand(value.trim())?;
                    self.vars.insert(name.trim().into(), expanded);
                } else if !statement.is_empty() {
                    if statement.starts_with('@') {
                        return Err(format!("Unsupported RICS directive: {statement}"));
                    }
                    declarations.push_str(&self.expand(statement)?);
                    declarations.push(';');
                }
                start = i + 1;
            }
            if parens == 0 && c == b'{' {
                let selector = self.expand(src[start..i].trim())?;
                if selector.is_empty() {
                    return Err("Empty selector".into());
                }
                let mut nest = 1;
                let mut end = i + 1;
                let mut q = 0u8;
                while end < bytes.len() && nest > 0 {
                    self.check()?;
                    let b = bytes[end];
                    if q != 0 {
                        if b == b'\\' {
                            end += 2;
                            continue;
                        }
                        if b == q {
                            q = 0;
                        }
                    } else if b == b'\'' || b == b'"' {
                        q = b;
                    } else if b == b'{' {
                        nest += 1;
                    } else if b == b'}' {
                        nest -= 1;
                    }
                    end += 1;
                }
                if nest != 0 {
                    return Err("Unclosed rule".into());
                }
                let children = selector
                    .split(',')
                    .map(str::trim)
                    .flat_map(|child| {
                        if parents.is_empty() {
                            vec![child.to_string()]
                        } else {
                            parents
                                .iter()
                                .map(|parent| {
                                    if child.contains('&') {
                                        child.replace('&', parent)
                                    } else {
                                        format!("{parent} {child}")
                                    }
                                })
                                .collect()
                        }
                    })
                    .collect::<Vec<_>>();
                if selector.starts_with('@') {
                    if !["@media", "@supports", "@layer", "@container"]
                        .iter()
                        .any(|p| selector.starts_with(p))
                    {
                        return Err(format!("Unsupported RICS directive: {selector}"));
                    }
                    out.push_str(&format!(
                        "{selector}{{{}}}",
                        self.block(&src[i + 1..end - 1], parents, depth + 1)?
                    ));
                } else {
                    out.push_str(&self.block(&src[i + 1..end - 1], &children, depth + 1)?);
                }
                i = end;
                start = end;
                continue;
            }
            if parens == 0 && c == b'}' {
                return Err("Unexpected closing brace".into());
            }
            i += 1;
        }
        let tail = src[start..].trim();
        if !tail.is_empty() {
            declarations.push_str(&self.expand(tail)?);
        }
        if !declarations.trim().is_empty() {
            if parents.is_empty() {
                if declarations.trim_start().starts_with("/*") {
                    out.insert_str(0, &declarations);
                } else {
                    return Err("Declaration outside a rule".into());
                }
            } else {
                out.insert_str(0, &format!("{}{{{declarations}}}", parents.join(",")));
            }
        }
        Ok(out)
    }
}
pub fn compile(source: &str, options: CompileOptions) -> CompileResult {
    let mut c = Compiler {
        vars: BTreeMap::new(),
        steps: 0,
        options,
        start: Instant::now(),
    };
    match c.block(source, &[], 0) {
        Ok(css) => CompileResult {
            css,
            errors: vec![],
            timed_out: false,
        },
        Err(error) => CompileResult {
            css: source.into(),
            timed_out: error == "Compilation limit reached",
            errors: vec![error],
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variables_nesting_and_parent_selectors() {
        let r=compile("$color: #f00; .parent { color: $color; .child { color: green; } &:hover {opacity:1;} }",CompileOptions::default());
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.css.contains(".parent .child"));
        assert!(r.css.contains(".parent:hover"));
        assert!(r.css.contains("#f00"));
    }
    #[test]
    fn bounds_and_invalid_input_return_source() {
        let src = "bad {{";
        assert!(!compile(src, CompileOptions::default()).errors.is_empty());
        assert!(
            compile(
                ".a {color:red;}",
                CompileOptions {
                    max_iterations: 1,
                    ..Default::default()
                }
            )
            .timed_out
        );
    }
}
