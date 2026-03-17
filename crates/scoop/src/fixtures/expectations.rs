//! 从 fixture 源文本解析期望（expectations）。
//!
//! 支持的指令（只在文件开头的注释区扫描）：
//! - `// EXPECT: pass`
//! - `// EXPECT: fail`
//! - `// EXPECT-ERROR: <substring>`

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    Pass,
    Fail,
}

#[derive(Debug, Clone)]
pub struct FixtureExpectation<'a> {
    pub expect: Expect,
    pub error_contains: Option<&'a str>,
}

impl<'a> FixtureExpectation<'a> {
    pub fn from_source(text: &'a str) -> Self {
        // 默认：pass
        let mut expect = Expect::Pass;
        let mut error_contains = None;

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
        }

        Self {
            expect,
            error_contains,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_expect_pass_default() {
        let exp = FixtureExpectation::from_source("fun main() {}\n");
        assert_eq!(exp.expect, Expect::Pass);
        assert_eq!(exp.error_contains, None);
    }

    #[test]
    fn parse_expect_fail_with_error() {
        let exp = FixtureExpectation::from_source(
            "// EXPECT: fail\n// EXPECT-ERROR: boom\n\nfun main() {}\n",
        );
        assert_eq!(exp.expect, Expect::Fail);
        assert_eq!(exp.error_contains, Some("boom"));
    }
}

