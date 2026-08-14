//! M2 回归门：凡 scoop2 typecheck 干净通过的 fixture，其全部函数树必须 **gap-free**
//!（C9：词汇表封闭；出现 gap 即上游决议泄漏或构造覆盖回退）。
//!
//! 语料：mir2 全量 + run-pass / hir / infer 采样（LIMIT 控制耗时；环境变量
//! `SCOOP2_GAP_DIRS` 可覆盖目录列表，逗号分隔）。

use std::path::{Path, PathBuf};

use scoop2_archive::pipeline::{build_program, typecheck_program};

#[test]
fn gap_survey_corpus() {
    let roots: Vec<PathBuf> = std::env::var("SCOOP2_GAP_DIRS")
        .map(|d| d.split(',').map(PathBuf::from).collect())
        .unwrap_or_else(|_| {
            let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
            ["mir2", "run-pass", "hir", "infer"]
                .iter()
                .map(|d| base.join(d))
                .collect()
        });
    const LIMIT: usize = 120;
    let mut files: Vec<PathBuf> = Vec::new();
    for dir in &roots {
        collect_scoop_files(dir, &mut files);
    }
    files.sort();
    files.truncate(LIMIT);
    run_gap_survey(&files);
}

fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_scoop_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "scoop") {
            out.push(p);
        }
    }
}

fn run_gap_survey(files: &[PathBuf]) {
    let mut failures: Vec<(String, usize, String, String)> = Vec::new();
    let mut ok_files = 0usize;
    let mut skip_files = 0usize;
    for f in files {
        let Ok(source) = scoop2_base::SourceFile::load(f) else {
            continue;
        };
        let mut program = build_program(&source);
        let Ok(hir) = typecheck_program(&mut program, None) else {
            skip_files += 1;
            continue;
        };
        let mut file_ok = true;
        for tf in &hir.files {
            for tree in &tf.trees {
                for (span, what) in &tree.gaps {
                    file_ok = false;
                    let snippet: String = source
                        .text()
                        .bytes()
                        .skip(span.start.saturating_sub(6))
                        .take(24)
                        .map(|b| b as char)
                        .collect();
                    failures.push((
                        f.file_name().unwrap().to_string_lossy().into_owned(),
                        span.start,
                        what.clone(),
                        snippet,
                    ));
                }
            }
        }
        let trees: usize = hir.files.iter().map(|tf| tf.trees.len()).sum();
        eprintln!(
            "  FILE {} gapfree={} trees={}",
            f.file_name().unwrap().to_string_lossy(),
            file_ok,
            trees
        );
        if file_ok {
            ok_files += 1;
        }
    }
    eprintln!(
        "=== gap survey: {} files, {} gap-free, {} skipped(typecheck失败，既有) ===",
        files.len(),
        ok_files,
        skip_files
    );
    // 直方图（开发视图；--nocapture 查看）。
    let mut hist: std::collections::BTreeMap<String, usize> = Default::default();
    for (_, _, what, _) in &failures {
        let key = what.split('（').next().unwrap_or(what).to_string();
        *hist.entry(key).or_default() += 1;
    }
    for (k, v) in &hist {
        eprintln!("  {v:4}  {k}");
    }
    assert!(
        failures.is_empty(),
        "gap-free 回归失败（{} 处）：{:#?}",
        failures.len(),
        failures
    );
}
