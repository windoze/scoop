//! Fixtures 覆盖矩阵检查（最小版）。
//!
//! 当前策略（对应 TODO:T0110 / PLAN §10.6）：
//! - 仅统计规范 `SCOOP_FULL_SPEC.md` 内出现的 doctest fixtures（带 `// FIXTURE:` 的 fenced code block）。
//! - 章节划分采用 `## N. Title` 这一层（即 spec 的一级编号章节）。
//! - 每个章节至少准备 1 个 `// EXPECT: pass` 与 1 个 `// EXPECT: fail`。
//! - 本工具只输出报告，不会因缺口返回错误（避免阻塞迭代）。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectationKind {
    Pass,
    Fail,
    Unknown,
    MissingFile,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Coverage {
    pass: usize,
    fail: usize,
    unknown: usize,
    missing_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    pub chapter_id: String,
    pub title: String,
    pub pass: usize,
    pub fail: usize,
    pub missing_pass: bool,
    pub missing_fail: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub chapter_count: usize,
    pub fixture_count: usize,
    pub unmapped_fixture_count: usize,
    pub gaps: Vec<Gap>,
}

impl Report {
    pub fn render(&self) -> String {
        let mut out = String::new();
        if self.gaps.is_empty() {
            out.push_str(&format!(
                "fixtures matrix: ok (chapters={}, fixtures={})",
                self.chapter_count, self.fixture_count
            ));
            if self.unmapped_fixture_count > 0 {
                out.push_str(&format!(
                    " (unmapped fixtures={})",
                    self.unmapped_fixture_count
                ));
            }
            return out;
        }

        out.push_str(&format!(
            "fixtures matrix: gaps (chapters={}, fixtures={}, missing={})\n",
            self.chapter_count,
            self.fixture_count,
            self.gaps.len()
        ));
        out.push_str("note: 当前仅统计规范内 `// FIXTURE:` 的 doctest fixtures（按 `## N. Title` 章节归类）。\n");

        for g in &self.gaps {
            let mut missing = Vec::new();
            if g.missing_pass {
                missing.push("pass");
            }
            if g.missing_fail {
                missing.push("fail");
            }
            out.push_str(&format!(
                "- §{} {}: pass={} fail={} missing=[{}]\n",
                g.chapter_id,
                g.title,
                g.pass,
                g.fail,
                missing.join(", ")
            ));
        }

        if self.unmapped_fixture_count > 0 {
            out.push_str(&format!(
                "unmapped fixtures (no chapter context): {}\n",
                self.unmapped_fixture_count
            ));
        }

        out
    }
}

#[derive(Debug, Clone)]
struct SpecFixtureRef {
    rel_path: PathBuf,
    chapter_id: Option<String>,
}

