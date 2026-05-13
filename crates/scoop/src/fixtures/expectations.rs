//! 从 fixture 源文本解析期望（expectations）。
//!
//! 支持的指令（只在文件开头的注释区扫描）：
//! - `// EXPECT: pass`
//! - `// EXPECT: fail`
//! - `// EXPECT-ERROR: <substring>`
//! - `// EXPECT-ERROR-CODE: <code>`（例如 `scoop::parse::expected`）
//! - `// EXPECT-ERROR-AT: <line>:<col>`（1-based）
//! - `// ARGS: <args...>`（按空白分割，原样传递给 driver/编译器阶段）
//! - `// ENV: KEY=VALUE`（可重复；按空白分割多个 `KEY=VALUE`）
//! - `// EXPECT-AST: <file>`（parse fixtures：将 AST dump 与 golden 文件做全文比对）
//! - `// RUN-STDOUT: <file>`（run-pass fixtures：stdout golden）
//! - `// RUN-STDERR: <file>`（run-pass fixtures：stderr golden）
//! - `// RUN-STDIN: <file>`（run-pass fixtures：stdin 输入文件，原样写入 stdin）
//! - `// RUN-MODE: run|dump-stackmaps`（run-pass fixtures：默认 run；可切换为运行 `scoop dump-stackmaps`）
//! - `// RUN-STDOUT-CONTAINS: <substring>`（run-pass fixtures：stdout 子串断言）
//! - `// RUN-STDERR-CONTAINS: <substring>`（run-pass fixtures：stderr 子串断言）
//! - `// RUN-STACKMAPS-RECORDS-GT: <n>`（run-pass fixtures：断言 `dump-stackmaps` 输出 records 数量 > n）
//! - `// EXPECT-EXIT: <code>`（run-pass fixtures：期望退出码）
//! - `// TIMEOUT: <ms>`（run-pass fixtures：超时毫秒）
//! - `// BUILD-LLVM-CONTAINS: <substring>`（build fixtures：断言 `--emit-llvm` 产物包含子串；可重复）
//! - `// BUILD-LLVM-REGEX: <regex>`（build fixtures：断言 `--emit-llvm` 产物匹配 regex；可重复）
//! - `// BUILD-LLVM-NOT-CONTAINS: <substring>`（build fixtures：断言 `--emit-llvm` 产物不包含子串；可重复）
//! - `// EXPECT-MONOMORPH-HIT: <n>`（cone fixtures：期望命中 pre-specialize 的实例数量）
//! - `// EXPECT-MONOMORPH-MISS: <n>`（cone fixtures：期望需要本地生成的实例数量）
//! - `// EXPECT-TYPE-MONOMORPH-HIT: <n>`（cone fixtures：期望命中 pre-specialize 的类型实例数量）
//! - `// EXPECT-TYPE-MONOMORPH-MISS: <n>`（cone fixtures：期望需要本地生成的类型实例数量）

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
    pub env: Vec<(String, String)>,
    pub ast_golden: Option<&'a str>,
    pub build_llvm_contains: Vec<&'a str>,
    pub build_llvm_regex: Vec<&'a str>,
    pub build_llvm_not_contains: Vec<&'a str>,
    pub run_stdout: Option<&'a str>,
    pub run_stderr: Option<&'a str>,
    pub run_stdin: Option<&'a str>,
    pub run_mode: Option<&'a str>,
    pub run_stdout_contains: Option<&'a str>,
    pub run_stderr_contains: Option<&'a str>,
    pub run_stackmaps_records_gt: Option<u32>,
    pub expect_exit: Option<i32>,
    pub timeout_ms: Option<u64>,
    pub expect_monomorph_hit: Option<usize>,
    pub expect_monomorph_miss: Option<usize>,
    pub expect_type_monomorph_hit: Option<usize>,
    pub expect_type_monomorph_miss: Option<usize>,
}

