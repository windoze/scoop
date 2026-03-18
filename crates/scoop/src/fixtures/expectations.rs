//! 从 fixture 源文本解析期望（expectations）。
//!
//! 支持的指令（只在文件开头的注释区扫描）：
//! - `// EXPECT: pass`
//! - `// EXPECT: fail`
//! - `// EXPECT-ERROR: <substring>`
//! - `// EXPECT-ERROR-CODE: <code>`（例如 `scoop::parse::expected`）
//! - `// EXPECT-ERROR-AT: <line>:<col>`（1-based）
//! - `// ARGS: <args...>`（按空白分割，原样传递给 driver/编译器阶段）
//! - `// EXPECT-AST: <file>`（parse fixtures：将 AST dump 与 golden 文件做全文比对）

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    Pass,
    Fail,
}

#[derive(Debug, Clone)]
pub struct FixtureExpectation<'a> {
    pub expect: Expect,
    pub error_contains: Option<&'a str>,
    pub error_code: Option<&'a str>,
    pub error_at: Option<(usize, usize)>,
    pub args: Vec<String>,
    pub ast_golden: Option<&'a str>,
}

impl<'a> FixtureExpectation<'a> {
    pub fn from_source(text: &'a str) -> Self {
        // 默认：pass
        let mut expect = Expect::Pass;
        let mut error_contains = None;
        let mut error_code = None;
        let mut error_at = None;
        let mut args = Vec::new();
        let mut ast_golden = None;

        // 只扫描开头若干行，避免把正文里的 `// EXPECT:` 误判为指令。
        for line in text.lines().take(32) {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("//") {
                // 一旦遇到非注释行，就停止扫描（文件头部注释区结束）
                break;
            }

            let directive = trimmed.trim_start_matches("//").trim();

            if let Some(rest) = directive.strip_prefix("EXPECT:") {
                let rest = rest.trim();
                expect = match rest {
                    "pass" => Expect::Pass,
                    "fail" => Expect::Fail,
                    _ => expect,
                };
            }

            if let Some(rest) = directive.strip_prefix("EXPECT-ERROR:") {
                error_contains = Some(rest.trim());
            }

            if let Some(rest) = directive.strip_prefix("EXPECT-ERROR-CODE:") {
                error_code = Some(rest.trim());
            }

            if let Some(rest) = directive.strip_prefix("EXPECT-ERROR-AT:") {
                error_at = parse_line_col(rest.trim());
            }

            if let Some(rest) = directive.strip_prefix("ARGS:") {
                let rest = rest.trim();
                args.extend(rest.split_whitespace().map(|s| s.to_string()));
            }

            if let Some(rest) = directive.strip_prefix("EXPECT-AST:") {
                ast_golden = Some(rest.trim());
            }
        }

        Self {
            expect,
            error_contains,
            error_code,
            error_at,
            args,
            ast_golden,
        }
    }
}

fn parse_line_col(s: &str) -> Option<(usize, usize)> {
    let (line, col) = s.split_once(':')?;
    let line = line.trim().parse::<usize>().ok()?;
    let col = col.trim().parse::<usize>().ok()?;
    Some((line, col))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_expect_pass_default() {
        let exp = FixtureExpectation::from_source("fun main() {}\n");
        assert_eq!(exp.expect, Expect::Pass);
        assert_eq!(exp.error_contains, None);
        assert_eq!(exp.error_code, None);
        assert_eq!(exp.error_at, None);
        assert!(exp.args.is_empty());
        assert_eq!(exp.ast_golden, None);
    }

    #[test]
    fn parse_expect_fail_with_error() {
        let exp = FixtureExpectation::from_source(
            "// EXPECT: fail\n// EXPECT-ERROR: boom\n// EXPECT-ERROR-CODE: scoop::parse::expected\n// EXPECT-ERROR-AT: 3:5\n\nfun main() {}\n",
        );
        assert_eq!(exp.expect, Expect::Fail);
        assert_eq!(exp.error_contains, Some("boom"));
        assert_eq!(exp.error_code, Some("scoop::parse::expected"));
        assert_eq!(exp.error_at, Some((3, 5)));
        assert!(exp.args.is_empty());
        assert_eq!(exp.ast_golden, None);
    }

    #[test]
    fn parse_args_whitespace_split() {
        let exp = FixtureExpectation::from_source(
            "// ARGS: --dump-ast  --emit-llvm   --gc-stress\nfun main() {}\n",
        );
        assert_eq!(
            exp.args,
            vec!["--dump-ast", "--emit-llvm", "--gc-stress"]
        );
        assert_eq!(exp.ast_golden, None);
    }

    #[test]
    fn parse_expect_ast_golden_path() {
        let exp = FixtureExpectation::from_source("// EXPECT-AST: hello.ast\nfun main() {}\n");
        assert_eq!(exp.ast_golden, Some("hello.ast"));
    }
}