pub fn run_check(spec_path: &Path, fixtures_root: &Path) -> Result<Report> {
    let spec_text = std::fs::read_to_string(spec_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取规范文件：{}", spec_path.display()))?;

    let (chapters, fixture_refs) = parse_spec_chapters_and_fixtures(&spec_text)?;

    let chapter_ids: BTreeSet<String> = chapters.iter().map(|c| c.id.clone()).collect();

    let mut coverage_by_chapter: BTreeMap<String, Coverage> = BTreeMap::new();
    let mut unmapped_fixture_count: usize = 0;

    for f in &fixture_refs {
        let Some(chapter_id) = &f.chapter_id else {
            unmapped_fixture_count += 1;
            continue;
        };
        if !chapter_ids.contains(chapter_id) {
            unmapped_fixture_count += 1;
            continue;
        }

        let kind = read_fixture_expectation(fixtures_root, &f.rel_path)?;
        let cov = coverage_by_chapter.entry(chapter_id.clone()).or_default();
        match kind {
            ExpectationKind::Pass => cov.pass += 1,
            ExpectationKind::Fail => cov.fail += 1,
            ExpectationKind::Unknown => cov.unknown += 1,
            ExpectationKind::MissingFile => {
                cov.unknown += 1;
                cov.missing_files += 1;
            }
        }
    }

    let mut gaps: Vec<Gap> = Vec::new();
    for ch in &chapters {
        let cov = coverage_by_chapter.get(&ch.id).copied().unwrap_or_default();
        let missing_pass = cov.pass == 0;
        let missing_fail = cov.fail == 0;
        if missing_pass || missing_fail {
            gaps.push(Gap {
                chapter_id: ch.id.clone(),
                title: ch.title.clone(),
                pass: cov.pass,
                fail: cov.fail,
                missing_pass,
                missing_fail,
            });
        }
    }

    Ok(Report {
        chapter_count: chapters.len(),
        fixture_count: fixture_refs.len(),
        unmapped_fixture_count,
        gaps,
    })
}

fn parse_spec_chapters_and_fixtures(
    spec_text: &str,
) -> Result<(Vec<Chapter>, Vec<SpecFixtureRef>)> {
    let mut chapters: Vec<Chapter> = Vec::new();
    let mut fixtures: Vec<SpecFixtureRef> = Vec::new();

    let mut in_block = false;
    let mut block_lines: Vec<&str> = Vec::new();
    let mut current_chapter_id: Option<String> = None;

    for line in spec_text.lines() {
        if let Some(ch) = parse_chapter_heading(line) {
            current_chapter_id = Some(ch.id.clone());
            chapters.push(ch);
        }

        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_block {
                // close
                if let Some(rel_path) = parse_fixture_path_from_block(&block_lines)? {
                    fixtures.push(SpecFixtureRef {
                        rel_path,
                        chapter_id: current_chapter_id.clone(),
                    });
                }
                block_lines.clear();
                in_block = false;
            } else {
                // open（忽略 info string）
                in_block = true;
            }
            continue;
        }

        if in_block {
            block_lines.push(line);
        }
    }

    if in_block {
        miette::bail!("规范文件存在未闭合的 fenced code block（缺少 ```）");
    }

    // 去重：避免同一章节标题被重复记录（例如 spec 中重复出现同编号）。
    // 这里保持“第一次出现”为准。
    let mut seen: BTreeSet<String> = BTreeSet::new();
    chapters.retain(|c| seen.insert(c.id.clone()));

    Ok((chapters, fixtures))
}

fn parse_chapter_heading(line: &str) -> Option<Chapter> {
    let line = line.trim_start();
    let rest = line.strip_prefix("## ")?;
    let rest = rest.trim();

    // 期望：`N. Title`
    let (num, after_dot) = rest.split_once('.')?;
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let title = after_dot.trim();
    if title.is_empty() {
        return None;
    }

    Some(Chapter {
        id: num.to_string(),
        title: title.to_string(),
    })
}

fn parse_fixture_path_from_block(lines: &[&str]) -> Result<Option<PathBuf>> {
    for line in lines.iter().take(64) {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("//") {
            break;
        }
        let directive = trimmed.trim_start_matches("//").trim();
        if let Some(rest) = directive.strip_prefix("FIXTURE:") {
            return Ok(Some(PathBuf::from(rest.trim())));
        }
    }
    Ok(None)
}