impl<'a> FixtureExpectation<'a> {
    pub fn from_source(text: &'a str) -> Self {
        // 默认：pass
        let mut expect = Expect::Pass;
        let mut error_contains = None;
        let mut error_code = None;
        let mut error_at = None;
        let mut args = Vec::new();
        let mut env = Vec::new();
        let mut ast_golden = None;
        let mut build_llvm_contains = Vec::new();
        let mut build_llvm_regex = Vec::new();
        let mut build_llvm_not_contains = Vec::new();
        let mut run_stdout = None;
        let mut run_stderr = None;
        let mut run_stdin = None;
        let mut run_mode = None;
        let mut run_stdout_contains = None;
        let mut run_stderr_contains = None;
        let mut run_stackmaps_records_gt = None;
        let mut expect_exit = None;
        let mut timeout_ms = None;
        let mut expect_monomorph_hit = None;
        let mut expect_monomorph_miss = None;
        let mut expect_type_monomorph_hit = None;
        let mut expect_type_monomorph_miss = None;

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

            if let Some(rest) = directive.strip_prefix("ENV:") {
                let rest = rest.trim();
                env.extend(parse_env_kv_pairs(rest));
            }

            if let Some(rest) = directive.strip_prefix("EXPECT-AST:") {
                ast_golden = Some(rest.trim());
            }

            if let Some(rest) = directive.strip_prefix("BUILD-LLVM-CONTAINS:") {
                let rest = rest.trim();
                if !rest.is_empty() {
                    build_llvm_contains.push(rest);
                }
            }

            if let Some(rest) = directive.strip_prefix("BUILD-LLVM-REGEX:") {
                let rest = rest.trim();
                if !rest.is_empty() {
                    build_llvm_regex.push(rest);
                }
            }

            if let Some(rest) = directive.strip_prefix("BUILD-LLVM-NOT-CONTAINS:") {
                let rest = rest.trim();
                if !rest.is_empty() {
                    build_llvm_not_contains.push(rest);
                }
            }

            if let Some(rest) = directive.strip_prefix("RUN-STDOUT:") {
                run_stdout = Some(rest.trim());
            }

            if let Some(rest) = directive.strip_prefix("RUN-STDERR:") {
                run_stderr = Some(rest.trim());
            }

            if let Some(rest) = directive.strip_prefix("RUN-STDIN:") {
                run_stdin = Some(rest.trim());
            }

            if let Some(rest) = directive.strip_prefix("RUN-MODE:") {
                run_mode = Some(rest.trim());
            }

            if let Some(rest) = directive.strip_prefix("RUN-STDOUT-CONTAINS:") {
                run_stdout_contains = Some(rest.trim());
            }

            if let Some(rest) = directive.strip_prefix("RUN-STDERR-CONTAINS:") {
                run_stderr_contains = Some(rest.trim());
            }

            if let Some(rest) = directive.strip_prefix("RUN-STACKMAPS-RECORDS-GT:") {
                run_stackmaps_records_gt = rest.trim().parse::<u32>().ok();
            }

            if let Some(rest) = directive.strip_prefix("EXPECT-EXIT:") {
                expect_exit = rest.trim().parse::<i32>().ok();
            }

            if let Some(rest) = directive.strip_prefix("TIMEOUT:") {
                timeout_ms = rest.trim().parse::<u64>().ok();
            }

            if let Some(rest) = directive.strip_prefix("EXPECT-MONOMORPH-HIT:") {
                expect_monomorph_hit = rest.trim().parse::<usize>().ok();
            }

            if let Some(rest) = directive.strip_prefix("EXPECT-MONOMORPH-MISS:") {
                expect_monomorph_miss = rest.trim().parse::<usize>().ok();
            }

            if let Some(rest) = directive.strip_prefix("EXPECT-TYPE-MONOMORPH-HIT:") {
                expect_type_monomorph_hit = rest.trim().parse::<usize>().ok();
            }

            if let Some(rest) = directive.strip_prefix("EXPECT-TYPE-MONOMORPH-MISS:") {
                expect_type_monomorph_miss = rest.trim().parse::<usize>().ok();
            }
        }

