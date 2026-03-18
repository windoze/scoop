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

fn parse_spec_chapters_and_fixtures(spec_text: &str) -> Result<(Vec<Chapter>, Vec<SpecFixtureRef>)> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

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