fn read_fixture_expectation(fixtures_root: &Path, rel_path: &Path) -> Result<ExpectationKind> {
    let abs = fixtures_root.join(rel_path);
    match std::fs::read_to_string(&abs) {
        Ok(text) => Ok(parse_expectation_from_source(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ExpectationKind::MissingFile),
        Err(e) => Err(e)
            .into_diagnostic()
            .wrap_err_with(|| format!("无法读取 fixture 文件：{}", abs.display())),
    }
}

fn parse_expectation_from_source(source: &str) -> ExpectationKind {
    for line in source.lines().take(32) {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("//") {
            break;
        }
        let directive = trimmed.trim_start_matches("//").trim();
        let Some(rest) = directive.strip_prefix("EXPECT:") else {
            continue;
        };
        match rest.trim() {
            "pass" => return ExpectationKind::Pass,
            "fail" => return ExpectationKind::Fail,
            _ => return ExpectationKind::Unknown,
        }
    }
    ExpectationKind::Unknown
}

// ---------------------------------------------------------------------------
// Stdlib 领域覆盖矩阵（T1803）
// ---------------------------------------------------------------------------

/// stdlib 领域定义：id / 显示名 / fixture 文件名前缀匹配模式。
///
/// 约定：一个 run-pass fixture 匹配某领域，当且仅当其文件名（不含路径与 `.scoop`）
/// 以该领域的某个前缀开头（`starts_with` 匹配）。
const STDLIB_DOMAINS: &[(&str, &str, &[&str])] = &[
    ("1", "Core types & primitives", &[
        "minimal_", "value_type_", "class_", "enum_", "option_", "struct_",
        "string_", "bool_", "int_", "float_", "array_", "tuple_",
    ]),
    ("2", "Properties / Delegates", &[
        "delegated_prop", "lazy_", "observable_", "vetoable_",
    ]),
    ("3", "Collections", &[
        "stdlib_iter_", "stdlib_set_", "stdlib_smoke_collections",
        "mutable_array_", "list_and_mutable_list",
        "stdlib_collections",
    ]),
    ("4", "Ranges / Progressions", &[
        "kotlin_ranges_", "stdlib_smoke_ranges",
    ]),
    ("5", "Text (String)", &[
        "stdlib_string_", "string_interp", "string_escape",
        "string_multiline", "string_trim",
    ]),
    ("6", "Text formatting", &[
        "stdlib_string_builder", "stdlib_format",
    ]),
    ("7", "Math", &[
        "stdlib_math",
    ]),
    ("8", "Hashing", &[
        "stdlib_hash",
    ]),
    ("9", "Random", &[
        "stdlib_random",
    ]),
    ("10", "Time", &[
        "std_env_time",
    ]),
    ("11", "IO (stdin/stdout/stderr)", &[
        "std_io_",
    ]),
    ("12", "File system", &[
        "std_fs_",
    ]),
    ("13", "Process / Env / Path", &[
        "std_process_", "std_path_", "std_env_",
    ]),
    ("14", "Concurrency / Threading", &[
        "std_sync_", "std_thread_",
    ]),
    ("15", "Task / Executor (async)", &[
        "std_task_",
    ]),
    ("16", "Net", &[
        "std_net_",
    ]),
    ("17", "Unsafe / Pointers", &[
        "unsafe_", "nogc_",
    ]),
    ("18", "Scope functions", &[
        "kotlin_scope_functions",
    ]),
    ("19", "Preconditions", &[
        "kotlin_require_", "kotlin_check_",
        "stdlib_smoke_test_and_preconditions",
    ]),
    ("20", "Test utilities", &[
        "std_test_",
    ]),
    ("21", "Reflection", &[
        "comptime_reflect", "comptime_fields", "comptime_variants",
    ]),
];

/// 按 stdlib 领域扫描 `run-pass/` 下的 fixture 覆盖度。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibDomainCoverage {
    pub domain_id: String,
    pub title: String,
    pub fixture_count: usize,
    pub fixture_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibReport {
    pub domain_count: usize,
    pub total_fixtures: usize,
    pub covered: Vec<StdlibDomainCoverage>,
    pub gaps: Vec<StdlibDomainCoverage>,
}

impl StdlibReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        let covered_count = self.covered.len();
        let gap_count = self.gaps.len();

        out.push_str(&format!(
            "stdlib coverage: {}/{} domains covered ({} fixtures)\n",
            covered_count, self.domain_count, self.total_fixtures
        ));

        if !self.gaps.is_empty() {
            out.push_str(&format!("\ngaps ({} domains without fixtures):\n", gap_count));
            for g in &self.gaps {
                out.push_str(&format!("  - §{} {}\n", g.domain_id, g.title));
            }
        }

        out.push_str("\ncovered domains:\n");
        for c in &self.covered {
            out.push_str(&format!(
                "  + §{} {} ({} fixtures)\n",
                c.domain_id, c.title, c.fixture_count
            ));
        }

        out
    }
}