        Self {
            expect,
            error_contains,
            error_code,
            error_at,
            args,
            env,
            ast_golden,
            build_llvm_contains,
            build_llvm_regex,
            build_llvm_not_contains,
            run_stdout,
            run_stderr,
            run_stdin,
            run_mode,
            run_stdout_contains,
            run_stderr_contains,
            run_stackmaps_records_gt,
            expect_exit,
            timeout_ms,
            expect_monomorph_hit,
            expect_monomorph_miss,
            expect_type_monomorph_hit,
            expect_type_monomorph_miss,
        }
    }
}

fn parse_line_col(s: &str) -> Option<(usize, usize)> {
    let (line, col) = s.split_once(':')?;
    let line = line.trim().parse::<usize>().ok()?;
    let col = col.trim().parse::<usize>().ok()?;
    Some((line, col))
}

fn parse_env_kv_pairs(s: &str) -> impl Iterator<Item = (String, String)> + '_ {
    s.split_whitespace().filter_map(|token| {
        let (key, value) = token.split_once('=')?;
        if key.trim().is_empty() {
            return None;
        }
        Some((key.trim().to_string(), value.to_string()))
    })
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
        assert!(exp.env.is_empty());
        assert_eq!(exp.ast_golden, None);
        assert!(exp.build_llvm_contains.is_empty());
        assert!(exp.build_llvm_regex.is_empty());
        assert!(exp.build_llvm_not_contains.is_empty());
        assert_eq!(exp.run_stdout, None);
        assert_eq!(exp.run_stderr, None);
        assert_eq!(exp.run_stdin, None);
        assert_eq!(exp.run_mode, None);
        assert_eq!(exp.run_stdout_contains, None);
        assert_eq!(exp.run_stderr_contains, None);
        assert_eq!(exp.run_stackmaps_records_gt, None);
        assert_eq!(exp.expect_exit, None);
        assert_eq!(exp.timeout_ms, None);
        assert_eq!(exp.expect_monomorph_hit, None);
        assert_eq!(exp.expect_monomorph_miss, None);
        assert_eq!(exp.expect_type_monomorph_hit, None);
        assert_eq!(exp.expect_type_monomorph_miss, None);
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
        assert!(exp.env.is_empty());
        assert_eq!(exp.ast_golden, None);
        assert!(exp.build_llvm_contains.is_empty());
        assert!(exp.build_llvm_regex.is_empty());
        assert!(exp.build_llvm_not_contains.is_empty());
        assert_eq!(exp.run_stdout, None);
        assert_eq!(exp.run_stderr, None);
        assert_eq!(exp.run_stdin, None);
        assert_eq!(exp.run_mode, None);
        assert_eq!(exp.run_stdout_contains, None);
        assert_eq!(exp.run_stderr_contains, None);
        assert_eq!(exp.run_stackmaps_records_gt, None);
        assert_eq!(exp.expect_exit, None);
        assert_eq!(exp.timeout_ms, None);
        assert_eq!(exp.expect_monomorph_hit, None);
        assert_eq!(exp.expect_monomorph_miss, None);
        assert_eq!(exp.expect_type_monomorph_hit, None);
        assert_eq!(exp.expect_type_monomorph_miss, None);
    }

    #[test]
    fn parse_args_whitespace_split() {
        let exp = FixtureExpectation::from_source(
            "// ARGS: --dump-ast  --emit-llvm   --gc-stress\nfun main() {}\n",
        );
        assert_eq!(exp.args, vec!["--dump-ast", "--emit-llvm", "--gc-stress"]);
        assert!(exp.env.is_empty());
        assert_eq!(exp.ast_golden, None);
        assert!(exp.build_llvm_contains.is_empty());
        assert!(exp.build_llvm_regex.is_empty());
        assert!(exp.build_llvm_not_contains.is_empty());
        assert_eq!(exp.run_stdout, None);
        assert_eq!(exp.run_stderr, None);
        assert_eq!(exp.run_stdin, None);
        assert_eq!(exp.run_stdout_contains, None);
        assert_eq!(exp.run_stderr_contains, None);
        assert_eq!(exp.expect_exit, None);
        assert_eq!(exp.timeout_ms, None);
        assert_eq!(exp.expect_monomorph_hit, None);
        assert_eq!(exp.expect_monomorph_miss, None);
        assert_eq!(exp.expect_type_monomorph_hit, None);
        assert_eq!(exp.expect_type_monomorph_miss, None);
    }

    #[test]
    fn parse_env_key_value_pairs() {
        let exp = FixtureExpectation::from_source(
            "// ENV: FOO=bar BAZ=qux\n// ENV: EMPTY=\nfun main() {}\n",
        );
        assert_eq!(
            exp.env,
            vec![
                ("FOO".to_string(), "bar".to_string()),
                ("BAZ".to_string(), "qux".to_string()),
                ("EMPTY".to_string(), "".to_string()),
            ]
        );
    }

    #[test]
    fn parse_expect_ast_golden_path() {
        let exp = FixtureExpectation::from_source("// EXPECT-AST: hello.ast\nfun main() {}\n");
        assert_eq!(exp.ast_golden, Some("hello.ast"));
    }

    #[test]
    fn parse_run_directives() {
        let exp = FixtureExpectation::from_source(
            "// RUN-STDOUT: out.txt\n// RUN-STDERR: err.txt\n// RUN-STDIN: in.txt\n// RUN-STDOUT-CONTAINS: hello\n// RUN-STDERR-CONTAINS: warn\n// EXPECT-EXIT: 42\n// TIMEOUT: 1500\nfun main() {}\n",
        );
        assert_eq!(exp.run_stdout, Some("out.txt"));
        assert_eq!(exp.run_stderr, Some("err.txt"));
        assert_eq!(exp.run_stdin, Some("in.txt"));
        assert_eq!(exp.run_mode, None);
        assert_eq!(exp.run_stdout_contains, Some("hello"));
        assert_eq!(exp.run_stderr_contains, Some("warn"));
        assert_eq!(exp.run_stackmaps_records_gt, None);
        assert_eq!(exp.expect_exit, Some(42));
        assert_eq!(exp.timeout_ms, Some(1500));
    }

    #[test]
    fn parse_build_llvm_contains_directives() {
        let exp = FixtureExpectation::from_source(
            "// BUILD-LLVM-CONTAINS: define\n// BUILD-LLVM-REGEX: __scoop_priv0__refactor_resume__h[0-9a-f]+\n// BUILD-LLVM-NOT-CONTAINS: inttoptr (i64 1\n// BUILD-LLVM-NOT-CONTAINS: inttoptr (i32 1\nfun main() {}\n",
        );
        assert_eq!(exp.build_llvm_contains, vec!["define"]);
        assert_eq!(
            exp.build_llvm_regex,
            vec!["__scoop_priv0__refactor_resume__h[0-9a-f]+"]
        );
        assert_eq!(
            exp.build_llvm_not_contains,
            vec!["inttoptr (i64 1", "inttoptr (i32 1"]
        );
    }

    #[test]
    fn parse_run_mode_and_stackmaps_records_gt() {
        let exp = FixtureExpectation::from_source(
            "// RUN-MODE: dump-stackmaps\n// RUN-STACKMAPS-RECORDS-GT: 0\nfun main() {}\n",
        );
        assert_eq!(exp.run_mode, Some("dump-stackmaps"));
        assert_eq!(exp.run_stackmaps_records_gt, Some(0));
    }

    #[test]
    fn parse_monomorph_expectations() {
        let exp = FixtureExpectation::from_source(
            "// EXPECT-MONOMORPH-HIT: 2\n// EXPECT-MONOMORPH-MISS: 1\nfun main() {}\n",
        );
        assert_eq!(exp.expect_monomorph_hit, Some(2));
        assert_eq!(exp.expect_monomorph_miss, Some(1));
    }

    #[test]
    fn parse_type_monomorph_expectations() {
        let exp = FixtureExpectation::from_source(
            "// EXPECT-TYPE-MONOMORPH-HIT: 1\n// EXPECT-TYPE-MONOMORPH-MISS: 3\nfun main() {}\n",
        );
        assert_eq!(exp.expect_type_monomorph_hit, Some(1));
        assert_eq!(exp.expect_type_monomorph_miss, Some(3));
    }
}