pub fn run_stdlib_check(fixtures_root: &Path) -> Result<StdlibReport> {
    let run_pass_dir = fixtures_root.join("run-pass");

    // Collect all .scoop filenames (stem only) from run-pass/.
    let mut fixture_stems: Vec<String> = Vec::new();
    if run_pass_dir.is_dir() {
        for entry in std::fs::read_dir(&run_pass_dir)
            .into_diagnostic()
            .wrap_err_with(|| format!("无法读取 run-pass 目录：{}", run_pass_dir.display()))?
        {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("scoop")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                fixture_stems.push(stem.to_string());
            }
        }
    }
    fixture_stems.sort();

    let mut covered: Vec<StdlibDomainCoverage> = Vec::new();
    let mut gaps: Vec<StdlibDomainCoverage> = Vec::new();
    let mut total_matched: usize = 0;

    for &(id, title, prefixes) in STDLIB_DOMAINS {
        let mut matched: Vec<String> = Vec::new();
        for stem in &fixture_stems {
            if prefixes.iter().any(|p| stem.starts_with(p)) {
                matched.push(stem.clone());
            }
        }
        let count = matched.len();
        total_matched += count;

        let entry = StdlibDomainCoverage {
            domain_id: id.to_string(),
            title: title.to_string(),
            fixture_count: count,
            fixture_names: matched,
        };

        if count > 0 {
            covered.push(entry);
        } else {
            gaps.push(entry);
        }
    }

    Ok(StdlibReport {
        domain_count: STDLIB_DOMAINS.len(),
        total_fixtures: total_matched,
        covered,
        gaps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn stdlib_coverage_reports_gaps_and_covered() {
        let dir = tempdir().unwrap();
        let fixtures_root = dir.path().join("tests").join("fixtures");
        let run_pass = fixtures_root.join("run-pass");
        std::fs::create_dir_all(&run_pass).unwrap();

        // Create fixtures matching some domains.
        std::fs::write(
            run_pass.join("std_io_basic.scoop"),
            "// EXPECT: pass\nfun main() {}\n",
        )
        .unwrap();
        std::fs::write(
            run_pass.join("stdlib_iter_basic.scoop"),
            "// EXPECT: pass\nfun main() {}\n",
        )
        .unwrap();
        std::fs::write(
            run_pass.join("kotlin_scope_functions_basic.scoop"),
            "// EXPECT: pass\nfun main() {}\n",
        )
        .unwrap();

        let report = run_stdlib_check(&fixtures_root).unwrap();

        // Should have 21 domains total.
        assert_eq!(report.domain_count, 21);

        // IO, Collections, and Scope functions should be covered.
        let covered_ids: Vec<&str> = report.covered.iter().map(|c| c.domain_id.as_str()).collect();
        assert!(covered_ids.contains(&"3"), "Collections should be covered");
        assert!(covered_ids.contains(&"11"), "IO should be covered");
        assert!(covered_ids.contains(&"18"), "Scope functions should be covered");

        // Math, Random, Net should be gaps.
        let gap_ids: Vec<&str> = report.gaps.iter().map(|g| g.domain_id.as_str()).collect();
        assert!(gap_ids.contains(&"7"), "Math should be a gap");
        assert!(gap_ids.contains(&"9"), "Random should be a gap");
        assert!(gap_ids.contains(&"16"), "Net should be a gap");
    }

    #[test]
    fn reports_missing_pass_or_fail_per_chapter() {
        let spec = r#"
# Spec

## 1. A

```scoop
// FIXTURE: spec_doctest/a.scoop
// EXPECT: pass
fun main() {}
```

## 2. B

```scoop
// FIXTURE: spec_doctest/b.scoop
// EXPECT: fail
fun main() {}
```

## 3. C

No fixtures here.
"#;

        let dir = tempdir().unwrap();
        let spec_path = dir.path().join("SCOOP_FULL_SPEC.md");
        std::fs::write(&spec_path, spec).unwrap();

        let fixtures_root = dir.path().join("tests").join("fixtures");
        std::fs::create_dir_all(fixtures_root.join("spec_doctest")).unwrap();
        std::fs::write(
            fixtures_root.join("spec_doctest/a.scoop"),
            "// EXPECT: pass\nfun main() {}\n",
        )
        .unwrap();
        std::fs::write(
            fixtures_root.join("spec_doctest/b.scoop"),
            "// EXPECT: fail\nfun main() {}\n",
        )
        .unwrap();

        let report = run_check(&spec_path, &fixtures_root).unwrap();
        assert_eq!(report.chapter_count, 3);
        assert_eq!(report.fixture_count, 2);

        let mut gaps = report.gaps.clone();
        gaps.sort_by(|a, b| a.chapter_id.cmp(&b.chapter_id));

        assert_eq!(
            gaps,
            vec![
                Gap {
                    chapter_id: "1".to_string(),
                    title: "A".to_string(),
                    pass: 1,
                    fail: 0,
                    missing_pass: false,
                    missing_fail: true,
                },
                Gap {
                    chapter_id: "2".to_string(),
                    title: "B".to_string(),
                    pass: 0,
                    fail: 1,
                    missing_pass: true,
                    missing_fail: false,
                },
                Gap {
                    chapter_id: "3".to_string(),
                    title: "C".to_string(),
                    pass: 0,
                    fail: 0,
                    missing_pass: true,
                    missing_fail: true,
                },
            ]
        );
    }
}
